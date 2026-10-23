// What the library pages share with the desktop app: the top bar (server
// name, Home / Movies / TV Shows / Streaming, search, updates, profile menu),
// the Movies and TV Shows grids with their filters, the Streaming page, and
// search results (the server's titles, then the addons').
package io.github.okaza03.valo.ui

import android.app.Activity
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import kotlinx.coroutines.launch
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Border
import androidx.tv.material3.Button
import androidx.tv.material3.ButtonDefaults
import androidx.tv.material3.Text
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Filter
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.Library
import io.github.okaza03.valo.Screen
import io.github.okaza03.valo.Sort
import io.github.okaza03.valo.UpdateState

private val POSTER_W = 124.dp

/** The top bar over a library page, and search results in place of the page while searching. */
@Composable
fun LibraryPage(model: AppModel, lib: Library, current: Screen, page: @Composable () -> Unit) {
    Column(Modifier.fillMaxSize()) {
        TopBar(model, current)
        if (model.query.isNotBlank()) SearchResults(model, lib) else page()
    }
}

@Composable
private fun TopBar(model: AppModel, current: Screen) {
    val searching = model.query.isNotBlank()
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().padding(start = EDGE, end = EDGE, top = 20.dp, bottom = 4.dp).height(44.dp),
    ) {
        var name = (model.client?.name ?: "Valo").uppercase()
        if (name.length > 28) name = name.take(27) + "…"
        Text(name, color = TEXT, fontSize = 19.sp, fontWeight = FontWeight.Black, letterSpacing = 0.76.sp, maxLines = 1)
        Spacer(Modifier.width(14.dp))
        Box(Modifier.width(1.dp).height(28.dp).background(LINE))
        Spacer(Modifier.width(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            val tabs = buildList {
                add("Home" to Screen.Home)
                add("Movies" to Screen.Grid(Kind.Movie))
                add("TV Shows" to Screen.Grid(Kind.Series))
                // the addons' catalogs, when the admin added any
                if (model.discover.sources().isNotEmpty()) add("Streaming" to Screen.Streaming)
            }
            for ((label, s) in tabs) {
                Tab(label, !searching && current == s, { model.openTab(s) })
            }
        }
        // the search box gives way when the bar is full
        Box(Modifier.weight(1f).padding(start = 16.dp), contentAlignment = Alignment.CenterEnd) {
            SearchBox(model, Modifier.widthIn(max = 250.dp).fillMaxWidth())
        }
        Spacer(Modifier.width(10.dp))
        UpdateButtons(model)
        Spacer(Modifier.width(6.dp))
        ProfileMenu(model)
    }
}

/**
 * The search box. The remote's arrows would get stuck moving the cursor in a
 * text field, so it's a button until OK is pressed, then the keyboard opens.
 */
@Composable
private fun SearchBox(model: AppModel, modifier: Modifier) {
    val focus = LocalFocusManager.current
    var editing by remember { mutableStateOf(false) }
    // back to the box after typing, unless the remote moved focus elsewhere
    var refocus by remember { mutableStateOf(false) }
    val field = remember { FocusRequester() }
    val button = remember { FocusRequester() }
    LaunchedEffect(editing) {
        if (editing) runCatching { field.requestFocus() } else if (refocus) runCatching { button.requestFocus() }
        refocus = false
    }
    if (editing) {
        var had by remember { mutableStateOf(false) }
        Field(
            model.query, model::search,
            modifier.focusRequester(field).onFocusChanged {
                if (it.isFocused) had = true else if (had) editing = false
            },
            hint = "Search movies & shows", imeAction = ImeAction.Search, compact = true,
            onDone = {
                // on to the results, or back to the box when there are none
                refocus = !focus.moveFocus(FocusDirection.Down)
                editing = false
            },
        )
        return
    }
    Button(
        onClick = { editing = true },
        modifier = modifier.height(38.dp).focusRequester(button),
        shape = ButtonDefaults.shape(RoundedCornerShape(2.dp)),
        scale = ButtonDefaults.scale(focusedScale = 1f),
        contentPadding = PaddingValues(horizontal = 10.dp),
        colors = ButtonDefaults.colors(
            containerColor = Color(0xFF16171B), contentColor = TEXT,
            focusedContainerColor = Color(0xFF16171B), focusedContentColor = TEXT,
        ),
        border = ButtonDefaults.border(
            border = Border(androidx.compose.foundation.BorderStroke(1.dp, LINE_HI), shape = RoundedCornerShape(2.dp)),
            focusedBorder = Border(androidx.compose.foundation.BorderStroke(2.dp, TEXT), shape = RoundedCornerShape(2.dp)),
        ),
    ) {
        Text(
            model.query.ifEmpty { "Search movies & shows" }, color = if (model.query.isEmpty()) MUTED else TEXT,
            fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.fillMaxWidth(),
        )
    }
}

