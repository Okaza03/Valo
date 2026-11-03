// Playing on a TV: starts titles on the Chromecast's own player and follows
// them. Progress is saved like local playback, so moving between the phone and
// the TV carries on where you were, and the next episode plays by itself.
package io.github.okaza03.valo.cast

import android.app.Application
import android.net.Uri
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import com.google.android.gms.cast.MediaError
import com.google.android.gms.cast.MediaInfo
import com.google.android.gms.cast.MediaLoadRequestData
import com.google.android.gms.cast.MediaMetadata
import com.google.android.gms.cast.MediaSeekOptions
import com.google.android.gms.cast.MediaStatus
import com.google.android.gms.cast.MediaTrack
import com.google.android.gms.cast.framework.CastContext
import com.google.android.gms.cast.framework.CastSession
import com.google.android.gms.cast.framework.SessionManagerListener
import com.google.android.gms.cast.framework.media.RemoteMediaClient
import com.google.android.gms.common.images.WebImage
import io.github.okaza03.valo.Addons
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Client
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Screen
import io.github.okaza03.valo.SubTrack
import io.github.okaza03.valo.Subtitle
import io.github.okaza03.valo.ui.ADDON_SUBS
import io.github.okaza03.valo.ui.subLabel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/** Wait this long for the addons' subtitles before starting without them. */
private const val ADDON_WAIT_MS = 3_000L
/** Save the position this often, like the local player. */
private const val SAVE_EVERY_MS = 5_000L
/** The TV lost the connection, which says nothing about the file (MediaError.DetailedErrorCode). */
private const val MEDIA_NETWORK = 103
private const val SKIP_WAIT_MS = 500L

/** What's on the TV. */
data class Casting(val itemId: String, val episodeId: String?, val title: String, val sub: String?, val art: String?) {
    val key get() = episodeId ?: itemId
}

/** A subtitle track the TV can show. */
data class CastTrack(val id: Long, val label: String)

/**
 * The Chromecast takes these files as they are. Others it may seem to accept
 * and then play without a picture, so they're rewrapped first (StereoStream).
 */
fun castsDirectly(url: String) = url.substringBefore('?').substringAfterLast('.').lowercase() in setOf("mp4", "m4v", "mov", "webm", "m3u8")

/**
 * The video has to become H.264 on this device: anything else it can't decode,
 * and it would play the sound with no picture and no error. Cast's plain
 * "Chromecast" model is the dongles without HEVC; TVs and newer ones have it.
 */
fun needsH264(codec: String?, model: String?): Boolean {
    val c = codec?.lowercase() ?: return false
    return when {
        isH264(c) -> false
        c.contains("265") || c.contains("hevc") -> model == "Chromecast"
        else -> true
    }
}

private fun isH264(codec: String) = codec.contains("264", true) || codec.contains("avc", true)

/** What the container is called to the Chromecast. It goes by the file's extension. */
fun castType(url: String): String = when (url.substringBefore('?').substringAfterLast('.').lowercase()) {
    "webm" -> "video/webm"
    "m3u8" -> "application/x-mpegurl"
    // Chromecasts play Matroska but won't take a load that says so
    else -> "video/mp4"
}

class Caster(private val app: Application, private val model: AppModel) {
    private val scope get() = model.viewModelScope
    // a paused TV stops reading for as long as it likes
    private val streamHttp = Client.http.newBuilder().readTimeout(0, TimeUnit.SECONDS).build()
    private val proxy = CastProxy(streamHttp)
    private var context: CastContext? = null
    private var session: CastSession? = null
    private var lastSave = 0L
    /** The title that has finished, so a repeated status doesn't move on twice. */
    private var ended: String? = null
    /** The TV stopped on an error, and its code when it gave one. */
    private var failed = false
    private var errorCode: Int? = null
    /** How the title on the TV was started, to start it again another way. */
    private class Start(val url: String, val auth: String?, var toH264: Boolean, val codec: String?, val meta: MediaMetadata, val tracks: List<MediaTrack>, val active: LongArray, val base: String, val durationMs: Long) {
        /** Kept between seeks, so its index isn't fetched each time. */
        var file: ServerFile? = null
    }
    private var started: Start? = null
    /** The phone is converting the sound to stereo, from this far in. */
    private var stereo = false
    @Volatile private var offsetMs = 0L
    /** TVs that turned surround sound down: they get stereo straight away from now on. */
    private val stereoOnly = HashSet<String>()
    /** TVs that turned down video that isn't H.264. */
    private val onlyH264 = HashSet<String>()

    /** The file on the TV: "mkv / hevc / eac3". */
    private var fileInfo: String? = null
    private var fileChannels: Int? = null
    /** A seek waiting to see if another comes. */
    private var skip: Job? = null
    /** Where this title started, for the next try. */
    private var startMs = 0L

