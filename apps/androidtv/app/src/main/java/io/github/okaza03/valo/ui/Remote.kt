// A title from the admin's addons that isn't on the server: its details, and
// a Request button that asks the admin for it (the desktop app's
// discover.rs remote_page).
package io.github.okaza03.valo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.focus.onFocusChanged
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Episode
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.Library
import io.github.okaza03.valo.Requests
import io.github.okaza03.valo.Screen

val OK = Color(0xFF62C88A)
val WARN = Color(0xFFF2B84B)

@Composable
fun RemoteScreen(model: AppModel, lib: Library, item: Item) {
    val d = model.discover
    val first = remember { FocusRequester() }
    LaunchedEffect(item.id) {
        d.wantItem(item)
        first.focusSoon()
    }
    var season by rememberSaveable(item.id) { mutableIntStateOf(-1) }
    // the first real season, not the specials
    if (season !in item.seasons.indices) season = item.seasons.indexOfFirst { it.number > 0 }.coerceAtLeast(0)
    var focusedEp by remember(item.id) { mutableStateOf<Episode?>(null) }
    val similar = model.similar(lib, item, 12)
    val list = rememberLazyListState()
    val scope = rememberCoroutineScope()

    Box(Modifier.fillMaxSize().background(BG)) {
        Backdrop(item.backdrop ?: item.poster)
        LazyColumn(
            state = list,
            contentPadding = PaddingValues(top = EDGE, bottom = EDGE),
            verticalArrangement = Arrangement.spacedBy(24.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            item {
                // the TV scrolls a focused button a third of the way down, which would cut the title off
                Column(Modifier.padding(horizontal = EDGE).fillMaxWidth(0.6f).onFocusChanged {
                    if (it.hasFocus) scope.launch {
                        delay(20)
                        list.animateScrollToItem(0)
                    }
                }) {
                    MonoText(
                        (listOf(if (item.kind == Kind.Movie) "Movie" else "Series") + item.genres.take(3)).joinToString("  ·  "),
                        12.sp, TEXT.copy(alpha = 0.65f),
                    )
                    Spacer(Modifier.height(10.dp))
                    if (item.logo != null) {
                        Logo(item.logo, item.title, Modifier.height(100.dp).width(400.dp))
                    } else {
                        Text(item.title, fontSize = 40.sp, fontWeight = FontWeight.Bold, color = TEXT, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    }
                    d.streamingOn(item.id)?.let {
                        Spacer(Modifier.height(8.dp))
                        Text("Streaming on $it", color = TEAL, fontSize = 15.sp)
                    }
                    Spacer(Modifier.height(12.dp))
                    val meta = buildList {
                        item.score?.let { add("★ %.1f".format(it) + (item.scoreSource?.let { s -> " $s" } ?: "")) }
                        item.year?.let { y -> add(if (item.endYear != null && item.endYear != y) "$y–${item.endYear}" else "$y") }
                        item.duration?.let { add(fmtRuntime(it)) }
                    }
                    if (meta.isNotEmpty()) Text(meta.joinToString("  ·  "), color = MUTED, fontSize = 16.sp)
                    if (item.makers.isNotEmpty()) {
                        Spacer(Modifier.height(8.dp))
                        PeopleLinks(if (item.kind == Kind.Movie) "Director" else "Created by", item.makers.distinct()) {
                            model.push(Screen.Person(it))
                        }
                    }
                    if (item.overview.isNotEmpty()) {
                        Spacer(Modifier.height(12.dp))
                        Text(item.overview, color = TEXT, fontSize = 16.sp, maxLines = 5, overflow = TextOverflow.Ellipsis)
                    }
                    Spacer(Modifier.height(24.dp))
                    RequestRow(model, item, first)
                }
            }

            if (item.cast.isNotEmpty()) {
                item(key = "cast") {
                    Column {
                        RowTitle("Top billed")
                        LazyRow(contentPadding = PaddingValues(horizontal = EDGE), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                            items(item.cast.take(15), key = { it.name }) { c ->
                                PersonCard(c.name, c.photo, c.character?.takeIf { it.isNotBlank() }) { model.push(Screen.Person(c.name)) }
                            }
                        }
                    }
                }
            }

            if (item.kind == Kind.Series && item.seasons.isNotEmpty()) {
                if (item.seasons.size > 1) item(key = "seasons") {
                    LazyRow(contentPadding = PaddingValues(horizontal = EDGE), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        itemsIndexed(item.seasons) { i, s -> TvButton(s.label, { season = i }, selected = i == season) }
                    }
                }
                item(key = "episodes/$season") {
                    val s = item.seasons.getOrNull(season) ?: return@item
                    Column {
                        RowTitle(s.label, s.episodes.size)
                        LazyRow(contentPadding = PaddingValues(horizontal = EDGE), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                            items(s.episodes, key = { "${s.number}/${it.number}" }) { ep ->
                                ArtCard(
                                    art = ep.still ?: item.backdrop, title = ep.title.ifEmpty { "Episode ${ep.number}" },
                                    sub = listOfNotNull("E${ep.number}", ep.aired?.let(::dateLong)).joinToString(" · "),
                                    width = 280.dp, ratio = 16f / 9f, onClick = {}, onFocus = { focusedEp = ep },
                                )
                            }
                        }
                        val shown = focusedEp?.takeIf { f -> s.episodes.any { it === f } }
                        if (shown != null && shown.overview.isNotEmpty()) {
                            Text(
                                shown.overview, color = MUTED, fontSize = 14.sp, maxLines = 3, overflow = TextOverflow.Ellipsis,
                                modifier = Modifier.padding(start = EDGE, end = EDGE, top = 16.dp).fillMaxWidth(0.6f),
                            )
                        }
                    }
                }
            }

            if (similar.isNotEmpty()) {
                item(key = "similar") {
                    Column {
                        RowTitle("On the server: like this, ready to watch")
                        LazyRow(contentPadding = PaddingValues(horizontal = EDGE), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                            items(similar, key = { it.id }) { i -> PosterCard(model, i) }
                        }
                    }
                }
            }

            val a = item.addon
            val rows = buildList {
                if (item.creators.isNotEmpty()) add("Created by" to item.creators.joinToString(", "))
                if (a != null && a.writers.isNotEmpty() && item.kind == Kind.Movie) add("Writers" to a.writers.take(4).joinToString(", "))
                item.released?.let { add("Released" to dateLong(it)) }
                a?.country?.let { add("Country" to it) }
                a?.awards?.let { add("Awards" to it) }
                a?.source?.takeIf { it.isNotEmpty() }?.let { add("Details from" to it) }
            }
            if (rows.isNotEmpty()) item(key = "info") {
                Panel(Modifier.padding(horizontal = EDGE).widthIn(max = 640.dp)) {
                    MonoText("Info", 12.sp, TEAL)
                    InfoRows(rows)
                }
            }
            if (d.loading || item.overview.isEmpty() && item.cast.isEmpty()) item(key = "loading") {
                MonoText("Looking up the details…", 11.sp, MUTED, Modifier.padding(horizontal = EDGE))
            }
        }
    }
}

/** "Not on the server" and Request, or how the request is doing. */
@Composable
private fun RequestRow(model: AppModel, item: Item, first: FocusRequester) {
    val d = model.discover
    val req = d.requestFor(item)
    // Request turns into Back once it's sent: focus goes along
    LaunchedEffect(req != null) { if (req != null) first.focusSoon() }
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
        val signedIn = model.client?.token != null
        val (color, status) = when (req?.status) {
            null -> MUTED to null
            "approved" -> TEAL to "approved, on its way"
            "available" -> OK to "now available"
            "denied" -> ERROR to "denied"
            else -> WARN to "waiting for the admin"
        }
        val text = if (req == null) "Not on the server" else buildString {
            append("Requested")
            req.requestedBy?.takeIf { it.isNotBlank() }?.let { append(" by $it") }
            req.created?.let { append(" on ${dateUnix(it)}") }
            append(" · $status")
        }
        val requests = d.requests
        val canAsk = req == null && signedIn && requests !is Requests.Failed
        if (canAsk) {
            TvButton(
                if (d.sending) "Requesting…" else "Request", { d.request(item) },
                Modifier.focusRequester(first), primary = true, enabled = !d.sending,
            )
        } else {
            TvButton("Back", { model.back() }, Modifier.focusRequester(first))
        }
        Spacer(Modifier.width(4.dp))
        Box(Modifier.size(8.dp).clip(CircleShape).background(color))
        Text(text, color = color, fontSize = 14.sp)
        req?.note?.takeIf { it.isNotBlank() }?.let { Text("“$it”", color = MUTED, fontSize = 14.sp, fontStyle = FontStyle.Italic) }
        if (req == null && requests is Requests.Failed) Text(requests.message, color = MUTED, fontSize = 14.sp)
    }
    d.sendError?.let {
        Spacer(Modifier.height(8.dp))
        Text("Couldn't send the request: $it", color = ERROR, fontSize = 14.sp)
    }
}
