// What's on the home page, in the order the server's admin picked.
package io.github.okaza03.valo.ui

import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.Library
import io.github.okaza03.valo.Screen

class HomeCard(
    val key: String,
    val item: Item,
    val title: String,
    val sub: String?,
    val art: String?,
    val progress: Float?,
    val open: Screen,
    /** A watched movie, with the "WATCHED" sash. */
    val watched: Boolean = false,
    /** A Continue watching card, which can be taken off the row. */
    val removable: Boolean = false,
)

class HomeRow(val title: String, val wide: Boolean, val cards: List<HomeCard>)

/** The home page's own rows in their usual order (their layout keys). The addon catalogs go between the first four and the rest. */
private val HOME_ROWS = listOf("continue", "mylist", "recent", "top", "movies", "series", "genres")

/** Titles per catalog row. */
private const val CATALOG_ROW = 40

fun homeRows(model: AppModel, lib: Library): List<HomeRow> {
    val out = ArrayList<HomeRow>()
    fun posters(title: String, items: List<Item>) = HomeRow(title, false, items.map { i ->
        HomeCard(
            "$title/${i.id}", i, i.title,
            // an addon's title the server doesn't have
            if (i.remote) listOfNotNull(i.year?.toString(), "not on server").joinToString(" · ") else i.year?.toString(),
            i.poster,
            if (i.kind == Kind.Movie) model.entry(i.id)?.takeIf { it.inProgress }?.fraction else null,
            Screen.Details(i.id),
            watched = model.watched(i),
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
