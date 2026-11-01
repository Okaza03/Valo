// The phone's look: the desktop app's colors and fonts on touch controls.
package io.github.okaza03.valo.ui

import android.view.ContextThemeWrapper
import androidx.annotation.DrawableRes
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.mediarouter.app.MediaRouteButton
import com.google.android.gms.cast.framework.CastButtonFactory
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.R
import io.github.okaza03.valo.Screen
import io.github.okaza03.valo.cast.Caster

/** Space kept clear at the screen's sides. */
val PAD = 16.dp

val LocalCast = staticCompositionLocalOf<Caster> { error("no Caster") }

private fun TextStyle.sans() = copy(fontFamily = Sans)

@Composable
fun ValoTheme(content: @Composable () -> Unit) {
    val t = Typography()
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = Color.White,
            onPrimary = INK,
            secondary = ACCENT,
            secondaryContainer = SURFACE_HI,
            onSecondaryContainer = TEXT,
            background = BG,
            onBackground = TEXT,
            surface = SURFACE,
            onSurface = TEXT,
            surfaceVariant = SURFACE_HI,
            onSurfaceVariant = MUTED,
            surfaceContainer = SURFACE,
            surfaceContainerHigh = SURFACE_HI,
            surfaceContainerHighest = SURFACE_HI,
            outline = LINE_HI,
            outlineVariant = LINE,
            error = ERROR,
        ),
        typography = Typography(
            t.displayLarge.sans(), t.displayMedium.sans(), t.displaySmall.sans(),
            t.headlineLarge.sans(), t.headlineMedium.sans(), t.headlineSmall.sans(),
            t.titleLarge.sans(), t.titleMedium.sans(), t.titleSmall.sans(),
            t.bodyLarge.sans(), t.bodyMedium.sans(), t.bodySmall.sans(),
            t.labelLarge.sans(), t.labelMedium.sans(), t.labelSmall.sans(),
        ),
        content = content,
    )
}

/** The desktop app's label style: monospace capitals, spaced out. */
@Composable
fun MonoText(text: String, size: TextUnit, color: Color = TEXT, modifier: Modifier = Modifier) {
    Text(
        text.uppercase(), color = color, fontSize = size, fontFamily = Mono, fontWeight = FontWeight.Medium,
        letterSpacing = size * 0.14f, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = modifier,
    )
}

@Composable
fun Ic(@DrawableRes id: Int, what: String?, modifier: Modifier = Modifier, tint: Color = TEXT) {
    Icon(painterResource(id), what, modifier, tint)
}

@Composable
fun IconBtn(@DrawableRes id: Int, what: String, tint: Color = TEXT, onClick: () -> Unit) {
    IconButton(onClick) { Ic(id, what, tint = tint) }
}

/** A round button that stays readable over artwork. */
@Composable
fun BackButton(onClick: () -> Unit) {
    Box(Modifier.padding(8.dp).size(48.dp).clip(CircleShape).background(Color.Black.copy(alpha = 0.45f)), contentAlignment = Alignment.Center) {
        IconBtn(R.drawable.ic_back, "Back", onClick = onClick)
    }
}

/**
 * The Cast button, which opens the device picker. It isn't there on phones
 * without Google Play services.
 */
@Composable
fun CastButton(modifier: Modifier = Modifier) {
    if (!LocalCast.current.available) return
    AndroidView(
        factory = { ctx ->
            MediaRouteButton(ContextThemeWrapper(ctx, androidx.appcompat.R.style.Theme_AppCompat)).also {
                CastButtonFactory.setUpMediaRouteButton(ctx.applicationContext, it)
            }
        },
        modifier = modifier.size(48.dp),
    )
}

/** The main button on a page, or a quieter one. */
@Composable
fun Btn(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, primary: Boolean = false, enabled: Boolean = true, @DrawableRes icon: Int? = null) {
    val shape = RoundedCornerShape(6.dp)
    val content: @Composable () -> Unit = {
        if (icon != null) {
            Ic(icon, null, Modifier.size(20.dp), if (primary) INK else TEXT)
            Spacer(Modifier.width(8.dp))
        }
        Text(text, maxLines = 1, overflow = TextOverflow.Ellipsis, fontWeight = FontWeight.Bold)
    }
    if (primary) {
        Button(onClick, modifier, enabled, shape) { content() }
    } else {
        OutlinedButton(onClick, modifier, enabled, shape, ButtonDefaults.outlinedButtonColors(contentColor = TEXT)) { content() }
    }
}

