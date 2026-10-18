// App state: which screen is up, the server session, the library, and
// keeping watch progress in step with the server (`/api/progress`).
package io.github.okaza03.valo

import android.app.Application
import android.widget.Toast
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import java.io.File

sealed interface Screen {
    data class Loading(val message: String) : Screen
    data class Setup(val error: String? = null) : Screen
    data class Login(val error: String? = null) : Screen
    data object Home : Screen
    /** All movies or all series, with filters (the desktop app's Movies and TV Shows tabs). */
    data class Grid(val kind: Kind) : Screen
    data class Details(val itemId: String) : Screen
    /** `episodeId` for series; `fromStart` ignores the resume point. */
    data class Player(val itemId: String, val episodeId: String? = null, val fromStart: Boolean = false) : Screen
    /** A cast member, director or creator, by name. */
    data class Person(val name: String) : Screen
    /** The signed-in user's watching stats and devices. */
    data object Profile : Screen
}

enum class Filter { All, InProgress, Unwatched, Watched }

/** `Director` sorts series by their creator. */
enum class Sort { Title, Year, Added, Director }

/** The profile page's "Your devices". */
sealed interface Devices {
    data object Loading : Devices
    /** The server doesn't list sessions. */
    data object Unsupported : Devices
    data class Failed(val message: String) : Devices
    data class Loaded(val list: List<DeviceSession>) : Devices
}

/** A grid page's choices and place, kept while you look at a title. */
class GridState {
    var filter by mutableStateOf(Filter.All)
    var sort by mutableStateOf(Sort.Title)
    /** Sort direction. */
    var desc by mutableStateOf(false)
    var genre by mutableStateOf<String?>(null)
    var list = LazyGridState()
    var focus: String? = null
}

/** Scrambles a hash so a different seed gives a different order (SplitMix64's finalizer). */
private fun mix(x: Long): Long {
    var z = x + -7046029254386353131L
    z = (z xor (z ushr 30)) * -4658895280553007687L
    z = (z xor (z ushr 27)) * -7723592293110705685L
    return z xor (z ushr 31)
}

/** What to play next in a series, and where in it. */
data class NextUp(val episode: Episode, val season: Season, val resume: Entry?, val started: Boolean)

/** Send our changes at most this often; playback saves every few seconds. */
private const val PUSH_EVERY_MS = 15_000L
/** Look for changes made elsewhere this often (not during playback). */
private const val PULL_EVERY_MS = 60_000L

class AppModel(app: Application) : AndroidViewModel(app) {
    val prefs = Prefs(app)
    private val progress = Progress(File(app.filesDir, "progress.json"))
    private val lists = Lists(File(app.filesDir, "lists.json"))

    private val stack = mutableStateListOf<Screen>(Screen.Loading("Starting…"))
    val screen get() = stack.last()

    var client: Client? by mutableStateOf(null)
        private set
    var library: Library? by mutableStateOf(null)
        private set
    var busy by mutableStateOf(false)
        private set
    /** The wordmark over everything while a signed-in session loads. */
    var splash by mutableStateOf(false)

    /** Bumped whenever progress changes, so screens showing it redraw. */
    var progressVersion by mutableIntStateOf(0)
        private set
    /** The same for My list and Continue watching removals. */
    private var listsVersion by mutableIntStateOf(0)

    /** Newly added titles, by id, waiting to be announced. */
    var arrivals: List<String> by mutableStateOf(emptyList())
        private set

    var devices: Devices by mutableStateOf(Devices.Loading)
        private set

    /** Orders the home rows and "You might also like": random, but the same all session. */
    private val shuffle = kotlin.random.Random.nextLong()

    /** Kept across screens so going back lands where you were. */
    val homeList = LazyListState()
    val homeRows = HashMap<String, LazyListState>()
    var homeFocus: String? = null
    val grids = mapOf(Kind.Movie to GridState(), Kind.Series to GridState())

    /** The top bar's search; results replace the page while it isn't empty. */
    var query by mutableStateOf("")
    val searchList = LazyGridState()
    var searchFocus: String? = null

