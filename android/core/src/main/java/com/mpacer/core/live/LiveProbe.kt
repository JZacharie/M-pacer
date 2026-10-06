package com.mpacer.core.live

import android.util.Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.io.IOException
import java.net.InetSocketAddress
import java.net.Socket
import javax.net.ssl.SSLSocket
import javax.net.ssl.SSLSocketFactory

/**
 * Test de la configuration MQTT depuis l'ecran Reglages de la montre.
 *
 * Saisir une adresse de broker sur un ecran rond est error-prone : le bouton
 * « Tester la connexion » ouvre une connexion avec exactement les reglages
 * affiches (brouillon non enregistre), verifie le CONNACK, publie un point de
 * test **hors du sujet de suivi** puis se deconnecte.
 *
 * Le sujet de test est `<prefixe>/test/<montre>` : le backend s'abonne a
 * `<prefixe>/live/+` (docs/10), la page /live n'est donc jamais polluee par un
 * essai de reglages. Rien n'est retenu (retain = false) : le broker reste propre.
 *
 * Un seul test a la fois ; le fil est un fil de fond, jamais un wake lock.
 */
object LiveProbe {

    private const val TAG = "LiveProbe"
    private const val CONNECT_TIMEOUT_MS = 8000
    private const val READ_TIMEOUT_MS = 8000
    private const val KEEP_ALIVE_S = 15
    private const val CHARGE_TEST = "{\"probe\":true}"

    private val _state = MutableStateFlow(ProbeState())
    /** Etat du dernier test, observe par l'ecran des reglages. */
    val state: StateFlow<ProbeState> = _state.asStateFlow()

    @Volatile private var enCours = false

    /**
     * Teste [config] sans l'enregistrer : adresse, identifiants et TLS
     * exactement tels qu'ils sont affiches.
     */
    fun test(config: LiveConfig) {
        if (enCours) return
        val propre = LiveConfig.normalise(config)
        val adresse = BrokerAddress.parse(propre.url)
        if (adresse == null) {
            _state.value = ProbeState(
                message = if (propre.url.isBlank()) {
                    "Adresse du broker vide"
                } else {
                    "Adresse illisible : " + propre.url
                },
            )
            return
        }
        enCours = true
        _state.value = ProbeState(
            running = true,
            message = "Connexion a " + adresse.host + ":" + adresse.port + "...",
        )
        Thread({ executer(propre, adresse) }, "mpacer-live-probe").apply {
            isDaemon = true
            start()
        }
    }

    /** Sujet de test : hors de `<prefixe>/live/+`, donc invisible sur /live. */
    internal fun testTopic(config: LiveConfig): String {
        val prefixe = config.topicPrefix.trim().trimEnd('/').ifBlank { "mpacer" }
        val montre = config.device.trim().replace(' ', '-').ifBlank { "probe" }
        return prefixe + "/test/" + montre
    }

    private fun executer(config: LiveConfig, adresse: BrokerAddress) {
        var socket: Socket? = null
        try {
            val identifiants = adresse.credentials(config)
            val brut = Socket()
            brut.tcpNoDelay = true
            brut.connect(InetSocketAddress(adresse.host, adresse.port), CONNECT_TIMEOUT_MS)
            val flux: Socket = if (adresse.tls) {
                // Meme politique que LiveTracker : certificat du serveur et nom
                // d'hote verifies, aucune confiance aveugle.
                val securise = (SSLSocketFactory.getDefault() as SSLSocketFactory)
                    .createSocket(brut, adresse.host, adresse.port, true) as SSLSocket
                securise.startHandshake()
                securise
            } else {
                brut
            }
            socket = flux
            flux.soTimeout = READ_TIMEOUT_MS
            val sortie = flux.getOutputStream()
            sortie.write(
                MqttCodec.connect(
                    "mpacer-probe-" + (System.currentTimeMillis() % 1_000_000L),
                    identifiants.first,
                    identifiants.second,
                    KEEP_ALIVE_S,
                )
            )
            sortie.flush()
            val reponse = MqttCodec.readPacket(flux.getInputStream())
                ?: throw IOException("aucune reponse du broker")
            if (reponse.type != MqttCodec.CONNACK) {
                throw IOException("reponse inattendue du broker")
            }
            val code = reponse.payload.lastOrNull()?.toInt()?.and(0xFF) ?: 0xFF
            if (code != 0) throw IOException(refus(code))
            sortie.write(MqttCodec.publish(testTopic(config), CHARGE_TEST.toByteArray(Charsets.UTF_8), false))
            sortie.flush()
            _state.value = ProbeState(
                message = "Broker joignable et authentifie (" + adresse.host + ":" + adresse.port + ")",
                ok = true,
            )
        } catch (erreur: Exception) {
            Log.w(TAG, "test MQTT impossible : " + erreur.message)
            _state.value = ProbeState(
                message = "Echec : " + (erreur.message ?: erreur.javaClass.simpleName),
            )
        } finally {
            try {
                socket?.getOutputStream()?.write(MqttCodec.disconnect())
            } catch (_: Exception) {
                // La socket est deja perdue : rien a signaler.
            }
            try {
                socket?.close()
            } catch (_: Exception) {
            }
            enCours = false
        }
    }

    /** Meme lecture des codes de refus que le broker MQTT 3.1.1. */
    private fun refus(code: Int): String = when (code) {
        1 -> "version MQTT refusee"
        2 -> "identifiant client refuse"
        3 -> "broker indisponible"
        4 -> "identifiant ou mot de passe refuse"
        5 -> "authentification refusee"
        else -> "connexion refusee par le broker (code " + code + ")"
    }
}

/** Resultat du dernier test de connexion. */
data class ProbeState(
    val running: Boolean = false,
    val ok: Boolean = false,
    val message: String = "Aucun test : le bouton verifie l'adresse et les identifiants.",
)
