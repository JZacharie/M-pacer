package com.mpacer.core

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Reprise des seances du compte (voir [SyncClient.pullWorkouts]).
 *
 * Seule la partie pure est testee ici : le tri entre ce que le backend possede
 * et ce que le telephone possede deja. Le reseau, lui, se verifie sur appareil.
 */
class SyncClientTest {

    @Test
    fun les_identifiants_sont_lus_dans_l_ordre_du_backend() {
        val corps = """
            {"total": 3, "items": [
                {"id": "1700000000003", "distance_m": 5000.0},
                {"id": "1700000000002", "distance_m": 10000.0},
                {"id": "1700000000001", "distance_m": 21097.5}
            ]}
        """.trimIndent()
        assertEquals(
            listOf("1700000000003", "1700000000002", "1700000000001"),
            SyncClient.workoutIds(corps),
        )
    }

    @Test
    fun un_corps_illisible_ne_ramene_rien() {
        assertEquals(emptyList<String>(), SyncClient.workoutIds("pas du json"))
        assertEquals(emptyList<String>(), SyncClient.workoutIds("{}"))
        // Une seance sans identifiant ne peut pas etre reprise : on l'ignore.
        assertEquals(emptyList<String>(), SyncClient.workoutIds("""{"items":[{"distance_m":5000.0}]}"""))
    }

    @Test
    fun seules_les_seances_absentes_sont_reprises() {
        val distantes = listOf("a", "b", "c", "d")
        val locales = setOf("b", "d")
        assertEquals(listOf("a", "c"), SyncClient.missingIds(distantes, locales))
    }

    @Test
    fun une_archive_vide_reprend_tout() {
        assertEquals(listOf("a", "b"), SyncClient.missingIds(listOf("a", "b"), emptySet()))
    }

    @Test
    fun une_seance_deja_presente_n_est_pas_reprise_deux_fois() {
        // C'est ce qui rend la reprise repetable sans creer de doublon : le
        // fichier local porte l'identifiant de la seance.
        val distantes = listOf("1700000000000")
        assertEquals(emptyList<String>(), SyncClient.missingIds(distantes, setOf("1700000000000")))
    }
}
