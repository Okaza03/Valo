// What the TV and phone screens share: the desktop app's colors and fonts,
// server artwork, and how numbers and dates are written.
package io.github.okaza03.valo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.requiredHeight
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.foundation.text.BasicText
import androidx.compose.ui.text.TextStyle
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.platform.LocalDensity
import kotlin.math.sqrt
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
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

val OK = Color(0xFF62C88A)
val WARN = Color(0xFFF2B84B)
/** The desktop app's gold, for ratings, quality and progress on details pages (details::GOLD). */
val GOLD = Color(0xFFE8C46A)
/** Small captions ("Cast", "File · Metadata") (details::TEAL). */
val TEAL = Color(0xFF6ECDBE)

/** A row's caption, with how many it holds. */

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

/** The play button's label while an admin moves a title's files. */
const val MOVING_LABEL = "Unavailable — being moved"

/** The artwork of the titles being moved, from the latest library: it gets the MOVING band. */
var movingArt by mutableStateOf(emptySet<String>())

/**
 * A server image; a plain tile while it loads or when there is none. The
 * artwork of a title being moved carries the MOVING band, wherever it shows.
 */
@Composable
fun Art(url: String?, modifier: Modifier = Modifier, scale: ContentScale = ContentScale.Crop) {
    Box(modifier.background(SURFACE)) {
        if (url != null) {
            AsyncImage(model = url, contentDescription = null, contentScale = scale, modifier = Modifier.fillMaxSize())
            if (url in movingArt) MovingTape()
        }
    }
}

/**
 * The red "MOVING" band across a title's artwork while an admin moves its
 * files, as on the desktop app and the website: the art dims, and a band runs
 * edge to edge at -30°, its text sized to the art (its width, or its height
 * when it's wide).
 */
@Composable
fun MovingTape(modifier: Modifier = Modifier) {
    BoxWithConstraints(modifier.fillMaxSize().clipToBounds().background(Color.Black.copy(alpha = 0.5f)), contentAlignment = Alignment.Center) {
        val s = minOf(maxWidth, maxHeight, 240.dp)
        // long enough to cross the art corner to corner
        val length = sqrt(maxWidth.value * maxWidth.value + maxHeight.value * maxHeight.value).dp + s * 2
        val text = with(LocalDensity.current) { (s * 0.17f).toSp() }
        Box(
            Modifier.requiredWidth(length).requiredHeight(s * 0.26f).rotate(-30f)
                .shadow(6.dp).background(Color(0xFFD4202B))
                .drawBehind {
                    val y = size.height * 0.11f
                    val w = (s * 0.012f).toPx().coerceAtLeast(1f)
                    drawLine(Color.White, Offset(0f, y), Offset(size.width, y), w)
                    drawLine(Color.White, Offset(0f, size.height - y), Offset(size.width, size.height - y), w)
                },
            contentAlignment = Alignment.Center,
        ) {
            BasicText(
                "MOVING", style = TextStyle(color = Color.White, fontSize = text, fontWeight = FontWeight.Black),
                maxLines = 1, softWrap = false,
            )
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

@Composable
fun ProgressBar(fraction: Float, modifier: Modifier = Modifier) {
    Box(modifier.height(4.dp).clip(RoundedCornerShape(2.dp)).background(Color.White.copy(alpha = 0.27f))) {
        Box(Modifier.fillMaxHeight().fillMaxWidth(fraction.coerceIn(0f, 1f)).background(ACCENT))
    }
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

/** "Jan 5, 2021" from "2021-01-05". */
fun dateLong(s: String): String {
    val p = s.take(10).split('-').mapNotNull { it.toIntOrNull() }
    val months = listOf("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec")
    return when {
        p.size == 3 && p[1] in 1..12 -> "${months[p[1] - 1]} ${p[2]}, ${p[0]}"
        else -> s
    }
}

/** "Oct 23, 2026" from unix seconds. */
fun dateUnix(t: Long): String =
    java.text.SimpleDateFormat("MMM d, yyyy", java.util.Locale.US).format(java.util.Date(t * 1000))

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
