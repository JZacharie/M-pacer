package com.mpacer.core.live

/**
 * Reglages du suivi en direct (MQTT) pendant une seance.
 *
 * Sans adresse de broker, [configured] est faux : la montre n'ouvre aucune
 * connexion, ne cree aucun fil et ne consomme donc rien de plus qu'avant.
 * Publier en direct est une fonctionnalite opt-in.
 */
data class LiveConfig(
    val enabled: Boolean = true,
    /** Adresse du broker : "mqtt://hote:1883" ou "mqtts://hote:8883". */
    val url: String = "",
    /** Prefixe des sujets ; la montre publie sur <prefixe>/live/<montre>. */
    val topicPrefix: String = "mpacer",
    /** Nom de la montre dans le sujet (vide = identifiant local). */
    val device: String = "",
    val username: String = "",
    val password: String = "",
    /** Periode de publication en course (s). */
    val intervalS: Int = 10,
    /** Periode de publication en pause (s) : la montre ne bouge plus. */
    val pausedIntervalS: Int = 60,
    /** Precision GPS au-dela de laquelle un point n'est pas publie (m). */
    val minAccuracyM: Double = 50.0,
    /**
     * Conserver le dernier message sur le broker : un proche qui ouvre le suivi
     * apres le depart voit immediatement la position courante.
     */
    val retain: Boolean = true,
    /**
     * Taille maximale de la file d'attente (positions non encore ecrites).
     *
     * Vingt minutes a la cadence de course (10 s) : c'est la duree pendant
     * laquelle une coupure reseau reste rattrapable pour les suiveurs. Au-dela,
     * la file compresse l'historique (un point sur deux) au lieu de le perdre.
     */
    val maxQueue: Int = MAX_QUEUE_POINTS,
) {
    /** Vrai si le suivi en direct est reellement actif. */
    val configured: Boolean get() = enabled && url.isNotBlank()

    /**
     * Nom d'appareil effectivement publie : il apparait dans le sujet MQTT et
     * c'est lui que l'application revendique aupres du backend pour le partage
     * entre amis (voir [com.mpacer.core.social.FriendsClient]). Une seule
     * definition, donc un sujet et une revendication toujours d'accord.
     */
    fun deviceName(deviceId: String): String =
        device.trim().replace(' ', '-').ifBlank { deviceId }

    /** Sujet publie par l'appareil. */
    fun topic(deviceId: String): String {
        val prefixe = topicPrefix.trim().trimEnd('/').ifBlank { "mpacer" }
        return prefixe + "/live/" + deviceName(deviceId)
    }

    /** Periode effective, pause comprise, bornes appliquees. */
    fun intervalMs(paused: Boolean): Long =
        (if (paused) pausedIntervalS else intervalS)
            .coerceIn(MIN_INTERVAL_S, MAX_INTERVAL_S) * 1000L

    companion object {
        /**
         * Bornes de la cadence : en dessous de 5 s le GPS n'apporte rien de plus,
         * au dela de 5 min le suivi n'est plus « en direct ».
         */
        const val MIN_INTERVAL_S = 5
        const val MAX_INTERVAL_S = 300

        /** File d'attente par defaut : 20 minutes a la cadence de course. */
        const val MAX_QUEUE_POINTS = 120

        val DEFAULT = LiveConfig()

        /** Periodes proposees sur l'ecran de reglages. */
        val INTERVALS = listOf(5, 10, 30, 60)

        /** Normalise une saisie utilisateur. */
        fun normalise(config: LiveConfig): LiveConfig = config.copy(
            url = config.url.trim(),
            username = config.username.trim(),
            password = config.password.trim(),
            intervalS = config.intervalS.coerceIn(MIN_INTERVAL_S, MAX_INTERVAL_S),
            pausedIntervalS = config.pausedIntervalS.coerceIn(MIN_INTERVAL_S, MAX_INTERVAL_S),
            minAccuracyM = config.minAccuracyM.coerceIn(5.0, 500.0),
            maxQueue = config.maxQueue.coerceIn(2, 128),
        )
    }
}

