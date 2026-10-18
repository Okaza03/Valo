// Person pages (a cast member, director or creator, built from what the
// library knows about them) and the signed-in user's profile page, with
// watching stats and "Your devices" (the desktop app's browse/people.rs).
package io.github.okaza03.valo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Devices
import io.github.okaza03.valo.Entry
import io.github.okaza03.valo.Filter
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.Library
import io.github.okaza03.valo.Screen

private fun same(a: String, b: String) = a.trim().equals(b.trim(), ignoreCase = true)

/** A title someone is credited on, with the character they played. */
private class Credit(val item: Item, val character: String?)

/** Everything one person did in the library, newest first. */
private class Person(lib: Library, name: String) {
    val movies = ArrayList<Credit>()
    val shows = ArrayList<Credit>()
    val directed = ArrayList<Credit>()
    val created = ArrayList<Credit>()
    var photo: String? = null

    init {
        for (item in lib.all) {
            item.cast.find { same(it.name, name) }?.let { m ->
                photo = photo ?: m.photo
                val c = Credit(item, m.character?.takeIf { it.isNotBlank() })
                if (item.kind == Kind.Movie) movies += c else shows += c
            }
            if (item.director?.split(',')?.any { same(it, name) } == true) directed += Credit(item, null)
            if (item.creators.any { same(it, name) }) created += Credit(item, null)
        }
        // newest first, titles without a year last
        for (l in listOf(movies, shows, directed, created)) l.sortWith(compareBy({ it.item.year == null }, { -(it.item.year ?: 0) }))
    }

    val titles get() = (movies + shows + directed + created).map { it.item }.distinctBy { it.id }

    val roles get() = buildList {
        if (movies.isNotEmpty() || shows.isNotEmpty()) add("Actor")
        if (directed.isNotEmpty()) add("Director")
        if (created.isNotEmpty()) add("Creator")
    }
}

/** "1999–2021", or one year. */
private fun span(items: List<Item>): String? {
    val lo = items.mapNotNull { it.year }.minOrNull() ?: return null
    val hi = items.mapNotNull { it.endYear ?: it.year }.maxOrNull() ?: lo
    return if (hi > lo) "$lo–$hi" else "$lo"
}

/** The `n` most common of `names`, most common first, ties A to Z. */
private fun top(names: List<String>, n: Int): List<Pair<String, Int>> =
    names.groupingBy { it }.eachCount().entries
        .sortedWith(compareByDescending<Map.Entry<String, Int>> { it.value }.thenBy { it.key })
        .take(n).map { it.key to it.value }