    /** The Cast SDK runs here; it needs Google Play services. */
    var available by mutableStateOf(false)
        private set
    /** The TV we're connected to. */
    var device by mutableStateOf<String?>(null)
        private set
    var now by mutableStateOf<Casting?>(null)
        private set
    var playing by mutableStateOf(false)
        private set
    var buffering by mutableStateOf(false)
        private set
    var positionMs by mutableLongStateOf(0L)
        private set
    var durationMs by mutableLongStateOf(0L)
        private set
    var error by mutableStateOf<String?>(null)
        private set
    var tracks by mutableStateOf<List<CastTrack>>(emptyList())
        private set
    var activeTrack by mutableStateOf<Long?>(null)
        private set

    private val client get() = session?.remoteMediaClient

    private val listener = object : SessionManagerListener<CastSession> {
        override fun onSessionStarted(s: CastSession, id: String) = attach(s)
        override fun onSessionResumed(s: CastSession, wasSuspended: Boolean) = attach(s)
        override fun onSessionEnding(s: CastSession) = save()
        override fun onSessionEnded(s: CastSession, e: Int) = detach()
        override fun onSessionStartFailed(s: CastSession, e: Int) {
            error = "Couldn't connect to the TV"
        }
        override fun onSessionResumeFailed(s: CastSession, e: Int) = detach()
        override fun onSessionStarting(s: CastSession) {}
        override fun onSessionResuming(s: CastSession, id: String) {}
        // the Wi-Fi dropped for a moment: it may come back by itself
        override fun onSessionSuspended(s: CastSession, reason: Int) {}
    }

    private val callback = object : RemoteMediaClient.Callback() {
        override fun onStatusUpdated() = status()
        override fun onMediaError(e: MediaError) {
            errorCode = e.detailedErrorCode
            if (failed) report()
        }
    }

    private val progress = RemoteMediaClient.ProgressListener { pos, dur ->
        if (skip == null) positionMs = pos + offsetMs
        durationMs = started?.durationMs?.takeIf { stereo && it > 0 } ?: dur
        val at = System.currentTimeMillis()
        if (playing && at - lastSave >= SAVE_EVERY_MS) {
            lastSave = at
            save()
        }
    }

    init {
        CastService.caster = this
        // without Google Play services this fails, and the app just has no Cast button
        runCatching {
            CastContext.getSharedInstance(app, Executors.newSingleThreadExecutor())
                .addOnSuccessListener { c ->
                    context = c
                    available = true
                    c.sessionManager.addSessionManagerListener(listener, CastSession::class.java)
                    c.sessionManager.currentCastSession?.let(::attach)
                }
        }
    }

    fun close() {
        stop()
        context?.sessionManager?.removeSessionManagerListener(listener, CastSession::class.java)
        if (CastService.caster === this) CastService.caster = null
    }

    private fun attach(s: CastSession) {
        session = s
        device = s.castDevice?.friendlyName ?: "your TV"
        error = null
        s.remoteMediaClient?.let {
            it.registerCallback(callback)
            it.addProgressListener(progress, 1_000)
        }
    }

    private fun detach() {
        session?.remoteMediaClient?.let {
            it.unregisterCallback(callback)
            it.removeProgressListener(progress)
        }
        session = null
        device = null
        now = null
        playing = false
        buffering = false
        tracks = emptyList()
        activeTrack = null
        proxy.stop()
        CastService.end(app)
        model.playerClosed()
    }

    private fun status() {
        val c = client ?: return
        val st = c.mediaStatus
        val wasPlaying = playing
        playing = st?.playerState == MediaStatus.PLAYER_STATE_PLAYING
        buffering = st?.playerState == MediaStatus.PLAYER_STATE_BUFFERING || st?.playerState == MediaStatus.PLAYER_STATE_LOADING
        activeTrack = st?.activeTrackIds?.firstOrNull()
        if (playing) error = null
        if (wasPlaying != playing && now != null) CastService.show(app)
        if (st?.playerState != MediaStatus.PLAYER_STATE_IDLE) return
        when (st.idleReason) {
            MediaStatus.IDLE_REASON_FINISHED -> finished()
            MediaStatus.IDLE_REASON_ERROR -> {
                if (failed) return
                failed = true
                // it had the file and turned it down: often it's the sound, which we can change
                val refused = proxy.delivery.requests > 0 && errorCode != MEDIA_NETWORK
                val st = started
                val from = positionMs.coerceAtLeast(startMs)
                when {
                    !refused || st == null -> report()
                    !stereo -> {
                        if ((fileChannels ?: 0) > 2) session?.castDevice?.deviceId?.let(stereoOnly::add)
                        loadStereo(from)
                    }
                    // the sound wasn't it: the last thing left to change is the video
                    !st.toH264 && st.codec?.let(::isH264) == false -> {
                        session?.castDevice?.deviceId?.let(onlyH264::add)
                        st.toH264 = true
                        loadStereo(from)
                    }
                    else -> report()
                }
            }
        }
    }

