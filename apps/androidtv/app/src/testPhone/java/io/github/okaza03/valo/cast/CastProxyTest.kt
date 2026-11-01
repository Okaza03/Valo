package io.github.okaza03.valo.cast

import com.sun.net.httpserver.HttpServer
import okhttp3.OkHttpClient
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.net.HttpURLConnection
import java.net.InetSocketAddress
import java.net.Socket
import java.net.URI

class CastProxyTest {
    private val file = ByteArray(300_000) { (it % 251).toByte() }
    private val seenAuth = ArrayList<String?>()
    private lateinit var server: HttpServer
    private val proxy = CastProxy(OkHttpClient())
    private val upstream get() = "http://127.0.0.1:${server.address.port}/video.mkv"

    @Before
    fun up() {
        server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        server.createContext("/video.mkv") { x ->
            seenAuth += x.requestHeaders.getFirst("Authorization")
            val range = x.requestHeaders.getFirst("Range")
            x.responseHeaders.add("Content-Type", "application/octet-stream")
            x.responseHeaders.add("Accept-Ranges", "bytes")
            if (range != null) {
                val (a, b) = range.removePrefix("bytes=").split('-').let { it[0].toInt() to (it[1].toIntOrNull() ?: (file.size - 1)) }
                val part = file.copyOfRange(a, b + 1)
                x.responseHeaders.add("Content-Range", "bytes $a-$b/${file.size}")
                x.sendResponseHeaders(206, part.size.toLong())
                x.responseBody.use { it.write(part) }
            } else {
                x.sendResponseHeaders(200, file.size.toLong())
                x.responseBody.use { it.write(file) }
            }
        }
        server.start()
        proxy.start()
    }

    @After
    fun down() {
        proxy.stop()
        server.stop(0)
    }

    private fun open(path: String, method: String = "GET", range: String? = null) =
        (URI("http://127.0.0.1:${proxy.port}$path").toURL().openConnection() as HttpURLConnection).apply {
            requestMethod = method
            range?.let { setRequestProperty("Range", it) }
        }

    @Test
    fun wholeFileCarriesTheSignInUpstreamOnly() {
        val c = open(proxy.add(upstream, "Bearer secret", "video/mp4"))
        assertEquals(200, c.responseCode)
        assertEquals("video/mp4", c.contentType)
        assertEquals("*", c.getHeaderField("Access-Control-Allow-Origin"))
        assertEquals(null, c.getHeaderField("Authorization"))
        assertArrayEquals(file, c.inputStream.readBytes())
        assertEquals("Bearer secret", seenAuth.single())
    }

    @Test
    fun rangesPassThroughForSeeking() {
        val c = open(proxy.add(upstream, null), range = "bytes=1000-1999")
        assertEquals(206, c.responseCode)
        assertEquals("bytes 1000-1999/${file.size}", c.getHeaderField("Content-Range"))
        assertEquals("1000", c.getHeaderField("Content-Length"))
        assertArrayEquals(file.copyOfRange(1000, 2000), c.inputStream.readBytes())
    }

    @Test
    fun headSendsNoBody() {
        val c = open(proxy.add(upstream, null), "HEAD")
        assertEquals(200, c.responseCode)
        assertEquals("bytes", c.getHeaderField("Accept-Ranges"))
        assertEquals(0, c.inputStream.readBytes().size)
    }

    @Test
    fun onlyKnownLinksAnswer() {
        assertEquals(404, open("/not-a-link").responseCode)
        assertEquals(404, open("/").responseCode)
        val path = proxy.add(upstream, "Bearer secret")
        assertEquals(405, open(path, "DELETE").responseCode)
        proxy.clear()
        assertEquals(404, open(path).responseCode)
        assertTrue(seenAuth.isEmpty())
    }

    @Test
    fun linksCantBeGuessed() {
        val a = proxy.add(upstream, null)
        val b = proxy.add(upstream, null)
        assertEquals(33, a.length) // "/" and 128 random bits
        assertTrue(a != b)
    }

    @Test
    fun madeContentIsBuiltOnceWhenAskedFor() {
        var made = 0
        val path = proxy.add("text/vtt") { made++; "WEBVTT\n".toByteArray() }
        assertEquals(0, made)
        repeat(2) {
            val c = open(path)
            assertEquals("text/vtt", c.contentType)
            assertEquals("WEBVTT\n", c.inputStream.readBytes().decodeToString())
        }
        assertEquals(1, made)
    }

    @Test
    fun corsPreflightAnswers() {
        val c = open("/whatever", "OPTIONS")
        assertEquals(204, c.responseCode)
        assertEquals("*", c.getHeaderField("Access-Control-Allow-Origin"))
    }

    @Test
    fun aClientHangingUpDoesntStopTheProxy() {
        val path = proxy.add(upstream, null)
        Socket("127.0.0.1", proxy.port).use {
            it.getOutputStream().write("GET $path HTTP/1.1\r\n\r\n".toByteArray())
            it.getInputStream().read()
        }
        assertEquals(200, open(path).responseCode)
    }
}
