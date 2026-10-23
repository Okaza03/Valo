// Shared look: the desktop app's colors, fonts, cards, buttons, tabs and
// menus, sized for a TV across the room and driven by the remote's D-pad.
package io.github.okaza03.valo.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.zIndex
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import androidx.tv.material3.Border
import androidx.tv.material3.Button
import androidx.tv.material3.ButtonDefaults
import androidx.tv.material3.Card
import androidx.tv.material3.CardDefaults
import androidx.tv.material3.Glow
import androidx.tv.material3.LocalContentColor
import androidx.tv.material3.LocalTextStyle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.ProvideTextStyle
import androidx.tv.material3.Text
import androidx.tv.material3.darkColorScheme
import coil3.compose.AsyncImage
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.R
import kotlin.math.pow
import kotlin.math.roundToInt

// the desktop app's palette (apps/valo/src/ui/mod.rs)
val BG = Color(0xFF121317)
val SURFACE = Color(0xFF191A1F)
val SURFACE_HI = Color(0xFF222329)
val LINE = Color(0xFF2C2D33)
val LINE_HI = Color(0xFF565860)
val ACCENT = Color(0xFF4A7CF7)
val TEXT = Color(0xFFECECEE)
val MUTED = Color(0xFF8B8D94)
/** Text on white (selected tabs, primary buttons). */
val INK = Color(0xFF0E0F12)
val ERROR = Color(0xFFFF786E)

// and its fonts (apps/valo/assets/fonts, Latin only)
val Sans = FontFamily(
    Font(R.font.noto_sans),
    Font(R.font.noto_sans_bold, FontWeight.Bold),
    Font(R.font.noto_sans_black, FontWeight.Black),
)
val Mono = FontFamily(Font(R.font.noto_sans_mono, FontWeight.Medium))

/** Space kept clear at the screen's edges, which TVs may crop. */
val EDGE = 48.dp

@Composable
fun ValoTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = ACCENT,
            onPrimary = Color.White,
            background = BG,
            surface = SURFACE,
            onSurface = TEXT,
            surfaceVariant = SURFACE_HI,
            onSurfaceVariant = MUTED,
            error = ERROR,
        ),
    ) {
        ProvideTextStyle(LocalTextStyle.current.merge(TextStyle(fontFamily = Sans)), content)
    }
}

/** The desktop app's label style: monospace capitals, spaced out. */
@Composable
fun MonoText(text: String, size: TextUnit, color: Color = TEXT, modifier: Modifier = Modifier) {
    Text(
        text.uppercase(), color = color, fontSize = size, fontFamily = Mono, fontWeight = FontWeight.Medium,
        letterSpacing = size * 0.14f, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = modifier,
    )
}

/** A stable muted color per title, for artwork that's missing (util::color_for). */
fun colorFor(s: String): Color {
    var h = 2166136261u
    for (b in s.encodeToByteArray()) h = (h xor b.toUByte().toUInt()) * 16777619u
    // egui's Hsva is in linear light; convert like it does
    val hue = (h % 360u).toFloat() / 60f
    val (sat, v) = 0.45f to 0.42f
    val c = v * sat
    val x = c * (1 - kotlin.math.abs(hue % 2 - 1))
    val (r, g, b) = when (hue.toInt()) {
        0 -> Triple(c, x, 0f)
        1 -> Triple(x, c, 0f)
        2 -> Triple(0f, c, x)
        3 -> Triple(0f, x, c)
        4 -> Triple(x, 0f, c)
        else -> Triple(c, 0f, x)
    }
    fun srgb(l: Float) = if (l <= 0.0031308f) 12.92f * l else 1.055f * l.toDouble().pow(1 / 2.4).toFloat() - 0.055f
    val m = v - c
    return Color(srgb(r + m), srgb(g + m), srgb(b + m))
}

