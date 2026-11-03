// Making a file playable on a Chromecast as it plays, like VLC does. The sound
// becomes stereo, and the video is copied as it is unless the TV can't decode
// it, when the phone's hardware turns it into H.264. It all happens as the TV
// reads, so nothing is stored and the phone does no more than it has to. The
// result is a fragmented MP4, which can be written from start to end without
// going back. That also means the TV can't seek in it: seeking starts a new one
// (see Caster).
package io.github.okaza03.valo.cast

import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaCodecList
import android.media.MediaDataSource
import android.media.MediaExtractor
import android.media.MediaFormat
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.MediaFormatUtil
import androidx.media3.common.util.UnstableApi
import androidx.media3.extractor.AacUtil
import androidx.media3.muxer.BufferInfo
import androidx.media3.muxer.FragmentedMp4Muxer
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.IOException
import java.io.OutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.roundToInt

/** Fetched from the server this much at a time. */
private const val CHUNK = 1 shl 20
/** Chunks kept: the extractor hops between the video and the sound. */
private const val KEEP = 8
/** Chunks of the file's index kept; a film's is a few MB. */
private const val MAX_INDEX = 32
private const val STEREO_BITRATE = 192_000
/** Quality of video made H.264: about 6 Mbps at 1080p. */
private const val BITS_PER_PIXEL = 3
private const val MIN_VIDEO_BITRATE = 1_500_000
/** The most H.264 level 4.1 takes: 1920x1088. */
private const val MAX_LEVEL41_PIXELS = 1920 * 1088
/** Video frames looked at to find how they're reordered: far more than a GOP's B-frames. */
private const val LOOK_AHEAD = 64
/**
 * What the decoder and encoder add to the start of the sound between them,
 * in samples. It's left out, or the sound would run late (measured with a
 * flash and a beep at 44.1 and 48 kHz).
 */
private const val CODEC_DELAY = 4096L
/** Keeps the sound from being held back for long; the TV starts on the first fragment. */
private const val FRAGMENT_MS = 1_000L
private const val WAIT_US = 5_000L
/** The sides and centre come in at -3 dB. */
private const val SIDE = 0.707f
/** Takes the sum back down, leaving speech at a level you'd expect; peaks are clipped. */
private const val GAIN = 0.6f

private fun clip(v: Float) = v.roundToInt().coerceIn(-32768, 32767).toShort()

/**
 * Interleaved 16-bit sound as stereo. Android's AAC decoder gives front left,
 * front right, centre, LFE, back left, back right, then the sides of 7.1. The
 * LFE is left out, as usual for stereo.
 */
fun downmix(pcm: ShortArray, channels: Int): ShortArray {
    if (channels == 2) return pcm
    val frames = pcm.size / channels
    val out = ShortArray(frames * 2)
    for (f in 0 until frames) {
        val at = f * channels
        when {
            channels == 1 -> {
                out[f * 2] = pcm[at]
                out[f * 2 + 1] = pcm[at]
            }
            channels < 6 -> {
                out[f * 2] = pcm[at]
                out[f * 2 + 1] = pcm[at + 1]
            }
            else -> {
                val c = pcm[at + 2] * SIDE
                var l = pcm[at] + c + pcm[at + 4] * SIDE
                var r = pcm[at + 1] + c + pcm[at + 5] * SIDE
                if (channels >= 8) {
                    l += pcm[at + 6] * SIDE
                    r += pcm[at + 7] * SIDE
                }
                out[f * 2] = clip(l * GAIN)
                out[f * 2 + 1] = clip(r * GAIN)
            }
        }
    }
    return out
}

/** What H.264 says it is, from its first parameter set: "avc1.640029". */
fun avcCodec(csd: ByteArray): String? {
    for (i in 0 until csd.size - 3) {
        val start = csd[i].toInt() == 0 && csd[i + 1].toInt() == 0 && csd[i + 2].toInt() == 1
        if (start && csd[i + 3].toInt() and 0x1f == 7 && i + 6 < csd.size) {
            return "avc1." + (4..6).joinToString("") { "%02X".format(csd[i + it]) }
        }
    }
    return null
}

/**
 * How far an MP4 without an edit list pushes the video back. Frames come in
 * decode order, B-frames shown before frames stored ahead of them, and no frame
 * may be decoded after it's shown, so the clock starts that much late.
 */
