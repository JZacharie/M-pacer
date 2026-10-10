package com.mpacer.core.live

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * La file d'attente est ce qui evite un trou dans la trace partagee : si elle
 * se trompe, les proches voient une ligne droite entre deux positions separees
 * de vingt minutes, ou la montre consomme de la memoire sans fin.
 */
class LiveQueueTest {

    private fun paquet(tMs: Long) = LiveQueue.Entry(tMs, ByteArray(4))

    private fun horodatages(file: LiveQueue): List<Long> {
        val sortie = mutableListOf<Long>()
        while (true) {
            val entree = file.removeFirst() ?: break
            sortie.add(entree.tMs)
        }
        return sortie
    }

    @Test
    fun anEmptyQueueGivesNothing() {
        val file = LiveQueue()
        assertTrue(file.isEmpty)
        assertEquals(0, file.size)
        assertNull(file.removeFirst())
    }

    @Test
    fun pointsLeaveInTheirArrivalOrder() {
        val file = LiveQueue()
        file.add(paquet(1_000))
        file.add(paquet(2_000))
        file.add(paquet(3_000))
        assertEquals(listOf(1_000L, 2_000L, 3_000L), horodatages(file))
    }

    @Test
    fun theWindowIsMeasuredFromTheNewestPoint() {
        val file = LiveQueue(maxPoints = 200, windowMs = 60_000)
        file.add(paquet(0))
        file.add(paquet(30_000))
        assertEquals(2, file.size)
        // Un point vieux de plus d'une minute par rapport au plus recent part.
        file.add(paquet(120_000))
        assertEquals(listOf(120_000L), horodatages(file))
        assertEquals(2, file.dropped)
    }

    @Test
    fun aLongOutageIsCompressedInsteadOfTruncated() {
        val file = LiveQueue(maxPoints = 4, windowMs = 1_000_000)
        for (index in 0 until 8) file.add(paquet(index * 10_000L))
        assertEquals(4, file.size)
        val restants = horodatages(file)
        assertEquals(restants.toString(), 0L, restants.first())
        assertEquals(restants.toString(), 70_000L, restants.last())
        assertEquals(4, file.dropped)
    }

    @Test
    fun stoppingKeepsOnlyTheCurrentPosition() {
        val file = LiveQueue()
        file.add(paquet(1_000))
        file.add(paquet(2_000))
        file.add(paquet(3_000))
        file.keepNewest()
        assertEquals(1, file.size)
        assertEquals(listOf(3_000L), horodatages(file))
        assertEquals(2, file.dropped)
    }

    @Test
    fun clearingResetsTheCounters() {
        val file = LiveQueue(maxPoints = 2, windowMs = 1_000_000)
        for (index in 0 until 8) file.add(paquet(index * 10_000L))
        assertTrue(file.dropped > 0)
        file.clear()
        assertTrue(file.isEmpty)
        assertEquals(0, file.dropped)
    }

    @Test
    fun theDefaultQueueCoversTwentyMinutesOfPublishing() {
        // 120 points a dix secondes : la duree d'une coupure que le suivi peut
        // encore rattraper, et la taille par defaut des reglages.
        assertEquals(LiveConfig.MAX_QUEUE_POINTS, LiveQueue.MAX_POINTS)
        assertEquals(20 * 60 * 1000L, LiveQueue.WINDOW_MS)
        assertEquals(LiveQueue.MAX_POINTS, LiveConfig.DEFAULT.maxQueue)
    }
}