/** Focus this once it's on screen; lazy lists add their items a frame or two late. */
suspend fun FocusRequester.focusSoon() {
    repeat(20) {
        try {
            requestFocus()
            return
        } catch (_: IllegalStateException) {
            kotlinx.coroutines.delay(25)
        }
    }
}

/** A server image; a plain tile while it loads or when there is none. */
@Composable
fun Art(url: String?, modifier: Modifier = Modifier, scale: ContentScale = ContentScale.Crop) {
    Box(modifier.background(SURFACE)) {
        if (url != null) {
            AsyncImage(model = url, contentDescription = null, contentScale = scale, modifier = Modifier.fillMaxSize())
        }
    }
}

/** A title's logo, drawn as is: no tile behind it. */
@Composable
fun Logo(url: String, title: String, modifier: Modifier = Modifier) {
    AsyncImage(
        model = url, contentDescription = title, contentScale = ContentScale.Fit,
        alignment = Alignment.CenterStart, modifier = modifier,
    )
}

/** Big artwork behind a screen, fading into the background on the left and bottom. */
@Composable
fun BoxScope.Backdrop(url: String?) {
    if (url == null) return
    Box(Modifier.align(Alignment.TopEnd).fillMaxWidth(0.72f).fillMaxHeight(0.75f)) {
        AsyncImage(model = url, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
        Box(Modifier.fillMaxSize().background(Brush.horizontalGradient(0f to BG, 0.45f to BG.copy(alpha = 0.35f), 1f to Color.Transparent)))
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.5f to Color.Transparent, 1f to BG)))
    }
}

@Composable
fun ProgressBar(fraction: Float, modifier: Modifier = Modifier) {
    Box(modifier.height(4.dp).clip(RoundedCornerShape(2.dp)).background(Color.White.copy(alpha = 0.27f))) {
        Box(Modifier.fillMaxHeight().fillMaxWidth(fraction.coerceIn(0f, 1f)).background(ACCENT))
    }
}

/** One artwork card with a caption, for rows and grids. */
@Composable
fun ArtCard(
    art: String?,
    title: String,
    sub: String?,
    width: Dp,
    ratio: Float,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    progress: Float? = null,
    /** A watched movie: a "WATCHED" sash across the top right corner. */
    watched: Boolean = false,
    /** Wide cards that start playback show ▶ when focused. */
    plays: Boolean = false,
    onLongClick: (() -> Unit)? = null,
    onFocus: () -> Unit = {},
    focusRequester: FocusRequester? = null,
) {
    var focused by remember { mutableStateOf(false) }
    val shape = RoundedCornerShape(6.dp)
    Column(modifier.width(width)) {
        Card(
            onClick = onClick,
            onLongClick = onLongClick,
            shape = CardDefaults.shape(shape),
            scale = CardDefaults.scale(focusedScale = 1.05f),
            border = CardDefaults.border(focusedBorder = Border(BorderStroke(2.dp, TEXT), shape = shape)),
            colors = CardDefaults.colors(containerColor = SURFACE),
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(ratio)
                .then(if (focusRequester != null) Modifier.focusRequester(focusRequester) else Modifier)
                .onFocusChanged {
                    focused = it.isFocused
                    if (it.isFocused) onFocus()
                },
        ) {
            Box(Modifier.fillMaxSize()) {
                if (art != null) {
                    Art(art, Modifier.fillMaxSize())
                } else {
                    Placeholder(title)
                }
                if (watched) WatchedSash()
                if (progress != null && progress > 0f) {
                    ProgressBar(progress, Modifier.align(Alignment.BottomStart).fillMaxWidth().padding(8.dp, 0.dp, 8.dp, 5.dp))
                }
                if (plays && focused) {
                    Box(
                        Modifier.align(Alignment.Center).size(48.dp).clip(CircleShape).background(Color.Black.copy(alpha = 0.63f)),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text("▶", color = Color.White, fontSize = 20.sp, modifier = Modifier.padding(start = 3.dp))
                    }
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(
            title, maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 14.sp,
            color = if (focused) TEXT else TEXT.copy(alpha = 0.85f),
            modifier = Modifier.padding(start = 2.dp),
        )
        if (sub != null) {
            Text(sub, maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 12.sp, color = MUTED, modifier = Modifier.padding(start = 2.dp))
        }
    }
}

/** The desktop app's "WATCHED" sash: a band across the corner, clipped by the card. */
@Composable
private fun BoxScope.WatchedSash() {
    // the band's middle sits 17dp in from both edges
    Box(Modifier.align(Alignment.TopEnd).size(34.dp)) {
        Box(
            Modifier.align(Alignment.Center).requiredWidth(90.dp).height(14.dp).rotate(45f).background(ACCENT),
            contentAlignment = Alignment.Center,
        ) {
            Text("WATCHED", color = Color.White, fontSize = 7.5.sp, fontWeight = FontWeight.Bold, letterSpacing = 0.5.sp, maxLines = 1)
        }
    }
}

/** A colored tile with the title on it, for artwork that's missing. */
@Composable
private fun BoxScope.Placeholder(title: String) {
    Box(Modifier.fillMaxSize().background(colorFor(title)))
    Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.5f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.55f))))
    Text(
        title, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center,
        modifier = Modifier.align(Alignment.Center).padding(8.dp),
    )
}

