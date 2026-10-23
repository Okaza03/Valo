// A movie's or series' page: artwork, details, play buttons and My list; for
// series the seasons and their episodes; then cast and crew, "You might also
// like", file and metadata, and critics (the desktop app's details pages).
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Episode
import io.github.okaza03.valo.FileInfo
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.Library
import io.github.okaza03.valo.Screen
import io.github.okaza03.valo.Season
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

@Composable
fun DetailsScreen(model: AppModel, lib: Library, item: Item) {
    val play = remember { FocusRequester() }
    LaunchedEffect(item.id) {
        // what the server's details are missing, from the addons; cast photos
        model.discover.wantItem(item)
        runCatching { play.requestFocus() }
    }
    val next = if (item.kind == Kind.Series) model.nextUp(item) else null
    // start on the season of the episode that's up next
    var season by rememberSaveable(item.id) {
        mutableIntStateOf(item.seasons.indexOfFirst { it == next?.season }.coerceAtLeast(0))
    }
    var focusedEp by remember(item.id) { mutableStateOf<Episode?>(null) }
    val similar = model.similar(lib, item, if (item.kind == Kind.Movie) 10 else 20)
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
                // the TV scrolls a focused button to a third of the way down, which
                // would cut off the title above the play buttons: show the top instead
                Column(Modifier.padding(horizontal = EDGE).fillMaxWidth(0.55f).onFocusChanged {
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
                        Text(item.title, fontSize = 40.sp, fontWeight = FontWeight.Bold, color = TEXT)
                    }
                    Spacer(Modifier.height(12.dp))
                    Text(metaLine(item), color = MUTED, fontSize = 16.sp)
                    Spacer(Modifier.height(12.dp))
                    Text(item.overview, color = TEXT, fontSize = 16.sp, maxLines = 5, overflow = TextOverflow.Ellipsis)
                    if (item.makers.isNotEmpty()) {
                        Spacer(Modifier.height(8.dp))
                        PeopleLinks(if (item.kind == Kind.Movie) "Director" else "Created by", item.makers.distinct()) {
                            model.push(Screen.Person(it))
                        }
                    }
                    credits(item).forEach { (label, text) ->
                        Spacer(Modifier.height(8.dp))
                        Text(
                            "$label  $text", color = MUTED, fontSize = 14.sp,
                            maxLines = 1, overflow = TextOverflow.Ellipsis,
                        )
                    }
                    Spacer(Modifier.height(24.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                        when {
                            item.kind == Kind.Movie -> {
                                val entry = model.entry(item.id)
                                val e = entry?.takeIf { it.inProgress }
                                TvButton(
                                    when {
                                        e != null -> "Resume  ·  ${fmtRuntime(e.dur - e.pos)} left"
                                        entry?.watched == true -> "Watch Again"
                                        else -> "Play"
                                    },
                                    { model.push(Screen.Player(item.id)) },
                                    primary = true, modifier = Modifier.focusRequester(play),
                                )
                                if (e != null) {
                                    TvButton("Start over", { model.push(Screen.Player(item.id, fromStart = true)) })
                                }
                            }
                            next != null -> {
                                val ep = next.episode
                                val what = next.season.code(ep.number)
                                TvButton(
                                    when {
                                        next.resume != null -> "Resume $what"
                                        next.started -> "Play $what"
                                        else -> "Play"
                                    },
                                    { model.push(Screen.Player(item.id, ep.id)) },
                                    primary = true, modifier = Modifier.focusRequester(play),
                                )
                                if (next.resume != null) {
                                    TvButton("Start over", { model.push(Screen.Player(item.id, ep.id, fromStart = true)) })
                                }
                            }
                        }
                        val listed = model.onList(item.id)
                        TvButton(if (listed) "✓ My list" else "+ My list", { model.toggleList(item.id) })
                        if (item.kind == Kind.Movie) {
                            val entry = model.entry(item.id)
                            val status = when {
                                entry?.watched == true -> "Watched"
                                entry?.inProgress == true -> "Ready to stream · %.0f%% watched".format(entry.fraction * 100)
                                else -> "Ready to stream"
                            }
                            Spacer(Modifier.width(6.dp))
                            Text("●  $status", color = GOLD, fontSize = 13.sp, maxLines = 1)
                        }
                    }
                }
            }

            if (item.kind == Kind.Series && item.seasons.size > 1) {
                item {
                    LazyRow(
                        contentPadding = PaddingValues(horizontal = EDGE),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        itemsIndexed(item.seasons) { i, s ->
                            TvButton(s.label, { season = i }, selected = i == season)
                        }
                    }
                }
            }

            if (item.kind == Kind.Series) {
                item(key = "episodes/$season") {
                    val s = item.seasons.getOrNull(season) ?: return@item
                    Column {
                        val n = s.episodes.size
                        RowTitle(s.label, n)
                        val state = rememberLazyListState(
                            s.episodes.indexOfFirst { it.id == next?.episode?.id }.coerceAtLeast(0),
                        )
                        LazyRow(
                            state = state,
                            contentPadding = PaddingValues(horizontal = EDGE),
                            horizontalArrangement = Arrangement.spacedBy(16.dp),
                        ) {
                            items(s.episodes, key = { it.id }) { ep ->
                                val e = model.entry(ep.id)
                                val sub = buildList {
                                    add("E${ep.number}")
                                    ep.duration?.takeIf { it > 0 }?.let { add(fmtRuntime(it)) }
                                    if (e?.watched == true) add("Watched")
                                }.joinToString(" · ")
                                ArtCard(
                                    art = ep.still ?: item.backdrop, title = ep.title.ifEmpty { "Episode ${ep.number}" },
                                    sub = sub, width = 280.dp, ratio = 16f / 9f,
                                    progress = e?.let { if (it.watched) 1f else it.fraction },
                                    onClick = { model.push(Screen.Player(item.id, ep.id)) },
                                    // hold OK to play from the beginning
                                    onLongClick = { model.push(Screen.Player(item.id, ep.id, fromStart = true)) },
                                    onFocus = { focusedEp = ep },
                                )
                            }
                        }
                        // the focused episode's story, like the desktop app's episode page
                        val shown = focusedEp?.takeIf { f -> s.episodes.any { it.id == f.id } }
                        if (shown != null) EpisodeInfo(item, s, n, shown)
                    }
                }
            }

            val people = people(item)
            if (people.isNotEmpty()) {
                item(key = "people") {
                    Column {
                        RowTitle(if (item.kind == Kind.Movie) "Cast & crew" else "Series cast & creators")
                        LazyRow(
                            contentPadding = PaddingValues(horizontal = EDGE),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            items(people, key = { "${it.first}/${it.third}" }) { (name, photo, role) ->
                                PersonCard(name, photo, role) { model.push(Screen.Person(name)) }
                            }
                        }
                    }
                }
            }

            if (similar.isNotEmpty()) {
                item(key = "similar") {
                    Column {
                        RowTitle("You might also like")
                        LazyRow(
                            contentPadding = PaddingValues(horizontal = EDGE),
                            horizontalArrangement = Arrangement.spacedBy(16.dp),
                        ) {
                            items(similar, key = { it.id }) { i -> PosterCard(model, i) }
                        }
                    }
                }
            }

            val rows = if (item.kind == Kind.Movie) movieRows(item) else seriesRows(item)
            if (rows.isNotEmpty() || item.reviews.isNotEmpty()) item(key = "info") {
                Row(
                    Modifier.padding(horizontal = EDGE).fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(24.dp),
                ) {
                    if (rows.isNotEmpty()) Panel(Modifier.weight(1f).widthIn(max = 560.dp)) {
                        MonoText(if (item.kind == Kind.Movie) "File · Metadata" else "Info", 12.sp, TEAL)
                        InfoRows(rows) { k -> if (k in FILE_ROWS) GOLD else TEXT }
                    }
                    if (item.reviews.isNotEmpty()) {
                        Panel(Modifier.weight(1f)) {
                            MonoText("From critics", 12.sp, MUTED)
                            for (r in item.reviews.take(3)) {
                                Text(
                                    "“${r.quote}”", color = TEXT.copy(alpha = 0.85f), fontSize = 15.sp,
                                    fontStyle = FontStyle.Italic, maxLines = 4, overflow = TextOverflow.Ellipsis,
                                )
                                val by = listOfNotNull(r.source, r.score)
                                if (by.isNotEmpty()) Text("— ${by.joinToString(" · ")}", color = MUTED, fontSize = 12.sp)
                            }
                        }
                    } else {
                        Spacer(Modifier.weight(1f))
                    }
                }
            }
        }
    }
}

