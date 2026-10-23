// A popup over the page (What's new, New arrivals, Change password). It's a
// window of its own, so the remote's focus can't wander onto the page behind
// it, and Back closes it.
package io.github.okaza03.valo.ui

import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import kotlinx.coroutines.launch

/**
 * A box of `width` in the middle of the screen: `content`, which scrolls,
 * over `footer` (its buttons), which stays put so focusing a button doesn't
 * scroll the content away. Up and down move between the things that take
 * focus, and scroll `content` where there's nothing more to move to, so long
 * notes can be read to the end. Ask for focus inside it (a LaunchedEffect
 * with requestFocus).
 */
@Composable
fun Modal(
    width: Dp,
    onClose: () -> Unit,
    scroll: ScrollState = rememberScrollState(),
    /** How dark the page behind goes; the player keeps the picture visible. */
    dim: Float = 0.7f,
    footer: @Composable ColumnScope.() -> Unit,
    content: @Composable ColumnScope.() -> Unit,
) {
    Dialog(
        onDismissRequest = onClose,
        properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
    ) {
        // the page behind goes dark, like the desktop app's dialogs
        val window = (LocalView.current.parent as? DialogWindowProvider)?.window
        SideEffect { window?.setDimAmount(dim) }
        val focus = LocalFocusManager.current
        val scope = rememberCoroutineScope()
        val step = with(LocalDensity.current) { 140.dp.toPx() }
        val shape = RoundedCornerShape(4.dp)
        Column(
            Modifier
                .width(width)
                .heightIn(max = (LocalConfiguration.current.screenHeightDp * 0.9f).dp)
                .clip(shape)
                .background(BG)
                .border(1.dp, LINE, shape)
                .onPreviewKeyEvent { e ->
                    val (dir, by) = when (e.key) {
                        Key.DirectionDown -> FocusDirection.Down to step
                        Key.DirectionUp -> FocusDirection.Up to -step
                        else -> return@onPreviewKeyEvent false
                    }
                    if (e.type == KeyEventType.KeyDown && !focus.moveFocus(dir)) {
                        scope.launch { scroll.animateScrollBy(by) }
                    }
                    true
                },
        ) {
            Box(Modifier.weight(1f, fill = false)) {
                Column(Modifier.verticalScroll(scroll).padding(top = 32.dp), content = content)
                // there's more below
                if (scroll.canScrollForward) {
                    Box(
                        Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(48.dp)
                            .background(Brush.verticalGradient(listOf(Color.Transparent, BG))),
                    )
                }
            }
            Column(Modifier.padding(horizontal = 32.dp).padding(top = 12.dp, bottom = 28.dp), content = footer)
        }
    }
}
