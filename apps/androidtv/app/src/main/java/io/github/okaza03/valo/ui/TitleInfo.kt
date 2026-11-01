// The facts on title, person and profile pages, for the TV and phone screens.
package io.github.okaza03.valo.ui

import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.Entry
import io.github.okaza03.valo.FileInfo
import io.github.okaza03.valo.Filter
import io.github.okaza03.valo.Item
import io.github.okaza03.valo.Kind
import io.github.okaza03.valo.Library
import io.github.okaza03.valo.Sort
import io.github.okaza03.valo.GridState

/** Rows drawn in gold: about the file. */
val FILE_ROWS = setOf("Quality", "Container", "Audio", "Bitrate")

private fun fileRows(f: FileInfo?): List<Pair<String, String>> = buildList {
    if (f == null) return@buildList
    f.quality?.let { add("Quality" to it) }
    listOfNotNull(f.container, f.videoCodec).joinToString(" · ").takeIf { it.isNotEmpty() }?.let { add("Container" to it) }
    f.audioCodec?.let { a -> add("Audio" to (a + (f.audioChannels?.let { " · $it" } ?: ""))) }
    f.bitrate?.let { add("Bitrate" to bitrate(it)) }
    f.size?.let { add("File size" to bytes(it)) }
    f.name?.let { add("File" to it) }
}

fun movieRows(item: Item): List<Pair<String, String>> = buildList {
    item.director?.let { add("Director" to it) }
    item.released?.let { add("Released" to dateLong(it)) }
    item.duration?.takeIf { it > 0 }?.let { add("Runtime" to fmtRuntime(it)) }
    item.score?.let { add("Score" to ("%.1f".format(it) + (item.scoreSource?.let { s -> " · $s" } ?: ""))) }
    item.collection?.let { add("Collection" to it) }
    item.budget?.let { add("Budget" to money(it)) }
    item.revenue?.let { add("Box office" to money(it)) }
    addAll(fileRows(item.file))
}

fun seriesRows(item: Item): List<Pair<String, String>> = buildList {
    if (item.creators.isNotEmpty()) add("Created by" to item.creators.joinToString(", "))
    item.network?.let { add("Network" to it) }
    item.status?.let { add("Status" to it) }
    item.year?.let { y -> add("Years" to if (item.endYear != null && item.endYear != y) "$y–${item.endYear}" else "$y") }
    item.score?.let { add("Score" to ("%.1f".format(it) + (item.scoreSource?.let { s -> " · $s" } ?: ""))) }
    val eps = item.episodes.size
    add("In library" to "$eps episode${if (eps == 1) "" else "s"}")
}

/** Directors or creators first, then the cast: (name, photo, part). */
fun people(item: Item): List<Triple<String, String?, String?>> {
    val makers = item.makers.distinct().map { n ->
        val role = if (item.kind == Kind.Movie) "Director" else "Creator"
        Triple(n, item.cast.find { it.name == n }?.photo, role)
    }
    val cast = item.cast.take(20).map { Triple(it.name, it.photo, it.character?.takeIf { c -> c.isNotBlank() }) }
    return (makers + cast).distinctBy { it.first to it.third }
}

/** The header's other credits; directors and creators go above them as links. */
fun credits(item: Item): List<Pair<String, String>> = buildList {
    if (item.cast.isNotEmpty()) add("Starring" to item.cast.take(5).joinToString(", ") { it.name })
    item.network?.let { add("Network" to it) }
}

fun same(a: String, b: String) = a.trim().equals(b.trim(), ignoreCase = true)

/** A title someone is credited on, with the character they played. */
class Credit(val item: Item, val character: String?)