/** Under the episodes: the focused one's title, details and story. */
@Composable
private fun EpisodeInfo(series: Item, season: Season, count: Int, ep: Episode) {
    Column(Modifier.padding(start = EDGE, end = EDGE, top = 16.dp).fillMaxWidth(0.6f)) {
        val meta = buildList {
            add("${season.code(ep.number, "S%02d E%02d")} of $count")
            ep.duration?.takeIf { it > 0 }?.let { add(fmtRuntime(it)) }
            ep.aired?.let { add("Aired ${dateLong(it)}") }
            ep.score?.let { add("★ %.1f".format(it)) }
            ep.file?.quality?.let { add(it) }
        }
        Text(meta.joinToString("  ·  "), color = GOLD, fontSize = 13.sp)
        Spacer(Modifier.height(4.dp))
        Text(
            ep.title.ifEmpty { "Episode ${ep.number}" }, color = TEXT, fontSize = 18.sp, fontWeight = FontWeight.Bold,
            maxLines = 1, overflow = TextOverflow.Ellipsis,
        )
        if (ep.overview.isNotEmpty()) {
            Spacer(Modifier.height(4.dp))
            Text(ep.overview, color = MUTED, fontSize = 14.sp, maxLines = 3, overflow = TextOverflow.Ellipsis)
        }
        if (series.rating != null) {
            Spacer(Modifier.height(4.dp))
            MonoText(series.rating, 11.sp, MUTED)
        }
    }
}

