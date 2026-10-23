// The home page: the focused title's artwork and details on top, rows of
// titles below (Continue watching, My list, Recently added, Top rated,
// Movies, TV shows, genres and the addons' catalogs), in the order the
// server's admin picked. Hold OK on a card for its menu.
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.Library
import io.github.okaza03.valo.Screen
import kotlinx.coroutines.launch

private class HomeCard(
    val key: String,
    val item: Item,
    val title: String,
    val sub: String?,
    val art: String?,
    val progress: Float?,
    val open: Screen,
    /** A Continue watching card, which can be taken off the row. */
    val removable: Boolean = false,
)

private class HomeRow(val title: String, val wide: Boolean, val cards: List<HomeCard>)

/** The home page's own rows in their usual order (their layout keys). The addon catalogs go between the first four and the rest. */
private val HOME_ROWS = listOf("continue", "mylist", "recent", "top", "movies", "series", "genres")

/** Titles per catalog row. */
private const val CATALOG_ROW = 40

private fun rows(model: AppModel, lib: Library): List<HomeRow> {
    val out = ArrayList<HomeRow>()
    fun posters(title: String, items: List<Item>) = HomeRow(title, false, items.map { i ->
        HomeCard(
            "$title/${i.id}", i, i.title,
            // an addon's title the server doesn't have
            if (i.remote) listOfNotNull(i.year?.toString(), "not on server").joinToString(" · ") else i.year?.toString(),
            i.poster,
            if (i.kind == Kind.Movie) model.entry(i.id)?.takeIf { it.inProgress }?.fraction else null,
            Screen.Details(i.id),
        )
    })
    val layout = model.layout
    val catalogs = model.discover.shown.filter { layout.shows(it.key) }
    val keys = HOME_ROWS.take(4) + catalogs.map { it.key } + HOME_ROWS.drop(4)
    for (key in layout.ordered(keys)) {
        if (!layout.shows(key)) continue
        when (key) {
            "continue" -> {
                val cont = model.continueWatching(lib).map { (item, next) ->
                    if (next == null) {
                        val e = model.entry(item.id)
                        HomeCard(
                            "cont/${item.id}", item, item.title, e?.let { "${fmtRuntime(it.dur - it.pos)} left" },
                            item.thumb ?: item.backdrop ?: item.poster, e?.fraction, Screen.Player(item.id), removable = true,
                        )
                    } else {
                        val ep = next.episode
                        HomeCard(
                            "cont/${item.id}", item, item.title, "${next.season.code(ep.number)} · ${ep.title}",
                            ep.still ?: item.thumb ?: item.backdrop ?: item.poster, next.resume?.fraction ?: 0f,
                            Screen.Player(item.id, ep.id), removable = true,
                        )
                    }
                }
                if (cont.isNotEmpty()) out += HomeRow("Continue watching", true, cont)
            }
            "mylist" -> model.myList(lib).takeIf { it.isNotEmpty() }?.let { out += posters("My list", it) }
            "recent" -> lib.all.filter { it.added != null }.sortedByDescending { it.added }.take(24)
                .takeIf { it.isNotEmpty() }?.let { out += posters("Recently added", it) }
            "top" -> model.topRated(lib).takeIf { it.isNotEmpty() }?.let { out += posters("Top rated", it.take(24)) }
            // the rest in a random order (the same all session), so the home page isn't A to Z
            "movies" -> if (lib.movies.isNotEmpty()) out += posters("Movies", model.shuffled(lib.movies, "movies").take(30))
            "series" -> if (lib.series.isNotEmpty()) out += posters("TV shows", model.shuffled(lib.series, "series").take(30))
            "genres" -> for (g in lib.genres().filter { layout.shows("genre:$it") }.take(8)) {
                val items = model.shuffled(lib.all.filter { g in it.genres }, g).take(30)
                if (items.size >= 2) out += posters(g, items)
            }
            else -> catalogs.find { it.key == key }?.let { c ->
                // titles we have play, the rest can be requested
                val items = c.ids.take(CATALOG_ROW).mapNotNull { model.discover.find(lib, it) }
                if (items.isNotEmpty()) out += posters(c.title, items)
            }
        }
    }
    return out
}

