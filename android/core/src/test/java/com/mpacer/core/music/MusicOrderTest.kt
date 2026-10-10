package com.mpacer.core.music

import kotlin.random.Random
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

/**
 * L'ordre de lecture est ce que l'utilisateur choisit sur la montre : dans
 * l'ordre de la playlist, ou melange. Il doit garder toutes les pistes, ne
 * jamais en inventer, et rester reproductible pour un tirage donne.
 */
class MusicOrderTest {

    private fun piste(id: Int) = LocalTrack(
        id = "p" + id,
        title = "Piste " + id,
        file = "/sdcard/Music/run/p" + id + ".mp3",
    )

    private val liste = (0 until 10).map(::piste)

    @Test
    fun fixedModeKeepsThePlaylistOrder() {
        assertEquals(liste, ordreLecture(liste, shuffle = false, hasard = Random(1)))
    }

    @Test
    fun shuffleKeepsEveryTrackExactlyOnce() {
        val melange = ordreLecture(liste, shuffle = true, hasard = Random(42))
        assertEquals(liste.size, melange.size)
        assertEquals(liste.map { it.id }.toSet(), melange.map { it.id }.toSet())
    }

    @Test
    fun shuffleActuallyChangesTheOrder() {
        val melange = ordreLecture(liste, shuffle = true, hasard = Random(42))
        assertNotEquals(liste.map { it.id }, melange.map { it.id })
    }

    @Test
    fun shuffleIsReproducibleForAGivenSeed() {
        assertEquals(
            ordreLecture(liste, shuffle = true, hasard = Random(7)).map { it.id },
            ordreLecture(liste, shuffle = true, hasard = Random(7)).map { it.id },
        )
    }

    @Test
    fun emptyAndSingleTrackListsAreReturnedAsIs() {
        assertEquals(emptyList<LocalTrack>(), ordreLecture(emptyList(), shuffle = true))
        val seule = listOf(piste(1))
        assertEquals(seule, ordreLecture(seule, shuffle = true, hasard = Random(3)))
    }

    @Test
    fun aTrackWithoutAFileIsStillFilteredOutByThePlaylist() {
        // Rappel du contrat : seules les pistes jouables entrent dans la file.
        val playlist = LocalPlaylist(
            id = "run",
            name = "Run",
            tracks = listOf(piste(1), piste(2).copy(file = null)),
        )
        assertEquals(listOf("p1"), playlist.playable.map { it.id })
    }
}