/** "Check for updates", and "Update x.y.z" once there is one. */
@Composable
private fun UpdateButtons(model: AppModel) {
    when (val u = model.update) {
        is UpdateState.Available -> {
            Tab("Update ${u.release.version}", true, model::installUpdate)
            Spacer(Modifier.width(6.dp))
        }
        is UpdateState.Downloading -> {
            Tab("Downloading ${u.percent}%", true, {})
            Spacer(Modifier.width(6.dp))
        }
        else -> {}
    }
    val checking = model.update == UpdateState.Checking
    Button(
        onClick = { model.checkUpdates(quiet = false) },
        modifier = Modifier.size(40.dp),
        shape = ButtonDefaults.shape(RoundedCornerShape(2.dp)),
        scale = focusScale(),
        glow = focusGlow(),
        contentPadding = PaddingValues(0.dp),
        colors = ButtonDefaults.colors(
            containerColor = Color.Transparent, contentColor = if (checking) MUTED else TEXT,
            focusedContainerColor = Color.White, focusedContentColor = INK,
        ),
    ) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            RefreshIcon(androidx.tv.material3.LocalContentColor.current)
        }
    }
}

/** The signed-in user, with a menu: their profile and account actions. */
@Composable
private fun ProfileMenu(model: AppModel) {
    val ctx = LocalContext.current
    val user = model.prefs.user?.takeIf { model.client?.token != null } ?: "Guest"
    var open by remember { mutableStateOf(false) }
    var wasOpen by remember { mutableStateOf(false) }
    val button = remember { FocusRequester() }
    LaunchedEffect(open) {
        if (open) wasOpen = true else if (wasOpen) runCatching { button.requestFocus() }
    }
    Box {
        Button(
            onClick = { open = true },
            modifier = Modifier.height(40.dp).focusRequester(button),
            shape = ButtonDefaults.shape(RoundedCornerShape(2.dp)),
            scale = focusScale(),
            glow = focusGlow(),
            contentPadding = PaddingValues(start = 10.dp, end = 12.dp),
            colors = ButtonDefaults.colors(
                containerColor = if (open) SURFACE_HI else Color.Transparent, contentColor = TEXT,
                focusedContainerColor = Color.White, focusedContentColor = INK,
            ),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Avatar(user, 24.dp)
                Spacer(Modifier.width(10.dp))
                val ink = androidx.tv.material3.LocalContentColor.current
                MonoText(user, 12.sp, ink)
                Spacer(Modifier.width(10.dp))
                Text("▾", color = ink.copy(alpha = 0.6f), fontSize = 13.sp)
            }
        }
        Menu(
            open = open,
            onDismiss = { open = false },
            alignEnd = true,
            header = {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(6.dp)) {
                    Avatar(user, 36.dp)
                    Spacer(Modifier.width(12.dp))
                    Column {
                        Text(user, color = TEXT, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                        MonoText(model.client?.name ?: "", 11.sp, MUTED)
                    }
                }
            },
            items = listOf(
                MenuItem("Profile") { model.push(Screen.Profile) },
                null,
                MenuItem(if (model.busy) "Refreshing…" else "Refresh library") { model.loadLibrary() },
                MenuItem("Change server", model::changeServer),
                null,
                if (model.client?.token != null) MenuItem("Sign out", model::signOut) else null,
                MenuItem("Quit") { (ctx as? Activity)?.finish() },
            ).let { l -> l.filterIndexed { i, it -> it != null || (i > 0 && l[i - 1] != null) } },
        )
    }
}