/*
 * How focus shows on every button: it turns white with dark text, grows a
 * little and glows, so it stands out from a selected (white) one. No outlines.
 */
@Composable
fun focusScale() = ButtonDefaults.scale(focusedScale = 1.08f)

@Composable
fun focusGlow() = ButtonDefaults.glow(focusedGlow = Glow(Color.White.copy(alpha = 0.55f), 10.dp))

/**
 * The desktop app's buttons: white with dark text (primary) or dark with a
 * light outline, labels in monospace capitals. Focused ones turn white.
 */
@Composable
fun TvButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    primary: Boolean = false,
    selected: Boolean = false,
    enabled: Boolean = true,
) {
    val white = primary || selected
    Button(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.height(40.dp),
        shape = ButtonDefaults.shape(RoundedCornerShape(2.dp)),
        scale = focusScale(),
        glow = focusGlow(),
        contentPadding = PaddingValues(horizontal = 18.dp),
        colors = ButtonDefaults.colors(
            containerColor = if (white) Color.White else Color.Black.copy(alpha = 0.24f),
            contentColor = if (white) INK else TEXT,
            focusedContainerColor = Color.White,
            focusedContentColor = INK,
            disabledContainerColor = if (white) Color.White.copy(alpha = 0.5f) else Color.Black.copy(alpha = 0.24f),
            disabledContentColor = if (white) INK else MUTED,
        ),
        border = ButtonDefaults.border(
            border = if (white) Border.None else Border(BorderStroke(1.dp, LINE_HI), shape = RoundedCornerShape(2.dp)),
            focusedBorder = Border.None,
        ),
    ) {
        Text(
            text.uppercase(), fontSize = 13.sp, fontFamily = Mono, fontWeight = FontWeight.Medium,
            letterSpacing = 1.8.sp, maxLines = 1,
        )
    }
}

/** A top-bar tab: plain text, or a white block when selected (chrome::tab). */
@Composable
fun Tab(text: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier, count: Int? = null) {
    Button(
        onClick = onClick,
        modifier = modifier.height(32.dp),
        shape = ButtonDefaults.shape(RoundedCornerShape(1.dp)),
        scale = focusScale(),
        glow = focusGlow(),
        contentPadding = PaddingValues(horizontal = 10.dp),
        colors = ButtonDefaults.colors(
            containerColor = if (selected) Color.White else Color.Transparent,
            contentColor = if (selected) INK else TEXT,
            focusedContainerColor = Color.White,
            focusedContentColor = INK,
        ),
    ) {
        TabLabel(text, count)
    }
}

