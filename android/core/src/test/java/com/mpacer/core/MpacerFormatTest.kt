package com.mpacer.core

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Le formatage est le seul code d'affichage partage par la montre et le
 * telephone : s'il se trompe, les deux interfaces mentent en meme temps.
 */
class MpacerFormatTest {

    @Test
    fun paceIsRenderedInMinutesAndSeconds() {
        assertEquals("5:00", MpacerFormat.pace(300.0))
        assertEquals("1:05", MpacerFormat.pace(65.0))
        // L'allure est arrondie a la seconde : 299,6 s partent en 5:00, pas en 4:59.
        assertEquals("5:00", MpacerFormat.pace(299.6))
        assertEquals("4:59", MpacerFormat.pace(299.4))
    }

    @Test
    fun aMissingPaceIsShownAsDashes() {
        assertEquals("--:--", MpacerFormat.pace(null))
        assertEquals("--:--", MpacerFormat.pace(0.0))
        assertEquals("--:--", MpacerFormat.pace(-1.0))
        assertEquals("--:--", MpacerFormat.pace(Double.NaN))
    }

    @Test
    fun durationGrowsAnHourFieldOnlyWhenUseful() {
        assertEquals("0:00", MpacerFormat.duration(0.0))
        assertEquals("1:05", MpacerFormat.duration(65.0))
        assertEquals("59:59", MpacerFormat.duration(3599.0))
        assertEquals("1:01:01", MpacerFormat.duration(3661.0))
    }

    @Test
    fun distanceFollowsTheUnitSystem() {
        assertEquals("1.00 km", MpacerFormat.distance(1000.0))
        assertEquals("10.00 km", MpacerFormat.distance(10_000.0))
        assertEquals("1.00 mi", MpacerFormat.distance(1609.344, imperial = true))
        // Un mille vaut 1,609 km : la conversion ne doit pas s'inverser.
        assertEquals("0.62 mi", MpacerFormat.distance(1000.0, imperial = true))
    }

    @Test
    fun theDecimalPointNeverBecomesAComma() {
        // Un telephone ou une montre en francais formaterait "1,00" : le JSON de
        // la charge utile MQTT et l'affichage doivent rester en point.
        val ancienne = java.util.Locale.getDefault()
        try {
            java.util.Locale.setDefault(java.util.Locale.FRANCE)
            assertEquals("1.00 km", MpacerFormat.distance(1000.0))
            assertEquals("5:00", MpacerFormat.pace(300.0))
        } finally {
            java.util.Locale.setDefault(ancienne)
        }
    }
}
