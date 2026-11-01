// Full-screen playback on the TV (VideoPlayer has the rest). The controls
// (ExoPlayer's own) show the title on top while they're up. Holding OK opens
// the subtitles.
package io.github.okaza03.valo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.media3.ui.PlayerView
import androidx.tv.material3.Text
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Screen
import io.github.okaza03.valo.Subtitle

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

@Composable
fun PlayerScreen(model: AppModel, item: Item, screen: Screen.Player) {
    val key = screen.episodeId ?: item.id
    var panel by remember(key) { mutableStateOf(false) }

    // hold OK for the subtitle panel
    DisposableEffect(key) {
        PlayerKeys.onHold = { panel = true }
        onDispose { PlayerKeys.onHold = null }
    }

    VideoPlayer(
        model, item, screen,
        setup = {
            isFocusable = true
            isFocusableInTouchMode = true
            post { requestFocus() }
        },
        onView = { PlayerKeys.view = it },
        // the focused button just disappeared: keep the remote on the player
        onHud = { up -> if (!up) requestFocus() },
    ) { s ->
        if (s.hud && s.error == null) {
            Column(
                Modifier.fillMaxWidth()
                    .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.7f), Color.Transparent)))
                    .padding(start = EDGE, end = EDGE, top = 28.dp, bottom = 40.dp),
            ) {
                Text(
                    item.title, color = Color.White, fontSize = 24.sp, fontWeight = FontWeight.Bold,
                    maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
                val episode = s.episode
                if (episode != null) {
                    val season = item.seasons.find { c -> c.episodes.any { it.id == episode.id } }
                    val what = listOfNotNull(
                        season?.code(episode.number),
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
                files = s.files, fromAddons = s.fromAddons, embedded = s.embedded, picked = s.picked, delayMs = s.delayMs,
                loading = s.loading, error = s.subError,
                onPick = s.pick, onDelay = s.setDelay, onClose = { panel = false },
            )
        }
        s.error?.let {
            Column(Modifier.align(Alignment.Center).width(640.dp)) {
                Text(it, color = ERROR, fontSize = 18.sp)
                Spacer(Modifier.height(12.dp))
                Text("Press Back to return.", color = MUTED, fontSize = 15.sp)
            }
        }
    }
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
            for (f in files) Choice(subLabel(f), SubPick.File(f))
            if (embedded.isNotEmpty()) MonoText("In the video", 11.sp, MUTED)
            for (e in embedded) Choice(e.label, e)
            if (fromAddons.isNotEmpty()) MonoText("From the addons", 11.sp, MUTED)
            for (f in fromAddons) Choice(subLabel(f), SubPick.File(f))
            if (loading) Text("Loading the subtitles…", color = MUTED, fontSize = 13.sp)
            error?.let { Text(it, color = ERROR, fontSize = 13.sp) }
            if (picked is SubPick.Embedded) Text("The timing of subtitles in the video can't be moved.", color = MUTED, fontSize = 13.sp)
        }
    }
}