@Composable
fun PersonScreen(model: AppModel, lib: Library, name: String) {
    val p = remember(lib, name) { Person(lib, name) }
    val titles = p.titles
    val first = remember { FocusRequester() }
    LaunchedEffect(name) { first.focusSoon() }
    // their best-known part: the highest-scored title they acted in
    val known = (p.movies + p.shows).maxByOrNull { it.item.score ?: 0f }
    val costars = top(
        titles.flatMap { i -> (i.cast.map { it.name } + i.makers).distinct() }.filter { !same(it, name) }, 8,
    )

    Box(Modifier.fillMaxSize().background(BG)) {
        Backdrop(titles.filter { it.backdrop != null }.maxByOrNull { it.year ?: 0 }?.backdrop)
        LazyColumn(
            contentPadding = PaddingValues(top = EDGE, bottom = EDGE),
            verticalArrangement = Arrangement.spacedBy(28.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            item {
                Row(Modifier.padding(horizontal = EDGE), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(150.dp).clip(CircleShape)) {
                        if (p.photo != null) Art(p.photo, Modifier.fillMaxSize()) else Avatar(name, 150.dp)
                    }
                    Spacer(Modifier.width(32.dp))
                    Column(Modifier.fillMaxWidth(0.7f)) {
                        MonoText(p.roles.ifEmpty { listOf("Person") }.joinToString("  ·  "), 12.sp, TEAL)
                        Spacer(Modifier.height(6.dp))
                        Text(name, fontSize = 40.sp, fontWeight = FontWeight.Black, color = TEXT, maxLines = 2, overflow = TextOverflow.Ellipsis)
                        Spacer(Modifier.height(6.dp))
                        val n = titles.size
                        Text(
                            listOfNotNull("$n title${if (n == 1) "" else "s"} in your library", span(titles)).joinToString("  ·  "),
                            color = TEXT.copy(alpha = 0.85f), fontSize = 15.sp,
                        )
                        Spacer(Modifier.height(14.dp))
                        if (known != null) {
                            val label = "Known for ${known.item.title}" + (known.character?.let { " as $it" } ?: "")
                            TvButton(label, { model.push(Screen.Details(known.item.id)) }, Modifier.focusRequester(first))
                        } else {
                            TvButton("Back", { model.back() }, Modifier.focusRequester(first))
                        }
                    }
                }
            }
            if (titles.isEmpty()) {
                item { MonoText("Nothing in your library with them", 12.sp, MUTED, Modifier.padding(horizontal = EDGE)) }
            }
            for ((title, list) in listOf("Movies" to p.movies, "TV shows" to p.shows, "Directed" to p.directed, "Created" to p.created)) {
                if (list.isEmpty()) continue
                item(key = title) {
                    Column {
                        RowTitle(title, list.size)
                        LazyRow(
                            contentPadding = PaddingValues(horizontal = EDGE),
                            horizontalArrangement = Arrangement.spacedBy(16.dp),
                        ) {
                            items(list, key = { it.item.id }) { c ->
                                val year = c.item.year?.toString()
                                val sub = when {
                                    c.character != null && year != null -> "as ${c.character} · $year"
                                    c.character != null -> "as ${c.character}"
                                    else -> year
                                }
                                PosterCard(model, c.item, sub)
                            }
                        }
                    }
                }
            }
            if (costars.isNotEmpty()) {
                item(key = "costars") {
                    Column {
                        RowTitle("Often works with")
                        LazyRow(
                            contentPadding = PaddingValues(horizontal = EDGE),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            items(costars, key = { it.first }) { (who, k) ->
                                val photo = titles.firstNotNullOfOrNull { i -> i.cast.find { same(it.name, who) }?.photo }
                                PersonCard(who, photo, "$k title${if (k == 1) "" else "s"}") { model.push(Screen.Person(who)) }
                            }
                        }
                    }
                }
            }
            if (titles.isNotEmpty()) {
                item(key = "about") {
                    val genres = top(titles.flatMap { it.genres }, 4)
                    val scores = titles.mapNotNull { it.score }
                    val watched = titles.count { model.status(it) == Filter.Watched }
                    val rows = buildList {
                        if (p.roles.isNotEmpty()) add("Roles" to p.roles.joinToString(", "))
                        for ((k, n) in listOf("Movies" to p.movies.size, "TV shows" to p.shows.size, "Directed" to p.directed.size, "Created" to p.created.size)) {
                            if (n > 0) add(k to n.toString())
                        }
                        span(titles)?.let { add("Years" to it) }
                        if (genres.isNotEmpty()) add("Genres" to genres.joinToString(", ") { it.first })
                        if (scores.isNotEmpty()) add("Average score" to "%.1f".format(scores.average()))
                        add("You've watched" to "$watched of ${titles.size}")
                    }
                    Panel(Modifier.padding(horizontal = EDGE).width(560.dp)) {
                        MonoText("About", 12.sp, TEAL)
                        InfoRows(rows)
                    }
                }
            }
        }
    }
}

