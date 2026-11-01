// "Is there a newer Valo?" — asks GitHub for the latest release, like the
// desktop app (apps/valo/src/update.rs), and installs this app's APK.
package io.github.okaza03.valo

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.widget.Toast
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import okhttp3.Request
import java.io.File
import java.io.IOException
import java.security.MessageDigest

data class Release(val version: String, val apk: String, val apkName: String, val sums: String?)

@Serializable
private data class GhRelease(@SerialName("tag_name") val tag: String = "", val assets: List<GhAsset> = emptyList())

@Serializable
private data class GhAsset(val name: String = "", @SerialName("browser_download_url") val url: String = "")

sealed interface UpdateState {
    data object Idle : UpdateState
    data object Checking : UpdateState
    data class Available(val release: Release) : UpdateState
    /** `percent` of the download. */
    data class Downloading(val release: Release, val percent: Int) : UpdateState
}

object Updates {
    const val VERSION = BuildConfig.VERSION_NAME

    /** "v1.2.3-beta" -> [1, 2, 3] */
    private fun parse(v: String): List<Int> =
        v.trim().trimStart('v', 'V').split('-', '+').first().split('.').map { it.toIntOrNull() ?: 0 }

    fun newer(latest: String, current: String): Boolean {
        val (a, b) = parse(latest) to parse(current)
        for (i in 0 until maxOf(a.size, b.size)) {
            val d = a.getOrElse(i) { 0 } - b.getOrElse(i) { 0 }
            if (d != 0) return d > 0
        }
        return false
    }

    /** The latest release if it's newer than this build and has our APK; null if we're current. */
    fun latest(): Release? {
        val req = Request.Builder().url(BuildConfig.RELEASES_URL)
            .header("Accept", "application/vnd.github+json").build()
        val r = try {
            Client.http.newCall(req).execute().use { res ->
                when {
                    res.code == 404 -> throw IOException("no releases published yet")
                    !res.isSuccessful -> throw IOException("GitHub answered ${res.code}")
                    else -> json.decodeFromString<GhRelease>(res.body.string())
                }
            }
        } catch (e: IOException) {
            throw IOException(e.message?.takeIf { "GitHub" in it || "releases" in it } ?: "can't reach GitHub", e)
        }
        val version = r.tag.trimStart('v', 'V')
        val apk = r.assets.find { it.name.endsWith(BuildConfig.APK_SUFFIX) } ?: return null
        val sums = r.assets.find { it.name == "SHA256SUMS.txt" }?.url
        return if (newer(version, VERSION)) Release(version, apk.url, apk.name, sums) else null
    }

    /**
     * Download the APK into the cache, checked against SHA256SUMS.txt. A
     * finished, checked download of the same version is used again.
     */
    fun download(rel: Release, dir: File, progress: (Int) -> Unit): File {
        val apk = File(dir, "valo-${rel.version}.apk")
        if (apk.isFile && apk.length() > 0) return apk
        val expected = rel.sums?.let { url ->
            val text = Client.http.newCall(Request.Builder().url(url).build()).execute().use { r ->
                if (!r.isSuccessful) throw IOException("can't download SHA256SUMS.txt (${r.code})")
                r.body.string()
            }
            // "<hash>  target/dist/valo-v0.5.0-androidtv.apk"
            text.lines().map { it.trim().split(Regex("\\s+"), 2) }
                .find { it.size == 2 && it[1].trimStart('*').substringAfterLast('/') == rel.apkName }
                ?.get(0)?.lowercase()
                ?: throw IOException("${rel.apkName} isn't listed in SHA256SUMS.txt")
        }
        dir.mkdirs()
        dir.listFiles()?.forEach { it.delete() }
        val part = File(dir, "valo.apk.part")
        val sha = MessageDigest.getInstance("SHA-256")
        Client.http.newCall(Request.Builder().url(rel.apk).build()).execute().use { r ->
            if (!r.isSuccessful) throw IOException("the download failed (${r.code})")
            val body = r.body
            val total = body.contentLength()
            var done = 0L
            var shown = -1
            body.byteStream().use { input ->
                part.outputStream().use { out ->
                    val buf = ByteArray(64 * 1024)
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        out.write(buf, 0, n)
                        sha.update(buf, 0, n)
                        done += n
                        val pct = if (total > 0) (done * 100 / total).toInt() else 0
                        if (pct != shown) progress(pct).also { shown = pct }
                    }
                }
            }
        }
        val got = sha.digest().joinToString("") { "%02x".format(it) }
        if (expected != null && got != expected) {
            part.delete()
            throw IOException("the download doesn't match SHA256SUMS.txt")
        }
        part.renameTo(apk)
        return apk
    }

    /** Valo may install apps (Android 8 asks the user once, in Settings). */
    fun mayInstall(ctx: Context): Boolean =
        Build.VERSION.SDK_INT < 26 || ctx.packageManager.canRequestPackageInstalls()

    /** Open the Settings page where the user allows Valo to install apps. */
    fun askToInstall(ctx: Context) {
        val ask = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${ctx.packageName}"))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        runCatching { ctx.startActivity(ask) }
    }

    /**
     * Hand the APK to Android's package installer as an install session;
     * Android then shows its "update this app?" screen (see [InstallReceiver]).
     * A session is more dependable than the old install intent, which some TVs
     * close again right away.
     */
    fun install(ctx: Context, apk: File) {
        val installer = ctx.packageManager.packageInstaller
        val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL)
        params.setAppPackageName(ctx.packageName)
        params.setSize(apk.length())
        val id = installer.createSession(params)
        installer.openSession(id).use { session ->
            session.openWrite("valo.apk", 0, apk.length()).use { out ->
                apk.inputStream().use { it.copyTo(out) }
                session.fsync(out)
            }
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or
                if (Build.VERSION.SDK_INT >= 31) PendingIntent.FLAG_MUTABLE else 0
            val done = PendingIntent.getBroadcast(ctx, id, Intent(ctx, InstallReceiver::class.java), flags)
            session.commit(done.intentSender)
        }
    }
}

/** Hears back from an install session: shows Android's confirm screen, or why it failed. */
class InstallReceiver : BroadcastReceiver() {
    override fun onReceive(ctx: Context, intent: Intent) {
        when (val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                @Suppress("DEPRECATION")
                val confirm = intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT) ?: return
                runCatching { ctx.startActivity(confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
            }
            PackageInstaller.STATUS_SUCCESS, PackageInstaller.STATUS_FAILURE_ABORTED -> {}
            else -> {
                val why = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE) ?: "error $status"
                Toast.makeText(ctx, "Couldn't install the update: $why", Toast.LENGTH_LONG).show()
            }
        }
    }
}