@Composable
private fun TabLabel(text: String, count: Int?) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(text.uppercase(), fontSize = 12.sp, fontFamily = Mono, fontWeight = FontWeight.Medium, letterSpacing = 1.6.sp, maxLines = 1)
        if (count != null) {
            Spacer(Modifier.width(9.dp))
            Text(
                count.toString(), fontSize = 12.sp, fontFamily = Mono, letterSpacing = 1.6.sp,
                color = LocalContentColor.current.copy(alpha = 0.55f),
            )
        }
    }
}

/** A bordered row of options, the selected one white (ui::segmented). */
@Composable
fun <T> Segmented(value: T, items: List<Triple<T, String, Int?>>, onPick: (T) -> Unit, modifier: Modifier = Modifier) {
    // the outline goes behind the options, so a focused (grown) one covers it
    Row(modifier.height(IntrinsicSize.Min).drawBehind { drawRect(LINE, style = Stroke(1.dp.toPx())) }) {
        items.forEachIndexed { i, (v, label, count) ->
            if (i > 0) Box(Modifier.width(1.dp).fillMaxHeight().background(LINE))
            val sel = v == value
            var focused by remember { mutableStateOf(false) }
            Button(
                onClick = { onPick(v) },
                // grown on focus, so drawn above its neighbours
                modifier = Modifier.height(38.dp).zIndex(if (focused) 1f else 0f).onFocusChanged { focused = it.isFocused },
                shape = ButtonDefaults.shape(RoundedCornerShape(0.dp)),
                scale = focusScale(),
                glow = focusGlow(),
                contentPadding = PaddingValues(horizontal = 14.dp),
                colors = ButtonDefaults.colors(
                    containerColor = if (sel) Color.White else Color.Transparent,
                    contentColor = if (sel) INK else TEXT,
                    focusedContainerColor = Color.White,
                    focusedContentColor = INK,
                ),
            ) {
                TabLabel(label, count)
            }
        }
    }
}

/** A row in a [Menu]. */
class MenuItem(val text: String, val onClick: () -> Unit)

/**
 * A dropdown below its button, like the desktop app's popups. The first item
 * takes focus; Back closes it.
 */
@Composable
fun Menu(
    open: Boolean,
    onDismiss: () -> Unit,
    items: List<MenuItem?>,
    alignEnd: Boolean = false,
    header: (@Composable () -> Unit)? = null,
) {
    if (!open) return
    val first = remember { FocusRequester() }
    Popup(
        alignment = if (alignEnd) Alignment.TopEnd else Alignment.TopStart,
        offset = IntOffset(0, with(androidx.compose.ui.platform.LocalDensity.current) { 46.dp.roundToPx() }),
        onDismissRequest = onDismiss,
        properties = PopupProperties(focusable = true),
    ) {
        LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
        Column(
            Modifier
                .widthIn(min = 250.dp)
                .width(IntrinsicSize.Max)
                .clip(RoundedCornerShape(4.dp))
                .background(SURFACE)
                .border(1.dp, LINE, RoundedCornerShape(4.dp))
                .padding(6.dp),
        ) {
            if (header != null) {
                header()
                Divider()
            }
            var firstSet = false
            for (item in items) {
                if (item == null) {
                    Divider()
                    continue
                }
                val req = if (!firstSet) first.also { firstSet = true } else null
                var focused by remember { mutableStateOf(false) }
                Button(
                    onClick = { onDismiss(); item.onClick() },
                    modifier = Modifier.fillMaxWidth().height(36.dp)
                        .then(if (req != null) Modifier.focusRequester(req) else Modifier)
                        .onFocusChanged { focused = it.isFocused },
                    shape = ButtonDefaults.shape(RoundedCornerShape(2.dp)),
                    scale = ButtonDefaults.scale(focusedScale = 1f),
                    contentPadding = PaddingValues(horizontal = 10.dp),
                    colors = ButtonDefaults.colors(
                        containerColor = Color.Transparent,
                        contentColor = TEXT,
                        focusedContainerColor = Color.White,
                        focusedContentColor = INK,
                    ),
                ) {
                    Text(item.text, fontSize = 15.sp, modifier = Modifier.fillMaxWidth())
                }
            }
        }
    }
}

