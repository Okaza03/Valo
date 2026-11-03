// Draws decoded video into the H.264 encoder through the GPU, which scales it
// down to what a Chromecast can take and brings HDR into normal range. Every
// call has to come from the thread that made it: that's where its GL context is.
package io.github.okaza03.valo.cast

import android.graphics.SurfaceTexture
import android.opengl.EGL14
import android.opengl.EGLConfig
import android.opengl.EGLExt
import android.opengl.GLES11Ext
import android.opengl.GLES20
import android.os.Handler
import android.os.HandlerThread
import android.view.Surface
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.FloatBuffer

/** A frame that doesn't turn up in this long isn't coming. */
private const val FRAME_WAIT_MS = 2_000L
/** Tells EGL the surface is an encoder's. */
private const val EGL_RECORDABLE_ANDROID = 0x3142

/** The largest a Chromecast dongle takes: H.264 level 4.1. */
private const val MAX_W = 1920
private const val MAX_H = 1080

/** `w` by `h` scaled to fit a Chromecast, keeping its shape, in even sizes as encoders want. */
fun fitForCast(w: Int, h: Int): Pair<Int, Int> {
    val scale = minOf(1.0, MAX_W.toDouble() / w, MAX_H.toDouble() / h)
    return ((w * scale).toInt() and 1.inv()) to ((h * scale).toInt() and 1.inv())
}

private const val VERTEX = """
attribute vec2 pos;
attribute vec2 uv;
uniform mat4 m;
varying vec2 v;
void main() {
    gl_Position = vec4(pos, 0.0, 1.0);
    v = (m * vec4(uv, 0.0, 1.0)).xy;
}
"""

private const val PLAIN = """
#extension GL_OES_EGL_image_external : require
precision mediump float;
uniform samplerExternalOES tex;
varying vec2 v;
void main() {
    gl_FragColor = texture2D(tex, v);
}
"""

/**
 * HDR10 as ordinary video: the picture's PQ curve undone to light, wide BT.2020
 * color moved to BT.709, highlights rolled off (Reinhard, with 100 nits as
 * paper white), and the ordinary gamma put on.
 */
private const val FROM_PQ = """
#extension GL_OES_EGL_image_external : require
precision highp float;
uniform samplerExternalOES tex;
varying vec2 v;
vec3 pqToLight(vec3 e) {
    vec3 p = pow(max(e, vec3(0.0)), vec3(1.0 / 78.84375));
    vec3 n = max(p - 0.8359375, vec3(0.0));
    vec3 d = 18.8515625 - 18.6875 * p;
    return pow(n / d, vec3(1.0 / 0.1593017578125)) * 100.0;
}
void main() {
    vec3 l = pqToLight(texture2D(tex, v).rgb);
    l = mat3(1.6605, -0.1246, -0.0182, -0.5876, 1.1329, -0.1006, -0.0728, -0.0083, 1.1187) * l;
    l = max(l, vec3(0.0));
    float y = dot(l, vec3(0.2126, 0.7152, 0.0722));
    float k = (1.0 + y / 100.0) / (1.0 + y);
    gl_FragColor = vec4(pow(l * k, vec3(1.0 / 2.2)), 1.0);
}
"""

class FrameDrawer(target: Surface, private val w: Int, private val h: Int, pq: Boolean) {
    private val display = EGL14.eglGetDisplay(EGL14.EGL_DEFAULT_DISPLAY)
    private val context: android.opengl.EGLContext
    private val surface: android.opengl.EGLSurface
    private val program: Int
    private val texture: Int
    private val texture2: SurfaceTexture
    private val thread = HandlerThread("cast-frames").apply { start() }
    private val lock = Object()
    private var ready = false
    private val matrix = FloatArray(16)
    private val corners = buffer(-1f, -1f, 1f, -1f, -1f, 1f, 1f, 1f)
    private val uvs = buffer(0f, 0f, 1f, 0f, 0f, 1f, 1f, 1f)

    /** Where the decoder draws. */
    val input: Surface

