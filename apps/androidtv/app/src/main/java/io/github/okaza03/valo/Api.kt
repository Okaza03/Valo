// Talking to the media server: the Valo API from docs/api/.
// Folder-listing servers aren't supported here; the desktop app crawls those.
package io.github.okaza03.valo

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.put
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import java.io.IOException
import java.util.concurrent.TimeUnit

val json = Json {
    ignoreUnknownKeys = true
    // lists sent as null, numbers where we expect nothing, …
    coerceInputValues = true
    explicitNulls = false
    isLenient = true
}

// ------------------------------------------------------------------ model

enum class Kind { Movie, Series }

@Serializable
data class CastMember(val name: String = "", val character: String? = null, val photo: String? = null)

@Serializable
data class FileInfo(
    val quality: String? = null,
    val container: String? = null,
    @SerialName("video_codec") val videoCodec: String? = null,
    @SerialName("audio_codec") val audioCodec: String? = null,
    @SerialName("audio_channels") val audioChannels: Int? = null,
    /** Bits per second. */
    val bitrate: Long? = null,
    /** Bytes. */
    val size: Long? = null,
    val name: String? = null,
    val path: String? = null,
)

/** A critic's quote. */
@Serializable
data class Review(val quote: String = "", val source: String? = null, val score: String? = null)

/** A subtitle file for a movie or episode; picked from the player's subtitle button. */
@Serializable
data class Subtitle(
    val id: String = "",
    /** "en", "nb", …; "und" when unknown. */
    val language: String = "und",
    val label: String? = null,
    /** "srt", "vtt", "ass" or "ssa"; taken from the URL when missing. */
    val format: String? = null,
    val forced: Boolean = false,
    val sdh: Boolean = false,
    val url: String = "",
)

@Serializable
data class Episode(
    /** "episode:<server id>" once the library is loaded. */
    val id: String = "",
    val number: Int = 0,
    val title: String = "",
    val overview: String = "",
    val still: String? = null,
    /** Seconds. */
    val duration: Double? = null,
    val stream: String = "",
    val aired: String? = null,
    val score: Float? = null,
    val file: FileInfo? = null,
    val subtitles: List<Subtitle> = emptyList(),
)

@Serializable
data class Season(val number: Int = 0, val title: String? = null, val episodes: List<Episode> = emptyList()) {
    val label get() = title ?: if (number == 0) "Specials" else "Season $number"
}

@Serializable
data class Item(
    /** "movie:<server id>" or "series:<server id>" once the library is loaded. */
    val id: String = "",
    val title: String = "",
    val year: Int? = null,
    val overview: String = "",
    val poster: String? = null,
    val backdrop: String? = null,
    /** Landscape artwork for wide cards such as Continue watching. */
    val thumb: String? = null,
    val logo: String? = null,
    val genres: List<String> = emptyList(),
    val rating: String? = null,
    val duration: Double? = null,
    val added: Long? = null,
    val stream: String? = null,
    /** Movies only. */
    val subtitles: List<Subtitle> = emptyList(),
    val seasons: List<Season> = emptyList(),
    val director: String? = null,
    val creators: List<String> = emptyList(),
    val cast: List<CastMember> = emptyList(),
    /** "YYYY-MM-DD", or a database timestamp starting with it. */
    val released: String? = null,
    @SerialName("end_year") val endYear: Int? = null,
    val network: String? = null,
    /** Series: "Ended", "Returning Series", … */
    val status: String? = null,
    /** US dollars. */
    val budget: Long? = null,
    /** Box office, US dollars. */
    val revenue: Long? = null,
    val score: Float? = null,
    @SerialName("score_source") val scoreSource: String? = null,
    val file: FileInfo? = null,
    val reviews: List<Review> = emptyList(),
    /** `"Batman"` or `{"name": "Batman", …}`; see [collection]. */
    @SerialName("collection") val collectionRaw: JsonElement? = null,
    /** Its place in the collection, 0 first. */
    @SerialName("collection_index") val collectionIndex: Int? = null,
    @kotlinx.serialization.Transient val kind: Kind = Kind.Movie,
) {
    val episodes get() = seasons.flatMap { it.episodes }

    /** The film series it belongs to, which keeps its titles together A to Z. */
    val collection: String?
        get() = when (val c = collectionRaw) {
            is JsonPrimitive -> c.contentOrNull?.takeIf { c.isString }
            is JsonObject -> (c["name"] as? JsonPrimitive)?.contentOrNull
            else -> null
        }?.trim()?.ifEmpty { null }

    /** The movie's director, else the series' first creator. */
    val maker get() = director ?: creators.firstOrNull()

    /** Everyone who directed or created it ("A, B" directors are split). */
    val makers get() = (director?.split(',').orEmpty() + creators).map { it.trim() }.filter { it.isNotEmpty() }
}