    private fun report() {
        val made = started?.file?.h264
        val video = if (started?.toH264 == true) {
            "video converted" + (made?.let { f -> " to ${f.initializationData.firstOrNull()?.let(::avcCodec)} ${f.width}x${f.height}" } ?: "")
        } else {
            "video copied"
        }
        error = castFailure(errorCode, proxy.delivery, fileInfo, fileChannels, stereo) +
            " · ${session?.castDevice?.modelName ?: "unknown model"} · $video"
    }

    /** Remember where the TV is. */
    private fun save() {
        val n = now ?: return
        val dur = durationMs.takeIf { it > 0 } ?: return
        model.save(n.key, positionMs / 1000.0, dur / 1000.0)
    }

    private fun finished() {
        val n = now ?: return
        if (ended == n.key) return
        ended = n.key
        durationMs.takeIf { it > 0 }?.let { model.save(n.key, it / 1000.0, it / 1000.0) }
        val on = (model.screen as? Screen.Player)?.let { (it.episodeId ?: it.itemId) == n.key } == true
        val item = model.library?.find(n.itemId)
        val next = n.episodeId?.let { id -> item?.let { model.episodeAfter(it, id) } }
        if (item != null && next != null) {
            play(item, next.id, fromStart = true)
            if (on) model.swap(Screen.Player(item.id, next.id))
        } else {
            now = null
            model.playerClosed()
            if (on) model.back()
        }
    }

    private fun tvAddress(s: CastSession) =
        runCatching { s.castDevice?.let { if (it.hasIPv4Address()) it.ipAddress else it.inetAddress } }.getOrNull()

    /** The subtitles as tracks, each made into WebVTT when the TV first asks for it. */
    private fun subtitleTracks(subs: List<Subtitle>, c: Client, base: String): List<MediaTrack> = subs.mapIndexed { i, sub ->
        val path = proxy.add("text/vtt; charset=utf-8") { SubTrack.load(c, sub).toVtt().toByteArray() }
        MediaTrack.Builder(i + 1L, MediaTrack.TYPE_TEXT)
            .setName(subLabel(sub))
            .setSubtype(MediaTrack.SUBTYPE_SUBTITLES)
            .setContentId(base + path)
            .setContentType("text/vtt")
            .apply { sub.language.takeIf { it != "und" }?.let(::setLanguage) }
            .build()
    }

