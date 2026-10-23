// Full-screen playback with Media3/ExoPlayer, which uses the TV's hardware
// decoders. Remembers where you stopped and plays the next episode on its own.
// The controls (ExoPlayer's own) show the title on top while they're up.
// Holding OK opens the subtitles: the server's files, the addons' and the
// video's own, and the files' timing (Subs.kt).
package io.github.okaza03.valo.ui

import androidx.annotation.OptIn
import android.view.View
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.TrackGroup
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.PlayerView
import androidx.media3.ui.SubtitleView
import androidx.tv.material3.Text
import io.github.okaza03.valo.Addons
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.SubTrack
import io.github.okaza03.valo.Client
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Screen
import io.github.okaza03.valo.Subtitle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester

/** Save the position this often while playing. */
private const val SAVE_EVERY_MS = 5_000L

/**
 * The player's view while one is on screen. When the controls hide while one
 * of their buttons has focus (after picking subtitles, say), Android hands
 * focus to the window instead, and the remote's keys stop reaching the
 * player; MainActivity sends them back here.
 */
object PlayerKeys {
    var view: PlayerView? = null
    /** OK held down (or the remote's subtitles key): the subtitle panel. */
    var onHold: (() -> Unit)? = null
}

/** What's on as subtitles. */
private sealed interface SubPick {
    data object Off : SubPick
    /** A file, ours to draw. */
    data class File(val sub: Subtitle) : SubPick
    /** A track in the video, ExoPlayer's to draw. */
    data class Embedded(val group: TrackGroup, val index: Int, val label: String, val language: String?) : SubPick
}

/** Subtitles from the addons, at most this many. */
private const val ADDON_SUBS = 12

