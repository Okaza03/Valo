// The library pages: the bottom tabs and top bar, the home page, the Movies
// and TV Shows grids with their filters, the Streaming page, and search
// results (the server's titles, then the addons').
package io.github.okaza03.valo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
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
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Filter
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.Library
import io.github.okaza03.valo.R
import io.github.okaza03.valo.Screen
import io.github.okaza03.valo.Sort
import io.github.okaza03.valo.cast.Caster
import kotlinx.coroutines.launch

private val POSTER_W = 104.dp

/** Home, Movies, TV Shows, Streaming and Profile. */
@Composable
fun BottomBar(model: AppModel) {
    val current = model.screen
    val tabs = buildList {
        add(Triple("Home", R.drawable.ic_home, Screen.Home))
        add(Triple("Movies", R.drawable.ic_movie, Screen.Grid(Kind.Movie)))
        add(Triple("TV Shows", R.drawable.ic_tv, Screen.Grid(Kind.Series)))
        // the addons' catalogs, when the admin turned any on
        if (model.discover.sources().isNotEmpty()) add(Triple("Streaming", R.drawable.ic_streaming, Screen.Streaming))
        add(Triple("Profile", R.drawable.ic_person, Screen.Profile))
    }
    NavigationBar(containerColor = SURFACE) {
        for ((label, icon, s) in tabs) {
            val on = current == s && model.query.isBlank()
            NavigationBarItem(
                selected = on,
                onClick = { model.openTab(s) },
                icon = { Ic(icon, label, tint = if (on) INK else MUTED) },
                label = { Text(label, fontSize = 11.sp, maxLines = 1, color = if (on) TEXT else MUTED) },
                colors = NavigationBarItemDefaults.colors(indicatorColor = Color.White),
            )
        }
    }
}

