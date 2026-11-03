package io.github.okaza03.valo.cast

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class DownmixTest {
    private fun frame(vararg s: Int) = s.map { it.toShort() }.toShortArray()

    @Test
    fun stereoIsLeftAlone() {
        val pcm = frame(1, 2, 3, 4)
        assertArrayEquals(pcm, downmix(pcm, 2))
    }

    @Test
    fun monoGoesToBothSides() = assertArrayEquals(frame(5, 5, 6, 6), downmix(frame(5, 6), 1))

    @Test
    fun surroundFoldsDownWithoutTheLfe() {
        // front left, front right, centre, LFE, back left, back right
        val left = downmix(frame(10000, 0, 0, 0, 0, 0), 6)
        assertEquals(6000, left[0].toInt())
        assertEquals(0, left[1].toInt())
        val centre = downmix(frame(0, 0, 10000, 0, 0, 0), 6)
        assertEquals(centre[0], centre[1])
        assertTrue(centre[0] > 4000)
        assertArrayEquals(frame(0, 0), downmix(frame(0, 0, 0, 30000, 0, 0), 6))
        val back = downmix(frame(0, 0, 0, 0, 0, 10000), 6)
        assertEquals(0, back[0].toInt())
        assertTrue(back[1] > 4000)
    }

    @Test
    fun loudPeaksClipInsteadOfWrappingAround() {
        val loud = downmix(frame(32767, 32767, 32767, 0, 32767, 32767), 6)
        assertEquals(32767, loud[0].toInt())
        assertEquals(-32768, downmix(frame(-32768, 0, -32768, 0, -32768, 0), 6)[0].toInt())
    }

    @Test
    fun bFramesDelayTheClock() {
        assertEquals(0, reorderDelayUs(listOf(0, 40, 80, 120)))
        // stored I P B B, shown I B B P
        assertEquals(40, reorderDelayUs(listOf(0, 120, 40, 80, 240, 160, 200)))
        assertEquals(0, reorderDelayUs(emptyList()))
    }

    @Test
    fun chromecastsGetTheContainersTheyPlay() {
        assertTrue(castsDirectly("http://s/a%20b.mp4"))
        assertTrue(castsDirectly("http://s/a.MOV?x=1"))
        assertTrue(!castsDirectly("http://s/a.mkv"))
        assertTrue(!castsDirectly("http://s/a.avi?token=b.mp4"))
    }

    @Test
    fun onlyWhatTheDeviceCantDecodeBecomesH264() {
        assertTrue(!needsH264("h264", "Chromecast"))
        assertTrue(!needsH264("AVC", "Chromecast"))
        assertTrue(needsH264("hevc", "Chromecast"))
        assertTrue(!needsH264("hevc", "Chromecast Ultra"))
        assertTrue(!needsH264("h265", "Smart TV Pro"))
        assertTrue(needsH264("mpeg4", "Smart TV Pro"))
        assertTrue(!needsH264(null, "Chromecast"))
    }

    @Test
    fun h264SaysWhatItIs() {
        val high41 = byteArrayOf(0, 0, 0, 1, 0x67, 0x64, 0, 0x29, 0x2c)
        assertEquals("avc1.640029", avcCodec(high41))
        assertEquals("avc1.42C029", avcCodec(byteArrayOf(0, 0, 1, 0x27, 0x42, 0xC0.toByte(), 0x29, 1)))
        assertEquals(null, avcCodec(byteArrayOf(0, 0, 1, 0x68, 1, 2, 3)))
    }

    @Test
    fun videoIsScaledToFitAChromecast() {
        assertEquals(1920 to 1080, fitForCast(3840, 2160))
        assertEquals(1920 to 800, fitForCast(3840, 1600))
        assertEquals(1280 to 720, fitForCast(1280, 720)) // never up
        assertEquals(1440 to 1080, fitForCast(2880, 2160))
        assertEquals(1918 to 1078, fitForCast(1919, 1079)) // even
    }

    @Test
    fun everyFrameComesOut() {
        assertEquals(20, downmix(ShortArray(60), 6).size)
        assertEquals(20, downmix(ShortArray(80), 8).size)
    }
}
