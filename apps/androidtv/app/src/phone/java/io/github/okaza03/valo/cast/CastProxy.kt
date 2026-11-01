// A small web server on the phone, for the Chromecast. A Chromecast can't send
// our sign-in header, and it won't fetch subtitles without CORS, so it gets
// links to the phone instead: each one is a long random path that stands for
// one file, and the phone asks the server for it with the sign-in. Range
// requests pass straight through, so seeking works and nothing is stored.
package io.github.okaza03.valo.cast

import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.BufferedOutputStream
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.net.ServerSocket
import java.net.Socket
import java.net.SocketException
import java.security.SecureRandom
import java.util.concurrent.ConcurrentHashMap
import kotlin.concurrent.thread

/** What a link stands for. */
private sealed interface Route {
    /** A file on the server; `type` is what to call it when the server doesn't say. */
    class Remote(val url: String, val auth: String?, val type: String?) : Route

    /** Made when it's asked for, once: subtitles the Chromecast may never want. */
    class Made(val type: String, val make: () -> ByteArray) : Route {
        val data by lazy(make)
    }
}

class CastProxy(private val http: OkHttpClient) {
    private val routes = ConcurrentHashMap<String, Route>()
    private val random = SecureRandom()
    @Volatile private var socket: ServerSocket? = null

    val port get() = socket?.localPort ?: 0

    /** Start listening, if we aren't. Links only work once they're added. */
    @Synchronized
    fun start() {
        if (socket != null) return
        val s = ServerSocket(0)
        socket = s
        thread(name = "cast-proxy", isDaemon = true) {
            while (true) {
                val c = try {
                    s.accept()
                } catch (_: SocketException) {
                    return@thread // closed
                }
                thread(name = "cast-proxy-client", isDaemon = true) { c.use(::serve) }
            }
        }
    }

    /** Stop listening and forget every link. */
    @Synchronized
    fun stop() {
        socket?.close()
        socket = null
        routes.clear()
    }

    /** Forget every link, as when a new title starts; what's playing stops working. */
    fun clear() = routes.clear()

    /** A path for this file on the server. */
    fun add(url: String, auth: String?, type: String? = null) = put(Route.Remote(url, auth, type))

    /** A path for something made on demand. */
    fun add(type: String, make: () -> ByteArray) = put(Route.Made(type, make))

    private fun put(r: Route): String {
        val bytes = ByteArray(16).also(random::nextBytes)
        val id = bytes.joinToString("") { "%02x".format(it) }
        routes[id] = r
        return "/$id"
    }

    private fun serve(c: Socket) {
        try {
            c.soTimeout = TIMEOUT_MS
            val input = c.getInputStream()
            val out = BufferedOutputStream(c.getOutputStream(), BUFFER)
            val head = readHead(input) ?: return
            val lines = head.split("\r\n")
            val (method, target) = lines[0].split(' ').takeIf { it.size >= 2 }?.let { it[0] to it[1] } ?: return
            val headers = lines.drop(1).mapNotNull { l ->
                l.indexOf(':').takeIf { it > 0 }?.let { l.substring(0, it).trim().lowercase() to l.substring(it + 1).trim() }
            }.toMap()
            if (method == "OPTIONS") return answer(out, "204 No Content", emptyList(), null)
            val route = routes[target.substringBefore('?').trimStart('/')]
            when {
                method != "GET" && method != "HEAD" -> answer(out, "405 Method Not Allowed", emptyList(), null)
                route == null -> answer(out, "404 Not Found", emptyList(), null)
                route is Route.Made -> {
                    val data = route.data
                    answer(out, "200 OK", listOf("Content-Type" to route.type, "Content-Length" to data.size.toString()), data.takeIf { method == "GET" })
                }
                route is Route.Remote -> relay(route, method, headers["range"], out)
            }
        } catch (_: IOException) {
            // the Chromecast hung up: it seeks by dropping one request and sending another
        }
    }

    private fun relay(r: Route.Remote, method: String, range: String?, out: OutputStream) {
        val req = Request.Builder().url(r.url).method(method, null).apply {
            r.auth?.let { header("Authorization", it) }
            range?.let { header("Range", it) }
        }.build()
        http.newCall(req).execute().use { up ->
            val headers = buildList {
                // ours when we know better: servers often say application/octet-stream
                add("Content-Type" to (r.type ?: up.header("Content-Type") ?: "application/octet-stream"))
                up.header("Content-Length")?.let { add("Content-Length" to it) }
                up.header("Content-Range")?.let { add("Content-Range" to it) }
                add("Accept-Ranges" to (up.header("Accept-Ranges") ?: "bytes"))
            }
            writeHead(out, "${up.code} ${up.message.ifEmpty { "OK" }}", headers)
            if (method == "GET") up.body.byteStream().copyTo(out, BUFFER)
            out.flush()
        }
    }

    private fun answer(out: OutputStream, status: String, headers: List<Pair<String, String>>, body: ByteArray?) {
        writeHead(out, status, headers)
        body?.let(out::write)
        out.flush()
    }

    private fun writeHead(out: OutputStream, status: String, headers: List<Pair<String, String>>) {
        val text = buildString {
            append("HTTP/1.1 ").append(status).append("\r\n")
            for ((k, v) in headers) append(k).append(": ").append(v).append("\r\n")
            // the Chromecast's page lives on another origin, and fetches subtitles from there
            append("Access-Control-Allow-Origin: *\r\n")
            append("Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n")
            append("Access-Control-Allow-Headers: Range, Content-Type\r\n")
            append("Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges\r\n")
            append("Connection: close\r\n\r\n")
        }
        out.write(text.toByteArray(Charsets.ISO_8859_1))
    }

    /** The request up to its blank line; null when it's cut short or too long. */
    private fun readHead(input: InputStream): String? {
        val b = StringBuilder()
        while (b.length < MAX_HEAD) {
            val c = input.read()
            if (c < 0) return null
            b.append(c.toChar())
            if (b.endsWith("\r\n\r\n")) return b.substring(0, b.length - 4)
        }
        return null
    }

    private companion object {
        const val BUFFER = 64 * 1024
        const val MAX_HEAD = 16 * 1024
        /** A paused Chromecast leaves its connection open and idle for a long time. */
        const val TIMEOUT_MS = 30 * 60 * 1000
    }
}