/** What's on the TV, under the page, while you browse. */
@Composable
fun MiniCast(model: AppModel, cast: Caster) {
    val n = cast.now ?: return
    Column(Modifier.fillMaxWidth().background(SURFACE_HI).clickable { model.push(Screen.Player(n.itemId, n.episodeId)) }) {
        if (cast.durationMs > 0) {
            ProgressBar(cast.positionMs.toFloat() / cast.durationMs, Modifier.fillMaxWidth())
        }
        Row(Modifier.padding(start = PAD, end = 4.dp, top = 6.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Art(n.art, Modifier.height(44.dp).aspectRatio(2f / 3f))
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(n.title, color = TEXT, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text("Playing on ${cast.device ?: "your TV"}", color = MUTED, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            IconBtn(if (cast.playing) R.drawable.ic_pause else R.drawable.ic_play, if (cast.playing) "Pause" else "Play", onClick = cast::togglePause)
            IconBtn(R.drawable.ic_close, "Stop casting", onClick = cast::stop)
        }
    }
}

/** The top bar over a library page, and search results in place of the page while searching. */
@Composable
fun LibraryPage(model: AppModel, lib: Library, page: @Composable () -> Unit) {
    var searching by remember { mutableStateOf(model.query.isNotEmpty()) }
    if (model.query.isNotEmpty()) searching = true
    Column(Modifier.fillMaxSize()) {
        if (searching) {
            SearchBar(model) { searching = false }
        } else {
            Row(Modifier.fillMaxWidth().padding(start = PAD, end = 4.dp).height(56.dp), verticalAlignment = Alignment.CenterVertically) {
                var name = (model.client?.name ?: "Valo").uppercase()
                if (name.length > 24) name = name.take(23) + "…"
                Text(name, color = TEXT, fontSize = 17.sp, fontWeight = FontWeight.Black, letterSpacing = 0.7.sp, maxLines = 1, modifier = Modifier.weight(1f))
                CastButton()
                IconBtn(R.drawable.ic_search, "Search") { searching = true }
            }
        }
        if (model.query.isNotBlank()) SearchResults(model, lib) else page()
    }
}

@Composable
private fun SearchBar(model: AppModel, onClose: () -> Unit) {
    val focus = LocalFocusManager.current
    val field = remember { FocusRequester() }
    LaunchedEffect(Unit) { if (model.query.isEmpty()) runCatching { field.requestFocus() } }
    Row(Modifier.fillMaxWidth().padding(start = PAD, end = 4.dp, top = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        OutlinedTextField(
            model.query, model::search, Modifier.weight(1f).focusRequester(field),
            placeholder = { Text("Search movies & shows", color = MUTED) },
            singleLine = true,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { focus.clearFocus() }),
        )
        IconBtn(R.drawable.ic_close, "Close search") {
            model.search("")
            onClose()
        }
    }
}

@Composable
private fun SearchResults(model: AppModel, lib: Library) {
    val hits = model.searchHits(lib)
    // the addons' titles the server doesn't have, once they answer
    val found = model.discover.results?.takeIf { it.first == model.query.trim() }
    val more = found?.second.orEmpty().mapNotNull { model.discover.remote[it] }
    PosterGrid(
        model, hits, model.searchList,
        empty = if (found?.third == true) "Searching…" else "Nothing matches",
        extra = more,
        extraHeader = {
            Column(Modifier.padding(top = 18.dp, bottom = 4.dp)) {
                Row {
                    MonoText("Not on the server", 12.sp, TEAL)
                    Spacer(Modifier.width(10.dp))
                    MonoText(if (found?.third == true) "${more.size}…" else more.size.toString(), 12.sp, MUTED)
                }
                Text("Open one to request it.", color = MUTED, fontSize = 13.sp)
            }
        },
    ) {
        Row(Modifier.padding(top = 8.dp, bottom = 4.dp)) {
            MonoText("Results for “${model.query.trim()}”", 12.sp, modifier = Modifier.weight(1f, fill = false))
            Spacer(Modifier.width(10.dp))
            MonoText(hits.size.toString(), 12.sp, MUTED)
        }
    }
}

/** Poster cards in rows that fill the width. */
@Composable
private fun PosterGrid(
    model: AppModel,
    items: List<Item>,
    state: LazyGridState,
    empty: String,
    sub: (Item) -> String? = { it.year?.toString() },
    /** More cards after `items`, under their own heading. */
    extra: List<Item> = emptyList(),
    extraHeader: @Composable () -> Unit = {},
    header: @Composable () -> Unit,
) {
    LazyVerticalGrid(
        columns = GridCells.Adaptive(POSTER_W),
        state = state,
        contentPadding = PaddingValues(start = PAD, end = PAD, bottom = PAD),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        item(key = "header", span = { GridItemSpan(maxLineSpan) }) { header() }
        if (items.isEmpty()) {
            item(key = "empty", span = { GridItemSpan(maxLineSpan) }) {
                Box(Modifier.fillMaxWidth().padding(top = 40.dp), contentAlignment = Alignment.Center) {
                    MonoText(if (extra.isEmpty()) empty else "Nothing on the server matches", 12.sp, MUTED)
                }
            }
        }
        items(items, key = { it.id }) { i -> PosterCard(model, i, sub = sub(i)) }
        if (extra.isNotEmpty()) {
            item(key = "extra", span = { GridItemSpan(maxLineSpan) }) { extraHeader() }
            items(extra, key = { "extra/" + it.id }) { i ->
                val what = listOfNotNull(i.year?.toString(), if (i.kind == Kind.Movie) "Movie" else "Series").joinToString(" · ")
                PosterCard(model, i, sub = what)
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(model: AppModel, lib: Library) {
    val rows = homeRows(model, lib)
    // the top of the page: the first title that isn't already on a Continue watching card
    val feature = rows.firstOrNull { !it.wide }?.cards?.firstOrNull()
    val firstRow = rows.firstOrNull()?.title
    // Continue watching shows up once progress arrives from the server
    LaunchedEffect(firstRow) {
        if (firstRow == "Continue watching" && model.homeList.firstVisibleItemIndex <= 1) model.homeList.scrollToItem(0)
    }
    LibraryPage(model, lib) {
        PullToRefreshBox(model.busy, model::loadLibrary, Modifier.fillMaxSize()) {
            LazyColumn(
                state = model.homeList,
                contentPadding = PaddingValues(bottom = PAD),
                verticalArrangement = Arrangement.spacedBy(22.dp),
                modifier = Modifier.fillMaxSize(),
            ) {
                if (rows.isEmpty()) {
                    item {
                        Text("No movies or series were found on this server.", color = MUTED, fontSize = 15.sp, modifier = Modifier.padding(PAD))
                    }
                }
                feature?.let { f -> item(key = "feature") { Feature(model, f.item) } }
                items(rows, key = { it.title }) { row ->
                    val state = model.homeRows.getOrPut(row.title) { LazyListState() }
                    Column {
                        RowTitle(row.title)
                        LazyRow(
                            state = state,
                            contentPadding = PaddingValues(horizontal = PAD),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            items(row.cards, key = { it.key }) { c ->
                                ArtCard(
                                    c.art, c.title, c.sub, if (row.wide) 16f / 9f else 2f / 3f,
                                    { model.push(c.open) },
                                    Modifier.width(if (row.wide) 220.dp else 112.dp),
                                    progress = c.progress, watched = c.watched, plays = row.wide,
                                    menu = {
                                        if (c.removable) {
                                            listOf(
                                                "Remove from Continue watching" to { model.hide(c.item.id) },
                                                "Open ${c.item.title}" to { model.push(Screen.Details(c.item.id)) },
                                            )
                                        } else {
                                            listItem(model, c.item)
                                        }
                                    },
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

/** One title, large, at the top of the home page. */
@Composable
private fun Feature(model: AppModel, item: Item) {
    Box(Modifier.fillMaxWidth().aspectRatio(16f / 10f).clickable { model.push(Screen.Details(item.id)) }) {
        Art(item.backdrop ?: item.poster, Modifier.fillMaxSize())
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.35f to Color.Transparent, 1f to BG)))
        Column(Modifier.align(Alignment.BottomStart).padding(horizontal = PAD)) {
            if (item.logo != null) {
                Logo(item.logo, item.title, Modifier.height(64.dp).fillMaxWidth(0.7f))
            } else {
                Text(item.title, fontSize = 26.sp, fontWeight = FontWeight.Bold, color = TEXT, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            Spacer(Modifier.height(6.dp))
            Text(metaLine(item), color = MUTED, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** Movies or TV Shows: every title, filtered and sorted like on the desktop. */
@Composable
fun GridScreen(model: AppModel, lib: Library, kind: Kind) {
    val g = model.grids.getValue(kind)
    val all = if (kind == Kind.Movie) lib.movies else lib.series
    val genres = all.flatMap { it.genres }.distinct().sorted()
    if (g.genre != null && g.genre !in genres) g.genre = null
    val t = gridTitles(model, g, all)
    LibraryPage(model, lib) {
        PosterGrid(
            model, t.items, g.list, empty = "Nothing here",
            sub = if (g.sort == Sort.Director) { i -> i.maker } else { i -> i.year?.toString() },
        ) {
            Column(Modifier.padding(bottom = 4.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Chips(
                    g.filter,
                    listOf(
                        Triple(Filter.All, "All", t.inGenre.size),
                        Triple(Filter.InProgress, "In progress", t.count(Filter.InProgress)),
                        Triple(Filter.Unwatched, "Unwatched", t.count(Filter.Unwatched)),
                        Triple(Filter.Watched, "Watched", t.count(Filter.Watched)),
                    ),
                    { g.filter = it },
                    edge = 0.dp,
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Choice("Sort", g.sort to g.desc, sortChoices(all, kind), Modifier.weight(1f)) { (sort, desc) ->
                        g.sort = sort
                        g.desc = desc
                    }
                    if (genres.isNotEmpty()) {
                        Choice("Genre", g.genre, listOf<Pair<String?, String>>(null to "All") + genres.map { it to it }, Modifier.weight(1f)) {
                            g.genre = it
                        }
                    }
                }
            }
        }
    }
}

/** Everything on one streaming service (or other catalog), as a grid. */
@Composable
fun StreamingScreen(model: AppModel, lib: Library) {
    val d = model.discover
    val sources = d.sources()
    val st = model.streaming
    val pick = st.service?.takeIf { it in sources } ?: sources.firstOrNull()
    val mine = d.shown.filter { it.source == pick }
    fun ids(k: Kind) = mine.filter { it.kind == k }.flatMap { it.ids }.distinct()
    val movies = ids(Kind.Movie)
    val series = ids(Kind.Series)
    if (st.kind != null && ids(st.kind!!).isEmpty()) st.kind = null
    val items = when (st.kind) {
        Kind.Movie -> movies
        Kind.Series -> series
        null -> movies + series
    }.mapNotNull { d.find(lib, it) }
    val scope = rememberCoroutineScope()

    LibraryPage(model, lib) {
        if (pick == null) {
            Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally) {
                Spacer(Modifier.height(120.dp))
                if (d.loading) CircularProgressIndicator(Modifier.size(24.dp)) else MonoText("No catalogs yet", 12.sp, MUTED)
            }
            return@LibraryPage
        }
        PosterGrid(
            model, items, st.list, empty = "Nothing here",
            sub = { i -> if (i.remote) listOfNotNull(i.year?.toString(), "not on server").joinToString(" · ") else i.year?.toString() },
        ) {
            Column(Modifier.padding(bottom = 4.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(pick, fontSize = 26.sp, fontWeight = FontWeight.Bold, color = TEXT)
                Text("Play what's already on your server, and request anything that isn't.", color = MUTED, fontSize = 13.sp)
                // a new service starts at the top
                Chips(pick, sources.map { Triple(it, it, null) }, {
                    st.service = it
                    scope.launch { st.list.scrollToItem(0) }
                }, edge = 0.dp)
                if (movies.isNotEmpty() && series.isNotEmpty()) {
                    Chips(
                        st.kind,
                        listOf(
                            Triple(null, "All", movies.size + series.size),
                            Triple(Kind.Movie, "Movies", movies.size),
                            Triple(Kind.Series, "TV shows", series.size),
                        ),
                        {
                            st.kind = it
                            scope.launch { st.list.scrollToItem(0) }
                        },
                        edge = 0.dp,
                    )
                }
            }
        }
    }
}
