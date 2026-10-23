// "What's new": after an update, what changed (the desktop app's
// apps/valo/src/ui/whats_new.rs). Only the big things go in NOTES. And
// "New on <server>": titles added since the library was last seen
// (browse/arrivals.rs).
package io.github.okaza03.valo.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.Library
import io.github.okaza03.valo.Screen
import io.github.okaza03.valo.Updates

class Note(val title: String, val text: String)

/**
 * Newest last. TV-side changes only; leave small fixes out, and admin
 * features too: they're for the server's admin, who knows about them.
 */
private val NOTES: List<Pair<String, List<Note>>> = listOf(
    "0.8.2" to listOf(
        Note("Updates in one go", "Choosing Update downloads the new version and opens the installer right away."),
        Note("What's new", "After an update, this page shows what changed."),
    ),
    "0.8.3" to listOf(
        Note("Subtitles", "Subtitles in a movie's or series' Subs folder on the server can be picked with the subtitle button, and the language you pick comes on by itself next time."),
    ),
    "0.8.5" to listOf(
        Note("Controls that come back", "After picking subtitles, any button on the remote brings the player's controls back. They show the title, and a Next button for series."),
        Note("My list", "Hold OK on a movie or series, or use + My list on its page, to keep it in the My list row on the home page."),
        Note("Cast and more", "Details pages show the cast and crew (open anyone to see their other titles), You might also like, file details and critics' quotes."),
        Note("Your profile", "Profile in the top-right menu has your watching stats, recently watched, and every device you're signed in on."),
        Note("New arrivals", "Movies and series added to your server since you last looked are shown when you open Valo."),
    ),
    "0.8.10" to listOf(
        Note("Popups you can read", "What's new and New arrivals keep the remote to themselves, and long notes scroll with up and down."),
        Note("Streaming and requests", "If your server's admin has added Stremio addons, Home has rows like Popular and Netflix, and the new Streaming page shows what's on each service. Titles that aren't on the server can be requested."),
        Note("Search beyond the server", "Search also finds movies and series the server doesn't have, so you can open them and ask for them."),
        Note("Everything they've been in", "A person's page lists all their movies and series, and which of them are on the server."),
        Note("Subtitles, your way", "Hold OK while watching to pick subtitles, including ones from the addons, and to move them earlier or later when they're out of sync."),
        Note("Your home page, your way", "Your server's admin can choose which rows show on Home, and in what order."),
        Note("Change your password", "When your server's admin asks you for a new password, Valo asks each time it opens until you change it."),
    ),
)

/** The notes for versions after `seen`, up to this build; a fallback when none were written. */
fun notesSince(seen: String): List<Note> {
    val notes = NOTES.filter { (v, _) -> Updates.newer(v, seen) && !Updates.newer(v, Updates.VERSION) }
        .flatMap { it.second }
    return notes.ifEmpty { listOf(Note("Fixes and small improvements", "Valo ${Updates.VERSION} makes things work better.")) }
}

@Composable
fun WhatsNew(notes: List<Note>, onClose: () -> Unit) {
    val ok = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { ok.requestFocus() } }
    Modal(620.dp, onClose, footer = {
        TvButton("Got it", onClose, Modifier.focusRequester(ok), primary = true)
        Spacer(Modifier.height(10.dp))
        Text("Full release notes: github.com/Okaza03/Valo/releases", fontSize = 13.sp, color = MUTED)
    }) {
        Column(Modifier.padding(horizontal = 32.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            MonoText("Valo ${Updates.VERSION}", 12.sp, MUTED)
            Text("What's new", fontSize = 28.sp, fontWeight = FontWeight.Bold, color = TEXT)
            Spacer(Modifier.height(10.dp))
            for (n in notes) {
                Text(n.title, fontSize = 18.sp, fontWeight = FontWeight.Bold, color = TEXT)
                Text(n.text, fontSize = 15.sp, color = MUTED)
                Spacer(Modifier.height(8.dp))
            }
        }
    }
}

/** At most this many posters; the rest are counted. */
private const val SHOWN = 12

/** "1, 2 and 3" */
private fun andList(parts: List<String>) =
    if (parts.size < 2) parts.joinToString("") else parts.dropLast(1).joinToString(", ") + " and " + parts.last()

/** "2 new movies, 1 new series and 3 new seasons" */
private fun summary(movies: Int, series: Int, seasons: Int) = andList(
    listOfNotNull(
        when (movies) { 0 -> null; 1 -> "1 new movie"; else -> "$movies new movies" },
        when (series) { 0 -> null; 1 -> "1 new series"; else -> "$series new series" },
        when (seasons) { 0 -> null; 1 -> "1 new season"; else -> "$seasons new seasons" },
    ),
)

/** "Season 2", "Seasons 1–3", "Seasons 1, 3 and 5", "Season 1 and Specials" */
private fun seasonsLabel(nums: List<Int>): String {
    val n = nums.filter { it > 0 }.distinct().sorted()
    val numbered = when {
        n.isEmpty() -> null
        n.size == 1 -> "Season ${n[0]}"
        n.last() - n.first() == n.size - 1 -> "Seasons ${n.first()}–${n.last()}"
        else -> "Seasons " + andList(n.map { it.toString() })
    }
    return when {
        numbered == null -> "Specials"
        0 in nums -> "$numbered and Specials"
        else -> numbered
    }
}

@Composable
fun Arrivals(model: AppModel, lib: Library) {
    val items = model.arrivals.mapNotNull(lib::find)
    LaunchedEffect(items.isEmpty()) { if (items.isEmpty()) model.closeArrivals() }
    if (items.isEmpty()) return
    val close = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { close.requestFocus() } }
    Modal(820.dp, model::closeArrivals, footer = {
        if (items.size > SHOWN) {
            Text("And ${items.size - SHOWN} more.", fontSize = 15.sp, color = MUTED)
            Spacer(Modifier.height(8.dp))
        }
        TvButton("Close", model::closeArrivals, Modifier.focusRequester(close), primary = true)
    }) {
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            val movies = items.count { it.kind == Kind.Movie }
            val series = items.count { it.kind == Kind.Series && it.id !in model.newSeasons }
            val seasons = model.newSeasons.values.sumOf { it.size }
            Column(Modifier.padding(horizontal = 32.dp)) {
                MonoText(summary(movies, series, seasons), 12.sp, MUTED)
                Text("New on ${model.client?.name ?: "your server"}", fontSize = 28.sp, fontWeight = FontWeight.Bold, color = TEXT)
            }
            Spacer(Modifier.height(10.dp))
            LazyRow(
                contentPadding = PaddingValues(horizontal = 32.dp),
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                items(items.take(SHOWN), key = { it.id }) { i ->
                    // a series: the seasons it came with, or the ones added since
                    val new = model.newSeasons[i.id]
                    val sub = when {
                        i.kind != Kind.Series -> i.year?.toString()
                        new != null -> "New: ${seasonsLabel(new)}"
                        else -> {
                            val label = if (i.seasons.isEmpty()) "Series" else seasonsLabel(i.seasons.map { it.number })
                            listOfNotNull(label, i.year?.toString()).joinToString(" · ")
                        }
                    }
                    ArtCard(
                        art = i.poster, title = i.title, sub = sub, width = 124.dp, ratio = 2f / 3f,
                        onClick = {
                            model.closeArrivals()
                            model.push(Screen.Details(i.id))
                        },
                    )
                }
            }
        }
    }
}
