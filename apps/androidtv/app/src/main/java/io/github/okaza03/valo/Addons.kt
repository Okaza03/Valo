// Stremio addons the server's admin added (`GET /api/addons`): rows of titles
// for the home page and the Streaming page (catalogs), details for titles
// (metadata), search beyond the server, and subtitles. Streams are never
// asked for; videos always come from our server. The desktop app's addons.rs.
package io.github.okaza03.valo

import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.intOrNull
import okhttp3.Request
import java.net.URLEncoder
import java.util.concurrent.TimeUnit

/** Titles kept per catalog (the home page shows fewer). */
private const val ROW_MAX = 100
/** Search results per catalog. */
private const val SEARCH_MAX = 30

/** A catalog as a row: "Netflix · Movies". */
class CatalogRow(
    /** How the admin's layout names it: "catalog:<addon id>:movie:top". */
    val key: String,
    val title: String,
    /** "Netflix" */
    val source: String,
    /** From an addon that only lists titles (Streaming Catalogs): a streaming service's list. */
    val service: Boolean,
    val kind: Kind,
    val items: List<Item>,
)

/** A catalog that takes `search=…`. */
class Searchable(val addon: Addon, val kind: String, val id: String)

class Catalogs(val addons: List<Addon>, val rows: List<CatalogRow>, val search: List<Searchable>)

object Addons {
    private val http = Client.http.newBuilder().callTimeout(15, TimeUnit.SECONDS).build()
    private val quick = Client.http.newBuilder().callTimeout(8, TimeUnit.SECONDS).build()

    /** An IMDb id as "tt0111161", from "tt0111161", "111161" or an IMDb link. */
    fun imdbId(s: String?): String? {
        val t = s?.trim()?.lowercase() ?: return null
        if (t.isNotEmpty() && t.all { it.isDigit() }) return "tt" + t.padStart(7, '0')
        // the first "tt" followed by digits ("https" has a "tt" too)
        var at = t.indexOf("tt")
        while (at >= 0) {
            val digits = t.substring(at + 2).takeWhile { it.isDigit() }
            if (digits.length >= 5) return "tt$digits"
            at = t.indexOf("tt", at + 1)
        }
        return null
    }

    /** "The Backrooms!" and "backrooms" alike. */
    fun titleKey(t: String): String {
        val words = t.split(Regex("[^\\p{L}\\p{N}]+")).filter { it.isNotEmpty() }.map { it.lowercase() }
        val skip = words.size > 1 && words.first() in setOf("the", "a", "an")
        return words.drop(if (skip) 1 else 0).joinToString("")
    }

    fun kindName(k: Kind) = if (k == Kind.Movie) "movie" else "series"

    /** The id an addon title goes by in the app: "addon:movie:tt1375666". */
    fun remoteId(kind: String, id: String) = "addon:$kind:$id"

    /** "movie" and "tt1375666" back from a [remoteId]. */
    fun splitRemote(id: String): Pair<String, String>? =
        id.removePrefix("addon:").takeIf { it != id }?.let { it.substringBefore(':') to it.substringAfter(':') }

    private fun segment(s: String) = URLEncoder.encode(s, "UTF-8").replace("+", "%20")

    private fun get(a: Addon, path: String, client: okhttp3.OkHttpClient = http): JsonElement? {
        val base = a.url.removeSuffix("/manifest.json").trimEnd('/')
        val req = Request.Builder().url("$base/$path").header("Accept", "application/json").build()
        return runCatching {
            client.newCall(req).execute().use { r -> if (r.isSuccessful) json.parseToJsonElement(r.body.string()) else null }
        }.getOrNull()
    }

    private fun call(a: Addon, resource: String, kind: String, id: String) = get(a, "$resource/$kind/${segment(id)}.json")

    private fun metasOf(v: JsonElement?) = ((v as? JsonObject)?.get("metas") as? JsonArray).orEmpty()

    /** The addons, and every catalog of theirs as a row (empty ones left out). */
    suspend fun catalogs(client: Client): Catalogs = coroutineScope {
        val addons = client.addons()
        val rows = addons.flatMap { a -> a.catalogs.filter { it.kind == "movie" || it.kind == "series" }.map { a to it } }
            .map { (a, c) ->
                async(kotlinx.coroutines.Dispatchers.IO) {
                    val items = metasOf(call(a, "catalog", c.kind, c.id)).mapNotNull { toItem(it, c.kind, a.name) }.take(ROW_MAX)
                    // "Prime  Video" -> "Prime Video"
                    val name = c.name.ifBlank { a.name }.split(Regex("\\s+")).filter { it.isNotEmpty() }.joinToString(" ")
                    if (items.isEmpty()) null else CatalogRow(
                        key = "catalog:${a.key}:${c.kind}:${c.id}",
                        title = "$name · ${if (c.kind == "movie") "Movies" else "TV shows"}",
                        source = name,
                        service = a.resources.none { it.name == "meta" },
                        kind = if (c.kind == "movie") Kind.Movie else Kind.Series,
                        items = items,
                    )
                }
            }
        val search = async(kotlinx.coroutines.Dispatchers.IO) { searchable(addons) }
        Catalogs(addons, rows.awaitAll().filterNotNull(), search.await())
    }

