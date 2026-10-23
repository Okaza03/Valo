// What the admin's Stremio addons add to browsing (the desktop app's
// browse/discover.rs): catalog rows on the home page and the Streaming page,
// details for titles that aren't on the server (with a Request button),
// details our own titles are missing (filled in, never replaced), search
// beyond the server, and people's filmographies.
package io.github.okaza03.valo

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** Metadata lookups per batch. */
private const val BATCH = 24
/** Search the addons once typing pauses this long. */
private const val TYPING_MS = 400L

/** A catalog, its titles as ids: ours where the IMDb id matches, else the addon's. */
class Shown(val key: String, val title: String, val source: String, val service: Boolean, val kind: Kind, val ids: List<String>)

/** The user's own requests. */
sealed interface Requests {
    data object Loading : Requests
    /** The server doesn't take requests, or they couldn't be loaded. */
    data class Failed(val message: String) : Requests
    data class Loaded(val list: List<MediaRequest>) : Requests
}

/** A person's filmography. */
sealed interface Films {
    data object Loading : Films
    data class Failed(val message: String) : Films
    data class Done(val f: Filmography) : Films
}

/** Our titles, to find an addon's title among them. */
class Ours(lib: Library) {
    private val byImdb = HashMap<Pair<Kind, String>, String>()
    private val byTitle = HashMap<Pair<Kind, String>, MutableList<Pair<Int?, String>>>()

    init {
        for (i in lib.all) {
            Addons.imdbId(i.imdbId)?.let { byImdb[i.kind to it] = i.id }
            byTitle.getOrPut(i.kind to Addons.titleKey(i.title)) { ArrayList() } += i.year to i.id
        }
    }

    /** By IMDb id; else by title and year (a year apart is fine: release dates differ by country). */
    fun find(it: Item): String? {
        Addons.imdbId(it.imdbId)?.let { id -> byImdb[it.kind to id]?.let { found -> return found } }
        val same = byTitle[it.kind to Addons.titleKey(it.title)] ?: return null
        fun gap(y: Int?) = if (y != null && it.year != null) kotlin.math.abs(y - it.year) else null
        // the closest year wins, for remakes with the same name
        return same.filter { (y, _) -> (gap(y) ?: 0) <= 1 }.minByOrNull { (y, _) -> gap(y) ?: 2 }?.second
    }
}

class Discover(private val scope: CoroutineScope, private val model: AppModel) {
    /** The admin's addons, once loaded. */
    var addons: List<Addon> = emptyList()
        private set
    private var rows: List<CatalogRow> = emptyList()
    private var searchable: List<Searchable> = emptyList()
    private var load: Job? = null
    /** The addon list has come in (possibly empty). */
    private var loaded = false

    var shown: List<Shown> by mutableStateOf(emptyList())
        private set
    /** Still loading the catalogs. */
    var loading by mutableStateOf(false)
        private set
    /** Addon titles, by [Addons.remoteId]. */
    val remote = mutableStateMapOf<String, Item>()

    /** From metadata addons, by "movie:tt1375666" (null: none of them has it). */
    private val metas = HashMap<String, Item?>()
    private val want = ArrayList<Pair<String, String>>()
    private val asked = HashSet<String>()
    private var fetching: Job? = null

    var requests: Requests by mutableStateOf(Requests.Loading)
        private set
    private var requestsAsked = false
    var sending by mutableStateOf(false)
        private set
    var sendError: String? by mutableStateOf(null)
        private set

    val films = mutableStateMapOf<String, Films>()
    /** Cast photos from Wikidata, by name (null: it has none). */
    private val photos = HashMap<String, String?>()
    private var photosJob: Job? = null

    /** The last addon search: the query, the titles found (ours left out), and whether more may come. */
    var results: Triple<String, List<String>, Boolean>? by mutableStateOf(null)
        private set
    private var searchJob: Job? = null

    private fun hasMeta() = addons.any { a -> a.resources.any { it.name == "meta" } }

    // ----------------------------------------------------------------- loading

    /** The library (re)loaded: reload the addons and their catalogs. */
    fun reload(c: Client) {
        load?.cancel()
        loading = true
        requestsAsked = false
        requests = Requests.Loading
        load = scope.launch {
            val cats = runCatching { withContext(Dispatchers.IO) { Addons.catalogs(c) } }.getOrNull()
            loading = false
            loaded = true
            addons = cats?.addons.orEmpty()
            rows = cats?.rows.orEmpty()
            searchable = cats?.search.orEmpty()
            results = null
            model.library?.let { lib ->
                queueGaps(lib)
                rebuild(lib)
            }
            fetchMore()
        }
    }

