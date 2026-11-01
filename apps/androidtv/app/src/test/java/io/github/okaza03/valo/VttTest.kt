package io.github.okaza03.valo

import org.junit.Assert.assertEquals
import org.junit.Test

class VttTest {
    @Test
    fun writesWebVtt() {
        val out = vtt(listOf(VttCue(1_500, 3_004, "Hello"), VttCue(3_723_004, 3_725_000, "Line one\n\nLine two")))
        assertEquals(
            "WEBVTT\n\n00:00:01.500 --> 00:00:03.004\nHello\n\n" +
                "01:02:03.004 --> 01:02:05.000\nLine one\nLine two\n\n",
            out,
        )
    }

    @Test
    fun noCuesIsStillAFile() = assertEquals("WEBVTT\n\n", vtt(emptyList()))
}
