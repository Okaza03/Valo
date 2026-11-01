// Watching on the phone: full screen and sideways, with ExoPlayer's touch
// controls (VideoPlayer has the rest). Double tap a side to jump 10 seconds.
// While connected to a TV, this page is the remote instead.
package io.github.okaza03.valo.ui

import android.annotation.SuppressLint
import android.app.Activity
import android.content.pm.ActivityInfo
import android.view.GestureDetector
import android.view.MotionEvent
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.Player
import androidx.media3.ui.PlayerView
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.R
import io.github.okaza03.valo.Screen
import io.github.okaza03.valo.Subtitle
import io.github.okaza03.valo.cast.Caster

/** Jump this far with the skip buttons. */
private const val SKIP_MS = 10_000L

@Composable
fun PlayerScreen(model: AppModel, item: Item, screen: Screen.Player) {
    val cast = LocalCast.current
    val key = screen.episodeId ?: item.id
    // "from the start" counts once: moving to the TV and back mid-film carries on
    val fresh = remember(key) { mutableStateOf(screen.fromStart) }
    if (cast.device != null) {
        CastRemote(model, cast, item, screen, fresh.value) { fresh.value = false }
    } else {
        // a player already running keeps its place; a new one reads this
        PhonePlayer(model, item, if (fresh.value) screen else screen.copy(fromStart = false)) { fresh.value = false }
    }
}

/** Sideways with the system bars hidden, for as long as the video is up. */
@Composable
private fun FullScreen() {
    val view = LocalView.current
    val activity = LocalContext.current as Activity
    DisposableEffect(Unit) {
        val bars = WindowCompat.getInsetsController(activity.window, view)
        val orientation = activity.requestedOrientation
        activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        bars.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        bars.hide(WindowInsetsCompat.Type.systemBars())
        onDispose {
            activity.requestedOrientation = orientation
            bars.show(WindowInsetsCompat.Type.systemBars())
        }
    }
}