/** Everything one person did in the library, newest first. */
class Person(lib: Library, name: String) {
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
fun span(items: List<Item>): String? {
    val lo = items.mapNotNull { it.year }.minOrNull() ?: return null
    val hi = items.mapNotNull { it.endYear ?: it.year }.maxOrNull() ?: lo
    return if (hi > lo) "$lo–$hi" else "$lo"
}

/** The `n` most common of `names`, most common first, ties A to Z. */
fun top(names: List<String>, n: Int): List<Pair<String, Int>> =
    names.groupingBy { it }.eachCount().entries
        .sortedWith(compareByDescending<Map.Entry<String, Int>> { it.value }.thenBy { it.key })
        .take(n).map { it.key to it.value }

/** What each history entry is: a movie, or an episode of a series. */
class Watch(val item: Item, val episodeId: String?, val sub: String, val art: String?, val e: Entry)

/** "2h 5m", or a dash for nothing. */
fun watchTime(s: Double) = if (s > 0) fmtRuntime(s) else "—"

/** The profile page's numbers, counted from the watch history. */
class WatchStats(model: AppModel, lib: Library) {
    val entries = model.history()
    var movieSecs = 0.0
        private set
    var epSecs = 0.0
        private set
    var moviesDone = 0
        private set
    var epsDone = 0
        private set
    var inProgress = 0
        private set
    val recent = ArrayList<Watch>()
    val showsDone: Int
    val onList: Int
    val genres: List<Map.Entry<String, Double>>
    /** (series, episodes finished, seconds) */
    val shows: List<Triple<Item, Int, Double>>

    init {
        fun secs(e: Entry) = if (e.dur > 0) minOf(e.pos, e.dur) else e.pos
        val genreSecs = HashMap<String, Double>()
        val perShow = HashMap<String, Pair<Int, Double>>()
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
                recent += Watch(s, ep.id, "${season.code(ep.number, "S%02dE%02d")} · ${ep.title}", ep.still ?: s.backdrop ?: s.poster, e)
                s
            }
            if (e.inProgress) inProgress++
            for (g in item.genres) genreSecs[g] = (genreSecs[g] ?: 0.0) + secs(e)
        }
        showsDone = lib.series.count { model.status(it) == Filter.Watched }
        onList = model.myList(lib).size
        genres = genreSecs.entries.sortedWith(compareByDescending<Map.Entry<String, Double>> { it.value }.thenBy { it.key }).take(6)
        shows = perShow.mapNotNull { (id, v) -> lib.find(id)?.let { Triple(it, v.first, v.second) } }.sortedByDescending { it.third }.take(5)
    }
}

/** A grid page's titles: the genre picked, with how many are at each status. */
class GridTitles(val inGenre: List<Item>, val items: List<Item>, private val status: List<Filter>) {
    fun count(f: Filter) = status.count { it == f }
}

/** Movies or series, filtered and sorted like on the desktop. */
fun gridTitles(model: AppModel, g: GridState, all: List<Item>): GridTitles {
    val inGenre = all.filter { i -> g.genre.let { it == null || it in i.genres } }
    val status = inGenre.map(model::status)
    val items = inGenre.zip(status).filter { g.filter == Filter.All || it.second == g.filter }.map { it.first }
    // titles without the key go last either way; the library comes A to Z
    fun <K : Comparable<K>> by(key: (Item) -> K?): List<Item> {
        val (with, without) = items.partition { key(it) != null }
        val sorted = if (g.desc) with.sortedByDescending { key(it)!! } else with.sortedBy { key(it)!! }
        return sorted + without
    }
    val sorted = when (g.sort) {
        Sort.Title -> if (g.desc) items.reversed() else items
        Sort.Year -> by { it.year }
        Sort.Added -> by { it.added }
        Sort.Director -> by { it.maker?.lowercase() }
    }
    return GridTitles(inGenre, sorted, status)
}

/** Each sort both ways (the desktop app's direction button): ((sort, descending), label). */
fun sortChoices(all: List<Item>, kind: Kind): List<Pair<Pair<Sort, Boolean>, String>> = buildList {
    val maker = if (kind == Kind.Movie) "Director" else "Creator"
    add((Sort.Title to false) to "A – Z")
    add((Sort.Title to true) to "Z – A")
    add((Sort.Year to true) to "Newest first")
    add((Sort.Year to false) to "Oldest first")
    if (all.any { it.added != null }) {
        add((Sort.Added to true) to "Recently added")
        add((Sort.Added to false) to "Added first")
    }
    if (all.any { it.maker != null }) {
        add((Sort.Director to false) to "$maker A – Z")
        add((Sort.Director to true) to "$maker Z – A")
    }
}
