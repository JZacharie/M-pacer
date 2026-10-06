package com.mpacer.watch.live

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * La politique de publication est le garde-fou du budget radio : si elle se
 * trompe, la montre publie trop (batterie) ou pas assez (suivi inutile).
 */
class LivePolicyTest {

    private val config = LiveConfig(
        url = "mqtt://broker.local:1883",
        intervalS = 10,
        pausedIntervalS = 60,
        minAccuracyM = 50.0,
    )

    private fun decide(maintenantMs: Long, dernierMs: Long, enPause: Boolean = false, accuracyM: Double? = 5.0) =
        LivePolicy.decide(maintenantMs, dernierMs, enPause, accuracyM, config)

    @Test
    fun theFirstPointGoesOutImmediately() {
        assertEquals(LivePolicy.Decision.Publish, decide(1_000, 0))
    }

    @Test
    fun onePointPerPeriodOnly() {
        assertEquals(LivePolicy.Decision.TooSoon, decide(9_000, 1_000))
        assertEquals(LivePolicy.Decision.Publish, decide(11_000, 1_000))
    }

    @Test
    fun aPauseSlowsTheCadenceWithoutStoppingIt() {
        assertEquals(LivePolicy.Decision.TooSoon, decide(30_000, 1_000, enPause = true))
        assertEquals(LivePolicy.Decision.Publish, decide(61_000, 1_000, enPause = true))
        // En course, la meme periode publie deja.
        assertEquals(LivePolicy.Decision.Publish, decide(30_000, 1_000))
    }

    @Test
    fun anImpreciseFixIsDropped() {
        assertEquals(LivePolicy.Decision.Imprecise, decide(60_000, 1_000, accuracyM = 80.0))
        // Precision inconnue : on ne prive pas le suivi pour autant.
        assertEquals(LivePolicy.Decision.Publish, decide(60_000, 1_000, accuracyM = null))
    }

    @Test
    fun withoutABrokerNothingHappens() {
        assertEquals(
            LivePolicy.Decision.Inactif,
            LivePolicy.decide(60_000, 0, false, 5.0, LiveConfig.DEFAULT),
        )
        assertEquals(
            LivePolicy.Decision.Inactif,
            LivePolicy.decide(60_000, 0, false, 5.0, config.copy(enabled = false)),
        )
    }

    @Test
    fun theIntervalStaysWithinItsBounds() {
        assertEquals(5_000L, LiveConfig(intervalS = 1).intervalMs(false))
        assertEquals(300_000L, LiveConfig(intervalS = 6_000).intervalMs(false))
        assertEquals(60_000L, LiveConfig(pausedIntervalS = 60).intervalMs(true))
    }

    @Test
    fun aStateChangeIsPublishedOnce() {
        assertEquals(true, LivePolicy.etatChange("run", "pause"))
        assertEquals(false, LivePolicy.etatChange("run", "run"))
        assertEquals(true, LivePolicy.etatChange(null, "run"))
    }

    @Test
    fun theTopicFollowsThePrefixeAndTheDevice() {
        assertEquals("mpacer/live/montre-a1b2", LiveConfig().topic("montre-a1b2"))
        assertEquals("course/live/montre-a1b2", LiveConfig(topicPrefix = "course/").topic("montre-a1b2"))
        assertEquals("mpacer/live/fenix", LiveConfig(device = "fenix").topic("montre-a1b2"))
        assertEquals("mpacer/live/Fenix-7", LiveConfig(device = "Fenix 7").topic("montre-a1b2"))
        // Un prefixe vide retombe sur la valeur par defaut.
        assertEquals("mpacer/live/montre", LiveConfig(topicPrefix = "  ").topic("montre"))
    }

    @Test
    fun userInputIsNormalised() {
        val propre = LiveConfig.normalise(
            LiveConfig(
                url = "  mqtt://broker:1883  ",
                username = " coureur ",
                intervalS = 1,
                pausedIntervalS = 9_999,
                minAccuracyM = 1_000.0,
                maxQueue = 1_000,
            )
        )
        assertEquals("mqtt://broker:1883", propre.url)
        assertEquals("coureur", propre.username)
        assertEquals(LiveConfig.MIN_INTERVAL_S, propre.intervalS)
        assertEquals(LiveConfig.MAX_INTERVAL_S, propre.pausedIntervalS)
        assertEquals(500.0, propre.minAccuracyM, 0.001)
        assertEquals(128, propre.maxQueue)
    }

    @Test
    fun brokerAddressesAreParsedWithoutCrashing() {
        val simple = BrokerAddress.parse("mqtt://broker.local")
        assertEquals("broker.local", simple?.host)
        assertEquals(1883, simple?.port)
        assertEquals(false, simple?.tls)

        val securise = BrokerAddress.parse("mqtts://mqtt.exemple.org:8884")
        assertEquals("mqtt.exemple.org", securise?.host)
        assertEquals(8884, securise?.port)
        assertEquals(true, securise?.tls)

        val sansSchema = BrokerAddress.parse("192.168.0.10:1884")
        assertEquals("192.168.0.10", sansSchema?.host)
        assertEquals(1884, sansSchema?.port)

        val avecIdentifiants = BrokerAddress.parse("mqtt://coureur:secret@broker.local:1883")
        assertEquals("broker.local", avecIdentifiants?.host)

        assertNull(BrokerAddress.parse(""))
        assertNull(BrokerAddress.parse("   "))
        assertNull(BrokerAddress.parse("http://broker.local"))
        assertNull(BrokerAddress.parse("mqtt://broker.local:port"))
        assertNull(BrokerAddress.parse("mqtt://broker.local:0"))
    }
}