@Composable
private fun Divider() {
    Box(Modifier.padding(vertical = 6.dp).fillMaxWidth().height(1.dp).background(LINE))
}

/** A round badge with the name's first letter (chrome::avatar). */
@Composable
fun Avatar(name: String, size: Dp) {
    Box(Modifier.size(size).clip(CircleShape).background(ACCENT), contentAlignment = Alignment.Center) {
        Text(
            name.firstOrNull()?.uppercase() ?: "?", color = TEXT, fontWeight = FontWeight.Black,
            fontSize = (size.value * 0.52f).sp, lineHeight = (size.value * 0.52f).sp,
        )
    }
}

/** The circular arrow the desktop app uses for "check for updates". */
@Composable
fun RefreshIcon(color: Color, modifier: Modifier = Modifier) {
    Canvas(modifier.size(18.dp)) {
        val r = size.minDimension * 0.39f
        val c = center
        val start = -0.35 * Math.PI
        val sweep = 1.65 * Math.PI
        val pts = (0..24).map { i ->
            val a = start + i / 24.0 * sweep
            Offset(c.x + (kotlin.math.cos(a) * r).toFloat(), c.y + (kotlin.math.sin(a) * r).toFloat())
        }
        val line = Path().apply {
            moveTo(pts[0].x, pts[0].y)
            pts.drop(1).forEach { lineTo(it.x, it.y) }
        }
        drawPath(line, color, style = Stroke(width = 1.8.dp.toPx()))
        val tip = pts[0]
        val u = size.minDimension / 18f
        val head = Path().apply {
            moveTo(tip.x - 4.5f * u, tip.y - 2f * u)
            lineTo(tip.x + 2.5f * u, tip.y - 3.5f * u)
            lineTo(tip.x + 1f * u, tip.y + 3.5f * u)
            close()
        }
        drawPath(head, color)
    }
}

/** A text field that shows the on-screen keyboard when the remote's OK button is pressed. */
@Composable
fun TvTextField(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    password: Boolean = false,
    keyboard: KeyboardType = KeyboardType.Text,
    imeAction: ImeAction = ImeAction.Next,
    onDone: () -> Unit = {},
) {
    Column(modifier) {
        MonoText(label, 12.sp, MUTED)
        Spacer(Modifier.height(6.dp))
        Field(value, onChange, Modifier.fillMaxWidth(), password = password, keyboard = keyboard, imeAction = imeAction, onDone = onDone)
    }
}

