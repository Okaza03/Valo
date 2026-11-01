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
import io.github.okaza03.valo.Season
import io.github.okaza03.valo.Updates

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

@Composable
fun Arrivals(model: AppModel, lib: Library) {
    val items = model.arrivals.mapNotNull(lib::find)
    val gone = model.departures.values.sortedBy { it.lowercase() }
    val empty = items.isEmpty() && gone.isEmpty()
    LaunchedEffect(empty) { if (empty) model.closeArrivals() }
    if (empty) return
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
                MonoText(summary(movies, series, seasons, gone.size), 12.sp, MUTED)
                val server = model.client?.name ?: "your server"
                val title = if (items.isEmpty()) "Removed from $server" else "New on $server"
                Text(title, fontSize = 28.sp, fontWeight = FontWeight.Bold, color = TEXT)
            }
            Spacer(Modifier.height(10.dp))
            if (items.isNotEmpty()) LazyRow(
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
                        watched = model.watched(i),
                        onClick = {
                            model.closeArrivals()
                            model.push(Screen.Details(i.id))
                        },
                    )
                }
            }
            if (gone.isNotEmpty()) Column(Modifier.padding(horizontal = 32.dp)) {
                if (items.isNotEmpty()) {
                    Spacer(Modifier.height(10.dp))
                    Text("No longer on the server", fontSize = 17.sp, fontWeight = FontWeight.Bold, color = TEXT)
                    Spacer(Modifier.height(4.dp))
                }
                gone.take(SHOWN).forEach { Text(it, fontSize = 15.sp, color = MUTED) }
                if (gone.size > SHOWN) Text("And ${gone.size - SHOWN} more.", fontSize = 15.sp, color = MUTED)
            }
        }
    }
}