    /** Fill a freshly loaded library from what's known, and ask for the rest of what its cards are missing. */
    fun onLibrary(lib: Library): Library {
        val filled = apply(lib)
        if (loaded) {
            queueGaps(filled)
            rebuild(filled)
            fetchMore()
        }
        return filled
    }

    private fun metaFor(i: Item) = Addons.imdbId(i.imdbId)?.let { metas["${Addons.kindName(i.kind)}:$it"] }

    private fun apply(lib: Library): Library {
        fun fix(l: List<Item>) = l.map { i -> metaFor(i)?.let { Addons.fill(i, it) } ?: i }
        return Library(fix(lib.movies), fix(lib.series))
    }

    /** Titles without a poster, backdrop or overview. */
    private fun queueGaps(lib: Library) {
        if (!hasMeta()) return
        for (i in lib.all) {
            if (i.poster != null && i.backdrop != null && i.overview.isNotEmpty()) continue
            Addons.imdbId(i.imdbId)?.let { wantMeta(Addons.kindName(i.kind), it) }
        }
    }

    /** Ask the metadata addons about this title (once). */
    private fun wantMeta(kind: String, id: String, now: Boolean = false) {
        if (loaded && !hasMeta()) return
        val key = "$kind:$id"
        if (key in metas) return
        if (asked.add(key)) want += kind to id
        // the page on screen needs it: ahead of the rest
        if (now) want.indexOf(kind to id).takeIf { it > 0 }?.let { want.add(0, want.removeAt(it)) }
    }

    /** The full details for the title whose page just opened. */
    fun wantItem(item: Item) {
        sendError = null
        wantPhotos(item)
        val r = Addons.splitRemote(item.id)
        if (r != null) wantMeta(r.first, r.second, now = true)
        else Addons.imdbId(item.imdbId)?.let { wantMeta(Addons.kindName(item.kind), it, now = true) }
        fetchMore()
        if (item.remote && !requestsAsked) loadRequests()
    }

    private fun fetchMore() {
        if (fetching?.isActive == true || !loaded || want.isEmpty()) return
        val batch = want.take(BATCH).also { want.subList(0, it.size).clear() }
        val a = addons
        fetching = scope.launch {
            val found = withContext(Dispatchers.IO) {
                batch.map { (k, id) -> async { "$k:$id" to runCatching { Addons.meta(a, k, id) }.getOrNull() } }.awaitAll()
            }
            for ((key, m) in found) {
                metas[key] = m
                // a remote title's page shows the full details
                if (m != null) remote["addon:$key"]?.let { remote["addon:$key"] = Addons.fill(it, m) }
            }
            model.fillLibrary(::apply)
            fetching = null
            fetchMore()
        }
    }

    /** The rows as ids, matching addon titles to ours. */
    private fun rebuild(lib: Library) {
        val ours = Ours(lib)
        shown = rows.map { r ->
            Shown(r.key, r.title, r.source, r.service, r.kind, r.items.map { keep(ours, it) })
        }
    }

    /** Our id for an addon's title, or its own once it's kept in [remote]. */
    private fun keep(ours: Ours?, it: Item): String {
        ours?.find(it)?.let { return it }
        if (it.id !in remote) {
            val m = Addons.splitRemote(it.id)?.let { (k, id) -> metas["$k:$id"] }
            remote[it.id] = if (m != null) Addons.fill(it, m) else it
        }
        return it.id
    }

    fun find(lib: Library, id: String): Item? = lib.find(id) ?: remote[id]

    // --------------------------------------------------------------- streaming

    /** The catalogs' names ("Netflix", "HBO Max", …) in the admin's order: the streaming services, or every catalog when none is one. */
    fun sources(): List<String> {
        val services = shown.any { it.service }
        return shown.filter { it.service || !services }.map { it.source }.distinct()
    }

    /** "Netflix, Prime Video": the streaming services listing this title. */
    fun streamingOn(id: String): String? =
        shown.filter { s -> s.service && id in s.ids }.map { it.source }.distinct().takeIf { it.isNotEmpty() }?.joinToString(", ")

    // ---------------------------------------------------------------- requests

    private fun loadRequests() {
        val c = model.client ?: return
        if (c.token == null) return
        requestsAsked = true
        scope.launch {
            val r = runCatching { withContext(Dispatchers.IO) { c.myRequests() } }
            requests = r.fold({ Requests.Loaded(it) }, { Requests.Failed(it.message ?: "Can't load your requests") })
        }
    }