/**
 * Where an item goes A to Z: under its collection's name, in the collection's
 * order (else oldest first), otherwise under its title (api::title_key).
 */
private val titleOrder: Comparator<Item> = compareBy<Item>(
    { (it.collection ?: it.title).lowercase() },
    { if (it.collection != null) it.collectionIndex ?: Int.MAX_VALUE else 0 },
    { if (it.collection != null) it.year ?: Int.MAX_VALUE else 0 },
    { if (it.collection != null) it.title.lowercase() else "" },
)

@Serializable
data class Library(val movies: List<Item> = emptyList(), val series: List<Item> = emptyList()) {
    val all get() = movies + series

    fun find(id: String) = movies.find { it.id == id } ?: series.find { it.id == id }

    /** The series an episode belongs to. */
    fun seriesOf(episodeId: String) = series.find { s -> s.episodes.any { it.id == episodeId } }

    /** An episode with its season and series. */
    fun findEpisode(episodeId: String): Triple<Item, Season, Episode>? {
        for (s in series) for (season in s.seasons) {
            season.episodes.find { it.id == episodeId }?.let { return Triple(s, season, it) }
        }
        return null
    }

    /** Genres ordered by how many titles have them. */
    fun genres(): List<String> = all.flatMap { it.genres }.groupingBy { it }.eachCount()
        .entries.sortedWith(compareByDescending<Map.Entry<String, Int>> { it.value }.thenBy { it.key })
        .map { it.key }

    /** Resolve relative URLs, tag ids by kind, drop what can't be played. */
    fun finish(base: String): Library {
        fun fix(u: String?) = u?.let { join(base, it) }
        fun subs(l: List<Subtitle>) = l.filter { it.url.isNotEmpty() }.map { it.copy(url = join(base, it.url)) }
        fun item(it: Item, kind: Kind): Item {
            val stream = fix(it.stream)
            val raw = it.id.ifEmpty { stream ?: it.title }
            // servers often number movies, series and episodes separately
            val tag = if (kind == Kind.Movie) "movie" else "series"
            return it.copy(
                id = "$tag:$raw", kind = kind, stream = stream, subtitles = subs(it.subtitles),
                poster = fix(it.poster), backdrop = fix(it.backdrop), thumb = fix(it.thumb), logo = fix(it.logo),
                released = it.released?.take(10),
                cast = it.cast.map { c -> c.copy(photo = fix(c.photo)) },
                seasons = it.seasons.sortedBy { s -> s.number }.map { s ->
                    s.copy(episodes = s.episodes.sortedBy { e -> e.number }.map { e ->
                        val es = join(base, e.stream)
                        e.copy(
                            id = "episode:" + e.id.ifEmpty { es }, stream = es, still = fix(e.still),
                            aired = e.aired?.take(10),
                            subtitles = subs(e.subtitles),
                        )
                    })
                },
            )
        }
        return Library(
            movies = movies.map { item(it, Kind.Movie) }.filter { it.stream != null }.sortedWith(titleOrder),
            series = series.map { item(it, Kind.Series) }
                .filter { s -> s.seasons.any { it.episodes.isNotEmpty() } }.sortedWith(titleOrder),
        )
    }
}