@Composable
private fun PhonePlayer(model: AppModel, item: Item, screen: Screen.Player, started: () -> Unit) {
    FullScreen()
    LaunchedEffect(Unit) { started() }
    var panel by remember(screen.episodeId) { mutableStateOf(false) }
    VideoPlayer(
        model, item, screen,
        setup = { player -> doubleTapToSkip(player) },
    ) { s ->
        if (s.hud || s.error != null) {
            Row(
                Modifier.fillMaxWidth()
                    .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.7f), Color.Transparent)))
                    .safeDrawingPadding().padding(horizontal = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconBtn(R.drawable.ic_back, "Back", Color.White) { model.back() }
                Column(Modifier.weight(1f)) {
                    Text(item.title, color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    val episode = s.episode
                    if (episode != null) {
                        val season = item.seasons.find { c -> c.episodes.any { it.id == episode.id } }
                        val what = listOfNotNull(season?.code(episode.number), episode.title.takeIf { it.isNotEmpty() }).joinToString("  ·  ")
                        Text(what, color = Color.White.copy(alpha = 0.8f), fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
                CastButton()
                IconBtn(R.drawable.ic_subtitles, "Subtitles", Color.White) { panel = true }
            }
        }
        if (panel) SubtitleSheet(s) { panel = false }
        s.error?.let {
            Column(Modifier.align(Alignment.Center).widthIn(max = 560.dp).padding(24.dp)) {
                Text(it, color = ERROR, fontSize = 16.sp)
            }
        }
    }
}

/** A double tap on the left or right side jumps back or ahead; single taps stay the controls'. */
@SuppressLint("ClickableViewAccessibility")
private fun PlayerView.doubleTapToSkip(player: Player) {
    val detector = GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
        override fun onDoubleTap(e: MotionEvent): Boolean {
            val by = if (e.x < width / 2f) -SKIP_MS else SKIP_MS
            player.seekTo((player.currentPosition + by).coerceAtLeast(0))
            return true
        }
    })
    setOnTouchListener { _, e ->
        detector.onTouchEvent(e)
        false
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Sheet(onClose: () -> Unit, content: @Composable () -> Unit) {
    // all the way up: sideways, half a sheet would hide the choices
    ModalBottomSheet(onClose, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true), containerColor = SURFACE) {
        Column(Modifier.padding(horizontal = PAD).padding(bottom = 24.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text("Subtitles", fontSize = 22.sp, fontWeight = FontWeight.Bold, color = TEXT)
            content()
        }
    }
}

@Composable
private fun SheetRow(text: String, on: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(6.dp)).background(if (on) Color.White else Color.Transparent).clickable(onClick = onClick).padding(12.dp),
    ) {
        Text(text, color = if (on) INK else TEXT, fontSize = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** Off, the files and the video's own tracks to pick from, and a file's timing. */
@Composable
private fun SubtitleSheet(s: PlayerState, onClose: () -> Unit) {
    @Composable
    fun Heading(text: String) = MonoText(text, 11.sp, MUTED, Modifier.padding(top = 10.dp, bottom = 2.dp))

    @Composable
    fun Files(list: List<Subtitle>) {
        for (f in list) SubPick.File(f).let { p -> SheetRow(subLabel(f), p == s.picked) { s.pick(p) } }
    }

    Sheet(onClose) {
        if (s.picked is SubPick.File) {
            Text("Timing: ${delayText(s.delayMs)}", color = if (s.delayMs == 0L) TEXT else GOLD, fontSize = 14.sp)
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                for ((label, by) in listOf("−0.5" to -500L, "−0.1" to -100L, "+0.1" to 100L, "+0.5" to 500L)) {
                    Btn(label, { s.setDelay(s.delayMs + by) }, Modifier.weight(1f))
                }
            }
            Text("Minus shows them earlier, plus later.", color = MUTED, fontSize = 12.sp)
        }
        SheetRow("Off", s.picked == SubPick.Off) { s.pick(SubPick.Off) }
        if (s.files.isNotEmpty()) Heading("On the server")
        Files(s.files)
        if (s.embedded.isNotEmpty()) Heading("In the video")
        for (e in s.embedded) SheetRow(e.label, e == s.picked) { s.pick(e) }
        if (s.fromAddons.isNotEmpty()) Heading("From the addons")
        Files(s.fromAddons)
        if (s.loading) Text("Loading the subtitles…", color = MUTED, fontSize = 13.sp)
        s.subError?.let { Text(it, color = ERROR, fontSize = 13.sp) }
    }
}

/** "1:02:03" or "2:03" */
private fun clock(ms: Long): String {
    val t = (ms / 1000).coerceAtLeast(0)
    return if (t >= 3600) "%d:%02d:%02d".format(t / 3600, t / 60 % 60, t % 60) else "%d:%02d".format(t / 60, t % 60)
}

/** What's on the TV, and the buttons for it. */
@Composable
private fun CastRemote(model: AppModel, cast: Caster, item: Item, screen: Screen.Player, fromStart: Boolean, started: () -> Unit) {
    val key = screen.episodeId ?: item.id
    LaunchedEffect(key) {
        if (cast.now?.key != key) cast.play(item, screen.episodeId, fromStart)
        started()
    }
    var panel by remember(key) { mutableStateOf(false) }
    /** Where the slider is held, until it's let go. */
    var dragging by remember(key) { mutableStateOf<Float?>(null) }
    val episode = screen.episodeId?.let { id -> item.episodes.find { it.id == id } }
    val season = episode?.let { e -> item.seasons.find { s -> s.episodes.any { it.id == e.id } } }
    val next = episode?.let { model.episodeAfter(item, it.id) }

    Box(Modifier.fillMaxSize().background(BG)) {
        Art(item.backdrop ?: item.poster, Modifier.fillMaxSize())
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0f to BG.copy(alpha = 0.6f), 0.7f to BG)))
        Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding()) {
            Row(Modifier.padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
                IconBtn(R.drawable.ic_back, "Back") { model.back() }
                Spacer(Modifier.weight(1f))
                CastButton()
            }
            Spacer(Modifier.weight(1f))
            Column(Modifier.padding(horizontal = 24.dp).fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                Art(item.poster, Modifier.height(240.dp).aspectRatio(2f / 3f).clip(RoundedCornerShape(8.dp)))
                Spacer(Modifier.height(20.dp))
                Text(item.title, color = TEXT, fontSize = 22.sp, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center, maxLines = 2, overflow = TextOverflow.Ellipsis)
                if (episode != null) {
                    Text(
                        listOfNotNull(season?.code(episode.number), episode.title.takeIf { it.isNotEmpty() }).joinToString("  ·  "),
                        color = MUTED, fontSize = 14.sp, textAlign = TextAlign.Center, maxLines = 1, overflow = TextOverflow.Ellipsis,
                    )
                }
                Spacer(Modifier.height(4.dp))
                MonoText("Playing on ${cast.device ?: "your TV"}", 11.sp, TEAL)
                cast.error?.let {
                    Spacer(Modifier.height(12.dp))
                    Text(it, color = ERROR, fontSize = 14.sp, textAlign = TextAlign.Center)
                }
            }
            Spacer(Modifier.weight(1f))
            Column(Modifier.padding(horizontal = 24.dp, vertical = 16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                val dur = cast.durationMs.toFloat().coerceAtLeast(1f)
                Slider(
                    value = (dragging ?: cast.positionMs.toFloat()).coerceIn(0f, dur),
                    onValueChange = { dragging = it },
                    onValueChangeFinished = {
                        dragging?.let { cast.seek(it.toLong()) }
                        dragging = null
                    },
                    valueRange = 0f..dur,
                    enabled = cast.durationMs > 0,
                )
                Row(Modifier.fillMaxWidth()) {
                    Text(clock(dragging?.toLong() ?: cast.positionMs), color = MUTED, fontSize = 12.sp)
                    Spacer(Modifier.weight(1f))
                    Text(clock(cast.durationMs), color = MUTED, fontSize = 12.sp)
                }
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    IconBtn(R.drawable.ic_subtitles, "Subtitles") { panel = true }
                    Skip("−10") { cast.seek((cast.positionMs - SKIP_MS).coerceAtLeast(0)) }
                    Box(Modifier.size(64.dp).clip(CircleShape).background(Color.White).clickable(onClick = cast::togglePause), contentAlignment = Alignment.Center) {
                        if (cast.buffering) {
                            CircularProgressIndicator(Modifier.size(28.dp), color = INK)
                        } else {
                            Ic(if (cast.playing) R.drawable.ic_pause else R.drawable.ic_play, if (cast.playing) "Pause" else "Play", Modifier.size(32.dp), INK)
                        }
                    }
                    Skip("+10") { cast.seek((cast.positionMs + SKIP_MS).coerceAtMost(cast.durationMs)) }
                    if (next != null) {
                        IconBtn(R.drawable.ic_next, "Next episode") { model.swap(Screen.Player(item.id, next.id)) }
                    } else {
                        Spacer(Modifier.size(48.dp))
                    }
                }
                Spacer(Modifier.height(16.dp))
                Btn("Stop casting", {
                    cast.stop()
                    model.back()
                }, Modifier.fillMaxWidth())
            }
        }
        if (panel) {
            Sheet({ panel = false }) {
                SheetRow("Off", cast.activeTrack == null) { cast.pickTrack(null) }
                for (t in cast.tracks) SheetRow(t.label, t.id == cast.activeTrack) { cast.pickTrack(t.id) }
                if (cast.tracks.isEmpty()) Text("This title has no subtitle files.", color = MUTED, fontSize = 13.sp)
                Text("Subtitles inside the video file can't be shown on a TV, and the timing can't be moved there.", color = MUTED, fontSize = 12.sp)
            }
        }
    }
}

@Composable
private fun Skip(text: String, onClick: () -> Unit) {
    Box(Modifier.size(48.dp).clip(CircleShape).clickable(onClick = onClick), contentAlignment = Alignment.Center) {
        Text(text, color = TEXT, fontSize = 14.sp, fontWeight = FontWeight.Bold)
    }
}