@OptIn(UnstableApi::class)
@Composable
fun PlayerScreen(model: AppModel, item: Item, screen: Screen.Player) {
    val episode = screen.episodeId?.let { id -> item.episodes.find { it.id == id } }
    val key = episode?.id ?: item.id
    val url = episode?.stream ?: item.stream ?: return
    val context = LocalContext.current
    var error by remember(key) { mutableStateOf<String?>(null) }
    /** The controls are up. */
    var hud by remember(key) { mutableStateOf(false) }
    val next = episode?.let { model.episodeAfter(item, it.id) }

    // subtitles: the server's files, the addons' (once they answer), the video's own tracks
    val files = episode?.subtitles ?: item.subtitles
    var fromAddons by remember(key) { mutableStateOf<List<Subtitle>>(emptyList()) }
    var embedded by remember(key) { mutableStateOf<List<SubPick.Embedded>>(emptyList()) }
    var picked by remember(key) { mutableStateOf<SubPick>(SubPick.Off) }
    var track by remember(key) { mutableStateOf<SubTrack?>(null) }
    var subError by remember(key) { mutableStateOf<String?>(null) }
    /** Positive: the subtitles show later. Each file has its own. */
    var delayMs by remember(key) { mutableLongStateOf(0L) }
    var panel by remember(key) { mutableStateOf(false) }
    var subView by remember { mutableStateOf<SubtitleView?>(null) }

    val player = remember(key) {
        val auth = model.client?.authFor(url)
        val http = OkHttpDataSource.Factory(Client.http)
            .setDefaultRequestProperties(if (auth != null) mapOf("Authorization" to auth) else emptyMap())
        ExoPlayer.Builder(context)
            .setMediaSourceFactory(DefaultMediaSourceFactory(http))
            // try the next decoder when the preferred one fails on this device
            .setRenderersFactory(DefaultRenderersFactory(context).setEnableDecoderFallback(true))
            .setAudioAttributes(
                AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MOVIE).build(),
                true,
            )
            .setSeekBackIncrementMs(10_000)
            .setSeekForwardIncrementMs(10_000)
            .build()
            .apply {
                val resume = model.entry(key)?.takeIf { it.inProgress && !screen.fromStart }
                // the language picked last comes on by itself
                trackSelectionParameters = trackSelectionParameters.buildUpon()
                    .setPreferredTextLanguage(model.prefs.subtitleLanguage)
                    .build()
                // the files are drawn by us (see SubTrack); ExoPlayer only has the video's own tracks
                val media = MediaItem.Builder().setUri(url).build()
                setMediaItem(media, ((resume?.pos ?: 0.0) * 1000).toLong())
                prepare()
                playWhenReady = true
            }
    }

    // the controls' Next button moves on to the next episode
    val shown = remember(player) {
        object : ForwardingPlayer(player) {
            private fun isNext(c: Int) = c == Player.COMMAND_SEEK_TO_NEXT || c == Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM

            override fun isCommandAvailable(command: Int) =
                if (isNext(command)) next != null else super.isCommandAvailable(command)

            override fun getAvailableCommands(): Player.Commands = super.getAvailableCommands().buildUpon()
                .addIf(Player.COMMAND_SEEK_TO_NEXT, next != null)
                .addIf(Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM, next != null)
                .build()

            override fun seekToNext() = playNext()

            override fun seekToNextMediaItem() = playNext()

            private fun playNext() {
                if (next != null) model.swap(Screen.Player(item.id, next.id))
            }
        }
    }

    fun save() {
        val dur = player.duration.takeIf { it != C.TIME_UNSET && it > 0 } ?: return
        model.save(key, player.currentPosition / 1000.0, dur / 1000.0)
    }

    DisposableEffect(player) {
        val listener = object : Player.Listener {
            override fun onPlaybackStateChanged(state: Int) {
                if (state != Player.STATE_ENDED) return
                save()
                if (next != null) model.swap(Screen.Player(item.id, next.id)) else model.back()
            }

            // the video's own subtitle tracks, and which of them ExoPlayer turned on
            override fun onTracksChanged(tracks: Tracks) {
                embedded = tracks.groups.filter { it.type == C.TRACK_TYPE_TEXT && !it.isAdaptiveSupported }.flatMap { g ->
                    (0 until g.length).filter(g::isTrackSupported).map { i ->
                        val f = g.getTrackFormat(i)
                        SubPick.Embedded(g.mediaTrackGroup, i, f.label ?: f.language?.let(Addons::langName) ?: "Track ${i + 1}", f.language)
                    }
                }
                if (picked == SubPick.Off) {
                    tracks.groups.filter { it.type == C.TRACK_TYPE_TEXT }.firstNotNullOfOrNull { g ->
                        (0 until g.length).firstOrNull(g::isTrackSelected)?.let { i -> embedded.find { it.group == g.mediaTrackGroup && it.index == i } }
                    }?.let { picked = it }
                }
            }

            override fun onPlayerError(e: PlaybackException) {
                error = "Can't play this: ${e.errorCodeName}" + (e.cause?.message?.let { "\n$it" } ?: "")
            }
        }
        player.addListener(listener)
        onDispose {
            save()
            player.removeListener(listener)
            player.release()
            model.playerClosed()
        }
    }

    fun pick(p: SubPick, remember: Boolean = true) {
        picked = p
        subError = null
        track = null
        val params = player.trackSelectionParameters.buildUpon()
        when (p) {
            is SubPick.Embedded -> params.setTrackTypeDisabled(C.TRACK_TYPE_TEXT, false)
                .setOverrideForType(TrackSelectionOverride(p.group, p.index))
            else -> params.setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true).clearOverridesOfType(C.TRACK_TYPE_TEXT)
        }
        player.trackSelectionParameters = params.build()
        if (remember) {
            model.prefs.subtitleLanguage = when (p) {
                SubPick.Off -> null
                is SubPick.File -> p.sub.language.takeIf { it != "und" }
                is SubPick.Embedded -> p.language
            }
        }
    }

    // the language picked last comes on by itself: a file of ours, else the video's own track, else the addons'
    val lang = model.prefs.subtitleLanguage
    LaunchedEffect(key) {
        files.firstOrNull { !it.forced && it.language == lang && lang != null }?.let { pick(SubPick.File(it), remember = false) }
        val imdb = Addons.imdbId(item.imdbId) ?: return@LaunchedEffect
        val season = episode?.let { e -> item.seasons.find { s -> s.episodes.any { it.id == e.id } } }
        val (kind, id) = if (episode != null && season != null) "series" to "$imdb:${season.number}:${episode.number}" else "movie" to imdb
        val found = withContext(Dispatchers.IO) { runCatching { Addons.subtitles(model.discover.addons, kind, id) }.getOrDefault(emptyList()) }
        // the language they use first
        fromAddons = found.sortedBy { if (it.language == lang) 0 else 1 }.take(ADDON_SUBS)
        if (picked == SubPick.Off && lang != null && files.none { it.language == lang }) {
            fromAddons.firstOrNull { it.language == lang }?.let { pick(SubPick.File(it), remember = false) }
        }
    }

    // a file's cues, loaded when it's picked
    LaunchedEffect(picked) {
        val f = (picked as? SubPick.File)?.sub ?: return@LaunchedEffect
        withContext(Dispatchers.IO) { runCatching { SubTrack.load(model.client, f) } }
            .onSuccess { track = it }
            .onFailure { subError = it.message ?: "Can't load the subtitles" }
    }

    // drawn in step with the video, moved by the timing
    LaunchedEffect(track, delayMs, subView) {
        val v = subView ?: return@LaunchedEffect
        val t = track
        if (t == null) {
            v.setCues(emptyList())
            return@LaunchedEffect
        }
        while (true) {
            v.setCues(t.at(player.currentPosition - delayMs))
            delay(50)
        }
    }

    // hold OK for the subtitle panel
    DisposableEffect(key) {
        PlayerKeys.onHold = { panel = true }
        onDispose { PlayerKeys.onHold = null }
    }

    // pause when the TV's Home button sends the app to the background
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    DisposableEffect(lifecycle, player) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_STOP) {
                player.pause()
                save()
                model.pushNow()
            }
        }
        lifecycle.addObserver(observer)
        onDispose { lifecycle.removeObserver(observer) }
    }

    LaunchedEffect(player) {
        while (true) {
            delay(SAVE_EVERY_MS)
            if (player.isPlaying) save()
        }
    }

    Box(Modifier.fillMaxSize().background(Color.Black)) {
        // a fresh view per episode, so the remote keeps working after autoplay
        androidx.compose.runtime.key(key) {
        AndroidView(
            factory = { ctx ->
                PlayerView(ctx).apply {
                    useController = true
                    controllerShowTimeoutMs = 4_000
                    // the subtitle panel (hold OK) has the server's and addons' files, and timing
                    setShowSubtitleButton(false)
                    setShowNextButton(next != null)
                    setShowPreviousButton(false)
                    setShowBuffering(PlayerView.SHOW_BUFFERING_ALWAYS)
                    keepScreenOn = true
                    isFocusable = true
                    isFocusableInTouchMode = true
                    setControllerVisibilityListener(PlayerView.ControllerVisibilityListener { v ->
                        hud = v == View.VISIBLE
                        // the focused button just disappeared: keep the remote on the player
                        if (v != View.VISIBLE) requestFocus()
                    })
                    post { requestFocus() }
                }
            },
            update = {
                it.player = shown
                PlayerKeys.view = it
            },
            onRelease = { if (PlayerKeys.view === it) PlayerKeys.view = null },
            modifier = Modifier.fillMaxSize(),
        )
        }
        // our subtitles, above the controls' bar while it's up
        AndroidView(
            factory = { ctx ->
                SubtitleView(ctx).apply {
                    setUserDefaultStyle()
                    setUserDefaultTextSize()
                    subView = this
                }
            },
            update = { it.setBottomPaddingFraction(if (hud) 0.22f else SubtitleView.DEFAULT_BOTTOM_PADDING_FRACTION) },
            modifier = Modifier.fillMaxSize(),
        )
        if (hud && error == null) {
            Column(
                Modifier.fillMaxWidth()
                    .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.7f), Color.Transparent)))
                    .padding(start = EDGE, end = EDGE, top = 28.dp, bottom = 40.dp),
            ) {
                Text(
                    item.title, color = Color.White, fontSize = 24.sp, fontWeight = FontWeight.Bold,
                    maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
                if (episode != null) {
                    val season = item.seasons.find { s -> s.episodes.any { it.id == episode.id } }
                    val what = listOfNotNull(
                        season?.let { "S${it.number} E${episode.number}" },
                        episode.title.takeIf { it.isNotEmpty() },
                    ).joinToString("  ·  ")
                    Text(what, color = Color.White.copy(alpha = 0.8f), fontSize = 16.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                Spacer(Modifier.height(6.dp))
                MonoText("Hold OK for subtitles", 11.sp, Color.White.copy(alpha = 0.6f))
            }
        }
        if (panel) {
            SubtitlePanel(
                files = files, fromAddons = fromAddons, embedded = embedded, picked = picked, delayMs = delayMs,
                loading = picked is SubPick.File && track == null && subError == null, error = subError,
                onPick = { pick(it) }, onDelay = { delayMs = it }, onClose = { panel = false },
            )
        }
        error?.let {
            Column(Modifier.align(Alignment.Center).width(640.dp)) {
                Text(it, color = ERROR, fontSize = 18.sp)
                Spacer(Modifier.height(12.dp))
                Text("Press Back to return.", color = MUTED, fontSize = 15.sp)
            }
        }
    }
}