fun reorderDelayUs(pts: List<Long>): Long {
    val shown = pts.sorted()
    return pts.indices.maxOfOrNull { shown[it] - pts[it] }?.coerceAtLeast(0) ?: 0
}

/** The file on the server, for MediaExtractor, read in chunks with range requests. */
class ServerFile(private val http: OkHttpClient, private val url: String, private val auth: String?) : MediaDataSource() {
    private var total = -1L
    /** What opening the file reads: its index, which every seek needs again. Kept for good. */
    private val index = HashMap<Long, ByteArray>()
    private var opening = true
    /** What the encoder makes of this file: asking takes a few frames, so it's only done once. */
    @Volatile var h264: Format? = null
    private val chunks = object : LinkedHashMap<Long, ByteArray>(KEEP, 1f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Long, ByteArray>) = size > KEEP
    }

    /** The file is open: from here on it's the film itself, which comes and goes. */
    @Synchronized
    fun opened() {
        opening = false
    }

    /** Back to the start of a new stream, which opens the file again. */
    @Synchronized
    fun reopen() {
        opening = true
    }

    private fun chunk(n: Long): ByteArray = index[n] ?: chunks.getOrPut(n) {
        val from = n * CHUNK
        val req = Request.Builder().url(url).header("Range", "bytes=$from-${from + CHUNK - 1}")
            .header("Accept-Encoding", "identity").apply { auth?.let { header("Authorization", it) } }.build()
        http.newCall(req).execute().use { r ->
            if (r.code != 206) throw IOException("the server answered ${r.code}")
            total = r.header("Content-Range")?.substringAfterLast('/')?.toLongOrNull() ?: throw IOException("no file size")
            r.body.bytes()
        }
    }.also { if (opening && index.size < MAX_INDEX) index[n] = it }

    @Synchronized
    override fun getSize(): Long {
        if (total < 0) chunk(0)
        return total
    }

    @Synchronized
    override fun readAt(position: Long, buffer: ByteArray, offset: Int, size: Int): Int {
        if (position >= getSize()) return -1
        val c = chunk(position / CHUNK)
        val at = (position % CHUNK).toInt()
        val n = minOf(size, c.size - at)
        if (n <= 0) return -1
        c.copyInto(buffer, offset, at, at + n)
        return n
    }

    override fun close() {}
}

/** One run of the video through decoder, GPU and H.264 encoder (FrameDrawer). */
@OptIn(UnstableApi::class)
private class H264Video(from: MediaFormat) {
    // by what it has to do: the first HEVC decoder may well not take 4K or 10-bit
    private val decoder = MediaCodec.createByCodecName(
        MediaCodecList(MediaCodecList.REGULAR_CODECS).findDecoderForFormat(from) ?: throw IOException("this phone can't decode this video"),
    )
    private val encoder = MediaCodec.createEncoderByType(MimeTypes.VIDEO_H264)
    private val info = MediaCodec.BufferInfo()
    private val drawer: FrameDrawer
    private var fed = false
    private var done = false
    /** What the encoder makes, once it has said. */
    var format: Format? = null
        private set

