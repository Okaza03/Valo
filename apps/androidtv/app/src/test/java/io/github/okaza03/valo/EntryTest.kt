package io.github.okaza03.valo

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class EntryTest {
    private fun at(pos: Double) = Entry(pos, 2700.0, 0)

    @Test
    fun aPeekIsntStarted() {
        assertFalse(at(5.0).started) // opened and closed again
        assertFalse(at(30.0).started)
        assertTrue(at(31.0).started)
        assertTrue(at(2700.0).started) // watched to the end
    }

    @Test
    fun aPeekIsNeitherInProgressNorWatched() {
        assertFalse(at(5.0).inProgress)
        assertFalse(at(5.0).watched)
        assertTrue(at(600.0).inProgress)
        assertTrue(at(2430.0).watched) // 90%
    }
}