    /** Start a title on the TV. `fromStart` ignores the resume point. */
    fun play(item: Item, episodeId: String?, fromStart: Boolean) {
        val s = session ?: return
        val c = model.client ?: return
        val episode = episodeId?.let { id -> item.episodes.find { it.id == id } }
        val url = episode?.stream ?: item.stream ?: return
        val key = episode?.id ?: item.id
        val season = episode?.let { e -> item.seasons.find { x -> x.episodes.any { it.id == e.id } } }
        val sub = episode?.let { e ->
            listOfNotNull(season?.code(e.number), e.title.takeIf { it.isNotEmpty() }).joinToString(" · ")
        }
        val art = item.poster ?: item.backdrop
        save()
        ended = null
        error = null
        failed = false
        errorCode = null
        val file = episode?.file ?: item.file
        fileInfo = file?.let { listOfNotNull(it.container, it.videoCodec, it.audioCodec).joinToString(" / ").ifEmpty { null } }
        fileChannels = file?.audioChannels
        positionMs = 0
        durationMs = 0
        tracks = emptyList()
        now = Casting(item.id, episode?.id, item.title, sub, art)
        val start = if (fromStart) 0.0 else model.entry(key)?.takeIf { it.inProgress }?.pos ?: 0.0
        startMs = (start * 1000).toLong()
        started = null
        stereo = false
        offsetMs = 0
        val lang = model.prefs.subtitleLanguage
        val files = episode?.subtitles ?: item.subtitles
        scope.launch {
            val imdb = Addons.imdbId(item.imdbId)
            val addons = if (imdb == null) emptyList() else withTimeoutOrNull(ADDON_WAIT_MS) {
                val (kind, id) = if (episode != null && season != null) "series" to "$imdb:${season.number}:${episode.number}" else "movie" to imdb
                withContext(Dispatchers.IO) {
                    runCatching { Addons.subtitles(model.discover.addons, kind, id) }.getOrDefault(emptyList())
                }
            }.orEmpty().sortedBy { if (it.language == lang) 0 else 1 }.take(ADDON_SUBS)
            // which of the phone's addresses the TV can reach it at
            val tv = tvAddress(s)
            val mine = withContext(Dispatchers.IO) { localAddresses(app) }
            val ip = pickAddress(tv, mine)?.hostAddress
            if (ip == null) {
                error = "Couldn't find this phone's Wi-Fi address. Is it on the same Wi-Fi as the TV?\n\n${addressReport(tv, mine)}"
                now = null
                return@launch
            }
            withContext(Dispatchers.IO) { proxy.start() }
            proxy.clear()
            CastService.show(app)
            val base = "http://$ip:${proxy.port}"
            val subs = files + addons
            val media = subtitleTracks(subs, c, base)
            val meta = MediaMetadata(if (episode != null) MediaMetadata.MEDIA_TYPE_TV_SHOW else MediaMetadata.MEDIA_TYPE_MOVIE).apply {
                putString(MediaMetadata.KEY_TITLE, item.title)
                sub?.let { putString(MediaMetadata.KEY_SUBTITLE, it) }
                art?.let { addImage(WebImage(Uri.parse(base + proxy.add(it, c.authFor(it))))) }
            }
            val info = MediaInfo.Builder(base + proxy.add(url, c.authFor(url), castType(url)))
                .setStreamType(MediaInfo.STREAM_TYPE_BUFFERED)
                .setContentType(castType(url))
                .setMetadata(meta)
                .setMediaTracks(media)
                .build()
            tracks = media.map { CastTrack(it.id, it.name.orEmpty()) }
            // the language picked last comes on by itself
            val on = subs.indexOfFirst { !it.forced && lang != null && it.language == lang }
            val active = if (on >= 0) longArrayOf(on + 1L) else longArrayOf()
            val known = ((episode?.duration ?: item.duration ?: 0.0) * 1000).toLong()
            val toH264 = needsH264(file?.videoCodec, s.castDevice?.modelName) || s.castDevice?.deviceId in onlyH264 && file?.videoCodec?.let(::isH264) == false
            started = Start(url, c.authFor(url), toH264, file?.videoCodec, meta, media, active, base, known)
            val req = MediaLoadRequestData.Builder()
                .setMediaInfo(info)
                .setAutoplay(true)
                .setCurrentTime(startMs)
                .apply { if (on >= 0) setActiveTrackIds(active) }
                .build()
            val rc = s.remoteMediaClient ?: return@launch
            if (toH264 || !castsDirectly(url) || (fileChannels ?: 0) > 2 && s.castDevice?.deviceId in stereoOnly) return@launch loadStereo(startMs)
            rc.load(req).setResultCallback { r ->
                if (!r.status.isSuccess && !failed) error = "The TV couldn't start this title"
            }
        }
    }

    /** Play the title with the phone turning its sound to stereo, from `ms` in. */
    private fun loadStereo(ms: Long) {
        val st = started ?: return
        val rc = client ?: return
        val file = st.file ?: ServerFile(streamHttp, st.url, st.auth).also { st.file = it }
        stereo = true
        failed = false
        errorCode = null
        proxy.restartDelivery()
        val fromUs = ms * 1000
        val path = proxy.addLive("video/mp4") { out ->
            writeStereo(file, fromUs, out, st.toH264) { base -> offsetMs = base / 1000 }
        }
        // what it sees as the start: seeking is a new stream, not a jump in this one
        offsetMs = ms
        positionMs = ms
        val info = MediaInfo.Builder(st.base + path)
            .setStreamType(MediaInfo.STREAM_TYPE_BUFFERED)
            .setContentType("video/mp4")
            .setMetadata(st.meta)
            .setMediaTracks(st.tracks)
            .build()
        val req = MediaLoadRequestData.Builder().setMediaInfo(info).setAutoplay(true)
            .apply { if (st.active.isNotEmpty()) setActiveTrackIds(st.active) }
            .build()
        rc.load(req).setResultCallback { r ->
            if (!r.status.isSuccess && !failed) error = "The TV couldn't start this title"
        }
    }

    fun togglePause() {
        client?.togglePlayback()
    }

    fun seek(ms: Long) {
        positionMs = ms
        if (stereo) {
            // a stream can't jump, only start again: wait for the last of a few quick taps
            skip?.cancel()
            skip = scope.launch {
                delay(SKIP_WAIT_MS)
                skip = null
                loadStereo(ms)
            }
            return
        }
        client?.seek(MediaSeekOptions.Builder().setPosition(ms).build())
    }

    /** The subtitles to show; null: none. */
    fun pickTrack(id: Long?) {
        activeTrack = id
        client?.setActiveMediaTracks(if (id == null) longArrayOf() else longArrayOf(id))
    }

    /** End casting; the TV goes back to its home screen. */
    fun stop() {
        save()
        context?.sessionManager?.endCurrentSession(true)
    }
}
