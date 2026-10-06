package com.mpacer.core.live

import java.util.Locale

/**
 * Charge utile publiee sur le broker : un JSON compact a champs courts.
 *
 * Un point pese 116 octets (136 avec le nom de la montre) : a une publication
 * toutes les dix secondes, cela represente moins de 70 ko par heure, en-tetes
 * IP, TCP et MQTT compris. Les
 * nombres sont formates avec la locale ROOT (jamais de virgule decimale, qui
 * rendrait le JSON invalide sur une montre configuree en francais).
 */
internal object LivePayload {

    /** Longueur maximale d'un nom de montre dans la charge utile. */
    private const val MAX_DEVICE = 24

    fun encode(
        tMs: Long,
        lat: Double,
        lon: Double,
        accuracyM: Double?,
        distanceM: Double?,
        paceSPerKm: Double?,
        heartRateBpm: Int?,
        batteryPercent: Int?,
        state: String,
        device: String,
    ): ByteArray {
        val json = StringBuilder(160)
        json.append("{\"t\":").append(tMs)
        // 6 decimales : environ 0,1 m, bien au-dela de ce qu'un suivi demande.
        json.append(",\"lat\":").append(decimal(lat, 6))
        json.append(",\"lon\":").append(decimal(lon, 6))
        accuracyM?.let { json.append(",\"acc\":").append(decimal(it, 1)) }
        distanceM?.let { json.append(",\"dist\":").append(decimal(it, 1)) }
        paceSPerKm?.let { json.append(",\"pace\":").append(decimal(it, 1)) }
        heartRateBpm?.let { json.append(",\"hr\":").append(it) }
        batteryPercent?.let { json.append(",\"bat\":").append(it) }
        json.append(",\"st\":\"").append(safe(state, 8)).append('"')
        if (device.isNotBlank()) {
            json.append(",\"dev\":\"").append(safe(device, MAX_DEVICE)).append('"')
        }
        json.append('}')
        return json.toString().toByteArray(Charsets.UTF_8)
    }

    /** Nombre decimal a point, quelle que soit la locale de la montre. */
    private fun decimal(value: Double, decimals: Int): String =
        if (!value.isFinite()) "0" else String.format(Locale.ROOT, "%." + decimals + "f", value)

    /** Texte sur : ni guillemet, ni antislash, ni caractere de controle. */
    private fun safe(value: String, maxLength: Int): String =
        value.filter { caractere ->
            caractere.isLetterOrDigit() || caractere == '-' || caractere == '_' || caractere == '.'
        }.take(maxLength)
}