    init {
        // no B-frames: they'd delay the clock
        val (w, h) = fitForCast(from.getInteger(MediaFormat.KEY_WIDTH), from.getInteger(MediaFormat.KEY_HEIGHT))
        fun format(limit: Boolean) = MediaFormat.createVideoFormat(MimeTypes.VIDEO_H264, w, h).apply {
            setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface)
            setInteger(MediaFormat.KEY_BIT_RATE, (w * h * BITS_PER_PIXEL).coerceAtLeast(MIN_VIDEO_BITRATE))
            setInteger(MediaFormat.KEY_FRAME_RATE, 30)
            setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 2)
            setInteger(MediaFormat.KEY_MAX_B_FRAMES, 0)
            if (limit) {
                setInteger(MediaFormat.KEY_PROFILE, MediaCodecInfo.CodecProfileLevel.AVCProfileHigh)
                setInteger(MediaFormat.KEY_LEVEL, MediaCodecInfo.CodecProfileLevel.AVCLevel41)
            }
        }
        // left to themselves, phones label it far above what a dongle will accept: 4.1 is 1080p
        val limit = w * h <= MAX_LEVEL41_PIXELS
        try {
            encoder.configure(format(limit), null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
        } catch (e: RuntimeException) {
            if (!limit) throw e
            encoder.reset()
            encoder.configure(format(false), null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
        }
        val pq = from.containsKey(MediaFormat.KEY_COLOR_TRANSFER) && from.getInteger(MediaFormat.KEY_COLOR_TRANSFER) == MediaFormat.COLOR_TRANSFER_ST2084
        drawer = FrameDrawer(encoder.createInputSurface(), w, h, pq)
        encoder.start()
        decoder.configure(from, drawer.input, null, 0)
        decoder.start()
    }

    /** Hand over one compressed frame; false when the decoder has no room yet. */
    fun feed(read: (ByteBuffer) -> Int, timeUs: Long): Boolean {
        val i = decoder.dequeueInputBuffer(WAIT_US)
        if (i < 0) return false
        decoder.queueInputBuffer(i, 0, read(decoder.getInputBuffer(i)!!), timeUs, 0)
        return true
    }

    /** Move along what's ready, handing over what's encoded. */
    fun pump(write: (ByteBuffer, Long, Boolean) -> Unit) {
        while (true) {
            val o = decoder.dequeueOutputBuffer(info, 0)
            if (o == MediaCodec.INFO_TRY_AGAIN_LATER) break
            if (o < 0) continue
            val last = info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0
            val at = info.presentationTimeUs
            val show = info.size > 0
            decoder.releaseOutputBuffer(o, show)
            if (show && drawer.draw(at)) encoded(write) // each frame: a full encoder would hold up the drawing
            if (last) encoder.signalEndOfInputStream()
        }
        encoded(write)
    }

    private fun encoded(write: (ByteBuffer, Long, Boolean) -> Unit) {
        while (true) {
            val o = encoder.dequeueOutputBuffer(info, 0)
            if (o == MediaCodec.INFO_TRY_AGAIN_LATER) break
            if (o == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) format = MediaFormatUtil.createFormatFromMediaFormat(encoder.outputFormat)
            if (o < 0) continue
            if (info.size > 0 && info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG == 0) {
                write(encoder.getOutputBuffer(o)!!, info.presentationTimeUs, info.flags and MediaCodec.BUFFER_FLAG_KEY_FRAME != 0)
            }
            encoder.releaseOutputBuffer(o, false)
            if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) done = true
        }
    }

    /** No more frames: wait out what's still in the codecs. */
    fun finish(write: (ByteBuffer, Long, Boolean) -> Unit) {
        while (!fed) {
            val i = decoder.dequeueInputBuffer(WAIT_US)
            if (i >= 0) {
                decoder.queueInputBuffer(i, 0, 0, 0, MediaCodec.BUFFER_FLAG_END_OF_STREAM)
                fed = true
            }
            pump(write)
        }
        while (!done) {
            pump(write)
            Thread.sleep(1)
        }
    }

    fun close() {
        runCatching { decoder.release() }
        runCatching { drawer.close() }
        runCatching { encoder.release() }
    }
}

/** One run of the sound through decoder, mixdown and encoder, and into the muxer. */
@OptIn(UnstableApi::class)
private class StereoSound(from: MediaFormat, private val rate: Int, private val muxer: FragmentedMp4Muxer, private val track: Int, private val startUs: Long) {
    private val decoder = MediaCodec.createDecoderByType(from.getString(MediaFormat.KEY_MIME)!!)
    private val encoder = MediaCodec.createEncoderByType(MimeTypes.AUDIO_AAC)
    private val info = MediaCodec.BufferInfo()
    private var channels = from.getInteger(MediaFormat.KEY_CHANNEL_COUNT)
    private var decoded = false
    private var encoded = false
    /** Stereo samples waiting for room in the encoder, and when they start. */
    private var pending = ShortArray(0)
    private var pendingAt = 0
    private var pendingUs = 0L
    private var started = false
    /** The first sound to keep; before that it's the codecs' own delay. */
    private val keepUs = startUs + CODEC_DELAY * 1_000_000 / rate