/** Choices in a row that scrolls, with how many there are of each. `edge`: the space at its ends. */
@Composable
fun <T> Chips(value: T, items: List<Triple<T, String, Int?>>, onPick: (T) -> Unit, modifier: Modifier = Modifier, edge: Dp = PAD) {
    LazyRow(modifier, contentPadding = PaddingValues(horizontal = edge), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        items(items, key = { it.second }) { (v, label, n) ->
            FilterChip(
                selected = v == value,
                onClick = { onPick(v) },
                label = { Text(if (n != null) "$label  $n" else label, maxLines = 1) },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Color.White, selectedLabelColor = INK),
            )
        }
    }
}

/** A button showing its current value, opening the choices. */
@Composable
fun <T> Choice(label: String, value: T, options: List<Pair<T, String>>, modifier: Modifier = Modifier, onPick: (T) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box(modifier) {
        OutlinedButton({ open = true }, Modifier.fillMaxWidth(), shape = RoundedCornerShape(6.dp), colors = ButtonDefaults.outlinedButtonColors(contentColor = TEXT), contentPadding = PaddingValues(horizontal = 12.dp)) {
            Text("$label: ", color = MUTED, fontSize = 13.sp, maxLines = 1)
            Text(options.find { it.first == value }?.second ?: "", fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
        }
        DropdownMenu(open, { open = false }) {
            for ((v, text) in options) {
                DropdownMenuItem({ Text(text) }, {
                    open = false
                    onPick(v)
                })
            }
        }
    }
}

/** One artwork card with a caption, for rows and grids. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ArtCard(
    art: String?,
    title: String,
    sub: String?,
    ratio: Float,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    progress: Float? = null,
    /** A watched movie: a "WATCHED" sash across the top right corner. */
    watched: Boolean = false,
    /** Cards that start playback show ▶. */
    plays: Boolean = false,
    /** Held down: these, as a menu. */
    menu: () -> List<Pair<String, () -> Unit>> = { emptyList() },
) {
    var open by remember { mutableStateOf<List<Pair<String, () -> Unit>>>(emptyList()) }
    Column(modifier) {
        Box(
            Modifier.fillMaxWidth().aspectRatio(ratio).clip(RoundedCornerShape(6.dp)).background(SURFACE)
                .combinedClickable(onClick = onClick, onLongClick = { open = menu() }),
        ) {
            if (art != null) Art(art, Modifier.fillMaxSize()) else Placeholder(title)
            if (watched) WatchedSash()
            if (progress != null && progress > 0f) {
                ProgressBar(progress, Modifier.align(Alignment.BottomStart).fillMaxWidth().padding(8.dp, 0.dp, 8.dp, 5.dp))
            }
            if (plays) {
                Box(
                    Modifier.align(Alignment.Center).size(40.dp).clip(CircleShape).background(Color.Black.copy(alpha = 0.55f)),
                    contentAlignment = Alignment.Center,
                ) {
                    Ic(R.drawable.ic_play, null, tint = Color.White)
                }
            }
            DropdownMenu(open.isNotEmpty(), { open = emptyList() }) {
                for ((text, act) in open) {
                    DropdownMenuItem({ Text(text) }, {
                        open = emptyList()
                        act()
                    })
                }
            }
        }
        Spacer(Modifier.height(6.dp))
        Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 13.sp, color = TEXT.copy(alpha = 0.9f))
        if (sub != null) Text(sub, maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 11.sp, color = MUTED)
    }
}

/** The desktop app's "WATCHED" sash: a band across the corner, clipped by the card. */
@Composable
private fun BoxScope.WatchedSash() {
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
        title, color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center,
        modifier = Modifier.align(Alignment.Center).padding(8.dp),
    )
}

