package com.mpacer.watch.live

import java.io.ByteArrayInputStream
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test

/**
 * Verifie la serialisation MQTT octet par octet : c'est le seul contrat entre la
 * montre et le broker, il doit etre juste sans broker sous la main.
 */
class MqttCodecTest {

    @Test
    fun remainingLengthUsesSevenBitsPerByte() {
        assertArrayEquals(byteArrayOf(0x00), MqttCodec.remainingLength(0))
        assertArrayEquals(byteArrayOf(0x7F), MqttCodec.remainingLength(127))
        assertArrayEquals(byteArrayOf(0x80.toByte(), 0x01), MqttCodec.remainingLength(128))
        assertArrayEquals(byteArrayOf(0xFF.toByte(), 0x7F), MqttCodec.remainingLength(16383))
        assertArrayEquals(
            byteArrayOf(0x80.toByte(), 0x80.toByte(), 0x01),
            MqttCodec.remainingLength(16384),
        )
    }

    @Test
    fun connectPacketFollowsTheSpecification() {
        val paquet = MqttCodec.connect("mpacer-a1b2", keepAliveS = 60)
        assertEquals(0x10, paquet[0].toInt() and 0xFF)
        assertEquals(paquet.size - 2, paquet[1].toInt() and 0xFF)
        // "MQTT", niveau 3.1.1, session propre, keep alive 60 s.
        val entete = byteArrayOf(
            0x00, 0x04,
            'M'.code.toByte(), 'Q'.code.toByte(), 'T'.code.toByte(), 'T'.code.toByte(),
            0x04, 0x02, 0x00, 0x3C,
        )
        assertArrayEquals(entete, paquet.copyOfRange(2, 12))
        assertEquals(0x00, paquet[12].toInt())
        assertEquals(11, paquet[13].toInt())
        assertEquals("mpacer-a1b2", String(paquet, 14, 11, Charsets.UTF_8))
    }

    @Test
    fun credentialsSetTheirFlags() {
        val paquet = MqttCodec.connect("montre", "coureur", "secret", 30)
        assertEquals(0xC2, paquet[9].toInt() and 0xFF)
        assertArrayEquals(
            byteArrayOf(0x00, 0x06) + "secret".toByteArray(Charsets.UTF_8),
            paquet.copyOfRange(paquet.size - 8, paquet.size),
        )
        assertEquals(30, ((paquet[10].toInt() and 0xFF) shl 8) or (paquet[11].toInt() and 0xFF))
    }

    @Test
    fun publishPacketCarriesItsTopicAndPayload() {
        val charge = "{\"t\":1}".toByteArray(Charsets.UTF_8)
        val paquet = MqttCodec.publish("mpacer/live/montre", charge, retain = true)
        assertEquals(0x31, paquet[0].toInt() and 0xFF)

        val lu = MqttCodec.readPacket(ByteArrayInputStream(paquet))!!
        assertEquals(MqttCodec.PUBLISH, lu.type)
        assertEquals(0x01, lu.flags)
        val taille = ((lu.payload[0].toInt() and 0xFF) shl 8) or (lu.payload[1].toInt() and 0xFF)
        assertEquals("mpacer/live/montre", String(lu.payload, 2, taille, Charsets.UTF_8))
        assertArrayEquals(charge, lu.payload.copyOfRange(2 + taille, lu.payload.size))
    }

    @Test
    fun withoutRetainTheLowBitStaysClear() {
        val paquet = MqttCodec.publish("mpacer/live/montre", "x".toByteArray(), retain = false)
        assertEquals(0x30, paquet[0].toInt() and 0xFF)
    }

    @Test
    fun aClosedStreamIsNotAnError() {
        assertNull(MqttCodec.readPacket(ByteArrayInputStream(ByteArray(0))))
    }

    @Test
    fun anImpossibleLengthIsRejected() {
        assertThrows(IllegalArgumentException::class.java) { MqttCodec.remainingLength(-1) }
        assertThrows(IllegalArgumentException::class.java) { MqttCodec.remainingLength(300000000) }
    }

    @Test
    fun accentedTopicsAreEncodedInUtf8() {
        val paquet = MqttCodec.publish("mpacer/live/ete-accents", "x".toByteArray(), retain = false)
        val lu = MqttCodec.readPacket(ByteArrayInputStream(paquet))!!
        val taille = ((lu.payload[0].toInt() and 0xFF) shl 8) or (lu.payload[1].toInt() and 0xFF)
        assertEquals("mpacer/live/ete-accents", String(lu.payload, 2, taille, Charsets.UTF_8))
    }

    @Test
    fun keepAliveAndPingPacketsAreConstantSize() {
        assertArrayEquals(byteArrayOf(0xC0.toByte(), 0x00), MqttCodec.pingreq())
        assertArrayEquals(byteArrayOf(0xE0.toByte(), 0x00), MqttCodec.disconnect())
    }
}