/** A poster that opens its title; hold OK for My list. */
@Composable
fun PosterCard(model: AppModel, i: Item, sub: String? = i.year?.toString(), width: androidx.compose.ui.unit.Dp = 124.dp) {
    WithMenu({ listOf(listItem(model, i)) }) { m, menu ->
        ArtCard(
            art = i.poster, title = i.title, sub = sub, width = width, ratio = 2f / 3f,
            progress = if (i.kind == Kind.Movie) model.entry(i.id)?.takeIf { it.inProgress }?.fraction else null,
            watched = model.watched(i),
            onClick = { model.push(Screen.Details(i.id)) },
            onLongClick = menu,
            modifier = m,
        )
    }
}

/** "Add to My list" or "Remove from My list"; a title that isn't on the server can only be opened (and requested there). */
fun listItem(model: AppModel, i: Item) =
    if (i.remote) MenuItem("Open ${i.title}") { model.push(Screen.Details(i.id)) }
    else MenuItem(if (model.onList(i.id)) "Remove from My list" else "Add to My list") { model.toggleList(i.id) }

/** Rows drawn in gold: about the file. */
private val FILE_ROWS = setOf("Quality", "Container", "Audio", "Bitrate")

private fun fileRows(f: FileInfo?): List<Pair<String, String>> = buildList {
    if (f == null) return@buildList
    f.quality?.let { add("Quality" to it) }
    listOfNotNull(f.container, f.videoCodec).joinToString(" · ").takeIf { it.isNotEmpty() }?.let { add("Container" to it) }
    f.audioCodec?.let { a -> add("Audio" to (a + (f.audioChannels?.let { " · $it" } ?: ""))) }
    f.bitrate?.let { add("Bitrate" to bitrate(it)) }
    f.size?.let { add("File size" to bytes(it)) }
    f.name?.let { add("File" to it) }
}

private fun movieRows(item: Item): List<Pair<String, String>> = buildList {
    item.director?.let { add("Director" to it) }
    item.released?.let { add("Released" to dateLong(it)) }
    item.duration?.takeIf { it > 0 }?.let { add("Runtime" to fmtRuntime(it)) }
    item.score?.let { add("Score" to ("%.1f".format(it) + (item.scoreSource?.let { s -> " · $s" } ?: ""))) }
    item.collection?.let { add("Collection" to it) }
    item.budget?.let { add("Budget" to money(it)) }
    item.revenue?.let { add("Box office" to money(it)) }
    addAll(fileRows(item.file))
}

private fun seriesRows(item: Item): List<Pair<String, String>> = buildList {
    if (item.creators.isNotEmpty()) add("Created by" to item.creators.joinToString(", "))
    item.network?.let { add("Network" to it) }
    item.status?.let { add("Status" to it) }
    item.year?.let { y -> add("Years" to if (item.endYear != null && item.endYear != y) "$y–${item.endYear}" else "$y") }
    item.score?.let { add("Score" to ("%.1f".format(it) + (item.scoreSource?.let { s -> " · $s" } ?: ""))) }
    val eps = item.episodes.size
    add("In library" to "$eps episode${if (eps == 1) "" else "s"}")
}

/** Directors or creators first, then the cast: (name, photo, part). */
private fun people(item: Item): List<Triple<String, String?, String?>> {
    val makers = item.makers.distinct().map { n ->
        val role = if (item.kind == Kind.Movie) "Director" else "Creator"
        Triple(n, item.cast.find { it.name == n }?.photo, role)
    }
    val cast = item.cast.take(20).map { Triple(it.name, it.photo, it.character?.takeIf { c -> c.isNotBlank() }) }
    return (makers + cast).distinctBy { it.first to it.third }
}

/** The header's other credits; directors and creators go above them as links. */
private fun credits(item: Item): List<Pair<String, String>> = buildList {
    if (item.cast.isNotEmpty()) add("Starring" to item.cast.take(5).joinToString(", ") { it.name })
    item.network?.let { add("Network" to it) }
}
