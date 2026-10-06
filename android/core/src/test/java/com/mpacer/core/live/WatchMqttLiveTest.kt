package com.mpacer.core.live

import java.io.ByteArrayOutputStream
import java.net.InetSocketAddress
import java.net.Socket
import javax.net.ssl.SSLSocket
import javax.net.ssl.SSLSocketFactory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assume.assumeTrue
import org.junit.Test

/**
 * Test d'integration optionnel : les paquets de la montre sont presentes a un
 * vrai broker, puis un point de test fait l'aller-retour.
 *
 * Il ne s'execute que si la variable d'environnement MPACER_MQTT_TEST_URL est
 * definie (aucun identifiant n'entre dans le depot) :
 *
 * ```
 * MPACER_MQTT_TEST_URL="mqtt://utilisateur:motdepasse@192.168.0.115:1883" \
 *   ./gradlew :core:testDebugUnitTest --tests '*WatchMqttLiveTest*'
 * ```
 *
 * Sans la variable, le test est ignore : la suite JVM reste hermetique au reseau.
 * Ce qui est verifie ici, ce sont les octets exacts emis par la montre
 * (MqttCodec : CONNECT, SUBSCRIBE, PUBLISH, DISCONNECT) et l'analyse de
 * l'adresse (BrokerAddress), c'est-a-dire exactement ce que le menu MQTT
 * configure depuis le poignet.
 */
class WatchMqttLiveTest {

    /** SUBSCRIBE (0x82) : le codec de la montre ne publie que, le test s'abonne. */
    private val subscribe = 0x82

    @Test
    fun theWatchPacketsMakeTheRoundTripThroughARealBroker() {
        val url = System.getenv("MPACER_MQTT_TEST_URL").orEmpty()
        assumeTrue("MPACER_MQTT_TEST_URL non defini : test d'integration ignore", url.isNotBlank())
        val adresse = BrokerAddress.parse(url)
        assertNotNull("adresse illisible : " + url, adresse)
        val config = LiveConfig(url = url)
        val identifiants = adresse!!.credentials(config)
        // Sujet de test : hors de <prefixe>/live/+, donc invisible sur /live.
        val sujet = System.getenv("MPACER_MQTT_TEST_TOPIC").orEmpty()
            .ifBlank { LiveProbe.testTopic(config) }
        val charge = LivePayload.encode(
            1_700_000_000_000L, 48.8566, 2.3522, 4.0, 1234.5, 300.0, 142, 78, "run", "mpacer-android",
        )

        val brut = Socket()
        brut.tcpNoDelay = true
        brut.connect(InetSocketAddress(adresse.host, adresse.port), 8000)
        val flux: Socket = if (adresse.tls) {
            val securise = (SSLSocketFactory.getDefault() as SSLSocketFactory)
                .createSocket(brut, adresse.host, adresse.port, true) as SSLSocket
            securise.startHandshake()
            securise
        } else {
            brut
        }
        flux.soTimeout = 8000
        try {
            val sortie = flux.getOutputStream()
            val entree = flux.getInputStream()

            sortie.write(MqttCodec.connect("mpacer-android-test", identifiants.first, identifiants.second, 30))
            sortie.flush()
            val connack = MqttCodec.readPacket(entree)
            assertNotNull("aucun CONNACK", connack)
            assertEquals(MqttCodec.CONNACK, connack!!.type)
            assertEquals("broker refuse la connexion", 0, connack.payload.last().toInt() and 0xFF)

            sortie.write(abonnement(sujet, 7))
            sortie.flush()
            val suback = MqttCodec.readPacket(entree)
            assertNotNull("aucun SUBACK", suback)
            assertEquals(MqttCodec.SUBACK, suback!!.type)

            sortie.write(MqttCodec.publish(sujet, charge, retain = false))
            sortie.flush()
            val revenu = MqttCodec.readPacket(entree)
            assertNotNull("le point de test n'est pas revenu du broker", revenu)
            assertEquals(MqttCodec.PUBLISH, revenu!!.type)
            val tailleSujet = ((revenu.payload[0].toInt() and 0xFF) shl 8) or (revenu.payload[1].toInt() and 0xFF)
            assertEquals(sujet, String(revenu.payload, 2, tailleSujet, Charsets.UTF_8))
            assertEquals(
                String(charge, Charsets.UTF_8),
                String(revenu.payload, 2 + tailleSujet, revenu.payload.size - 2 - tailleSujet, Charsets.UTF_8),
            )

            sortie.write(MqttCodec.disconnect())
            sortie.flush()
        } finally {
            flux.close()
        }
    }

    /** SUBSCRIBE (0x82) QoS 0 : le codec de la montre n'en a pas besoin, le test si. */
    private fun abonnement(sujet: String, idPaquet: Int): ByteArray {
        val charge = ByteArrayOutputStream()
        charge.write((idPaquet shr 8) and 0xFF)
        charge.write(idPaquet and 0xFF)
        charge.write(MqttCodec.utf8(sujet))
        charge.write(0x00)
        val corps = charge.toByteArray()
        return byteArrayOf(subscribe.toByte()) + MqttCodec.remainingLength(corps.size) + corps
    }
}