/** The bare input box, also used for search in the top bar. */
@Composable
fun Field(
    value: String,
    onChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    hint: String? = null,
    password: Boolean = false,
    keyboard: KeyboardType = KeyboardType.Text,
    imeAction: ImeAction = ImeAction.Next,
    compact: Boolean = false,
    onDone: () -> Unit = {},
) {
    var focused by remember { mutableStateOf(false) }
    val shape = RoundedCornerShape(2.dp)
    val focus = LocalFocusManager.current
    BasicTextField(
        value = value,
        onValueChange = onChange,
        singleLine = true,
        textStyle = TextStyle(color = TEXT, fontSize = if (compact) 15.sp else 18.sp, fontFamily = Sans),
        cursorBrush = SolidColor(TEXT),
        visualTransformation = if (password) PasswordVisualTransformation() else VisualTransformation.None,
        keyboardOptions = KeyboardOptions(
            keyboardType = if (password) KeyboardType.Password else keyboard,
            imeAction = imeAction,
            autoCorrectEnabled = false,
        ),
        keyboardActions = KeyboardActions(onDone = { onDone() }, onGo = { onDone() }, onSearch = { onDone() }),
        modifier = modifier
            .onFocusChanged { focused = it.isFocused }
            // the remote's up and down leave the field; a single line has nowhere to go
            .onPreviewKeyEvent { e ->
                val dir = when (e.key) {
                    Key.DirectionDown -> FocusDirection.Down
                    Key.DirectionUp -> FocusDirection.Up
                    else -> return@onPreviewKeyEvent false
                }
                if (e.type == KeyEventType.KeyDown) focus.moveFocus(dir)
                true
            }
            .clip(shape)
            .background(Color(0xFF16171B))
            .border(if (focused) 2.dp else 1.dp, if (focused) TEXT else LINE_HI, shape),
        decorationBox = { inner ->
            Box(
                Modifier.padding(horizontal = if (compact) 10.dp else 14.dp, vertical = if (compact) 7.dp else 12.dp),
                contentAlignment = Alignment.CenterStart,
            ) {
                if (value.isEmpty() && hint != null) {
                    Text(hint, color = MUTED, fontSize = if (compact) 15.sp else 18.sp, maxLines = 1)
                }
                inner()
            }
        },
    )
}

fun fmtRuntime(seconds: Double): String {
    val m = (seconds / 60).roundToInt()
    return if (m >= 60) "${m / 60}h ${m % 60}m" else "${m}m"
}

/** "2010 · 2h 28m · PG-13 · Sci-Fi, Action" */
fun metaLine(item: Item): String = buildList {
    item.year?.let { y -> add(if (item.endYear != null && item.endYear != y) "$y–${item.endYear}" else "$y") }
    when (item.kind) {
        Kind.Movie -> item.duration?.takeIf { it > 0 }?.let { add(fmtRuntime(it)) }
        Kind.Series -> {
            val n = item.seasonCount
            add("$n season${if (n == 1) "" else "s"}")
        }
    }
    item.rating?.let { add(it) }
    item.score?.let { add("★ %.1f".format(it)) }
    if (item.genres.isNotEmpty()) add(item.genres.take(3).joinToString(", "))
}.joinToString("  ·  ")

/** The desktop app's gold, for ratings, quality and progress on details pages (details::GOLD). */
val GOLD = Color(0xFFE8C46A)
/** Small captions ("Cast", "File · Metadata") (details::TEAL). */
val TEAL = Color(0xFF6ECDBE)

/** A row's caption, with how many it holds. */
@Composable
fun RowTitle(text: String, count: Int? = null, modifier: Modifier = Modifier) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = modifier.padding(start = EDGE, bottom = 10.dp)) {
        MonoText(text, 13.sp)
        if (count != null) {
            Spacer(Modifier.width(10.dp))
            MonoText(count.toString(), 13.sp, MUTED)
        }
    }
}

/**
 * A block of text the remote can land on, so a list scrolls down to it
 * (text alone can't take focus). Outlined while focused.
 */
@Composable
fun Panel(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    var focused by remember { mutableStateOf(false) }
    val shape = RoundedCornerShape(6.dp)
    Column(
        modifier
            .onFocusChanged { focused = it.isFocused }
            .focusable()
            .clip(shape)
            .background(SURFACE.copy(alpha = 0.85f))
            .border(if (focused) 2.dp else 1.dp, if (focused) TEXT else LINE, shape)
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) { content() }
}

