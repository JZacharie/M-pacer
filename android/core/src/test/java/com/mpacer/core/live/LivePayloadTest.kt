package com.mpacer.core.live

import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * La charge utile part toutes les dix secondes sur le reseau : elle doit rester
 * petite, valide en JSON et identique quelle que soit la langue de la montre.
 */
class LivePayloadTest {

    @Test
    fun thePayloadIsCompactJsonWithShortFields() {
        val charge = LivePayload.encode(
            1_700_000_000_000L, 48.8566, 2.3522, 4.0, 1234.5, 300.0, 142, 78, "run", "montre-a1b2",
        )
        val texte = String(charge, Charsets.UTF_8)
        assertEquals(
            "{\"t\":1700000000000,\"lat\":48.856600,\"lon\":2.352200,\"acc\":4.0," +
                "\"dist\":1234.5,\"pace\":300.0,\"hr\":142,\"bat\":78," +
                "\"st\":\"run\",\"dev\":\"montre-a1b2\"}",
            texte,
        )
        assertTrue("charge utile de " + charge.size + " octets", charge.size < 200)
    }

    @Test
    fun optionalFieldsAreOmitted() {
        val texte = String(
            LivePayload.encode(1, 48.0, 2.0, null, null, null, null, null, "stop", ""),
            Charsets.UTF_8,
        )
        assertEquals("{\"t\":1,\"lat\":48.000000,\"lon\":2.000000,\"st\":\"stop\"}", texte)
    }

    @Test
    fun theDecimalPointNeverDependsOnTheLocale() {
        val ancienne = Locale.getDefault()
        try {
            Locale.setDefault(Locale.FRANCE)
            val texte = String(
                LivePayload.encode(1, 48.5, 2.5, null, null, null, null, null, "run", ""),
                Charsets.UTF_8,
            )
            assertTrue(texte, texte.contains("48.500000"))
            assertFalse(texte, texte.contains("48,500000"))
        } finally {
            Locale.setDefault(ancienne)
        }
    }

    @Test
    fun hostileTextCannotBreakTheJson() {
        val texte = String(
            LivePayload.encode(1, 48.0, 2.0, null, null, null, null, null, "ru\"n", "a\"b\\c"),
            Charsets.UTF_8,
        )
        assertTrue(texte, texte.contains("\"dev\":\"abc\""))
        assertFalse(texte, texte.contains("\\"))
    }

    @Test
    fun theDeviceNameIsTruncated() {
        val texte = String(
            LivePayload.encode(1, 48.0, 2.0, null, null, null, null, null, "run", "m".repeat(200)),
            Charsets.UTF_8,
        )
        assertTrue(texte, texte.contains("\"dev\":\"" + "m".repeat(24) + "\""))
    }
}
