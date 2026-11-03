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
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong
import kotlin.concurrent.thread

/** How the video got to the TV. `status`: the server's last answer. */
data class Delivery(val requests: Int = 0, val status: Int? = null, val bytes: Long = 0)

/**
 * Why the TV stopped. It reports every failure the same way, so what really
 * happened comes from what it asked us for, then from its error code.
 */
fun castFailure(code: Int?, d: Delivery, file: String?, channels: Int? = null, stereo: Boolean = false): String {
    val why = when {
        d.requests == 0 && !stereo -> "The TV never asked this phone for the video, so it can't reach it. A firewall, VPN or guest network can block that."
        stereo -> "The TV won't play this file" + (file?.let { " ($it)" } ?: "") + ", even after converting it. It can be watched on this phone."
        d.status !in 200..299 -> "The server wouldn't give out the video" + (d.status?.let { " (it answered $it)" } ?: "") + "."
        code == 103 -> "The connection between the TV and this phone broke off."
        else -> "The TV can't play this file" + (file?.let { " ($it)" } ?: "") + ". Chromecasts play H.264 video " +
            "(4K models HEVC too) with AAC or MP3 sound; other files can only be watched on this phone."
    }
    val mb = "%.1f MB".format(java.util.Locale.US, d.bytes / 1_048_576.0)
    return "$why\n\nTV error ${code ?: "unknown"} · ${d.requests} requests · server ${d.status ?: "none"} · $mb sent"
}

/** What a link stands for. */
private sealed interface Route {
    /** A file on the server; `type` is what to call it when the server doesn't say. */
    class Remote(val url: String, val auth: String?, val type: String?) : Route

    /** Written as it's read, of no known length: a video being converted. */
    class Live(val type: String, val write: (OutputStream) -> Unit) : Route

    /** Made when it's asked for, once: subtitles the Chromecast may never want. */
    class Made(val type: String, val make: () -> ByteArray) : Route {
        val data by lazy(make)
    }
}

class CastProxy(private val http: OkHttpClient) {
    private val routes = ConcurrentHashMap<String, Route>()
    private val random = SecureRandom()
    @Volatile private var socket: ServerSocket? = null
    private val requests = AtomicInteger()
    private val bytes = AtomicLong()
    @Volatile private var status: Int? = null

    /** How the video has gone since the last [clear]. */
    val delivery get() = Delivery(requests.get(), status, bytes.get())

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
    fun clear() {
        routes.clear()
        restartDelivery()
    }

    /** Count from here on, keeping the links. */
    fun restartDelivery() {
        requests.set(0)
        bytes.set(0)
        status = null
    }

    /** A path for this file on the server. */
    fun add(url: String, auth: String?, type: String? = null) = put(Route.Remote(url, auth, type))

    /** A path for a video written as it's read. */
    fun addLive(type: String, write: (OutputStream) -> Unit) = put(Route.Live(type, write))

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
                    // subtitles that won't load mustn't take the video down with them
                    val data = runCatching { route.data }.getOrNull() ?: return answer(out, "500 Internal Server Error", emptyList(), null)
                    answer(out, "200 OK", listOf("Content-Type" to route.type, "Content-Length" to data.size.toString()), data.takeIf { method == "GET" })
                }
                route is Route.Remote -> relay(route, method, headers["range"], out)
                route is Route.Live -> live(route, method, out)
            }
        } catch (_: IOException) {
            // the Chromecast hung up: it seeks by dropping one request and sending another
        }
    }

    private fun live(r: Route.Live, method: String, out: OutputStream) {
        writeHead(out, "200 OK", listOf("Content-Type" to r.type, "Accept-Ranges" to "none"))
        // now, not with the first of the video: converting may take a moment, or fail
        out.flush()
        if (method != "GET") return
        requests.incrementAndGet()
        status = 200
        try {
            r.write(object : java.io.FilterOutputStream(out) {
                override fun write(b: ByteArray, off: Int, len: Int) {
                    out.write(b, off, len)
                    bytes.addAndGet(len.toLong())
                }
            })
            out.flush()
        } catch (e: IOException) {
            throw e // the TV hung up, which is how it stops
        } catch (_: Exception) {
            status = 500 // the phone couldn't decode it: the connection ends and the TV says so
        }
    }

    private fun relay(r: Route.Remote, method: String, range: String?, out: OutputStream) {
        // the file itself, byte for byte: Content-Length and ranges count those bytes
        val req = Request.Builder().url(r.url).method(method, null).header("Accept-Encoding", "identity").apply {
            r.auth?.let { header("Authorization", it) }
            range?.let { header("Range", it) }
        }.build()
        // the video is the file we name a type for; artwork doesn't count
        val video = r.type != null
        if (video) requests.incrementAndGet()
        val res = try {
            http.newCall(req).execute()
        } catch (_: IOException) {
            if (video) status = 502
            return answer(out, "502 Bad Gateway", emptyList(), null)
        }
        res.use { up ->
            if (video) status = up.code
            val headers = buildList {
                // ours when we know better: servers often say application/octet-stream
                add("Content-Type" to (r.type ?: up.header("Content-Type") ?: "application/octet-stream"))
                up.header("Content-Length")?.let { add("Content-Length" to it) }
                up.header("Content-Range")?.let { add("Content-Range" to it) }
                add("Accept-Ranges" to (up.header("Accept-Ranges") ?: "bytes"))
            }
            writeHead(out, "${up.code} ${up.message.ifEmpty { "OK" }}", headers)
            if (method == "GET") {
                val input = up.body.byteStream()
                val buf = ByteArray(BUFFER)
                while (true) {
                    val n = input.read(buf)
                    if (n < 0) break
                    out.write(buf, 0, n)
                    if (video) bytes.addAndGet(n.toLong())
                }
            }
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