@Composable
fun HomeScreen(model: AppModel, lib: Library) {
    val rows = rows(model, lib)
    var focused by remember { mutableStateOf(rows.firstOrNull()?.cards?.firstOrNull()?.item) }
    val target = model.homeFocus?.takeIf { k -> rows.any { r -> r.cards.any { it.key == k } } }
        ?: rows.firstOrNull()?.cards?.firstOrNull()?.key
    val requester = remember { FocusRequester() }
    val scope = rememberCoroutineScope()
    // again after a search, which replaced the rows
    LaunchedEffect(model.query.isBlank()) {
        if (model.query.isNotBlank()) return@LaunchedEffect
        // bring the remembered card's row into view, or there's nothing to focus
        val row = rows.indexOfFirst { r -> r.cards.any { it.key == target } }
        if (row >= 0) model.homeList.scrollToItem(row)
        requester.focusSoon()
    }

    // Continue watching shows up once progress arrives from the server. The
    // list keeps the row you're on in place, which would push it out of view.
    // (Not for My list, which appears when you add to it: the card you're on stays in view.)
    val firstRow = rows.firstOrNull()?.title
    LaunchedEffect(firstRow) {
        if (firstRow == "Continue watching" && model.homeList.firstVisibleItemIndex <= 1) model.homeList.scrollToItem(0)
    }

    Box(Modifier.fillMaxSize().background(BG)) {
        if (model.query.isBlank()) Backdrop(focused?.backdrop ?: focused?.poster)
        LibraryPage(model, lib, Screen.Home) { Column(Modifier.fillMaxSize()) {
            Hero(focused)
            if (rows.isEmpty()) {
                Text(
                    "No movies or series were found on this server.",
                    color = MUTED, fontSize = 18.sp, modifier = Modifier.padding(horizontal = EDGE),
                )
            }
            LazyColumn(
                state = model.homeList,
                contentPadding = PaddingValues(bottom = EDGE),
                verticalArrangement = Arrangement.spacedBy(24.dp),
                modifier = Modifier.fillMaxWidth().weight(1f),
            ) {
                items(rows, key = { it.title }) { row ->
                    val state = model.homeRows.getOrPut(row.title) { LazyListState() }
                    CardRow(row, state, target, requester, onFocus = { card ->
                        focused = card.item
                        model.homeFocus = card.key
                    }, onClick = { model.push(it.open) }, menu = { card ->
                        if (card.removable) {
                            listOf(
                                MenuItem("Remove from Continue watching") {
                                    model.hide(card.item.id)
                                    // its card is gone: on to the first one once the row redraws
                                    scope.launch {
                                        kotlinx.coroutines.delay(50)
                                        requester.focusSoon()
                                    }
                                },
                                MenuItem("Open ${card.item.title}") { model.push(Screen.Details(card.item.id)) },
                            )
                        } else {
                            listOf(listItem(model, card.item))
                        }
                    })
                }
            }
        } }
    }
}

/** The focused title, large. */
@Composable
private fun Hero(item: Item?) {
    Column(Modifier.fillMaxWidth().height(190.dp).padding(start = EDGE, top = 8.dp)) {
        if (item == null) return@Column
        if (item.logo != null) {
            Logo(item.logo, item.title, Modifier.height(80.dp).width(320.dp))
        } else {
            Text(
                item.title, fontSize = 32.sp, fontWeight = FontWeight.Bold, color = TEXT,
                maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.fillMaxWidth(0.55f),
            )
        }
        Spacer(Modifier.height(10.dp))
        Text(metaLine(item), color = MUTED, fontSize = 15.sp)
        Spacer(Modifier.height(10.dp))
        Text(
            item.overview, color = TEXT, fontSize = 15.sp, maxLines = 3, overflow = TextOverflow.Ellipsis,
            modifier = Modifier.fillMaxWidth(0.5f),
        )
    }
}

@Composable
private fun CardRow(
    row: HomeRow,
    state: LazyListState,
    target: String?,
    requester: FocusRequester,
    onFocus: (HomeCard) -> Unit,
    onClick: (HomeCard) -> Unit,
    menu: (HomeCard) -> List<MenuItem>,
) {
    Column {
        MonoText(row.title, 13.sp, modifier = Modifier.padding(start = EDGE, bottom = 10.dp))
        LazyRow(
            state = state,
            contentPadding = PaddingValues(horizontal = EDGE),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            items(row.cards, key = { it.key }) { card ->
                WithMenu({ menu(card) }) { m, open ->
                    ArtCard(
                        art = card.art, title = card.title, sub = card.sub,
                        width = if (row.wide) 200.dp else 112.dp,
                        ratio = if (row.wide) 16f / 9f else 2f / 3f,
                        progress = card.progress,
                        plays = row.wide,
                        onClick = { onClick(card) },
                        onLongClick = open,
                        onFocus = { onFocus(card) },
                        focusRequester = if (card.key == target) requester else null,
                        modifier = m,
                    )
                }
            }
        }
    }
}