/** One title's watch state as `/api/progress` sends and takes it. */
@Serializable
data class Watched(
    @SerialName("type") val kind: String = "",
    /** The library's id for it; a number or a string. */
    val id: JsonElement? = null,
    val position: Double = 0.0,
    val duration: Double = 0.0,
    val completed: Boolean = false,
    val updated: Long = 0,
) {
    /** The id the rest of the app uses ("movie:42"; see [Library.finish]). */
    fun key(): String? {
        val raw = (id as? JsonPrimitive)?.contentOrNull ?: return null
        return if (kind == "movie" || kind == "episode") "$kind:$raw" else null
    }
}

@Serializable
data class ServerInfo(val name: String? = null, val auth: Boolean = false)

@Serializable
data class Session(val token: String, val name: String? = null, val admin: Boolean = false)

/** A device signed in as the current user (`GET /api/sessions`). */
@Serializable
data class DeviceSession(
    /** Not the token: an id the server gives the session. */
    val id: JsonElement? = null,
    /** What the app said it is at sign-in: "Valo TV on SHIELD Android TV". */
    val device: String? = null,
    val ip: String? = null,
    /** Unix seconds. */
    val created: Long? = null,
    @SerialName("last_used") val lastUsed: Long? = null,
    /** The session this request came from: this device. */
    val current: Boolean = false,
) {
    val key get() = (id as? JsonPrimitive)?.contentOrNull ?: id?.toString().orEmpty()
}

// ----------------------------------------------------------------- client

/** The server rejected our token; the user has to sign in again. */
class Unauthorized : IOException("Your session expired. Please sign in again.")

class ServerError(message: String) : IOException(message)

/** "192.168.1.5:8080/" -> "http://192.168.1.5:8080" */
fun normalize(input: String): String {
    val s = input.trim().trimEnd('/')
    return if ("://" in s) s else "http://$s"
}

private fun origin(url: String): String {
    val start = url.indexOf("://").let { if (it < 0) 0 else it + 3 }
    val slash = url.indexOf('/', start)
    return if (slash < 0) url else url.substring(0, slash)
}

/** Resolve a possibly-relative URL against the server base. */
fun join(base: String, rel: String): String = when {
    "://" in rel -> rel
    rel.startsWith("/") -> origin(base) + rel
    else -> base.trimEnd('/') + "/" + rel
}

class Client(val base: String, val info: ServerInfo, @Volatile var token: String? = null) {
    val name get() = info.name ?: base.substringAfter("://")

    /** Requests to our server carry the token; images on a CDN don't. */
    fun authFor(url: String): String? = token?.takeIf { url.startsWith(base) }?.let { "Bearer $it" }

    private fun request(path: String) = Request.Builder().url("$base$path")
        .header("Accept", "application/json")
        .apply { token?.let { header("Authorization", "Bearer $it") } }

    private inline fun <T> call(req: Request, what: String, handle: (Response) -> T): T =
        try {
            http.newCall(req).execute().use(handle)
        } catch (e: IOException) {
            throw if (e is Unauthorized || e is ServerError) e else ServerError("$what: ${e.message}")
        }

    private fun Response.fail(what: String): Nothing =
        if (code == 401 || code == 403) throw Unauthorized()
        else throw ServerError("$what: the server answered $code")

    fun login(username: String, password: String): Session {
        val body = buildJsonObject {
            put("username", username); put("password", password)
            put("device", "Valo TV on ${android.os.Build.MODEL}")
        }
        val req = request("/api/login").post(body.toString().toRequestBody(JSON)).build()
        return call(req, "Sign-in failed") { r ->
            when (r.code) {
                200 -> runCatching { json.decodeFromString<Session>(r.body.string()) }
                    .getOrElse { throw ServerError("This server has no sign-in API (/api/login).") }
                400, 401, 403, 422 -> throw ServerError("Wrong username or password")
                404, 405, 501 -> throw ServerError("This server has no sign-in API (/api/login).")
                else -> r.fail("Sign-in failed")
            }
        }
    }

    /** Best effort; the session is forgotten here either way. */
    fun logout() {
        if (token == null) return
        runCatching { http.newCall(request("/api/logout").post(ByteArray(0).toRequestBody()).build()).execute().close() }
    }

