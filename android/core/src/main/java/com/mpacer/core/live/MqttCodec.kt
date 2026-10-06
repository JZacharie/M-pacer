package com.mpacer.core.live

import java.io.ByteArrayOutputStream
import java.io.EOFException
import java.io.InputStream

/**
 * Serialisation MQTT 3.1.1 reduite a ce dont la montre a besoin : se connecter,
 * publier en QoS 0, garder la liaison en vie, se deconnecter.
 *
 * Aucune bibliotheque : les paquets tiennent en quelques dizaines d'octets et le
 * code est testable sans broker (les tests relisent octet par octet ce qui est
 * ecrit). Cela evite aussi d'embarquer un client complet, avec ses fils et ses
 * tampons, dans une application de montre.
 */
internal object MqttCodec {

    const val CONNECT = 0x10
    const val CONNACK = 0x20
    const val PUBLISH = 0x30
    const val SUBACK = 0x90
    const val PINGRESP = 0xD0

    /** Taille maximale acceptee en lecture (un CONNACK fait 4 octets). */
    const val MAX_PACKET = 8192

    /**
     * En-tete de longueur restante : 7 bits par octet, 1 a 4 octets
     * (specification MQTT 3.1.1, section 2.2.3).
     */
    fun remainingLength(length: Int): ByteArray {
        require(length in 0..268435455) { "longueur de paquet hors bornes : " + length }
        val tampon = ByteArray(4)
        var reste = length
        var index = 0
        do {
            var octet = reste % 128
            reste /= 128
            if (reste > 0) octet = octet or 0x80
            tampon[index] = octet.toByte()
            index++
        } while (reste > 0)
        return tampon.copyOf(index)
    }

    /** Chaine MQTT : longueur sur deux octets, puis UTF-8. */
    fun utf8(value: String): ByteArray {
        val octets = value.toByteArray(Charsets.UTF_8)
        require(octets.size <= 65535) { "chaine MQTT trop longue" }
        val sortie = ByteArray(octets.size + 2)
        sortie[0] = ((octets.size ushr 8) and 0xFF).toByte()
        sortie[1] = (octets.size and 0xFF).toByte()
        octets.copyInto(sortie, 2)
        return sortie
    }

    /** Paquet CONNECT (3.1.1, session propre, sans will). */
    fun connect(
        clientId: String,
        username: String? = null,
        password: String? = null,
        keepAliveS: Int = 60,
    ): ByteArray {
        val charge = ByteArrayOutputStream(96)
        charge.write(utf8("MQTT"))
        charge.write(4) // niveau de protocole 3.1.1
        var drapeaux = 0x02 // session propre
        if (username != null) drapeaux = drapeaux or 0x80
        if (password != null) drapeaux = drapeaux or 0x40
        charge.write(drapeaux)
        charge.write((keepAliveS ushr 8) and 0xFF)
        charge.write(keepAliveS and 0xFF)
        charge.write(utf8(clientId))
        username?.let { charge.write(utf8(it)) }
        password?.let { charge.write(utf8(it)) }
        return paquet(CONNECT, charge.toByteArray())
    }

    /** Paquet PUBLISH en QoS 0 (aucun identifiant de paquet, aucun accuse). */
    fun publish(topic: String, payload: ByteArray, retain: Boolean): ByteArray {
        require(topic.isNotBlank()) { "sujet MQTT vide" }
        val sujet = utf8(topic)
        val charge = ByteArray(sujet.size + payload.size)
        sujet.copyInto(charge)
        payload.copyInto(charge, sujet.size)
        val type = if (retain) PUBLISH or 0x01 else PUBLISH
        return paquet(type, charge)
    }

    fun pingreq(): ByteArray = byteArrayOf(0xC0.toByte(), 0x00)

    fun disconnect(): ByteArray = byteArrayOf(0xE0.toByte(), 0x00)

    /** Paquet lu sur le flux. */
    class Packet(val type: Int, val flags: Int, val payload: ByteArray)

    /**
     * Lit un paquet complet. Renvoie null si le flux est ferme avant le premier
     * octet (deconnexion propre du broker) ; leve une exception sinon.
     */
    fun readPacket(input: InputStream): Packet? {
        val entete = input.read()
        if (entete < 0) return null
        var longueur = 0
        var multiplicateur = 1
        var octetsLus = 0
        while (true) {
            val octet = input.read()
            if (octet < 0) throw EOFException("longueur de paquet interrompue")
            longueur += (octet and 0x7F) * multiplicateur
            if ((octet and 0x80) == 0) break
            multiplicateur *= 128
            octetsLus++
            if (octetsLus > 3) throw IllegalStateException("longueur de paquet invalide")
        }
        if (longueur > MAX_PACKET) throw IllegalStateException("paquet trop grand : " + longueur)
        val charge = ByteArray(longueur)
        var lus = 0
        while (lus < longueur) {
            val n = input.read(charge, lus, longueur - lus)
            if (n < 0) throw EOFException("paquet interrompu")
            lus += n
        }
        return Packet(entete and 0xF0, entete and 0x0F, charge)
    }

    /** Assemble un paquet a partir de son octet de type et de sa charge. */
    private fun paquet(type: Int, charge: ByteArray): ByteArray {
        val entete = remainingLength(charge.size)
        val sortie = ByteArray(1 + entete.size + charge.size)
        sortie[0] = type.toByte()
        entete.copyInto(sortie, 1)
        charge.copyInto(sortie, 1 + entete.size)
        return sortie
    }
}
