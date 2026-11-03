// A movie's or series' page, and a title from the addons that isn't on the
// server, with its Request button (the desktop app's details pages).
package io.github.okaza03.valo.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Episode
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.Library
import io.github.okaza03.valo.R
import io.github.okaza03.valo.Requests
import io.github.okaza03.valo.Screen

/** The page's artwork and title. */
@Composable
private fun Header(item: Item) {
    Box(Modifier.fillMaxWidth().aspectRatio(16f / 10f)) {
        Art(item.backdrop ?: item.poster, Modifier.fillMaxSize())
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.4f to Color.Transparent, 1f to BG)))
    }
    Column(Modifier.padding(horizontal = PAD)) {
        MonoText(
            (listOf(if (item.kind == Kind.Movie) "Movie" else "Series") + item.genres.take(3)).joinToString("  ·  "),
            11.sp, TEXT.copy(alpha = 0.65f),
        )
        Spacer(Modifier.height(8.dp))
        if (item.logo != null) {
            Logo(item.logo, item.title, Modifier.height(72.dp).fillMaxWidth(0.8f))
        } else {
            Text(item.title, fontSize = 28.sp, fontWeight = FontWeight.Bold, color = TEXT)
        }
    }
}

/** Back and Cast, staying put while the page scrolls under them. */
@Composable
private fun BoxScope.TopButtons(model: AppModel) {
    BackButton { model.back() }
    Box(Modifier.align(Alignment.TopEnd).padding(8.dp).clip(CircleShape).background(Color.Black.copy(alpha = 0.45f))) {
        CastButton()
    }
}