/**
 * Adresse de broker analysee : hote, port, TLS et identifiants eventuels.
 *
 * Le couple utilisateur/mot de passe peut venir de l'URL
 * (`mqtt://joseph:secret@192.168.0.115:1883`) : c'est le moyen le plus simple de
 * configurer la montre depuis `adb`, sans clavier. Les reglages explicites
 * (LiveConfig.username / password) restent prioritaires.
 */
internal data class BrokerAddress(
    val host: String,
    val port: Int,
    val tls: Boolean,
    val username: String? = null,
    val password: String? = null,
) {

    /**
     * Identifiants effectifs : les reglages explicites de la montre priment sur
     * ceux trouves dans l'URL. On peut ainsi coller une URL complete
     * (`mqtt://joseph:secret@hote:1883`) et corriger le seul mot de passe sans
     * retaper l'adresse.
     */
    fun credentials(config: LiveConfig): Pair<String?, String?> = Pair(
        config.username.takeIf { it.isNotBlank() } ?: username,
        config.password.takeIf { it.isNotBlank() } ?: password,
    )

    companion object {
        /**
         * Analyse "mqtt://hote", "mqtts://hote:8883" ou "hote:1883".
         * Renvoie null si l'adresse n'est pas exploitable (aucun plantage).
         */
        fun parse(url: String): BrokerAddress? {
            val brut = url.trim()
            if (brut.isEmpty()) return null
            val tls: Boolean
            val reste: String
            val separateur = brut.indexOf("://")
            if (separateur > 0) {
                when (brut.substring(0, separateur).lowercase()) {
                    "mqtt", "tcp" -> tls = false
                    "mqtts", "ssl", "tls" -> tls = true
                    else -> return null
                }
                reste = brut.substring(separateur + 3)
            } else {
                tls = false
                reste = brut
            }
            // Identifiants eventuels dans l'URL (le mot de passe peut contenir
            // un '@' encode en %40 : on coupe au dernier '@').
            val identifiants = if (reste.contains('@')) reste.substringBeforeLast('@') else ""
            val apresIdentifiants = reste.substringAfterLast('@', reste).trimEnd('/')
            if (apresIdentifiants.isEmpty()) return null
            val deuxPoints = apresIdentifiants.lastIndexOf(':')
            val host: String
            var port = if (tls) 8883 else 1883
            if (deuxPoints > 0) {
                host = apresIdentifiants.substring(0, deuxPoints)
                val textePort = apresIdentifiants.substring(deuxPoints + 1)
                port = textePort.toIntOrNull() ?: return null
                if (port !in 1..65535) return null
            } else {
                host = apresIdentifiants
            }
            if (host.isBlank()) return null
            val utilisateur: String?
            val motDePasse: String?
            if (identifiants.isEmpty()) {
                utilisateur = null
                motDePasse = null
            } else {
                val coupe = identifiants.indexOf(':')
                if (coupe < 0) {
                    utilisateur = percentDecode(identifiants).takeIf { it.isNotBlank() }
                    motDePasse = null
                } else {
                    utilisateur = percentDecode(identifiants.substring(0, coupe)).takeIf { it.isNotBlank() }
                    motDePasse = percentDecode(identifiants.substring(coupe + 1)).takeIf { it.isNotEmpty() }
                }
            }
            return BrokerAddress(host, port, tls, utilisateur, motDePasse)
        }
    }
}
/** Decodage des caracteres encodes dans une URL (RFC 3986). */
private fun percentDecode(value: String): String {
    if (!value.contains('%')) return value
    val octets = java.io.ByteArrayOutputStream(value.length)
    var index = 0
    while (index < value.length) {
        val caractere = value[index]
        if (caractere == '%' && index + 2 < value.length) {
            val octet = value.substring(index + 1, index + 3).toIntOrNull(16)
            if (octet != null) {
                octets.write(octet)
                index += 3
                continue
            }
        }
        octets.write(caractere.code and 0xFF)
        index++
    }
    return String(octets.toByteArray(), Charsets.UTF_8)
}
