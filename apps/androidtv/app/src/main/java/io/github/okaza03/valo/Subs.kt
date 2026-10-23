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

/** A parsed subtitle file: the cues on screen at any time. */
@OptIn(UnstableApi::class)
class SubTrack(private val subtitle: androidx.media3.extractor.text.Subtitle) {
    /** The cues at `ms` into the video. */
    fun at(ms: Long): List<Cue> = subtitle.getCues(ms * 1000)

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
