// Person pages (a cast member, director or creator, with everything they've
// been in) and the signed-in user's profile: watching stats, devices, and the
// app's own settings.
package io.github.okaza03.valo.ui

import android.app.Activity
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
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Devices
import io.github.okaza03.valo.Filmography
import io.github.okaza03.valo.Films
import io.github.okaza03.valo.Filter
import io.github.okaza03.valo.Library
import io.github.okaza03.valo.Screen
import io.github.okaza03.valo.UpdateState
import io.github.okaza03.valo.Updates

@Composable
fun PersonScreen(model: AppModel, lib: Library, name: String) {
    val p = remember(lib, name) { Person(lib, name) }
    val titles = p.titles
    LaunchedEffect(name) { model.discover.loadFilms(name) }
    // their best-known part: the highest-scored title they acted in
    val known = (p.movies + p.shows).maxByOrNull { it.item.score ?: 0f }
    val costars = top(titles.flatMap { i -> (i.cast.map { it.name } + i.makers).distinct() }.filter { !same(it, name) }, 8)

    Box(Modifier.fillMaxSize().background(BG)) {
        LazyColumn(
            contentPadding = PaddingValues(top = 56.dp, bottom = PAD),
            verticalArrangement = Arrangement.spacedBy(22.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            item(key = "top") {
                Column(Modifier.padding(horizontal = PAD), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.size(96.dp).clip(CircleShape)) {
                            val photo = p.photo ?: model.discover.personPhoto(name)
                            if (photo != null) Art(photo, Modifier.fillMaxSize()) else Avatar(name, 96.dp)
                        }
                        Spacer(Modifier.width(16.dp))
                        Column {
                            MonoText(p.roles.ifEmpty { listOf("Person") }.joinToString("  ·  "), 11.sp, TEAL)
                            Text(name, fontSize = 26.sp, fontWeight = FontWeight.Black, color = TEXT, maxLines = 2, overflow = TextOverflow.Ellipsis)
                            val n = titles.size
                            Text(
                                listOfNotNull("$n title${if (n == 1) "" else "s"} in your library", span(titles)).joinToString("  ·  "),
                                color = TEXT.copy(alpha = 0.85f), fontSize = 13.sp,
                            )
                        }
                    }
                    if (known != null) {
                        Btn(
                            "Known for ${known.item.title}" + (known.character?.let { " as $it" } ?: ""),
                            { model.push(Screen.Details(known.item.id)) }, Modifier.fillMaxWidth(),
                        )
                    }
                }
            }
            if (titles.isEmpty()) {
                item { MonoText("Nothing in your library with them", 11.sp, MUTED, Modifier.padding(horizontal = PAD)) }
            }
            for ((title, list) in listOf("Movies" to p.movies, "TV shows" to p.shows, "Directed" to p.directed, "Created" to p.created)) {
                if (list.isEmpty()) continue
                item(key = title) {
                    val byId = list.associateBy { it.item.id }
                    PosterRow(title, list.map { it.item }, model, list.size) { i ->
                        val year = i.year?.toString()
                        val c = byId[i.id]?.character
                        when {
                            c != null && year != null -> "as $c · $year"
                            c != null -> "as $c"
                            else -> year
                        }
                    }
                }
            }
            if (costars.isNotEmpty()) {
                item(key = "costars") {
                    Column {
                        RowTitle("Often works with")
                        LazyRow(contentPadding = PaddingValues(horizontal = PAD), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            items(costars, key = { it.first }) { (who, k) ->
                                val photo = titles.firstNotNullOfOrNull { i -> i.cast.find { same(it.name, who) }?.photo }
                                PersonCard(who, photo, "$k title${if (k == 1) "" else "s"}") { model.push(Screen.Person(who)) }
                            }
                        }
                    }
                }
            }
            val films = model.discover.filmography(name)
            item(key = "films") {
                Column(Modifier.padding(horizontal = PAD)) {
                    MonoText("Everything they've been in", 11.sp, TEAL)
                    Text("Filmography", fontSize = 22.sp, fontWeight = FontWeight.Bold, color = TEXT)
                    when (films) {
                        Films.Loading -> MonoText("Looking them up…", 11.sp, MUTED)
                        is Films.Failed -> Text("Couldn't look them up: ${films.message}", color = MUTED, fontSize = 13.sp)
                        is Films.Done -> if (films.f.credits.isEmpty()) Text("Wikidata doesn't list any titles for them.", color = MUTED, fontSize = 13.sp)
                    }
                }
            }
            (films as? Films.Done)?.f?.takeIf { it.credits.isNotEmpty() }?.let { f ->
                item(key = "films/list") { FilmGrid(model, lib, f) }
            }
            if (titles.isNotEmpty()) {
                item(key = "about") {
                    val genres = top(titles.flatMap { it.genres }, 4)
                    val scores = titles.mapNotNull { it.score }
                    val watched = titles.count { model.status(it) == Filter.Watched }
                    Panel {
                        MonoText("About", 11.sp, TEAL)
                        InfoRows(
                            buildList {
                                if (p.roles.isNotEmpty()) add("Roles" to p.roles.joinToString(", "))
                                for ((k, n) in listOf("Movies" to p.movies.size, "TV shows" to p.shows.size, "Directed" to p.directed.size, "Created" to p.created.size)) {
                                    if (n > 0) add(k to n.toString())
                                }
                                span(titles)?.let { add("Years" to it) }
                                if (genres.isNotEmpty()) add("Genres" to genres.joinToString(", ") { it.first })
                                if (scores.isNotEmpty()) add("Average score" to "%.1f".format(scores.average()))
                                add("You've watched" to "$watched of ${titles.size}")
                            },
                        )
                    }
                }
            }
        }
        BackButton { model.back() }
    }
}