    fun library(): Library = call(request("/api/library").build(), "Can't load the library") { r ->
        if (r.code in listOf(404, 405, 501)) {
            throw ServerError("This server has a Valo API but no library (/api/library).")
        }
        if (!r.isSuccessful) r.fail("Can't load the library")
        runCatching { json.decodeFromString<Library>(r.body.string()) }
            .getOrElse { throw ServerError("The server sent a library this app doesn't understand: ${it.message}") }
            .finish(base)
    }

    /** The signed-in user's watch progress; null when the server doesn't keep it. */
    fun progress(): List<Watched>? = call(request("/api/progress").build(), "Can't load watch progress") { r ->
        when {
            // 401 also covers servers without sign-in, which have no user to keep it for
            r.code in listOf(401, 404, 405, 501) -> null
            !r.isSuccessful -> r.fail("Can't load watch progress")
            else -> json.decodeFromString<List<Watched>>(r.body.string())
        }
    }

    /**
     * Save one entry. Returns what the server stored, or null when it doesn't
     * know the item; throws to try again later.
     */
    fun saveProgress(key: String, position: Double, duration: Double): Watched? {
        val kind = key.substringBefore(':')
        val raw = key.substringAfter(':')
        // ids that are whole numbers go out as numbers
        val id = raw.toULongOrNull()?.takeIf { it.toString() == raw }
        val body = buildJsonObject {
            put("type", kind)
            if (id != null) put("id", id.toLong()) else put("id", raw)
            put("position", position)
            put("duration", duration)
        }
        val req = request("/api/progress").post(body.toString().toRequestBody(JSON)).build()
        return call(req, "Can't save watch progress") { r ->
            when {
                r.code == 404 || r.code == 422 -> null
                !r.isSuccessful -> r.fail("Can't save watch progress")
                else -> runCatching { json.decodeFromString<Watched>(r.body.string()) }.getOrNull()
            }
        }
    }

    /** Where the signed-in user is signed in; null when the server doesn't list sessions. */
    fun sessions(): List<DeviceSession>? = call(request("/api/sessions").build(), "Can't load your devices") { r ->
        when {
            r.code in listOf(404, 405, 501) -> null
            !r.isSuccessful -> r.fail("Can't load your devices")
            else -> runCatching { json.decodeFromString<List<DeviceSession>>(r.body!!.string()) }.getOrNull()
        }
    }

    /** Sign one of the user's devices out (`DELETE /api/sessions/<id>`). */
    fun endSession(id: String) {
        val url = "$base/api/sessions/" + java.net.URLEncoder.encode(id, "UTF-8").replace("+", "%20")
        val req = Request.Builder().url(url).header("Accept", "application/json")
            .apply { token?.let { header("Authorization", "Bearer $it") } }.delete().build()
        call(req, "Can't sign that device out") { r -> if (!r.isSuccessful) r.fail("Can't sign that device out") }
    }

    companion object {
        private val JSON = "application/json".toMediaType()

        val http: OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
            .addInterceptor { chain ->
                chain.proceed(chain.request().newBuilder()
                    .header("User-Agent", "Valo-AndroidTV/${BuildConfig.VERSION_NAME}").build())
            }
            .build()

        /** Find the Valo API at `address`. */
        fun connect(address: String): Client {
            val base = normalize(address)
            if (base.length <= "http://".length) {
                throw ServerError("Enter the server's address, e.g. 192.168.1.20:8080")
            }
            val info = try {
                val req = Request.Builder().url("$base/api/info").header("Accept", "application/json").build()
                http.newCall(req).execute().use { r ->
                    if (!r.isSuccessful) null
                    else runCatching { json.decodeFromString<ServerInfo>(r.body.string()) }.getOrNull()
                }
            } catch (e: IllegalArgumentException) {
                throw ServerError("That isn't a valid address.")
            } catch (e: IOException) {
                throw ServerError("No answer from $base. Check the address and that the server is running.")
            } ?: throw ServerError(
                "$base answered, but it has no Valo API (/api/info). " +
                    "Folder-listing servers only work in the desktop app."
            )
            return Client(base, info)
        }
    }
}