@Composable
fun DetailsScreen(model: AppModel, lib: Library, item: Item) {
    // what the server's details are missing, from the addons; cast photos
    LaunchedEffect(item.id) { model.discover.wantItem(item) }
    val next = if (item.kind == Kind.Series) model.nextUp(item) else null
    // start on the season of the episode that's up next
    var season by rememberSaveable(item.id) {
        mutableIntStateOf(item.seasons.indexOfFirst { it == next?.season }.coerceAtLeast(0))
    }
    val similar = model.similar(lib, item, if (item.kind == Kind.Movie) 10 else 20)

    Box(Modifier.fillMaxSize().background(BG)) {
        LazyColumn(
            contentPadding = PaddingValues(bottom = PAD),
            verticalArrangement = Arrangement.spacedBy(20.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            item(key = "top") {
                Column {
                    Header(item)
                    Column(Modifier.padding(horizontal = PAD), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Spacer(Modifier.height(2.dp))
                        Text(metaLine(item), color = MUTED, fontSize = 13.sp)
                        PlayButtons(model, item)
                        val listed = model.onList(item.id)
                        Btn(if (listed) "✓ My list" else "+ My list", { model.toggleList(item.id) }, Modifier.fillMaxWidth())
                        if (item.kind == Kind.Movie) {
                            val entry = model.entry(item.id)
                            val status = when {
                                entry?.watched == true -> "Watched"
                                entry?.inProgress == true -> "Ready to stream · %.0f%% watched".format(entry.fraction * 100)
                                else -> "Ready to stream"
                            }
                            Text("●  $status", color = GOLD, fontSize = 12.sp)
                        }
                        Text(item.overview, color = TEXT, fontSize = 14.sp)
                        if (item.makers.isNotEmpty()) {
                            PeopleLinks(if (item.kind == Kind.Movie) "Director" else "Created by", item.makers.distinct()) {
                                model.push(Screen.Person(it))
                            }
                        }
                        credits(item).forEach { (label, text) ->
                            Text("$label  $text", color = MUTED, fontSize = 13.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
                        }
                    }
                }
            }

            if (item.kind == Kind.Series && item.seasons.size > 1) {
                item(key = "seasons") {
                    Chips(season, item.seasons.mapIndexed { i, s -> Triple(i, s.label, null) }, { season = it })
                }
            }

            if (item.kind == Kind.Series) {
                val s = item.seasons.getOrNull(season)
                if (s != null) {
                    item(key = "episodes") { RowTitle(s.label, s.episodes.size) }
                    items(s.episodes, key = { it.id }) { ep -> EpisodeRow(model, item, ep) }
                }
            }

            val people = people(item)
            if (people.isNotEmpty()) {
                item(key = "people") {
                    Column {
                        RowTitle(if (item.kind == Kind.Movie) "Cast & crew" else "Series cast & creators")
                        LazyRow(contentPadding = PaddingValues(horizontal = PAD), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            items(people, key = { "${it.first}/${it.third}" }) { (name, photo, role) ->
                                PersonCard(name, photo, role) { model.push(Screen.Person(name)) }
                            }
                        }
                    }
                }
            }

            if (similar.isNotEmpty()) item(key = "similar") { PosterRow("You might also like", similar, model) }

            val rows = if (item.kind == Kind.Movie) movieRows(item) else seriesRows(item)
            if (rows.isNotEmpty()) {
                item(key = "info") {
                    Panel {
                        MonoText(if (item.kind == Kind.Movie) "File · Metadata" else "Info", 11.sp, TEAL)
                        InfoRows(rows) { k -> if (k in FILE_ROWS) GOLD else TEXT }
                    }
                }
            }
            if (item.reviews.isNotEmpty()) {
                item(key = "critics") {
                    Panel {
                        MonoText("From critics", 11.sp, MUTED)
                        for (r in item.reviews.take(3)) {
                            Text("“${r.quote}”", color = TEXT.copy(alpha = 0.85f), fontSize = 14.sp, fontStyle = FontStyle.Italic)
                            val by = listOfNotNull(r.source, r.score)
                            if (by.isNotEmpty()) Text("— ${by.joinToString(" · ")}", color = MUTED, fontSize = 12.sp)
                        }
                    }
                }
            }
        }
        TopButtons(model)
    }
}

/** Play, Resume or Watch Again, and Start over. */
@Composable
private fun PlayButtons(model: AppModel, item: Item) {
    val wide = Modifier.fillMaxWidth()
    if (item.unavailable) {
        Btn(MOVING_LABEL, {}, wide, enabled = false)
        return
    }
    if (item.kind == Kind.Movie) {
        val entry = model.entry(item.id)
        val e = entry?.takeIf { it.inProgress }
        Btn(
            when {
                e != null -> "Resume  ·  ${fmtRuntime(e.dur - e.pos)} left"
                entry?.watched == true -> "Watch Again"
                else -> "Play"
            },
            { model.push(Screen.Player(item.id)) }, wide, primary = true, icon = R.drawable.ic_play,
        )
        if (e != null) Btn("Start over", { model.push(Screen.Player(item.id, fromStart = true)) }, wide)
        return
    }
    val next = model.nextUp(item) ?: return
    val ep = next.episode
    val what = next.season.code(ep.number)
    Btn(
        when {
            next.resume != null -> "Resume $what"
            next.started -> "Play $what"
            else -> "Play"
        },
        { model.push(Screen.Player(item.id, ep.id)) }, wide, primary = true, icon = R.drawable.ic_play,
    )
    if (next.resume != null) Btn("Start over", { model.push(Screen.Player(item.id, ep.id, fromStart = true)) }, wide)
}

/** One episode: tap to play, hold to play from the beginning. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun EpisodeRow(model: AppModel, series: Item, ep: Episode) {
    val e = model.entry(ep.id)
    Row(
        Modifier.fillMaxWidth().combinedClickable(
            onClick = { model.push(Screen.Player(series.id, ep.id)) },
            onLongClick = { model.push(Screen.Player(series.id, ep.id, fromStart = true)) },
        ).padding(horizontal = PAD),
    ) {
        Box(Modifier.width(136.dp).aspectRatio(16f / 9f).clip(RoundedCornerShape(6.dp))) {
            Art(ep.still ?: series.backdrop, Modifier.fillMaxSize())
            val p = e?.let { if (it.watched) 1f else it.fraction }
            if (p != null && p > 0f) ProgressBar(p, Modifier.align(Alignment.BottomStart).fillMaxWidth().padding(6.dp, 0.dp, 6.dp, 4.dp))
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text("${ep.number}. ${ep.title.ifEmpty { "Episode ${ep.number}" }}", color = TEXT, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 2, overflow = TextOverflow.Ellipsis)
            val meta = buildList {
                ep.duration?.takeIf { it > 0 }?.let { add(fmtRuntime(it)) }
                ep.aired?.let { add(dateLong(it)) }
                ep.score?.let { add("★ %.1f".format(it)) }
                if (e?.watched == true) add("Watched")
            }
            if (meta.isNotEmpty()) Text(meta.joinToString(" · "), color = MUTED, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (ep.overview.isNotEmpty()) Text(ep.overview, color = MUTED, fontSize = 12.sp, maxLines = 3, overflow = TextOverflow.Ellipsis)
        }
    }
}

@Composable
fun RemoteScreen(model: AppModel, lib: Library, item: Item) {
    val d = model.discover
    LaunchedEffect(item.id) { d.wantItem(item) }
    var season by rememberSaveable(item.id) { mutableIntStateOf(-1) }
    // the first real season, not the specials
    if (season !in item.seasons.indices) season = item.seasons.indexOfFirst { it.number > 0 }.coerceAtLeast(0)
    val similar = model.similar(lib, item, 12)

    Box(Modifier.fillMaxSize().background(BG)) {
        LazyColumn(
            contentPadding = PaddingValues(bottom = PAD),
            verticalArrangement = Arrangement.spacedBy(20.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            item(key = "top") {
                Column {
                    Header(item)
                    Column(Modifier.padding(horizontal = PAD), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        d.streamingOn(item.id)?.let { Text("Streaming on $it", color = TEAL, fontSize = 14.sp) }
                        val meta = buildList {
                            item.score?.let { add("★ %.1f".format(it) + (item.scoreSource?.let { s -> " $s" } ?: "")) }
                            item.year?.let { y -> add(if (item.endYear != null && item.endYear != y) "$y–${item.endYear}" else "$y") }
                            item.duration?.let { add(fmtRuntime(it)) }
                        }
                        if (meta.isNotEmpty()) Text(meta.joinToString("  ·  "), color = MUTED, fontSize = 13.sp)
                        RequestRow(model, item)
                        if (item.overview.isNotEmpty()) Text(item.overview, color = TEXT, fontSize = 14.sp)
                        if (item.makers.isNotEmpty()) {
                            PeopleLinks(if (item.kind == Kind.Movie) "Director" else "Created by", item.makers.distinct()) {
                                model.push(Screen.Person(it))
                            }
                        }
                    }
                }
            }
            if (item.cast.isNotEmpty()) {
                item(key = "cast") {
                    Column {
                        RowTitle("Top billed")
                        LazyRow(contentPadding = PaddingValues(horizontal = PAD), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            items(item.cast.take(15), key = { it.name }) { c ->
                                PersonCard(c.name, c.photo, c.character?.takeIf { it.isNotBlank() }) { model.push(Screen.Person(c.name)) }
                            }
                        }
                    }
                }
            }
            remoteEpisodes(item, season) { season = it }
            if (similar.isNotEmpty()) item(key = "similar") { PosterRow("On the server: like this, ready to watch", similar, model) }
            val a = item.addon
            val rows = buildList {
                if (item.creators.isNotEmpty()) add("Created by" to item.creators.joinToString(", "))
                if (a != null && a.writers.isNotEmpty() && item.kind == Kind.Movie) add("Writers" to a.writers.take(4).joinToString(", "))
                item.released?.let { add("Released" to dateLong(it)) }
                a?.country?.let { add("Country" to it) }
                a?.awards?.let { add("Awards" to it) }
                a?.source?.takeIf { it.isNotEmpty() }?.let { add("Details from" to it) }
            }
            if (rows.isNotEmpty()) {
                item(key = "info") {
                    Panel {
                        MonoText("Info", 11.sp, TEAL)
                        InfoRows(rows)
                    }
                }
            }
            if (d.loading || item.overview.isEmpty() && item.cast.isEmpty()) {
                item(key = "loading") { MonoText("Looking up the details…", 11.sp, MUTED, Modifier.padding(horizontal = PAD)) }
            }
        }
        TopButtons(model)
    }
}

private fun LazyListScope.remoteEpisodes(item: Item, season: Int, onSeason: (Int) -> Unit) {
    if (item.kind != Kind.Series || item.seasons.isEmpty()) return
    if (item.seasons.size > 1) {
        item(key = "seasons") { Chips(season, item.seasons.mapIndexed { i, s -> Triple(i, s.label, null) }, onSeason) }
    }
    val s = item.seasons.getOrNull(season) ?: return
    item(key = "episodes") { RowTitle(s.label, s.episodes.size) }
    items(s.episodes, key = { "${s.number}/${it.number}" }) { ep ->
        Row(Modifier.fillMaxWidth().padding(horizontal = PAD)) {
            Art(ep.still ?: item.backdrop, Modifier.width(136.dp).aspectRatio(16f / 9f).clip(RoundedCornerShape(6.dp)))
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text("${ep.number}. ${ep.title.ifEmpty { "Episode ${ep.number}" }}", color = TEXT, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 2, overflow = TextOverflow.Ellipsis)
                ep.aired?.let { Text(dateLong(it), color = MUTED, fontSize = 12.sp) }
                if (ep.overview.isNotEmpty()) Text(ep.overview, color = MUTED, fontSize = 12.sp, maxLines = 3, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** "Not on the server" and Request, or how the request is doing. */
@Composable
private fun RequestRow(model: AppModel, item: Item) {
    val d = model.discover
    val req = d.requestFor(item)
    val requests = d.requests
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
    if (req == null && model.client?.token != null && requests !is Requests.Failed) {
        Btn(if (d.sending) "Requesting…" else "Request", { d.request(item) }, Modifier.fillMaxWidth(), primary = true, enabled = !d.sending)
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(8.dp).clip(CircleShape).background(color))
        Spacer(Modifier.width(8.dp))
        Text(text, color = color, fontSize = 13.sp)
    }
    req?.note?.takeIf { it.isNotBlank() }?.let { Text("“$it”", color = MUTED, fontSize = 13.sp, fontStyle = FontStyle.Italic) }
    if (req == null && requests is Requests.Failed) Text(requests.message, color = MUTED, fontSize = 13.sp)
    d.sendError?.let { Text("Couldn't send the request: $it", color = ERROR, fontSize = 13.sp) }
}