    init {
        decoder.configure(from, null, null, 0)
        decoder.start()
        val to = MediaFormat.createAudioFormat(MimeTypes.AUDIO_AAC, rate, 2).apply {
            setInteger(MediaFormat.KEY_AAC_PROFILE, MediaCodecInfo.CodecProfileLevel.AACObjectLC)
            setInteger(MediaFormat.KEY_BIT_RATE, STEREO_BITRATE)
            setInteger(MediaFormat.KEY_MAX_INPUT_SIZE, 1 shl 16)
        }
        encoder.configure(to, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
        encoder.start()
    }

    /** Hand over one compressed sample, or the end; false when the decoder has no room yet. */
    fun feed(read: (ByteBuffer) -> Int, timeUs: Long): Boolean {
        val i = decoder.dequeueInputBuffer(WAIT_US)
        if (i < 0) return false
        val size = read(decoder.getInputBuffer(i)!!)
        if (size < 0) {
            decoder.queueInputBuffer(i, 0, 0, 0, MediaCodec.BUFFER_FLAG_END_OF_STREAM)
        } else {
            decoder.queueInputBuffer(i, 0, size, timeUs, 0)
        }
        return true
    }

    /** Move along what's ready, as far as it goes. */
    fun pump() {
        while (fill()) { /* more room */ }
        while (true) {
            val i = encoder.dequeueOutputBuffer(info, 0)
            if (i == MediaCodec.INFO_TRY_AGAIN_LATER) break
            if (i < 0) continue // a new output format: we said what it would be
            val done = info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0
            val config = info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG != 0
            if (info.size > 0 && !config && info.presentationTimeUs >= keepUs) {
                muxer.writeSampleData(track, encoder.getOutputBuffer(i)!!, BufferInfo(info.presentationTimeUs - keepUs, info.size, C.BUFFER_FLAG_KEY_FRAME))
            }
            encoder.releaseOutputBuffer(i, false)
            if (done) encoded = true
        }
    }

    /** Sound from the decoder to the encoder: true while there may be more. */
    private fun fill(): Boolean {
        if (pendingAt >= pending.size) {
            if (decoded) return false
            val o = decoder.dequeueOutputBuffer(info, 0)
            if (o == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
                channels = decoder.outputFormat.getInteger(MediaFormat.KEY_CHANNEL_COUNT)
                return true
            }
            if (o < 0) return false
            val pcm = ShortArray(info.size / 2)
            decoder.getOutputBuffer(o)!!.order(ByteOrder.nativeOrder()).asShortBuffer().get(pcm)
            pending = downmix(pcm, channels)
            pendingAt = 0
            pendingUs = info.presentationTimeUs
            // the first sound starts late: silence up to where the track has to begin
            if (!started && pending.isNotEmpty()) {
                started = true
                val gap = ((pendingUs - startUs) * rate / 1_000_000).toInt().coerceAtLeast(0)
                pending = ShortArray(gap * 2) + pending
                pendingUs -= gap * 1_000_000L / rate
            }
            if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) decoded = true
            decoder.releaseOutputBuffer(o, false)
        }
        val i = encoder.dequeueInputBuffer(0)
        if (i < 0) return false
        val buf = encoder.getInputBuffer(i)!!.order(ByteOrder.nativeOrder())
        val n = minOf(buf.remaining() / 2, pending.size - pendingAt)
        buf.asShortBuffer().put(pending, pendingAt, n)
        val last = decoded && pendingAt + n >= pending.size
        encoder.queueInputBuffer(i, 0, n * 2, pendingUs, if (last) MediaCodec.BUFFER_FLAG_END_OF_STREAM else 0)
        pendingUs += n / 2 * 1_000_000L / rate
        pendingAt += n
        return true
    }

    /** Wait out what's still in the codecs. */
    fun finish() {
        while (!encoded) {
            pump()
            Thread.sleep(1)
        }
    }

    fun close() {
        runCatching { decoder.release() }
        runCatching { encoder.release() }
    }
}

/**
 * Write the file as an MP4 with stereo sound, from the last keyframe before
 * `startUs`. Its clock starts at zero there, which is returned first through
 * `onStart`. Blocks for as long as whoever reads `out` is reading.
 */