/** Label and value pairs, the labels in a column of their own (details::kv_rows). */
@Composable
fun InfoRows(rows: List<Pair<String, String>>, labelWidth: Dp = 130.dp, valueColor: (String) -> Color = { TEXT }) {
    for ((k, v) in rows) {
        Row {
            Text(k, color = MUTED, fontSize = 14.sp, modifier = Modifier.width(labelWidth))
            Text(v, color = valueColor(k), fontSize = 14.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** A person in a cast row: round photo (or initial), name, and their part. */
@Composable
fun PersonCard(name: String, photo: String?, role: String?, onClick: () -> Unit) {
    var focused by remember { mutableStateOf(false) }
    Column(Modifier.width(120.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Card(
            onClick = onClick,
            shape = CardDefaults.shape(CircleShape),
            scale = CardDefaults.scale(focusedScale = 1.08f),
            border = CardDefaults.border(focusedBorder = Border(BorderStroke(2.dp, TEXT), shape = CircleShape)),
            colors = CardDefaults.colors(containerColor = SURFACE),
            modifier = Modifier.size(96.dp).onFocusChanged { focused = it.isFocused },
        ) {
            if (photo != null) {
                Art(photo, Modifier.fillMaxSize())
            } else {
                Box(Modifier.fillMaxSize().background(colorFor(name)), contentAlignment = Alignment.Center) {
                    Text(name.firstOrNull()?.uppercase() ?: "?", color = Color.White, fontSize = 34.sp, fontWeight = FontWeight.Black)
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(
            name, fontSize = 14.sp, color = if (focused) TEXT else TEXT.copy(alpha = 0.85f),
            maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center,
        )
        if (role != null) {
            Text(role, fontSize = 12.sp, color = MUTED, maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center)
        }
    }
}

/**
 * Something with a menu that holding OK opens (a card's "Add to My list").
 * `content` gets a modifier for it, so focus comes back to it after the menu.
 */
@Composable
fun WithMenu(items: () -> List<MenuItem?>, content: @Composable (Modifier, openMenu: () -> Unit) -> Unit) {
    var open by remember { mutableStateOf(false) }
    var wasOpen by remember { mutableStateOf(false) }
    val back = remember { FocusRequester() }
    LaunchedEffect(open) {
        if (open) wasOpen = true else if (wasOpen) runCatching { back.requestFocus() }
    }
    Box {
        content(Modifier.focusRequester(back)) { open = true }
        Menu(open = open, onDismiss = { open = false }, items = if (open) items() else emptyList())
    }
}

/** "Jan 5, 2021" from "2021-01-05". */
fun dateLong(s: String): String {
    val p = s.take(10).split('-').mapNotNull { it.toIntOrNull() }
    val months = listOf("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec")
    return when {
        p.size == 3 && p[1] in 1..12 -> "${months[p[1] - 1]} ${p[2]}, ${p[0]}"
        else -> s
    }
}

/** "$160,000,000" */
fun money(n: Long): String = "$" + "%,d".format(java.util.Locale.US, n)

/** "1.4 GB" */
fun bytes(n: Long): String = when {
    n >= 1L shl 30 -> "%.1f GB".format(n / (1L shl 30).toDouble())
    n >= 1L shl 20 -> "%.0f MB".format(n / (1L shl 20).toDouble())
    else -> "%.0f KB".format(n / 1024.0)
}

/** "8.2 Mbps" */
fun bitrate(n: Long): String = "%.1f Mbps".format(n / 1_000_000.0)

/** "just now", "3 hours ago", "yesterday", "12 days ago", … (people::ago) */
fun ago(t: Long): String {
    val s = (System.currentTimeMillis() / 1000 - t).coerceAtLeast(0)
    fun plural(n: Long, unit: String) = "$n $unit${if (n == 1L) "" else "s"} ago"
    return when {
        s < 60 -> "just now"
        s < 3600 -> plural(s / 60, "minute")
        s < 86_400 -> plural(s / 3600, "hour")
        s < 2 * 86_400 -> "yesterday"
        s < 60 * 86_400 -> plural(s / 86_400, "day")
        s < 365 * 86_400 -> plural(s / (30 * 86_400), "month")
        else -> plural(s / (365 * 86_400), "year")
    }
}
