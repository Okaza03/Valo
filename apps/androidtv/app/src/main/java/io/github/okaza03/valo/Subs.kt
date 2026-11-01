// Subtitle files (the server's and the addons'), drawn by the app rather than
// ExoPlayer so their timing can be moved, like the desktop app's subs.rs.
// Subtitles inside the video file are still ExoPlayer's.
package io.github.okaza03.valo

import androidx.annotation.OptIn
import androidx.media3.common.text.Cue
import androidx.media3.common.util.UnstableApi
import androidx.media3.extractor.text.SubtitleParser
import androidx.media3.extractor.text.ssa.SsaParser
import androidx.media3.extractor.text.subrip.SubripParser
import androidx.media3.extractor.text.webvtt.WebvttParser
import okhttp3.Request
import java.util.zip.GZIPInputStream

/** How long the file's last cue stays up: nothing says when it ends. */
private const val LAST_CUE_US = 5_000_000L

/** One subtitle, in milliseconds. */
data class VttCue(val startMs: Long, val endMs: Long, val text: String)

/** "01:02:03.004" */
private fun vttTime(ms: Long) = "%02d:%02d:%02d.%03d".format(ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)

/** A WebVTT file. A blank line would end a cue, so they're taken out of the text. */
fun vtt(cues: List<VttCue>): String = buildString {
    append("WEBVTT\n\n")
    for (c in cues) {
        append(vttTime(c.startMs)).append(" --> ").append(vttTime(c.endMs)).append('\n')
        append(c.text.replace(Regex("\n\\s*\n"), "\n")).append("\n\n")
    }
}

/** A parsed subtitle file: the cues on screen at any time. */
@OptIn(UnstableApi::class)
class SubTrack(private val subtitle: androidx.media3.extractor.text.Subtitle) {
    /** The cues at `ms` into the video. */
    fun at(ms: Long): List<Cue> = subtitle.getCues(ms * 1000)

    /** The whole file as WebVTT, the only kind a Chromecast shows. */
    fun toVtt(): String {
        val n = subtitle.eventTimeCount
        val cues = (0 until n).mapNotNull { i ->
            val start = subtitle.getEventTime(i)
            val end = if (i + 1 < n) subtitle.getEventTime(i + 1) else start + LAST_CUE_US
            val text = subtitle.getCues(start).mapNotNull { it.text?.toString() }.joinToString("\n").trim()
            if (text.isEmpty()) null else VttCue(start / 1000, end / 1000, text)
        }
        return vtt(cues)
    }

    companion object {
        /** Download and parse `s`; our server's files carry the sign-in. */
        fun load(client: Client?, s: Subtitle): SubTrack {
            val req = Request.Builder().url(s.url).apply { client?.authFor(s.url)?.let { header("Authorization", it) } }.build()
            var data = Client.http.newCall(req).execute().use { r ->
                if (!r.isSuccessful) throw ServerError("Can't load the subtitles: the server answered ${r.code}")
                r.body.bytes()
            }
            // some addons send them gzipped
            if (data.size > 2 && data[0] == 0x1f.toByte() && data[1] == 0x8b.toByte()) data = GZIPInputStream(data.inputStream()).readBytes()
            return SubTrack(parser(s, data).parseToLegacySubtitle(data, 0, data.size))
        }

        @OptIn(UnstableApi::class)
        private fun parser(s: Subtitle, data: ByteArray): SubtitleParser {
            val head = String(data, 0, minOf(data.size, 64), Charsets.UTF_8).trimStart('﻿', ' ', '\r', '\n')
            val format = s.format ?: s.url.substringBefore('?').substringAfterLast('.').lowercase()
            return when {
                head.startsWith("WEBVTT") || format == "vtt" -> WebvttParser()
                head.startsWith("[Script Info]") || format == "ass" || format == "ssa" -> SsaParser()
                else -> SubripParser()
            }
        }
    }
}