    var update: UpdateState by mutableStateOf(UpdateState.Idle)
        private set

    /** "What's new" after an update; null when there's nothing to show. */
    var whatsNew: List<io.github.okaza03.valo.ui.Note>? by mutableStateOf(null)
        private set

    // sync state for the current session
    private var syncOff = false
    private var lastPull = 0L
    private val wake = Channel<Unit>(Channel.CONFLATED)

    init {
        // a fresh install has no server yet and nothing to tell; versions before
        // 0.8.2 didn't keep seenVersion, so an existing setup counts as updated
        val seen = prefs.seenVersion ?: if (prefs.server == null) Updates.VERSION else "0.8.1"
        if (Updates.newer(Updates.VERSION, seen)) whatsNew = io.github.okaza03.valo.ui.notesSince(seen)
        else prefs.seenVersion = Updates.VERSION
        val saved = prefs.server
        if (saved == null) replace(Screen.Setup()) else {
            splash = prefs.token != null
            connect(saved, startup = true)
        }
        viewModelScope.launch { syncLoop() }
        checkUpdates(quiet = true)
    }

    // ------------------------------------------------------------ navigation

    fun push(s: Screen) {
        if (s == screen) return
        stack += s
    }

    /** Home, Movies or TV Shows from the top bar. */
    fun openTab(s: Screen) {
        query = ""
        replace(Screen.Home)
        if (s != Screen.Home) {
            (s as? Screen.Grid)?.let { grids[it.kind]?.focus = null }
            push(s)
        }
    }

    fun search(q: String) {
        if (q != query) searchFocus = null
        query = q
    }

    /** Returns false when there's nothing to go back to. */
    fun back(): Boolean {
        // Back first clears a search, like Escape on the desktop
        if (query.isNotEmpty() && (screen == Screen.Home || screen is Screen.Grid)) {
            query = ""
            return true
        }
        if (stack.size <= 1) return false
        stack.removeAt(stack.lastIndex)
        return true
    }

    private fun replace(s: Screen) {
        stack.clear()
        stack += s
    }

    /** Swap the top screen, e.g. the player moving on to the next episode. */
    fun swap(s: Screen) {
        stack[stack.lastIndex] = s
    }

    // --------------------------------------------------------------- session

