// "What's new" after an update, and "New on <server>": titles added since the
// library was last seen.
package io.github.okaza03.valo.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.Library
import io.github.okaza03.valo.Screen
import io.github.okaza03.valo.Updates

/** At most this many titles; the rest are counted. */
private const val SHOWN = 12

@Composable
fun WhatsNew(notes: List<Note>, onClose: () -> Unit) {
    AlertDialog(
        onDismissRequest = onClose,
        title = { Text("What's new in ${Updates.VERSION}") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                for (n in notes) {
                    Column {
                        Text(n.title, fontSize = 15.sp, fontWeight = FontWeight.Bold, color = TEXT)
                        Text(n.text, fontSize = 14.sp, color = MUTED)
                    }
                }
            }
        },
        confirmButton = { TextButton(onClose) { Text("Got it") } },
    )
}

@Composable
fun Arrivals(model: AppModel, lib: Library) {
    val items = model.arrivals.mapNotNull(lib::find)
    val gone = model.departures.values.sorted()
    val server = model.client?.name ?: "your server"
    val movies = items.count { it.kind == Kind.Movie }
    val series = items.count { it.kind == Kind.Series && it.id !in model.newSeasons }
    val seasons = model.newSeasons.values.sumOf { it.size }
    AlertDialog(
        onDismissRequest = model::closeArrivals,
        title = { Text(if (items.isEmpty()) "Removed from $server" else "New on $server") },
        text = {
            Column(Modifier.heightIn(max = 420.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(summary(movies, series, seasons, gone.size), fontSize = 13.sp, color = MUTED)
                if (items.isNotEmpty()) {
                    LazyRow(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        items(items.take(SHOWN), key = { it.id }) { i ->
                            // a series: the seasons it came with, or the ones added since
                            val new = model.newSeasons[i.id]
                            val sub = when {
                                i.kind != Kind.Series -> i.year?.toString()
                                new != null -> "New: ${seasonsLabel(new)}"
                                else -> listOfNotNull(
                                    if (i.seasons.isEmpty()) "Series" else seasonsLabel(i.seasons.map { it.number }),
                                    i.year?.toString(),
                                ).joinToString(" · ")
                            }
                            ArtCard(
                                i.poster, i.title, sub, 2f / 3f,
                                {
                                    model.closeArrivals()
                                    model.push(Screen.Details(i.id))
                                },
                                Modifier.width(100.dp), watched = model.watched(i),
                            )
                        }
                    }
                    if (items.size > SHOWN) Text("And ${items.size - SHOWN} more.", fontSize = 13.sp, color = MUTED)
                }
                if (gone.isNotEmpty()) {
                    if (items.isNotEmpty()) Text("No longer on the server", fontSize = 15.sp, fontWeight = FontWeight.Bold, color = TEXT)
                    gone.take(SHOWN).forEach { Text(it, fontSize = 14.sp, color = MUTED) }
                    if (gone.size > SHOWN) Text("And ${gone.size - SHOWN} more.", fontSize = 13.sp, color = MUTED)
                }
            }
        },
        confirmButton = { TextButton(model::closeArrivals) { Text("Got it") } },
    )
}
