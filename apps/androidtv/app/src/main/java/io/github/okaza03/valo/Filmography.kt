// Everything a person has been in, from Wikidata (free, no key): movies and
// series they acted in or directed, with IMDb ids so the app can tell which
// are on the server, and a photo when Wikidata has one (filmography.rs).
package io.github.okaza03.valo

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.Request
import java.util.concurrent.TimeUnit

/** One of their titles. */
class Credit(val imdb: String, val title: String, val year: Int?, val kind: Kind)

/** A photo of them, and their titles newest first. */
class Filmography(val photo: String?, val credits: List<Credit>)

object Wikidata {
    private val http = Client.http.newBuilder().callTimeout(40, TimeUnit.SECONDS).build()

    private fun sparql(query: String): List<JsonObject> {
        val url = "https://query.wikidata.org/sparql".toHttpUrl().newBuilder().addQueryParameter("query", query).build()
        val req = Request.Builder().url(url)
            .header("Accept", "application/sparql-results+json")
            .header("User-Agent", "Valo-AndroidTV/${BuildConfig.VERSION_NAME} (https://github.com/Okaza03/Valo)")
            .build()
        val body = http.newCall(req).execute().use { r ->
            if (!r.isSuccessful) throw ServerError("Wikidata answered ${r.code}")
            r.body.string()
        }
        val v = json.parseToJsonElement(body) as? JsonObject
        return ((v?.get("results") as? JsonObject)?.get("bindings") as? JsonArray).orEmpty().filterIsInstance<JsonObject>()
    }

    private fun JsonObject.value(k: String) = ((this[k] as? JsonObject)?.get("value") as? JsonPrimitive)?.contentOrNull

    /** A Commons file link, as a 400-pixel-wide image. */
    private fun commons(url: String) = "https://commons.wikimedia.org/wiki/Special:FilePath/${url.substringAfterLast('/')}?width=400"

    private fun clean(name: String) = name.trim().replace("\\", "").replace("\"", "")

    /** Look `name` up. Among people with that exact name, the one with the most titles wins (the actor, not a namesake). */
    fun lookup(name: String): Filmography {
        val rows = sparql(
            """SELECT ?p (SAMPLE(?img) AS ?photo) ?imdb (SAMPLE(?t) AS ?title) (MIN(?d) AS ?date) (MAX(?s) AS ?series) WHERE {
  ?p rdfs:label "${clean(name)}"@en; wdt:P31 wd:Q5.
  OPTIONAL { ?p wdt:P18 ?img }
  ?work wdt:P161|wdt:P57 ?p; wdt:P345 ?imdb.
  FILTER(STRSTARTS(?imdb, "tt"))
  FILTER NOT EXISTS { ?work wdt:P31 wd:Q21191270 }
  OPTIONAL { ?work wdt:P577|wdt:P580 ?d }
  OPTIONAL { ?work rdfs:label ?t FILTER(LANG(?t) = "en") }
  BIND(EXISTS { ?work wdt:P31/wdt:P279* wd:Q5398426 } AS ?s)
} GROUP BY ?p ?imdb""",
        )
        val who = rows.mapNotNull { it.value("p") }.groupingBy { it }.eachCount().maxByOrNull { it.value }?.key
            ?: return Filmography(null, emptyList())
        val mine = rows.filter { it.value("p") == who }
        val credits = mine.mapNotNull { b ->
            Credit(
                imdb = b.value("imdb") ?: return@mapNotNull null,
                title = b.value("title") ?: return@mapNotNull null,
                year = b.value("date")?.take(4)?.toIntOrNull(),
                kind = if (b.value("series") == "true") Kind.Series else Kind.Movie,
            )
        }.sortedByDescending { it.year ?: Int.MIN_VALUE }
        return Filmography(mine.firstNotNullOfOrNull { it.value("photo") }?.let(::commons), credits)
    }

    /** Photos for these people, by name (only those Wikidata has one for); for a shared name, the best-known one's. */
    fun photos(names: List<String>): Map<String, String> {
        if (names.isEmpty()) return emptyMap()
        val values = names.joinToString(" ") { "\"${clean(it)}\"@en" }
        val rows = sparql(
            """SELECT ?name ?img ?links WHERE {
  VALUES ?name { $values }
  ?p rdfs:label ?name; wdt:P31 wd:Q5; wdt:P18 ?img; wikibase:sitelinks ?links.
}""",
        )
        val best = HashMap<String, Pair<Int, String>>()
        for (b in rows) {
            val n = b.value("name") ?: continue
            val img = b.value("img") ?: continue
            val links = b.value("links")?.toIntOrNull() ?: 0
            if ((best[n]?.first ?: -1) < links) best[n] = links to commons(img)
        }
        return best.mapValues { it.value.second }
    }
}