    fun connect(address: String, startup: Boolean = false) {
        busy = true
        if (startup) replace(Screen.Loading("Connecting to ${normalize(address).substringAfter("://")}…"))
        viewModelScope.launch {
            val res = runCatching { withContext(Dispatchers.IO) { Client.connect(address) } }
            busy = false
            res.onFailure { replace(Screen.Setup(it.message)); return@launch }
            val c = res.getOrThrow()
            if (prefs.server != c.base) {
                prefs.server = c.base
                prefs.token = null
                prefs.user = null
            }
            c.token = prefs.token
            client = c
            resetSync()
            if (c.info.auth && c.token == null) replace(Screen.Login()) else loadLibrary()
        }
    }

    fun login(user: String, password: String) {
        val c = client ?: return
        busy = true
        viewModelScope.launch {
            val res = runCatching { withContext(Dispatchers.IO) { c.login(user, password) } }
            busy = false
            res.onFailure { replace(Screen.Login(it.message)); return@launch }
            val s = res.getOrThrow()
            c.token = s.token
            prefs.token = s.token
            prefs.user = s.name ?: user
            resetSync()
            loadLibrary()
        }
    }

    fun loadLibrary() {
        val c = client ?: return
        if (library == null) {
            splash = true
            replace(Screen.Loading("Loading ${c.name}…"))
        }
        busy = true
        viewModelScope.launch {
            val res = runCatching { withContext(Dispatchers.IO) { c.library() } }
            busy = false
            res.onSuccess {
                val first = library == null
                noteArrivals(it)
                library = it
                if (first) {
                    query = ""
                    replace(Screen.Home)
                }
                pullNow()
            }.onFailure {
                when {
                    it is Unauthorized -> signedOut(it.message)
                    library == null -> replace(Screen.Setup(it.message))
                }
            }
        }
    }

    /** Same rules as the desktop app's Movies/TV Shows filter. */
    fun status(item: Item): Filter = when (item.kind) {
        Kind.Movie -> entry(item.id).let { e ->
            when {
                e?.watched == true -> Filter.Watched
                e?.inProgress == true -> Filter.InProgress
                else -> Filter.Unwatched
            }
        }
        Kind.Series -> {
            val eps = item.episodes.map { entry(it.id) }
            when {
                eps.isNotEmpty() && eps.all { it?.watched == true } -> Filter.Watched
                eps.any { (it?.pos ?: 0.0) > 30 } -> Filter.InProgress
                else -> Filter.Unwatched
            }
        }
    }

    /** `items` in a random order that stays the same all session; `salt` gives each row its own. */
    fun shuffled(items: List<Item>, salt: String): List<Item> =
        items.sortedBy { mix((salt + "\u0000" + it.id).hashCode().toLong() xor shuffle) }

    /** Up to `n` titles sharing a genre with `item`, the same ones each time. */
    fun similar(lib: Library, item: Item, n: Int): List<Item> =
        shuffled(lib.all.filter { it.id != item.id && it.genres.any { g -> g in item.genres } }, item.id).take(n)

    /** Best scored first; empty when too few titles have a score for the row to mean much. */
    fun topRated(lib: Library): List<Item> {
        val rated = lib.all.filter { it.score != null }
        if (rated.size < 5) return emptyList()
        return shuffled(rated, "top").sortedByDescending { it.score }
    }

    // --------------------------------------------------------------- my list

    fun onList(id: String): Boolean {
        listsVersion
        return lists.onList(prefix() + id)
    }

    fun toggleList(id: String) {
        lists.toggle(prefix() + id)
        listsVersion++
    }

    /** My list, newest first. */
    fun myList(lib: Library): List<Item> {
        listsVersion
        return lists.listed(prefix()).mapNotNull(lib::find)
    }

    /** Take a movie or series off Continue watching, until it's watched again. */
    fun hide(id: String) {
        lists.hide(prefix() + id)
        listsVersion++
    }

    // -------------------------------------------------------------- arrivals

    /** A library (re)loaded: queue what's new since last time. The first load of a library only remembers it. */
    private fun noteArrivals(lib: Library) {
        val now = lib.all.map { it.id }.toSet()
        val old = lists.known(prefix())
        if (old == null) {
            lists.setKnown(prefix(), now)
            return
        }
        arrivals = (arrivals + now.filter { it !in old && it !in arrivals }).filter { it in now }
    }

    /** The arrivals were shown: they count as seen, even if the app closes with them up. */
    fun closeArrivals() {
        library?.let { l -> lists.setKnown(prefix(), l.all.map { it.id }.toSet()) }
        arrivals = emptyList()
    }

    // --------------------------------------------------------------- devices

    fun loadDevices() {
        val c = client ?: return
        // keep the list up while it reloads, so the rows don't jump
        if (devices !is Devices.Loaded) devices = Devices.Loading
        viewModelScope.launch {
            val res = runCatching { withContext(Dispatchers.IO) { c.sessions() } }
            devices = res.fold(
                { if (it == null) Devices.Unsupported else Devices.Loaded(it.sortedByDescending { d -> d.current }) },
                {
                    if (it is Unauthorized) signedOut(it.message)
                    Devices.Failed(it.message ?: "Can't load your devices")
                },
            )
        }
    }

    fun signOutDevice(d: DeviceSession) {
        if (d.current) return signOut()
        val c = client ?: return
        viewModelScope.launch {
            val res = runCatching { withContext(Dispatchers.IO) { c.endSession(d.key) } }
            res.onSuccess { toast("${d.device ?: "That device"} is signed out") }
                .onFailure { toast(it.message ?: "Can't sign that device out") }
            loadDevices()
        }
    }

    /** Title, a genre's exact name, or a year, like the desktop app's search. */
    fun searchHits(lib: Library): List<Item> {
        val q = query.trim().lowercase()
        if (q.isEmpty()) return emptyList()
        return lib.all.filter { i ->
            q in i.title.lowercase() || i.genres.any { it.lowercase() == q } || i.year?.toString() == q
        }
    }

    // --------------------------------------------------------------- updates

    fun checkUpdates(quiet: Boolean) {
        if (update != UpdateState.Idle && update !is UpdateState.Available) return
        update = UpdateState.Checking
        viewModelScope.launch {
            val res = runCatching { withContext(Dispatchers.IO) { Updates.latest() } }
            update = res.getOrNull()?.let { UpdateState.Available(it) } ?: UpdateState.Idle
            if (!quiet) {
                val msg = res.fold(
                    { if (it == null) "Valo ${Updates.VERSION} is the latest version" else null },
                    { "Can't check for updates: ${it.message}" },
                )
                msg?.let { toast(it) }
            }
        }
    }

    fun installUpdate() {
        val rel = (update as? UpdateState.Available)?.release ?: return
        update = UpdateState.Downloading(rel, 0)
        val app = getApplication<Application>()
        viewModelScope.launch {
            val res = runCatching {
                withContext(Dispatchers.IO) {
                    Updates.download(rel, File(app.cacheDir, "updates")) { pct ->
                        viewModelScope.launch { (update as? UpdateState.Downloading)?.let { update = it.copy(percent = pct) } }
                    }
                }
            }
            update = UpdateState.Available(rel)
            res.onFailure { toast("Couldn't download the update: ${it.message}") }
            res.onSuccess { apk -> install(apk) }
        }
    }

    /** Downloaded, waiting for the user to allow Valo to install apps. */
    private var waitingApk: File? = null

    private fun install(apk: File) {
        val app = getApplication<Application>()
        if (!Updates.mayInstall(app)) {
            waitingApk = apk
            toast("Allow Valo to install apps, then press Back. The update starts by itself.")
            Updates.askToInstall(app)
            return
        }
        waitingApk = null
        viewModelScope.launch {
            runCatching { withContext(Dispatchers.IO) { Updates.install(app, apk) } }
                .onFailure { toast("Couldn't install the update: ${it.message}") }
        }
    }

    fun closeWhatsNew() {
        whatsNew = null
        prefs.seenVersion = Updates.VERSION
    }

    /** Back in Valo: carry on with an update that was waiting for permission. */
    fun resumed() {
        val apk = waitingApk ?: return
        if (Updates.mayInstall(getApplication())) install(apk) else waitingApk = null
    }

    private fun toast(msg: String) {
        Toast.makeText(getApplication(), msg, Toast.LENGTH_LONG).show()
    }

    private fun signedOut(message: String?) {
        client?.token = null
        prefs.token = null
        library = null
        replace(Screen.Login(message))
    }

    fun signOut() {
        val c = client ?: return
        viewModelScope.launch(Dispatchers.IO) { c.logout() }
        if (c.info.auth) signedOut(null) else changeServer()
    }

    fun changeServer() {
        client?.let { c -> viewModelScope.launch(Dispatchers.IO) { c.logout() } }
        client = null
        library = null
        prefs.token = null
        replace(Screen.Setup())
    }

    // -------------------------------------------------------------- progress

    /**
     * Where one user's progress lives in progress.json: keys are this plus the
     * item id. Each signed-in user of a server gets their own, like on the
     * desktop (config::user_prefix).
     */
    private fun prefix(c: Client? = client): String {
        val user = if (c?.info?.auth == true) prefs.user.orEmpty().lowercase() else ""
        return c?.base.orEmpty() + "|@" + user + "|"
    }

    /** Everything watched or started, newest first (the profile page's history). */
    fun history(): List<Pair<String, Entry>> {
        progressVersion
        return progress.recent(prefix())
    }

    fun entry(id: String): Entry? {
        // read so callers redraw when it changes
        progressVersion
        return progress.get(prefix() + id)
    }

    fun save(id: String, pos: Double, dur: Double) {
        progress.set(prefix() + id, pos, dur)
        progress.flush()
    }

    /** Progress was saved by the player; show it and send it now. */
    fun playerClosed() {
        progressVersion++
        pushNow()
    }

    fun nextUp(series: Item): NextUp? {
        val eps = series.seasons.flatMap { s -> s.episodes.map { s to it } }
        if (eps.isEmpty()) return null
        val seen = eps.withIndex().mapNotNull { (i, se) -> entry(se.second.id)?.let { i to it } }
            .maxByOrNull { it.second.updated }
        fun at(i: Int, resume: Entry?, started: Boolean) = NextUp(eps[i].second, eps[i].first, resume, started)
        return when {
            seen == null -> at(0, null, false)
            seen.second.inProgress -> at(seen.first, seen.second, true)
            seen.first + 1 < eps.size -> at(seen.first + 1, null, true)
            else -> at(0, null, true)
        }
    }

    /** The episode after `episodeId`, across seasons. */
    fun episodeAfter(series: Item, episodeId: String): Episode? {
        val eps = series.episodes
        val i = eps.indexOfFirst { it.id == episodeId }
        return eps.getOrNull(i + 1).takeIf { i >= 0 }
    }

    /** Continue watching: unfinished movies, and the next episode of each series. */
    fun continueWatching(lib: Library): List<Pair<Item, NextUp?>> {
        progressVersion
        listsVersion
        val seenSeries = HashSet<String>()
        val out = ArrayList<Pair<Item, NextUp?>>()
        for ((id, e) in progress.recent(prefix())) {
            if (out.size >= 20) break
            val movie = lib.movies.find { it.id == id }
            if (movie != null) {
                if (e.inProgress && !lists.hides(prefix() + movie.id, e.updated)) out += movie to null
                continue
            }
            val s = lib.seriesOf(id) ?: continue
            if (!seenSeries.add(s.id)) continue
            // newest first, so `e` is the series' latest watch
            if (lists.hides(prefix() + s.id, e.updated)) continue
            nextUp(s)?.let { out += s to it }
        }
        return out
    }

    // ------------------------------------------------------------------ sync

    private fun resetSync() {
        syncOff = false
        lastPull = 0
    }

    fun pullNow() {
        lastPull = 0
        wake.trySend(Unit)
    }

    fun pushNow() {
        wake.trySend(Unit)
    }

    private suspend fun syncLoop() {
        while (true) {
            withTimeoutOrNull(PUSH_EVERY_MS) { wake.receive() }
            val c = client ?: continue
            if (syncOff || library == null) continue
            val prefix = prefix(c)
            withContext(Dispatchers.IO) { push(c, prefix) }
            val playing = screen is Screen.Player
            if (!playing && System.currentTimeMillis() - lastPull >= PULL_EVERY_MS) {
                lastPull = System.currentTimeMillis()
                withContext(Dispatchers.IO) { pull(c, prefix) }
            }
        }
    }

    private fun push(c: Client, prefix: String) {
        for ((id, e) in progress.unsent(prefix)) {
            val saved = try {
                c.saveProgress(id, e.pos, e.dur)
            } catch (err: Exception) {
                // offline or the server hiccuped: try again later
                return
            }
            progress.sent(prefix, id, e, saved?.updated?.takeIf { it > 0 })
        }
        progress.flush()
    }

    private suspend fun pull(c: Client, prefix: String) {
        val list = try {
            c.progress()
        } catch (err: Exception) {
            return
        }
        if (list == null) {
            syncOff = true
            return
        }
        val remote = list.mapNotNull { w ->
            val key = w.key() ?: return@mapNotNull null
            var e = Entry(w.position.coerceAtLeast(0.0), w.duration.coerceAtLeast(0.0), w.updated)
            // a title the server calls completed counts as watched here too
            if (w.completed) {
                val dur = if (e.dur <= 0) maxOf(e.pos, 60.0) else e.dur
                e = e.copy(pos = dur, dur = dur)
            }
            key to e
        }
        if (progress.merge(prefix, remote)) {
            progress.flush()
            withContext(Dispatchers.Main) { progressVersion++ }
        }
    }
}
