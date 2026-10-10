package com.mpacer.companion.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/** Le resume de version affiche dans l'en-tete de la liste des seances. */
class VersionTest {

    @Test
    fun lhorodatage_iso_est_affiche_a_la_francaise() {
        assertEquals("10/10/2026 07:42", horodatageLisible("2026-10-10 07:42"))
    }

    @Test
    fun un_ancien_apk_sans_heure_reste_lisible() {
        assertEquals("10/10/2026", horodatageLisible("2026-10-10"))
    }

    @Test
    fun une_date_inconnue_est_rendue_telle_quelle() {
        assertEquals("hier", horodatageLisible("hier"))
        assertEquals("", horodatageLisible("  "))
    }

    @Test
    fun le_resume_porte_la_version_et_lhorodatage() {
        assertEquals(
            "Version 0.2.0 - compile le 10/10/2026 07:42",
            resumeVersion("0.2.0", "2026-10-10 07:42"),
        )
    }

    @Test
    fun un_nom_absent_reste_lisible() {
        assertEquals(
            "Version ? - compile le 10/10/2026 07:42",
            resumeVersion("  ", "2026-10-10 07:42"),
        )
    }
}