/** A dropdown showing its current value, like the desktop app's combo boxes. */
@Composable
private fun <T> Choice(label: String, value: T, options: List<Pair<T, String>>, onPick: (T) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Row(verticalAlignment = Alignment.CenterVertically) {
        MonoText(label, 11.sp, MUTED)
        Spacer(Modifier.width(10.dp))
        Box {
            Button(
                onClick = { open = true },
                modifier = Modifier.width(170.dp).height(38.dp),
                shape = ButtonDefaults.shape(RoundedCornerShape(2.dp)),
                scale = focusScale(),
                glow = focusGlow(),
                contentPadding = PaddingValues(horizontal = 12.dp),
                colors = ButtonDefaults.colors(
                    containerColor = Color(0xFF16171B), contentColor = TEXT,
                    focusedContainerColor = Color.White, focusedContentColor = INK,
                ),
                border = ButtonDefaults.border(
                    border = Border(androidx.compose.foundation.BorderStroke(1.dp, LINE_HI), shape = RoundedCornerShape(2.dp)),
                    focusedBorder = Border.None,
                ),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(options.find { it.first == value }?.second ?: "", fontSize = 14.sp, maxLines = 1, modifier = Modifier.weight(1f))
                    Text("▾", color = androidx.tv.material3.LocalContentColor.current.copy(alpha = 0.6f), fontSize = 13.sp)
                }
            }
            Menu(
                open = open,
                onDismiss = { open = false },
                items = options.map { (v, text) -> MenuItem(text) { onPick(v) } },
            )
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
    val inGenre = all.filter { i -> g.genre.let { it == null || it in i.genres } }
    val status = inGenre.map(model::status)
    fun count(f: Filter) = status.count { it == f }
    var items = inGenre.zip(status).filter { g.filter == Filter.All || it.second == g.filter }.map { it.first }
    // titles without the key go last either way; the library comes A to Z
    fun <K : Comparable<K>> by(key: (Item) -> K?): List<Item> {
        val (with, without) = items.partition { key(it) != null }
        val sorted = if (g.desc) with.sortedByDescending { key(it)!! } else with.sortedBy { key(it)!! }
        return sorted + without
    }
    items = when (g.sort) {
        Sort.Title -> if (g.desc) items.reversed() else items
        Sort.Year -> by { it.year }
        Sort.Added -> by { it.added }
        Sort.Director -> by { it.maker?.lowercase() }
    }
    val makerLabel = if (kind == Kind.Movie) "Director" else "Creator"

    Box(Modifier.fillMaxSize().background(BG)) {
        LibraryPage(model, lib, Screen.Grid(kind)) {
            PosterGrid(
                model, items, g.list, g.focus, { g.focus = it }, empty = "Nothing here",
                sub = if (g.sort == Sort.Director) { i -> i.maker } else { i -> i.year?.toString() },
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 14.dp, bottom = 10.dp)) {
                    Segmented(
                        g.filter,
                        listOf(
                            Triple(Filter.All, "All", inGenre.size),
                            Triple(Filter.InProgress, "In progress", count(Filter.InProgress)),
                            Triple(Filter.Unwatched, "Unwatched", count(Filter.Unwatched)),
                            Triple(Filter.Watched, "Watched", count(Filter.Watched)),
                        ),
                        { g.filter = it },
                    )
                    Spacer(Modifier.weight(1f).widthIn(min = 24.dp))
                    // each sort both ways (the desktop app's direction button)
                    Choice(
                        "Sort", g.sort to g.desc,
                        buildList {
                            add((Sort.Title to false) to "A – Z")
                            add((Sort.Title to true) to "Z – A")
                            add((Sort.Year to true) to "Newest first")
                            add((Sort.Year to false) to "Oldest first")
                            if (all.any { it.added != null }) {
                                add((Sort.Added to true) to "Recently added")
                                add((Sort.Added to false) to "Added first")
                            }
                            if (all.any { it.maker != null }) {
                                add((Sort.Director to false) to "$makerLabel A – Z")
                                add((Sort.Director to true) to "$makerLabel Z – A")
                            }
                        },
                    ) { (sort, desc) ->
                        g.sort = sort
                        g.desc = desc
                    }
                    if (genres.isNotEmpty()) {
                        Spacer(Modifier.width(24.dp))
                        Choice("Genre", g.genre, listOf<Pair<String?, String>>(null to "All genres") + genres.map { it to it }) {
                            g.genre = it
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun SearchResults(model: AppModel, lib: Library) {
    val hits = model.searchHits(lib)
    // the addons' titles the server doesn't have, once they answer
    val found = model.discover.results?.takeIf { it.first == model.query.trim() }
    val more = found?.second.orEmpty().mapNotNull { model.discover.remote[it] }
    // focus stays in the search box while typing; only coming back from a title moves it
    PosterGrid(
        model, hits, model.searchList, model.searchFocus, { model.searchFocus = it },
        empty = if (found?.third == true) "Searching…" else "Nothing matches",
        grab = model.searchFocus != null,
        extra = more,
        extraHeader = {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 18.dp, bottom = 4.dp)) {
                MonoText("Not on the server", 13.sp, TEAL)
                Spacer(Modifier.width(10.dp))
                MonoText(if (found?.third == true) "${more.size}…" else more.size.toString(), 13.sp, MUTED)
                Spacer(Modifier.width(16.dp))
                Text("Open one to request it.", color = MUTED, fontSize = 13.sp)
            }
        },
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 14.dp, bottom = 4.dp)) {
            MonoText("Results for “${model.query.trim()}”", 13.sp)
            Spacer(Modifier.width(10.dp))
            MonoText(hits.size.toString(), 13.sp, MUTED)
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

    Box(Modifier.fillMaxSize().background(BG)) {
        LibraryPage(model, lib, Screen.Streaming) {
            if (pick == null) {
                Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally) {
                    Spacer(Modifier.height(120.dp))
                    MonoText(if (d.loading) "Loading the catalogs" else "No catalogs yet", 13.sp, MUTED)
                }
                return@LibraryPage
            }
            run {
                PosterGrid(
                    model, items, st.list, st.focus, { st.focus = it }, empty = "Nothing here",
                    sub = { i -> if (i.remote) listOfNotNull(i.year?.toString(), "not on server").joinToString(" · ") else i.year?.toString() },
                ) {
                    Column(Modifier.padding(top = 14.dp, bottom = 10.dp)) {
                        MonoText("Streaming", 12.sp, TEAL)
                        Spacer(Modifier.height(4.dp))
                        Text(pick, fontSize = 32.sp, fontWeight = FontWeight.Bold, color = TEXT)
                        Text(
                            "What's on it now. Titles on the server play; the rest you can request.",
                            color = MUTED, fontSize = 15.sp,
                        )
                        Spacer(Modifier.height(14.dp))
                        // a new service starts at the top; focus stays on its button
                        Segmented(pick, sources.map { Triple(it, it, null) }, {
                            st.service = it
                            st.focus = null
                            scope.launch { st.list.scrollToItem(0) }
                        }, Modifier.horizontalScroll(rememberScrollState()))
                        if (movies.isNotEmpty() && series.isNotEmpty()) {
                            Spacer(Modifier.height(10.dp))
                            Segmented(
                                st.kind,
                                listOf(
                                    Triple(null, "All", movies.size + series.size),
                                    Triple(Kind.Movie, "Movies", movies.size),
                                    Triple(Kind.Series, "TV shows", series.size),
                                ),
                                {
                                    st.kind = it
                                    st.focus = null
                                    scope.launch { st.list.scrollToItem(0) }
                                },
                            )
                        }
                    }
                }
            }
        }
    }
}

/**
 * Poster cards in rows that fill the width. Focus comes back to the card
 * last opened, or starts on the first.
 */
@Composable
private fun PosterGrid(
    model: AppModel,
    items: List<Item>,
    state: LazyGridState,
    focusKey: String?,
    onFocus: (String) -> Unit,
    empty: String,
    grab: Boolean = true,
    sub: (Item) -> String? = { it.year?.toString() },
    /** More cards after `items`, under their own heading. */
    extra: List<Item> = emptyList(),
    extraHeader: @Composable () -> Unit = {},
    header: @Composable () -> Unit,
) {
    val target = focusKey?.takeIf { k -> items.any { it.id == k } || extra.any { it.id == k } } ?: items.firstOrNull()?.id
    val requester = remember { FocusRequester() }
    LaunchedEffect(Unit) {
        // its place in the grid: after the header (and the empty note), and the extras' heading
        val i = items.indexOfFirst { it.id == target }.takeIf { it >= 0 }?.let { it + 1 }
            ?: extra.indexOfFirst { it.id == target }.takeIf { it >= 0 }?.let { it + items.size + if (items.isEmpty()) 3 else 2 }
        if (i == null || !grab) return@LaunchedEffect
        val key = if (i > items.size + 1) "extra/$target" else target
        if (state.layoutInfo.visibleItemsInfo.none { it.key == key }) state.scrollToItem(i)
        requester.focusSoon()
    }
    LazyVerticalGrid(
        columns = GridCells.Adaptive(POSTER_W),
        state = state,
        contentPadding = PaddingValues(start = EDGE, end = EDGE, bottom = EDGE),
        horizontalArrangement = Arrangement.spacedBy(18.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
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
        items(items, key = { it.id }) { i -> GridCard(model, i, sub(i), onFocus, if (i.id == target) requester else null) }
        if (extra.isNotEmpty()) {
            item(key = "extra", span = { GridItemSpan(maxLineSpan) }) { extraHeader() }
            items(extra, key = { "extra/" + it.id }) { i ->
                val what = listOfNotNull(i.year?.toString(), if (i.kind == Kind.Movie) "Movie" else "Series").joinToString(" · ")
                GridCard(model, i, what, onFocus, if (i.id == target) requester else null)
            }
        }
    }
}

@Composable
private fun GridCard(model: AppModel, i: Item, sub: String?, onFocus: (String) -> Unit, requester: FocusRequester?) {
    WithMenu({ listOf(listItem(model, i)) }) { m, menu ->
        ArtCard(
            art = i.poster, title = i.title, sub = sub, width = POSTER_W, ratio = 2f / 3f,
            progress = if (i.kind == Kind.Movie) model.entry(i.id)?.takeIf { it.inProgress }?.fraction else null,
            onClick = { model.push(Screen.Details(i.id)) },
            onLongClick = menu,
            onFocus = { onFocus(i.id) },
            focusRequester = requester,
            modifier = m,
        )
    }
}