    /** The user's request for this title, if they made one. */
    fun requestFor(item: Item): MediaRequest? {
        val list = (requests as? Requests.Loaded)?.list ?: return null
        val imdb = Addons.imdbId(item.imdbId)
        return list.lastOrNull { r ->
            val rid = Addons.imdbId(r.imdbId)
            if (rid != null && imdb != null) rid == imdb
            else r.title.equals(item.title, ignoreCase = true) && (r.year == null || r.year == item.year)
        }
    }

    fun request(item: Item) {
        val c = model.client ?: return
        if (sending) return
        sending = true
        sendError = null
        scope.launch {
            val r = runCatching { withContext(Dispatchers.IO) { c.requestTitle(item) } }
            sending = false
            r.onSuccess { req ->
                (requests as? Requests.Loaded)?.let { requests = Requests.Loaded(it.list + req) }
                loadRequests()
            }.onFailure { sendError = it.message }
        }
    }

    // ------------------------------------------------------------------ search

    /**
     * The search box says `q`: search the addons once typing pauses. Titles
     * the library has are left out; its own results show them.
     */
    fun search(q: String) {
        val t = q.trim()
        if (results?.first == t) return
        searchJob?.cancel()
        if (searchable.isEmpty() || t.length < 2) {
            results = null
            return
        }
        results = Triple(t, emptyList(), true)
        val cats = searchable
        searchJob = scope.launch {
            delay(TYPING_MS)
            val parts = MutableList(cats.size) { emptyList<Item>() }
            var left = cats.size
            // each catalog's titles show as soon as it answers
            cats.forEachIndexed { n, s ->
                launch {
                    parts[n] = runCatching { withContext(Dispatchers.IO) { Addons.search(s, t) } }.getOrDefault(emptyList())
                    left--
                    val ours = model.library?.let(::Ours)
                    val ids = Addons.merge(parts).filter { ours?.find(it) == null }.map { keep(null, it) }
                    results = Triple(t, ids, left > 0)
                }
            }
        }
    }

    // ------------------------------------------------------------------ people

    /** Load a person's filmography, the first time it's asked for. */
    fun loadFilms(name: String) {
        val key = name.lowercase()
        if (key in films) return
        films[key] = Films.Loading
        scope.launch {
            val r = runCatching { withContext(Dispatchers.IO) { Wikidata.lookup(name) } }
            // the titles the server doesn't have open like any addon title
            r.getOrNull()?.credits?.forEach { c -> creditItem(c).let { if (it.id !in remote) remote[it.id] = it } }
            films[key] = r.fold({ Films.Done(it) }, { Films.Failed(it.message ?: "Couldn't look them up") })
        }
    }

    fun filmography(name: String): Films = films[name.lowercase()] ?: Films.Loading

    /** Their photo from Wikidata, once loaded. */
    fun personPhoto(name: String): String? = (films[name.lowercase()] as? Films.Done)?.f?.photo ?: photos[name]

    private fun creditItem(c: Credit) = Item(
        id = Addons.remoteId(Addons.kindName(c.kind), c.imdb), kind = c.kind, title = c.title, year = c.year,
        imdbId = c.imdb,
        poster = "https://images.metahub.space/poster/small/${c.imdb}/img",
        backdrop = "https://images.metahub.space/background/medium/${c.imdb}/img",
    )

    /** A filmography's titles as ids: ours (true) or an addon title to request (false). */
    fun credits(lib: Library, f: Filmography): List<Pair<String, Boolean>> {
        val ours = Ours(lib)
        return f.credits.map { c ->
            val it = creditItem(c)
            ours.find(it)?.let { id -> id to true } ?: (it.id to false)
        }
    }

    /** Photos for a title's cast members who have none. */
    private fun wantPhotos(item: Item) {
        val names = item.cast.take(15).filter { it.photo == null && it.name !in photos }.map { it.name }
        if (names.isEmpty() || photosJob?.isActive == true) return
        photosJob = scope.launch {
            val found = runCatching { withContext(Dispatchers.IO) { Wikidata.photos(names) } }.getOrDefault(emptyMap())
            for (n in names) photos[n] = found[n]
            fun fill(i: Item) = if (i.cast.none { it.photo == null && photos[it.name] != null }) i
            else i.copy(cast = i.cast.map { c -> if (c.photo == null) c.copy(photo = photos[c.name]) else c })
            for ((k, v) in remote.toMap()) fill(v).let { if (it !== v) remote[k] = it }
            model.fillLibrary { lib -> Library(lib.movies.map(::fill), lib.series.map(::fill)) }
        }
    }
}