@Composable
fun ProfileScreen(model: AppModel, lib: Library) {
    val c = model.client ?: return
    val signedIn = c.token != null
    val name = model.prefs.user?.takeIf { signedIn } ?: "Guest"
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) {
        first.focusSoon()
        if (signedIn) model.loadDevices()
    }

    // what each entry is: a movie, or an episode of a series
    class Watch(val item: Item, val episodeId: String?, val sub: String, val art: String?, val e: Entry)
    val entries = model.history()
    fun secs(e: Entry) = if (e.dur > 0) minOf(e.pos, e.dur) else e.pos
    var movieSecs = 0.0
    var epSecs = 0.0
    var moviesDone = 0
    var epsDone = 0
    var inProgress = 0
    val genreSecs = HashMap<String, Double>()
    val perShow = HashMap<String, Pair<Int, Double>>()
    val recent = ArrayList<Watch>()
    for ((id, e) in entries) {
        val movie = lib.movies.find { it.id == id }
        val item = if (movie != null) {
            movieSecs += secs(e)
            if (e.watched) moviesDone++
            recent += Watch(movie, null, movie.year?.let { "Movie · $it" } ?: "Movie", movie.thumb ?: movie.backdrop ?: movie.poster, e)
            movie
        } else {
            val (s, season, ep) = lib.findEpisode(id) ?: continue
            epSecs += secs(e)
            if (e.watched) epsDone++
            val (n, t) = perShow[s.id] ?: (0 to 0.0)
            perShow[s.id] = (n + if (e.watched) 1 else 0) to (t + secs(e))
            recent += Watch(s, ep.id, "S%02dE%02d · %s".format(season.number, ep.number, ep.title), ep.still ?: s.backdrop ?: s.poster, e)
            s
        }
        if (e.inProgress) inProgress++
        for (g in item.genres) genreSecs[g] = (genreSecs[g] ?: 0.0) + secs(e)
    }
    val showsDone = lib.series.count { model.status(it) == Filter.Watched }
    val onList = model.myList(lib).size
    fun time(s: Double) = if (s > 0) fmtRuntime(s) else "—"
    val genres = genreSecs.entries.sortedWith(compareByDescending<Map.Entry<String, Double>> { it.value }.thenBy { it.key }).take(6)
    val shows = perShow.mapNotNull { (id, v) -> lib.find(id)?.let { Triple(it, v.first, v.second) } }.sortedByDescending { it.third }.take(5)

    Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0f to colorFor(name).copy(alpha = 0.35f), 0.4f to BG))) {
        LazyColumn(
            contentPadding = PaddingValues(top = EDGE, bottom = EDGE),
            verticalArrangement = Arrangement.spacedBy(28.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            item {
                Row(Modifier.padding(horizontal = EDGE), verticalAlignment = Alignment.CenterVertically) {
                    Avatar(name, 120.dp)
                    Spacer(Modifier.width(28.dp))
                    Column {
                        MonoText("Profile", 12.sp, TEAL)
                        Spacer(Modifier.height(4.dp))
                        Text(name, fontSize = 40.sp, fontWeight = FontWeight.Black, color = TEXT)
                        Spacer(Modifier.height(4.dp))
                        MonoText("${if (signedIn) "Signed in to" else "Connected to"} ${c.name}  ·  ${c.base}", 11.sp, MUTED)
                        Spacer(Modifier.height(14.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                            if (signedIn) {
                                TvButton("Sign out", model::signOut, Modifier.focusRequester(first))
                                TvButton("Change server", model::changeServer)
                            } else {
                                TvButton("Change server", model::changeServer, Modifier.focusRequester(first))
                            }
                        }
                    }
                }
            }
            item {
                Row(Modifier.padding(horizontal = EDGE), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    for ((v, label) in listOf(
                        time(movieSecs + epSecs) to "Time watched",
                        "$moviesDone" to "Movies finished",
                        "$epsDone" to "Episodes finished",
                        "$showsDone" to "Shows completed",
                        "$inProgress" to "In progress",
                        "$onList" to "On My list",
                    )) {
                        Stat(v, label, Modifier.weight(1f))
                    }
                }
            }
            item(key = "recent") {
                Column {
                    RowTitle("Recently watched", recent.size.coerceAtMost(15))
                    if (recent.isEmpty()) {
                        Text("Nothing watched yet.", color = MUTED, fontSize = 15.sp, modifier = Modifier.padding(horizontal = EDGE))
                    }
                    LazyRow(
                        contentPadding = PaddingValues(horizontal = EDGE),
                        horizontalArrangement = Arrangement.spacedBy(16.dp),
                    ) {
                        items(recent.take(15), key = { it.episodeId ?: it.item.id }) { w ->
                            val state = if (w.e.watched) "Watched" else "%.0f%%".format(w.e.fraction * 100)
                            ArtCard(
                                art = w.art, title = w.item.title, sub = "${w.sub}  ·  $state  ·  ${ago(w.e.updated)}",
                                width = 240.dp, ratio = 16f / 9f, progress = if (w.e.watched) 1f else w.e.fraction,
                                onClick = { model.push(Screen.Details(w.item.id)) },
                            )
                        }
                    }
                }
            }
            item(key = "stats") {
                Row(Modifier.padding(horizontal = EDGE), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                    if (genres.isNotEmpty()) {
                        Panel(Modifier.weight(1f)) {
                            MonoText("Favorite genres", 12.sp, TEAL)
                            val max = genres.first().value.coerceAtLeast(1.0)
                            for ((g, s) in genres) Bar(g, fmtRuntime(s), (s / max).toFloat())
                        }
                    }
                    Panel(Modifier.weight(1f)) {
                        MonoText("Watching", 12.sp, TEAL)
                        InfoRows(
                            listOf(
                                "Movies" to time(movieSecs),
                                "TV shows" to time(epSecs),
                                "Started" to "${entries.size} movies & episodes",
                            ) + shows.map { (s, done, t) -> s.title to "$done of ${s.episodes.size} episodes · ${fmtRuntime(t)}" },
                            labelWidth = 170.dp,
                        )
                        Text("Counted from your watch history on this server.", color = MUTED, fontSize = 12.sp)
                    }
                }
            }
            if (signedIn) item(key = "devices") { DevicesSection(model) }
        }
    }
}

/** One big number with a label under it (people::stat). */
@Composable
private fun Stat(value: String, label: String, modifier: Modifier) {
    Column(
        modifier.clip(RoundedCornerShape(10.dp)).background(SURFACE).padding(18.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(value, color = TEXT, fontSize = 26.sp, fontWeight = FontWeight.Bold, maxLines = 1)
        Text(label, color = MUTED, fontSize = 13.sp, lineHeight = 16.sp, minLines = 2, maxLines = 2)
    }
}

/** A labelled bar, for "Favorite genres" (people::bar). */
@Composable
private fun Bar(label: String, value: String, f: Float) {
    Column {
        Row {
            Text(label, color = TEXT, fontSize = 14.sp, modifier = Modifier.weight(1f))
            MonoText(value, 11.sp, MUTED)
        }
        Spacer(Modifier.height(4.dp))
        Box(Modifier.fillMaxWidth().height(5.dp).clip(RoundedCornerShape(2.dp)).background(TEXT.copy(alpha = 0.08f))) {
            Box(Modifier.fillMaxHeight().fillMaxWidth(f.coerceIn(0f, 1f)).background(GOLD))
        }
    }
}

/** Where this user is signed in, and signing any of them out (devices.rs). */
@Composable
private fun DevicesSection(model: AppModel) {
    Column(Modifier.padding(horizontal = EDGE), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        MonoText("Your devices", 13.sp)
        when (val d = model.devices) {
            Devices.Loading -> Text("Loading…", color = MUTED, fontSize = 14.sp)
            Devices.Unsupported -> Text("This server doesn't list the devices you're signed in on.", color = MUTED, fontSize = 14.sp)
            is Devices.Failed -> Text(d.message, color = ERROR, fontSize = 14.sp)
            is Devices.Loaded -> {
                if (d.list.isEmpty()) Text("No devices.", color = MUTED, fontSize = 14.sp)
                for (s in d.list) {
                    Row(
                        Modifier.width(760.dp).clip(RoundedCornerShape(6.dp)).background(SURFACE).padding(horizontal = 18.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column(Modifier.weight(1f)) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text(s.device ?: "Unknown device", color = TEXT, fontSize = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                if (s.current) {
                                    Spacer(Modifier.width(10.dp))
                                    MonoText("This TV", 10.sp, TEAL)
                                }
                            }
                            val meta = listOfNotNull(
                                s.ip,
                                s.lastUsed?.let { "Last used ${ago(it)}" },
                                s.created?.let { "Signed in ${ago(it)}" },
                            )
                            if (meta.isNotEmpty()) MonoText(meta.joinToString("  ·  "), 10.sp, MUTED)
                        }
                        TvButton("Sign out", { model.signOutDevice(s) })
                    }
                }
            }
        }
    }
}