@OptIn(UnstableApi::class)
fun writeStereo(source: ServerFile, startUs: Long, out: OutputStream, toH264: Boolean = false, onStart: (Long) -> Unit = {}) {
    val ex = MediaExtractor()
    var sound: StereoSound? = null
    var made: H264Video? = null
    try {
        source.reopen()
        ex.setDataSource(source)
        source.opened()
        val formats = (0 until ex.trackCount).map { ex.getTrackFormat(it) }
        fun mime(f: MediaFormat) = f.getString(MediaFormat.KEY_MIME).orEmpty()
        val video = formats.indexOfFirst { if (toH264) mime(it).startsWith("video/") else mime(it) in FragmentedMp4Muxer.SUPPORTED_VIDEO_SAMPLE_MIME_TYPES }
        val audio = formats.indexOfFirst { mime(it).startsWith("audio/") }
        if (video < 0 || audio < 0) throw IOException("no video or sound this can stream")
        // a look ahead at the video alone, for where its clock starts
        ex.selectTrack(video)
        ex.seekTo(startUs, MediaExtractor.SEEK_TO_PREVIOUS_SYNC)
        val ahead = ArrayList<Long>()
        // converted video has no B-frames of its own: a few frames are enough to find the start
        while (ahead.size < (if (toH264) 8 else LOOK_AHEAD) && ex.sampleTime >= 0) {
            ahead += ex.sampleTime
            if (!ex.advance()) break
        }
        if (ahead.isEmpty()) throw IOException("no video at this position")
        // an open GOP shows a few frames before its keyframe
        val base = ahead.min()
        // The muxer starts each track's clock at zero and the video shows late, so the
        // sound has to begin that much earlier in the film to stay in step with it.
        val soundFrom = if (toH264) base else base - reorderDelayUs(ahead)
        if (toH264 && source.h264 == null) {
            ex.seekTo(startUs, MediaExtractor.SEEK_TO_PREVIOUS_SYNC)
            val probe = H264Video(formats[video])
            try {
                while (probe.format == null && ex.sampleTime >= 0) {
                    if (probe.feed({ b -> ex.readSampleData(b, 0) }, ex.sampleTime)) ex.advance()
                    probe.pump { _, _, _ -> }
                }
                source.h264 = probe.format
            } finally {
                probe.close()
            }
        }
        ex.selectTrack(audio)
        ex.seekTo(startUs, MediaExtractor.SEEK_TO_PREVIOUS_SYNC)
        onStart(base)

        val rate = formats[audio].getInteger(MediaFormat.KEY_SAMPLE_RATE)
        val muxer = FragmentedMp4Muxer.Builder(out).setFragmentDurationMs(FRAGMENT_MS).setSampleCopyingEnabled(true).build()
        val videoTrack = muxer.addTrack(if (toH264) source.h264 ?: throw IOException("no H.264 encoder") else MediaFormatUtil.createFormatFromMediaFormat(formats[video]))
        val config = AacUtil.buildAacLcAudioSpecificConfig(rate, 2)
        val audioTrack = muxer.addTrack(
            Format.Builder().setSampleMimeType(MimeTypes.AUDIO_AAC).setChannelCount(2).setSampleRate(rate)
                .setInitializationData(listOf(config)).build(),
        )
        val s = StereoSound(formats[audio], rate, muxer, audioTrack, soundFrom)
        sound = s
        val v = if (toH264) H264Video(formats[video]).also { made = it } else null
        val writeVideo = { b: ByteBuffer, at: Long, key: Boolean ->
            if (at >= base) muxer.writeSampleData(videoTrack, b, BufferInfo(at - base, b.remaining(), if (key) C.BUFFER_FLAG_KEY_FRAME else 0))
        }
        var buf = ByteBuffer.allocateDirect(1 shl 21)
        while (true) {
            val t = ex.sampleTrackIndex
            if (t < 0) break
            val at = ex.sampleTime
            if (t == video && v != null) {
                while (!v.feed({ b -> ex.readSampleData(b, 0) }, at)) v.pump(writeVideo)
                ex.advance()
                v.pump(writeVideo)
            } else if (t == video) {
                if (ex.sampleSize > buf.capacity()) buf = ByteBuffer.allocateDirect(ex.sampleSize.toInt())
                buf.clear()
                val n = ex.readSampleData(buf, 0)
                buf.position(0).limit(n)
                val key = ex.sampleFlags and MediaExtractor.SAMPLE_FLAG_SYNC != 0
                muxer.writeSampleData(videoTrack, buf, BufferInfo(at - base, n, if (key) C.BUFFER_FLAG_KEY_FRAME else 0))
                ex.advance()
            } else {
                // sound from before the keyframe still goes in: the decoder needs its run-up
                while (!s.feed({ b -> ex.readSampleData(b, 0) }, at)) s.pump()
                ex.advance()
            }
            s.pump()
        }
        while (!s.feed({ -1 }, 0)) s.pump()
        s.finish()
        v?.finish(writeVideo)
        muxer.close()
    } finally {
        sound?.close()
        made?.close()
        ex.release()
    }
}