    init {
        val version = IntArray(2)
        check(EGL14.eglInitialize(display, version, 0, version, 1)) { "no EGL" }
        val configs = arrayOfNulls<EGLConfig>(1)
        val count = IntArray(1)
        val want = intArrayOf(
            EGL14.EGL_RED_SIZE, 8, EGL14.EGL_GREEN_SIZE, 8, EGL14.EGL_BLUE_SIZE, 8, EGL14.EGL_ALPHA_SIZE, 8,
            EGL14.EGL_RENDERABLE_TYPE, EGL14.EGL_OPENGL_ES2_BIT, EGL_RECORDABLE_ANDROID, 1, EGL14.EGL_NONE,
        )
        check(EGL14.eglChooseConfig(display, want, 0, configs, 0, 1, count, 0) && count[0] > 0) { "no EGL config" }
        context = EGL14.eglCreateContext(display, configs[0], EGL14.EGL_NO_CONTEXT, intArrayOf(EGL14.EGL_CONTEXT_CLIENT_VERSION, 2, EGL14.EGL_NONE), 0)
        surface = EGL14.eglCreateWindowSurface(display, configs[0], target, intArrayOf(EGL14.EGL_NONE), 0)
        check(EGL14.eglMakeCurrent(display, surface, surface, context)) { "no EGL surface" }

        program = GLES20.glCreateProgram()
        GLES20.glAttachShader(program, shader(GLES20.GL_VERTEX_SHADER, VERTEX))
        GLES20.glAttachShader(program, shader(GLES20.GL_FRAGMENT_SHADER, if (pq) FROM_PQ else PLAIN))
        GLES20.glLinkProgram(program)
        val tex = IntArray(1)
        GLES20.glGenTextures(1, tex, 0)
        texture = tex[0]
        GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, texture)
        for ((k, v) in listOf(
            GLES20.GL_TEXTURE_MIN_FILTER to GLES20.GL_LINEAR, GLES20.GL_TEXTURE_MAG_FILTER to GLES20.GL_LINEAR,
            GLES20.GL_TEXTURE_WRAP_S to GLES20.GL_CLAMP_TO_EDGE, GLES20.GL_TEXTURE_WRAP_T to GLES20.GL_CLAMP_TO_EDGE,
        )) {
            GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, k, v)
        }
        texture2 = SurfaceTexture(texture)
        texture2.setOnFrameAvailableListener({
            synchronized(lock) {
                ready = true
                lock.notifyAll()
            }
        }, Handler(thread.looper))
        input = Surface(texture2)
    }

    private fun buffer(vararg v: Float): FloatBuffer =
        ByteBuffer.allocateDirect(v.size * 4).order(ByteOrder.nativeOrder()).asFloatBuffer().put(v).also { it.position(0) }

    private fun shader(type: Int, source: String): Int {
        val s = GLES20.glCreateShader(type)
        GLES20.glShaderSource(s, source)
        GLES20.glCompileShader(s)
        val ok = IntArray(1)
        GLES20.glGetShaderiv(s, GLES20.GL_COMPILE_STATUS, ok, 0)
        check(ok[0] != 0) { GLES20.glGetShaderInfoLog(s) }
        return s
    }

    /** The decoder just released a frame: draw it for the encoder. False if it never came. */
    fun draw(timeUs: Long): Boolean {
        synchronized(lock) {
            val until = System.currentTimeMillis() + FRAME_WAIT_MS
            while (!ready) {
                val left = until - System.currentTimeMillis()
                if (left <= 0) return false
                lock.wait(left)
            }
            ready = false
        }
        texture2.updateTexImage()
        texture2.getTransformMatrix(matrix)
        GLES20.glViewport(0, 0, w, h)
        GLES20.glUseProgram(program)
        GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, texture)
        GLES20.glUniformMatrix4fv(GLES20.glGetUniformLocation(program, "m"), 1, false, matrix, 0)
        for ((name, data) in listOf("pos" to corners, "uv" to uvs)) {
            val at = GLES20.glGetAttribLocation(program, name)
            GLES20.glEnableVertexAttribArray(at)
            GLES20.glVertexAttribPointer(at, 2, GLES20.GL_FLOAT, false, 0, data)
        }
        GLES20.glDrawArrays(GLES20.GL_TRIANGLE_STRIP, 0, 4)
        EGLExt.eglPresentationTimeANDROID(display, surface, timeUs * 1000)
        EGL14.eglSwapBuffers(display, surface)
        return true
    }

    fun close() {
        runCatching { input.release() }
        runCatching { texture2.release() }
        thread.quitSafely()
        EGL14.eglMakeCurrent(display, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_CONTEXT)
        EGL14.eglDestroySurface(display, surface)
        EGL14.eglDestroyContext(display, context)
        EGL14.eglReleaseThread()
    }
}