/** "Add to My list" or "Remove from My list"; a title that isn't on the server can only be opened. */
fun listItem(model: AppModel, i: Item): List<Pair<String, () -> Unit>> =
    if (i.remote) emptyList()
    else listOf((if (model.onList(i.id)) "Remove from My list" else "Add to My list") to { model.toggleList(i.id) })

/** A poster that opens its title; hold it for My list. */
@Composable
fun PosterCard(model: AppModel, i: Item, modifier: Modifier = Modifier, sub: String? = i.year?.toString()) {
    ArtCard(
        i.poster, i.title, sub, 2f / 3f, { model.push(Screen.Details(i.id)) }, modifier,
        progress = if (i.kind == Kind.Movie) model.entry(i.id)?.takeIf { it.inProgress }?.fraction else null,
        watched = model.watched(i),
        menu = { listItem(model, i) },
    )
}

@Composable
fun RowTitle(text: String, count: Int? = null, modifier: Modifier = Modifier) {
    Row(modifier.padding(start = PAD, end = PAD, bottom = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        MonoText(text, 12.sp, modifier = Modifier.weight(1f, fill = false))
        if (count != null) {
            Spacer(Modifier.width(8.dp))
            MonoText(count.toString(), 12.sp, MUTED)
        }
    }
}

/** A row of posters. */
@Composable
fun PosterRow(title: String, items: List<Item>, model: AppModel, count: Int? = null, width: Dp = 112.dp, sub: (Item) -> String? = { it.year?.toString() }) {
    Column {
        RowTitle(title, count)
        LazyRow(contentPadding = PaddingValues(horizontal = PAD), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            items(items, key = { it.id }) { i -> PosterCard(model, i, Modifier.width(width), sub(i)) }
        }
    }
}

/** Rounded panel for metadata, critics, info. */
@Composable
fun Panel(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Column(
        modifier.fillMaxWidth().padding(horizontal = PAD).clip(RoundedCornerShape(10.dp)).background(SURFACE).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) { content() }
}

@Composable
fun InfoRows(rows: List<Pair<String, String>>, valueColor: (String) -> Color = { TEXT }) {
    for ((k, v) in rows) {
        Row {
            Text(k, color = MUTED, fontSize = 13.sp, modifier = Modifier.width(110.dp))
            Text(v, color = valueColor(k), fontSize = 13.sp, modifier = Modifier.weight(1f))
        }
    }
}

/** A circle with their initial. */
@Composable
fun Avatar(name: String, size: Dp) {
    Box(Modifier.size(size).clip(CircleShape).background(colorFor(name)), contentAlignment = Alignment.Center) {
        Text(name.take(1).uppercase(), color = Color.White, fontSize = (size.value * 0.42f).sp, fontWeight = FontWeight.Bold)
    }
}

/** A person with their photo. */
@Composable
fun PersonCard(name: String, photo: String?, role: String?, onClick: () -> Unit) {
    Column(Modifier.width(84.dp).clickable(onClick = onClick), horizontalAlignment = Alignment.CenterHorizontally) {
        Box(Modifier.size(72.dp).clip(CircleShape)) {
            if (photo != null) Art(photo, Modifier.fillMaxSize()) else Avatar(name, 72.dp)
        }
        Spacer(Modifier.height(6.dp))
        Text(name, color = TEXT, fontSize = 12.sp, maxLines = 2, textAlign = TextAlign.Center, overflow = TextOverflow.Ellipsis)
        if (role != null) Text(role, color = MUTED, fontSize = 11.sp, maxLines = 1, textAlign = TextAlign.Center, overflow = TextOverflow.Ellipsis)
    }
}

/** "Director  Name, Name": each name opens their page. */
@Composable
fun PeopleLinks(label: String, names: List<String>, onOpen: (String) -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text("$label  ", color = MUTED, fontSize = 13.sp)
        for ((i, n) in names.take(3).withIndex()) {
            if (i > 0) Text(", ", color = MUTED, fontSize = 13.sp)
            Text(n, color = TEXT, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.clickable { onOpen(n) }.padding(vertical = 6.dp))
        }
    }
}