    /** One searchable catalog per addon and type, from the addons' manifests (the server only lists the ones that make rows). */
    private suspend fun searchable(addons: List<Addon>): List<Searchable> = coroutineScope {
        addons.map { a ->
            async(kotlinx.coroutines.Dispatchers.IO) {
                val cats = ((get(a, "manifest.json") as? JsonObject)?.get("catalogs") as? JsonArray).orEmpty()
                val out = ArrayList<Searchable>()
                for (c in cats.filterIsInstance<JsonObject>()) {
                    val kind = c.str("type") ?: continue
                    val id = c.str("id") ?: continue
                    if (kind != "movie" && kind != "series" || out.any { it.kind == kind }) continue
                    val extra = (c["extra"] as? JsonArray).orEmpty().filterIsInstance<JsonObject>()
                    val supported = c.strings("extraSupported")
                    val required = extra.filter { (it["isRequired"] as? JsonPrimitive)?.booleanOrNull == true }
                        .mapNotNull { it.str("name") } + c.strings("extraRequired")
                    val searches = extra.any { it.str("name") == "search" } || "search" in supported
                    if (searches && required.all { it == "search" }) out += Searchable(a, kind, id)
                }
                out
            }
        }.awaitAll().flatten()
    }

    /** One catalog's titles for a search (empty when it doesn't answer in time). */
    fun search(s: Searchable, q: String): List<Item> =
        metasOf(get(s.addon, "catalog/${s.kind}/${s.id}/search=${segment(q.trim())}.json", quick))
            .mapNotNull { toItem(it, s.kind, s.addon.name) }.take(SEARCH_MAX)

    /** Results from several catalogs as one list: they take turns, so the best movie and the best series both come first; no repeats. */
    fun merge(parts: List<List<Item>>): List<Item> {
        val seen = HashSet<Pair<Kind, String>>()
        val out = ArrayList<Item>()
        for (n in 0 until (parts.maxOfOrNull { it.size } ?: 0)) {
            for (it in parts.mapNotNull { p -> p.getOrNull(n) }) {
                if (seen.add(it.kind to (it.imdbId ?: it.id))) out += it
            }
        }
        return out
    }

    /**
     * Full details from the metadata addons that have this title, the first
     * one's winning. Addons that only know TMDB ids are asked too, once
     * another addon has told us the TMDB id; their cast comes with photos.
     */
    fun meta(addons: List<Addon>, kind: String, id: String): Item? {
        fun ask(a: Addon, id: String) = ((call(a, "meta", kind, id) as? JsonObject)?.get("meta"))?.let { toItem(it, kind, a.name) }
        var found: Item? = null
        for (a in addons.filter { it.serves("meta", kind, id) }) ask(a, id)?.let { found = mergeMeta(found, it) }
        val tmdb = found?.tmdbId?.let { (it as? JsonPrimitive)?.contentOrNull ?: it.toString() }
        if (!tmdb.isNullOrEmpty() && !id.startsWith("tmdb:")) {
            val tid = "tmdb:$tmdb"
            for (a in addons.filter { it.serves("meta", kind, tid) && !it.serves("meta", kind, id) }) {
                ask(a, tid)?.let { found = mergeMeta(found, it) }
            }
        }
        return found
    }

    private fun photos(c: List<CastMember>) = c.count { it.photo != null } to c.size

    private fun better(a: List<CastMember>, b: List<CastMember>): Boolean {
        val (pa, na) = photos(a)
        val (pb, nb) = photos(b)
        return pa > pb || pa == pb && na > nb
    }

    /** Add another addon's details: gaps filled, and a cast with photos (or a longer one) beats a short list of names. */
    private fun mergeMeta(found: Item?, m: Item): Item {
        if (found == null) return m
        val f = if (better(m.cast, found.cast)) found.copy(cast = m.cast) else found
        return fill(f, m)
    }

