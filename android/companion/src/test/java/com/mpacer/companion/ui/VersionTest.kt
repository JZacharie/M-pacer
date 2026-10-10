package com.mpacer.companion.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/** Le resume de version affiche dans l'en-tete de la liste des seances. */
class VersionTest {

    @Test
    fun lejour_iso_est_affiche_a_la_francaise() {
        assertEquals("10/10/2026", jourLisible("2026-10-10"))
    }

    @Test
    fun une_date_inconnue_est_rendue_telle_quelle() {
        assertEquals("hier", jourLisible("hier"))
        assertEquals("", jourLisible("  "))
    }

    @Test
    fun le_resume_porte_la_version_et_le_jour() {
        assertEquals(
            "Version 0.2.0 - compile le 10/10/2026",
            resumeVersion("0.2.0", "2026-10-10"),
        )
    }

    @Test
    fun un_nom_absent_reste_lisible() {
        assertEquals("Version ? - compile le 10/10/2026", resumeVersion("  ", "2026-10-10"))
    }
}