/** "On time", "0.3 s later", "1.5 s earlier" */
private fun delayText(ms: Long) = when {
    ms == 0L -> "On time"
    ms > 0 -> "%.1f s later".format(ms / 1000.0)
    else -> "%.1f s earlier".format(-ms / 1000.0)
}

/** Off, the files and the video's own tracks to pick from, and a file's timing. */
@Composable
private fun SubtitlePanel(
    files: List<Subtitle>,
    fromAddons: List<Subtitle>,
    embedded: List<SubPick.Embedded>,
    picked: SubPick,
    delayMs: Long,
    loading: Boolean,
    error: String?,
    onPick: (SubPick) -> Unit,
    onDelay: (Long) -> Unit,
    onClose: () -> Unit,
) {
    val current = remember { FocusRequester() }
    LaunchedEffect(Unit) { current.focusSoon() }
    fun label(s: Subtitle) = s.label ?: Addons.langName(s.language).let { if (s.forced) "$it (forced)" else it }

    @Composable
    fun Choice(text: String, p: SubPick) {
        val on = p == picked
        TvButton(
            (if (on) "✓  " else "") + text, { onPick(p) },
            Modifier.fillMaxWidth().then(if (on) Modifier.focusRequester(current) else Modifier), selected = on,
        )
    }

    Modal(560.dp, onClose, dim = 0.2f, footer = {
        if (picked is SubPick.File) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                MonoText("Timing", 11.sp, MUTED)
                Spacer(Modifier.width(12.dp))
                Text(delayText(delayMs), color = if (delayMs == 0L) TEXT else GOLD, fontSize = 15.sp)
            }
            Spacer(Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TvButton("−0.5 s", { onDelay(delayMs - 500) })
                TvButton("−0.1 s", { onDelay(delayMs - 100) })
                TvButton("+0.1 s", { onDelay(delayMs + 100) })
                TvButton("+0.5 s", { onDelay(delayMs + 500) })
                if (delayMs != 0L) TvButton("Reset", { onDelay(0) })
            }
            Spacer(Modifier.height(6.dp))
            Text("Minus shows them earlier, plus later.", color = MUTED, fontSize = 13.sp)
            Spacer(Modifier.height(14.dp))
        }
        TvButton("Done", onClose, primary = true, modifier = if (picked == SubPick.Off && files.isEmpty()) Modifier.focusRequester(current) else Modifier)
    }) {
        Column(Modifier.padding(horizontal = 32.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Subtitles", fontSize = 28.sp, fontWeight = FontWeight.Bold, color = TEXT)
            Spacer(Modifier.height(4.dp))
            Choice("Off", SubPick.Off)
            if (files.isNotEmpty()) MonoText("On the server", 11.sp, MUTED)
            for (f in files) Choice(label(f), SubPick.File(f))
            if (embedded.isNotEmpty()) MonoText("In the video", 11.sp, MUTED)
            for (e in embedded) Choice(e.label, e)
            if (fromAddons.isNotEmpty()) MonoText("From the addons", 11.sp, MUTED)
            for (f in fromAddons) Choice(label(f), SubPick.File(f))
            if (loading) Text("Loading the subtitles…", color = MUTED, fontSize = 13.sp)
            error?.let { Text(it, color = ERROR, fontSize = 13.sp) }
            if (picked is SubPick.Embedded) Text("The timing of subtitles in the video can't be moved.", color = MUTED, fontSize = 13.sp)
        }
    }
}