    /** Fill what `dst` is missing from `src`; anything `dst` has stays. */
    fun fill(dst: Item, src: Item): Item {
        val remote = dst.remote
        var d = dst.copy(
            overview = dst.overview.ifBlank { src.overview },
            genres = dst.genres.ifEmpty { src.genres },
            cast = when {
                dst.cast.isEmpty() -> src.cast
                // an addon's own title: the full details beat the catalog's short ones
                remote && better(src.cast, dst.cast) -> src.cast
                else -> dst.cast
            },
            creators = dst.creators.ifEmpty { src.creators },
            year = dst.year ?: src.year,
            poster = dst.poster ?: src.poster,
            backdrop = dst.backdrop ?: src.backdrop,
            logo = dst.logo ?: src.logo,
            duration = dst.duration ?: src.duration,
            director = dst.director ?: src.director,
            released = dst.released ?: src.released,
            endYear = dst.endYear ?: src.endYear,
            imdbId = dst.imdbId ?: src.imdbId,
            tmdbId = dst.tmdbId ?: src.tmdbId,
            score = dst.score ?: src.score,
            scoreSource = if (dst.score == null) src.scoreSource else dst.scoreSource,
            addon = when {
                dst.addon == null -> src.addon
                src.addon == null -> dst.addon
                else -> dst.addon.copy(
                    writers = dst.addon.writers.ifEmpty { src.addon.writers },
                    country = dst.addon.country ?: src.addon.country,
                    awards = dst.addon.awards ?: src.addon.awards,
                )
            },
        )
        if (remote && d.seasons.isEmpty()) return d.copy(seasons = src.seasons)
        // our episodes' gaps, from the addon's
        if (src.seasons.isNotEmpty()) {
            d = d.copy(seasons = d.seasons.map { season ->
                val from = src.seasons.find { it.number == season.number } ?: return@map season
                season.copy(episodes = season.episodes.map { e ->
                    val f = from.episodes.find { it.number == e.number } ?: return@map e
                    e.copy(
                        title = e.title.ifBlank { f.title }, overview = e.overview.ifBlank { f.overview },
                        still = e.still ?: f.still, aired = e.aired ?: f.aired,
                    )
                })
            })
        }
        return d
    }

    /** A Stremio meta object as one of our items (id: [remoteId]). */
    fun toItem(v: JsonElement, kind: String, source: String): Item? {
        val m = v as? JsonObject ?: return null
        // newer addons put genres, cast and directors in "links"
        fun links(cat: String) = (m["links"] as? JsonArray).orEmpty().filterIsInstance<JsonObject>()
            .filter { it.str("category") == cat }.mapNotNull { it.str("name") }
        fun orLinks(k: String, cat: String) = m.strings(k).ifEmpty { links(cat) }
        val id = m.str("id") ?: return null
        val title = m.str("name") ?: return null
        val released = m.str("released")?.take(10)
        val info = m.str("releaseInfo") ?: (m["year"] as? JsonPrimitive)?.contentOrNull
        val years = info?.split(Regex("[^0-9]+"))?.filter { it.length == 4 }?.mapNotNull { it.toIntOrNull() }.orEmpty()
        val score = m.str("imdbRating")?.toFloatOrNull()
        // the TMDB addon's cast has characters and photos
        val withPhotos = ((m["app_extras"] as? JsonObject)?.get("cast") as? JsonArray).orEmpty()
            .filterIsInstance<JsonObject>().mapNotNull { c ->
                CastMember(c.str("name") ?: return@mapNotNull null, c.str("character"), c.str("photo"))
            }
        val cast = withPhotos.ifEmpty { orLinks("cast", "Cast").map { CastMember(it) } }.take(30)
        val directors = orLinks("director", "Directors")
        val writers = orLinks("writer", "Writers")
        // "148 min", "2h 28min"
        val runtime = m.str("runtime")?.let { rt ->
            var secs = 0.0
            Regex("(\\d+)\\s*([a-z]?)").findAll(rt.lowercase()).forEach { r ->
                val n = r.groupValues[1].toDouble()
                secs += if (r.groupValues[2] == "h") n * 3600 else n * 60
            }
            secs.takeIf { it > 0 }
        }
        // a series' episodes
        val seasons = (m["videos"] as? JsonArray).orEmpty().filterIsInstance<JsonObject>().mapNotNull { e ->
            val se = (e["season"] as? JsonPrimitive)?.intOrNull ?: return@mapNotNull null
            val ep = (e["episode"] as? JsonPrimitive)?.intOrNull ?: (e["number"] as? JsonPrimitive)?.intOrNull ?: return@mapNotNull null
            se to Episode(
                number = ep, title = e.str("name") ?: e.str("title").orEmpty(),
                overview = e.str("overview") ?: e.str("description").orEmpty(),
                still = e.str("thumbnail"), aired = e.str("released")?.take(10),
            )
        }.groupBy({ it.first }, { it.second }).map { (n, eps) -> Season(n, episodes = eps.sortedBy { it.number }) }.sortedBy { it.number }
        return Item(
            id = remoteId(kind, id),
            kind = if (kind == "series") Kind.Series else Kind.Movie,
            title = title,
            overview = m.str("description").orEmpty(),
            poster = m.str("poster"),
            backdrop = m.str("background"),
            logo = m.str("logo"),
            genres = orLinks("genres", "Genres").ifEmpty { m.strings("genre") },
            imdbId = imdbId(m.str("imdb_id")) ?: if (id.startsWith("tt")) imdbId(id) else null,
            tmdbId = (m["moviedb_id"] ?: m["tmdb_id"])?.takeIf { it !is kotlinx.serialization.json.JsonNull }
                ?: id.removePrefix("tmdb:").takeIf { it != id }?.let { JsonPrimitive(it) },
            released = released,
            year = years.firstOrNull() ?: released?.take(4)?.toIntOrNull(),
            endYear = years.getOrNull(1),
            score = score,
            scoreSource = if (score != null) "IMDb" else null,
            cast = cast,
            director = directors.takeIf { it.isNotEmpty() }?.joinToString(", "),
            creators = if (kind == "series") writers else emptyList(),
            duration = runtime,
            seasons = seasons,
            addon = AddonInfo(writers = writers, country = m.str("country"), awards = m.str("awards"), source = source),
        )
    }