/** Which of a person's titles to show. */
private enum class FilmFilter { All, OnServer, NotOnServer }

/** Posters per line in the filmography. */
private const val FILM_COLS = 3

/** Everything a person has been in, and which of it is on the server. */
@Composable
private fun FilmGrid(model: AppModel, lib: Library, f: Filmography) {
    val ids = remember(lib, f) { model.discover.credits(lib, f) }
    var filter by rememberSaveable { mutableStateOf(FilmFilter.All) }
    val on = ids.count { it.second }
    val shown = ids.filter { (_, ours) ->
        when (filter) {
            FilmFilter.All -> true
            FilmFilter.OnServer -> ours
            FilmFilter.NotOnServer -> !ours
        }
    }.mapNotNull { (id, _) -> model.discover.find(lib, id) }
    Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Chips(
            filter,
            listOf(
                Triple(FilmFilter.All, "All", ids.size),
                Triple(FilmFilter.OnServer, "On the server", on),
                Triple(FilmFilter.NotOnServer, "Not on the server", ids.size - on),
            ),
            { filter = it },
        )
        if (shown.isEmpty()) MonoText("Nothing here", 11.sp, MUTED, Modifier.padding(horizontal = PAD))
        for (line in shown.chunked(FILM_COLS)) {
            Row(Modifier.padding(horizontal = PAD), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                for (i in line) {
                    val sub = listOfNotNull(i.year?.toString(), if (i.remote) "not on server" else null).joinToString(" · ")
                    PosterCard(model, i, Modifier.weight(1f), sub)
                }
                repeat(FILM_COLS - line.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

@Composable
fun ProfileScreen(model: AppModel, lib: Library) {
    val c = model.client ?: return
    val signedIn = c.token != null
    val name = model.prefs.user?.takeIf { signedIn } ?: "Guest"
    LaunchedEffect(Unit) { if (signedIn) model.loadDevices() }
    val w = WatchStats(model, lib)
    val ctx = LocalContext.current

    Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0f to colorFor(name).copy(alpha = 0.35f), 0.3f to BG))) {
        LazyColumn(
            contentPadding = PaddingValues(top = PAD, bottom = PAD),
            verticalArrangement = Arrangement.spacedBy(22.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            item {
                Row(Modifier.padding(horizontal = PAD), verticalAlignment = Alignment.CenterVertically) {
                    Avatar(name, 72.dp)
                    Spacer(Modifier.width(16.dp))
                    Column(Modifier.weight(1f)) {
                        MonoText("Profile", 11.sp, TEAL)
                        Text(name, fontSize = 26.sp, fontWeight = FontWeight.Black, color = TEXT, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text("${if (signedIn) "Signed in to" else "Connected to"} ${c.name}", color = MUTED, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                    CastButton()
                }
            }
            item {
                val stats = listOf(
                    watchTime(w.movieSecs + w.epSecs) to "Time watched",
                    "${w.moviesDone}" to "Movies finished",
                    "${w.epsDone}" to "Episodes finished",
                    "${w.showsDone}" to "Shows completed",
                    "${w.inProgress}" to "In progress",
                    "${w.onList}" to "On My list",
                )
                Column(Modifier.padding(horizontal = PAD), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (line in stats.chunked(3)) {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            for ((v, label) in line) Stat(v, label, Modifier.weight(1f))
                        }
                    }
                }
            }
            item(key = "recent") {
                Column {
                    RowTitle("Recently watched", w.recent.size.coerceAtMost(15))
                    if (w.recent.isEmpty()) Text("Nothing watched yet.", color = MUTED, fontSize = 14.sp, modifier = Modifier.padding(horizontal = PAD))
                    LazyRow(contentPadding = PaddingValues(horizontal = PAD), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        items(w.recent.take(15), key = { it.episodeId ?: it.item.id }) { r ->
                            val state = if (r.e.watched) "Watched" else "%.0f%%".format(r.e.fraction * 100)
                            ArtCard(
                                r.art, r.item.title, "${r.sub}  ·  $state  ·  ${ago(r.e.updated)}", 16f / 9f,
                                { model.push(Screen.Details(r.item.id)) }, Modifier.width(220.dp),
                                progress = if (r.e.watched) 1f else r.e.fraction,
                            )
                        }
                    }
                }
            }
            if (w.genres.isNotEmpty()) {
                item(key = "genres") {
                    Panel {
                        MonoText("Favorite genres", 11.sp, TEAL)
                        val max = w.genres.first().value.coerceAtLeast(1.0)
                        for ((g, s) in w.genres) Bar(g, fmtRuntime(s), (s / max).toFloat())
                    }
                }
            }
            item(key = "watching") {
                Panel {
                    MonoText("Watching", 11.sp, TEAL)
                    InfoRows(
                        listOf(
                            "Movies" to watchTime(w.movieSecs),
                            "TV shows" to watchTime(w.epSecs),
                            "Started" to "${w.entries.size} movies & episodes",
                        ) + w.shows.map { (s, done, t) -> s.title to "$done of ${s.episodes.size} episodes · ${fmtRuntime(t)}" },
                    )
                    Text("Counted from your watch history on this server.", color = MUTED, fontSize = 12.sp)
                }
            }
            if (signedIn) item(key = "devices") { DevicesSection(model) }
            item(key = "app") {
                Column(Modifier.padding(horizontal = PAD), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    MonoText("Valo ${Updates.VERSION}", 12.sp)
                    val wide = Modifier.fillMaxWidth()
                    when (val u = model.update) {
                        is UpdateState.Available -> Btn("Update to ${u.release.version}", model::installUpdate, wide, primary = true)
                        is UpdateState.Downloading -> Btn("Downloading ${u.percent}%", {}, wide, enabled = false)
                        UpdateState.Checking -> Btn("Checking…", {}, wide, enabled = false)
                        UpdateState.Idle -> Btn("Check for updates", { model.checkUpdates(quiet = false) }, wide)
                    }
                    Btn(if (model.busy) "Refreshing…" else "Refresh library", model::loadLibrary, wide, enabled = !model.busy)
                    Btn("Change server", model::changeServer, wide)
                    if (signedIn) Btn("Sign out", model::signOut, wide)
                    Btn("Quit", { (ctx as? Activity)?.finish() }, wide)
                }
            }
        }
    }
}

/** One big number with a label under it. */
@Composable
private fun Stat(value: String, label: String, modifier: Modifier) {
    Column(modifier.clip(RoundedCornerShape(10.dp)).background(SURFACE).padding(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(value, color = TEXT, fontSize = 20.sp, fontWeight = FontWeight.Bold, maxLines = 1)
        Text(label, color = MUTED, fontSize = 11.sp, lineHeight = 14.sp, minLines = 2, maxLines = 2)
    }
}

/** A labelled bar, for "Favorite genres". */
@Composable
private fun Bar(label: String, value: String, f: Float) {
    Column {
        Row {
            Text(label, color = TEXT, fontSize = 13.sp, modifier = Modifier.weight(1f))
            MonoText(value, 10.sp, MUTED)
        }
        Spacer(Modifier.height(4.dp))
        Box(Modifier.fillMaxWidth().height(5.dp).clip(RoundedCornerShape(2.dp)).background(TEXT.copy(alpha = 0.08f))) {
            Box(Modifier.fillMaxHeight().fillMaxWidth(f.coerceIn(0f, 1f)).background(GOLD))
        }
    }
}

/** Where this user is signed in, and signing any of them out. */
@Composable
private fun DevicesSection(model: AppModel) {
    Column(Modifier.padding(horizontal = PAD), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        MonoText("Your devices", 12.sp)
        when (val d = model.devices) {
            Devices.Loading -> Text("Loading…", color = MUTED, fontSize = 14.sp)
            Devices.Unsupported -> Text("This server doesn't list the devices you're signed in on.", color = MUTED, fontSize = 14.sp)
            is Devices.Failed -> Text(d.message, color = ERROR, fontSize = 14.sp)
            is Devices.Loaded -> {
                if (d.list.isEmpty()) Text("No devices.", color = MUTED, fontSize = 14.sp)
                for (s in d.list) {
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(6.dp)).background(SURFACE).padding(start = 14.dp, end = 8.dp, top = 8.dp, bottom = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column(Modifier.weight(1f)) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text(s.device ?: "Unknown device", color = TEXT, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                                if (s.current) {
                                    Spacer(Modifier.width(8.dp))
                                    MonoText("This phone", 9.sp, TEAL)
                                }
                            }
                            val meta = listOfNotNull(s.ip, s.lastUsed?.let { "Last used ${ago(it)}" })
                            if (meta.isNotEmpty()) Text(meta.joinToString("  ·  "), color = MUTED, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        Btn("Sign out", { model.signOutDevice(s) })
                    }
                }
            }
        }
    }
}
