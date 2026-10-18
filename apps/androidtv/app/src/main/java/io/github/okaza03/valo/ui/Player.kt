// Full-screen playback with Media3/ExoPlayer, which uses the TV's hardware
// decoders. Remembers where you stopped and plays the next episode on its own.
// The controls (ExoPlayer's own) show the title on top while they're up.
package io.github.okaza03.valo.ui

import androidx.annotation.OptIn
import android.net.Uri
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
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.PlayerView
import androidx.tv.material3.Text
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Client
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Screen
import io.github.okaza03.valo.Subtitle
import kotlinx.coroutines.delay

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
}

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
                val media = MediaItem.Builder()
                    .setUri(url)
                    .setSubtitleConfigurations((episode?.subtitles ?: item.subtitles).map(::subtitleConfig))
                    .build()
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

            // remember the language picked with the subtitle button, or that they were turned off
            override fun onTracksChanged(tracks: Tracks) {
                val text = tracks.groups.filter { it.type == C.TRACK_TYPE_TEXT }
                val picked = text.flatMap { g -> (0 until g.length).filter(g::isTrackSelected).map(g::getTrackFormat) }
                    .firstOrNull { it.selectionFlags and C.SELECTION_FLAG_FORCED == 0 }
                val saved = model.prefs.subtitleLanguage
                if (picked != null) {
                    picked.language?.let { if (it != saved) model.prefs.subtitleLanguage = it }
                } else if (saved != null && text.any { g ->
                        (0 until g.length).any { g.getTrackFormat(it).language == saved }
                    }
                ) {
                    model.prefs.subtitleLanguage = null
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
                    setShowSubtitleButton(true)
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
            }
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

/** A subtitle file from the library, for ExoPlayer (fetched with the stream's credentials). */
private fun subtitleConfig(s: Subtitle): MediaItem.SubtitleConfiguration {
    val format = s.format ?: s.url.substringBefore('?').substringAfterLast('.').lowercase()
    return MediaItem.SubtitleConfiguration.Builder(Uri.parse(s.url))
        .setMimeType(
            when (format) {
                "vtt" -> MimeTypes.TEXT_VTT
                "ass", "ssa" -> MimeTypes.TEXT_SSA
                else -> MimeTypes.APPLICATION_SUBRIP
            },
        )
        .setLanguage(s.language.takeIf { it.isNotEmpty() && it != "und" })
        .setLabel(s.label ?: s.language)
        .setId(s.id.ifEmpty { s.url })
        .setSelectionFlags(if (s.forced) C.SELECTION_FLAG_FORCED else 0)
        .build()
}