    // ------------------------------------------------------------- subtitles

    /** Subtitles from the addons for a movie ("tt1375666") or an episode ("tt0903747:1:2"). */
    fun subtitles(addons: List<Addon>, kind: String, id: String): List<Subtitle> =
        addons.filter { it.serves("subtitles", kind, id) }.flatMap { a ->
            ((call(a, "subtitles", kind, id) as? JsonObject)?.get("subtitles") as? JsonArray).orEmpty()
                .filterIsInstance<JsonObject>().mapNotNull { s ->
                    val url = s.str("url") ?: return@mapNotNull null
                    val lang = langCode(s.str("lang").orEmpty())
                    Subtitle(
                        id = "addon:" + (s.str("id") ?: url), language = lang,
                        label = "${langName(lang)} · ${a.name}", url = url,
                    )
                }
        }

    /** "eng", "English" and "en" are all "en". */
    fun langCode(lang: String): String {
        val l = lang.trim().lowercase()
        LANGS.firstOrNull { (two, three, name) -> l == two || l == three || l == name.lowercase() }?.let { return it.first }
        return l.ifEmpty { "und" }
    }

    fun langName(code: String) = LANGS.firstOrNull { it.first == code }?.third ?: code

    private val LANGS = listOf(
        Triple("en", "eng", "English"), Triple("nb", "nob", "Norwegian"), Triple("no", "nor", "Norwegian"),
        Triple("sv", "swe", "Swedish"), Triple("da", "dan", "Danish"), Triple("fi", "fin", "Finnish"),
        Triple("de", "ger", "German"), Triple("fr", "fre", "French"), Triple("es", "spa", "Spanish"),
        Triple("it", "ita", "Italian"), Triple("pt", "por", "Portuguese"), Triple("nl", "dut", "Dutch"),
        Triple("pl", "pol", "Polish"), Triple("ru", "rus", "Russian"), Triple("tr", "tur", "Turkish"),
        Triple("ar", "ara", "Arabic"), Triple("he", "heb", "Hebrew"), Triple("el", "gre", "Greek"),
        Triple("cs", "cze", "Czech"), Triple("hu", "hun", "Hungarian"), Triple("ro", "rum", "Romanian"),
        Triple("ja", "jpn", "Japanese"), Triple("ko", "kor", "Korean"), Triple("zh", "chi", "Chinese"),
        Triple("hi", "hin", "Hindi"), Triple("uk", "ukr", "Ukrainian"), Triple("vi", "vie", "Vietnamese"),
    )
}

private fun JsonObject.str(k: String) = (this[k] as? JsonPrimitive)?.contentOrNull?.trim()?.ifEmpty { null }

private fun JsonObject.strings(k: String) =
    (this[k] as? JsonArray).orEmpty().mapNotNull { (it as? JsonPrimitive)?.contentOrNull }
