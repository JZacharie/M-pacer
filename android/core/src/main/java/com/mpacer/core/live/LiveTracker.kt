package com.mpacer.core.live

import android.content.Context
import android.os.BatteryManager
import android.os.Process
import android.os.SystemClock
import android.util.Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import java.io.IOException
import java.io.OutputStream
import java.net.InetSocketAddress
import java.net.Socket
import javax.net.ssl.SSLSocket
import javax.net.ssl.SSLSocketFactory

/**
 * Publication de la position de la montre sur un broker MQTT pendant la seance.
 *
 * Le cout est borne par construction :
 *  * une seule publication par periode (10 s par defaut, 60 s en pause) : le GPS
 *    continue a 1 Hz, la politique ne retient qu'un point ;
 *  * aucun point publie si la precision GPS est mauvaise (50 m par defaut) ;
 *  * un seul fil, en priorite basse, cree au depart de la seance et detruit a
 *    l'arrivee ; aucun reveil, aucun verrou de sommeil, aucune minuterie ;
 *  * une file d'attente bornee (20 minutes, 120 points) : si le reseau tombe,
 *    la montre garde la trace et la republie en rafale a la reconnexion, en la
 *    compressant (un point sur deux) au lieu de la jeter a l'aveugle ;
 *  * QoS 0 et charge utile de 116 octets (132 avec l'altitude) : moins de 70 ko
 *    par heure, en-tetes compris.
 *
 * Sans adresse de broker ([LiveConfig.configured] faux), [start] ne fait rien :
 * le suivi en direct ne coute rien tant qu'il n'est pas configure.
 */
object LiveTracker {

    private const val TAG = "LiveTracker"
    private const val KEEP_ALIVE_S = 60
    private const val PING_MS = KEEP_ALIVE_S * 1000L / 2
    private const val CONNECT_TIMEOUT_MS = 8000
    private const val READ_TIMEOUT_MS = 15000
    private const val ATTENTE_MIN_MS = 2000L
    private const val ATTENTE_MAX_MS = 60000L
    private const val ATTENTE_FILE_MS = 1000L
    private const val VIDAGE_MAX_MS = 1500L
    /** Espacement des ecritures pendant la decharge d'un retard (ms). */
    private const val RAFALE_MS = 50L

    private val _state = MutableStateFlow(LiveState())
    /** Etat observe par l'ecran de reglages. */
    val state: StateFlow<LiveState> = _state.asStateFlow()

    private val verrou = Object()
    /**
     * Positions en attente de publication. Remplacee au depart de chaque seance
     * (taille reglee), puis accedee sous [verrou] uniquement.
     */
    private var file = LiveQueue()

    @Volatile private var actif = false
    private var fil: Thread? = null
    private var config = LiveConfig.DEFAULT
    private var clientId = "mpacer"
    private var sujet = ""
    private var etat = "run"
    private var enPause = false
    private var dernierMs = 0L
    private var derniere: Position? = null
    private var socket: Socket? = null
    private var sortie: OutputStream? = null
    private var contexte: Context? = null
    private var envoyes = 0L

    /** Derniere position connue, pour republier un changement d'etat. */
    private class Position(
        val tMs: Long,
        val lat: Double,
        val lon: Double,
        val accuracyM: Double?,
        val altitudeM: Double?,
        val distanceM: Double?,
        val paceSPerKm: Double?,
        val heartRateBpm: Int?,
        val batteryPercent: Int?,
    )

    /**
     * Demarre le suivi pour une seance.
     *
     * @param etatInitial etat de la seance ("arm" au demarrage suspendu, "run").
     */
    fun start(context: Context, etatInitial: String = "run") {
        arreter()
        val application = context.applicationContext
        contexte = application
        config = LiveSettings.load(application)
        if (!config.configured) {
            _state.value = LiveState(enabled = false, url = config.url)
            return
        }
        val deviceId = LiveSettings.deviceId(application)
        clientId = "mpacer-" + deviceId + "-" + (SystemClock.elapsedRealtime() % 1000000L)
        sujet = config.topic(deviceId)
        // Partage entre amis : le backend doit savoir a qui appartient ce nom
        // d'appareil avant de montrer quoi que ce soit. L'appel est fait au
        // mieux : sans jeton ou sans reseau, la seance continue normalement.
        com.mpacer.core.social.FriendsClient.registerDevice(
            application,
            config.deviceName(deviceId),
            label = android.os.Build.MODEL,
        )
        // Parcours planifie : publie une fois, au depart. Les suiveurs voient
        // alors le trace prevu et le pourcentage deja couvert, pas seulement
        // le point courant.
        PlannedRouteStore.load(application)?.let { route ->
            com.mpacer.core.social.FriendsClient.publishRoute(
                application,
                config.deviceName(deviceId),
                route.points,
            )
        }
        etat = etatInitial
        enPause = false
        dernierMs = 0L
        derniere = null
        envoyes = 0L
        synchronized(verrou) {
            // La file reprend la taille reglee : elle nait avec la seance et
            // repart d'un historique vide.
            file = LiveQueue(config.maxQueue)
        }
        _state.value = LiveState(enabled = true, url = config.url, topic = sujet)
        actif = true
        fil = Thread(Runnable { boucle() }, "mpacer-live").apply {
            isDaemon = true
            start()
        }
    }

    /** Un echantillon GPS (1 Hz) : la politique decide s'il part sur le broker. */
    fun position(
        tMs: Long,
        lat: Double,
        lon: Double,
        accuracyM: Double?,
        distanceM: Double?,
        paceSPerKm: Double?,
        heartRateBpm: Int?,
        engineState: String?,
        altitudeM: Double? = null,
    ) {
        if (!actif) return
        synchroniserEtat(engineState)
        val maintenant = SystemClock.elapsedRealtime()
        val decision = LivePolicy.decide(maintenant, dernierMs, enPause, accuracyM, config)
        if (decision != LivePolicy.Decision.Publish) return
        val point = Position(
            tMs, lat, lon, accuracyM, altitudeM, distanceM, paceSPerKm, heartRateBpm, batteryPercent(),
        )
        derniere = point
        dernierMs = maintenant
        publier(point, etat)
    }

    /**
     * Aligne l'etat publie sur celui du moteur (Running, Paused, AutoPaused...).
     *
     * C'est ce qui donne au suivi en direct son sens : un changement d'etat part
     * tout de suite, et la cadence passe a la periode longue pendant une pause
     * (auto-pause comprise) sans que le service ait a s'en occuper.
     */
    private fun synchroniserEtat(moteur: String?) {
        val nouveau = when (moteur) {
            "Paused", "AutoPaused" -> "pause"
            "Armed" -> "arm"
            "Stopped" -> "stop"
            else -> "run"
        }
        enPause = nouveau == "pause"
        if (LivePolicy.etatChange(etat, nouveau)) {
            etat = nouveau
            publierEtat(nouveau)
        }
    }

    /** Arret de la seance : dernier message retenu, puis fermeture propre. */
    fun stop() {
        if (!actif) return
        // La seance est finie : la position courante suffit, inutile de rejouer
        // vingt minutes d'historique pour un dernier message.
        synchronized(verrou) { file.keepNewest() }
        publierEtat("stop")
        if (_state.value.connected) drainer(VIDAGE_MAX_MS)
        arreter()
        _state.update { it.copy(enabled = false, connected = false) }
    }

    /** Relit les reglages (ecran Reglages) ; s'applique a la prochaine seance. */
    fun refresh(context: Context) {
        val application = context.applicationContext
        contexte = application
        config = LiveSettings.load(application)
        if (!actif) {
            _state.value = LiveState(
                enabled = config.configured,
                url = config.url,
                topic = config.topic(LiveSettings.deviceId(application)),
            )
        }
    }

    // ------------------------------------------------------------- fil de fond

    private fun arreter() {
        actif = false
        synchronized(verrou) { verrou.notifyAll() }
        fil?.join(500)
        fil = null
        fermer()
    }

    private fun boucle() {
        Process.setThreadPriority(Process.THREAD_PRIORITY_BACKGROUND)
        var attente = ATTENTE_MIN_MS
        while (actif) {
            if (!connecter()) {
                attendre(attente)
                attente = (attente * 2).coerceAtMost(ATTENTE_MAX_MS)
                continue
            }
            attente = ATTENTE_MIN_MS
            _state.update { it.copy(connected = true, error = null) }
            var dernierPing = SystemClock.elapsedRealtime()
            while (actif && socket != null) {
                val paquet = retirer(ATTENTE_FILE_MS)
                if (paquet != null) {
                    if (!ecrire(paquet.packet)) break
                    // Decharge en rafale : le retard accumule part d'un coup,
                    // espace de [RAFALE_MS] pour ne pas saturer la socket.
                    if (aDesPaquets()) attendre(RAFALE_MS)
                } else if (SystemClock.elapsedRealtime() - dernierPing >= PING_MS) {
                    // La liaison est gardee ouverte par un PINGREQ toutes les 30 s :
                    // beaucoup moins couteux que de se reconnecter a chaque point.
                    if (!ecrire(MqttCodec.pingreq())) break
                    dernierPing = SystemClock.elapsedRealtime()
                }
            }
            _state.update { it.copy(connected = false) }
            fermer()
            // La file n'est pas videe ici : c'est tout l'interet du suivi
            // resilient. Les positions de la coupure repartiront en rafale des
            // que le broker repondra, la file restant bornee par sa fenetre.
            _state.update { it.copy(dropped = file.dropped.toLong()) }
        }
    }

    private fun retirer(attenteMs: Long): LiveQueue.Entry? = synchronized(verrou) {
        if (file.isEmpty && actif) {
            try {
                verrou.wait(attenteMs)
            } catch (interrompu: InterruptedException) {
                Thread.currentThread().interrupt()
                return null
            }
        }
        if (!actif) null else file.removeFirst()
    }

    /** Reste-t-il un retard a decharger ? (espacement de la rafale) */
    private fun aDesPaquets(): Boolean = synchronized(verrou) { !file.isEmpty }

    private fun ecrire(paquet: ByteArray): Boolean {
        val flux = sortie ?: return false
        return try {
            flux.write(paquet)
            flux.flush()
            if ((paquet[0].toInt() and 0xF0) == MqttCodec.PUBLISH) {
                envoyes++
                _state.update {
                    it.copy(sent = envoyes, lastPublishMs = SystemClock.elapsedRealtime())
                }
            }
            true
        } catch (erreur: IOException) {
            Log.w(TAG, "ecriture MQTT impossible : " + erreur.message)
            _state.update { it.copy(error = erreur.message ?: "ecriture impossible") }
            false
        }
    }

    private fun connecter(): Boolean {
        val adresse = BrokerAddress.parse(config.url)
        if (adresse == null) {
            _state.update { it.copy(error = "adresse MQTT invalide") }
            return false
        }
        return try {
            val brut = Socket()
            brut.tcpNoDelay = true
            brut.connect(InetSocketAddress(adresse.host, adresse.port), CONNECT_TIMEOUT_MS)
            val flux: Socket = if (adresse.tls) {
                // La fabrique par defaut verifie le certificat du serveur et le
                // nom d'hote ; aucune confiance aveugle n'est accordee.
                val securise = (SSLSocketFactory.getDefault() as SSLSocketFactory)
                    .createSocket(brut, adresse.host, adresse.port, true) as SSLSocket
                securise.startHandshake()
                securise
            } else {
                brut
            }
            flux.soTimeout = READ_TIMEOUT_MS
            val sortieFlux = flux.getOutputStream()
            val identifiants = adresse.credentials(config)
            sortieFlux.write(
                MqttCodec.connect(clientId, identifiants.first, identifiants.second, KEEP_ALIVE_S)
            )
            sortieFlux.flush()
            val reponse = MqttCodec.readPacket(flux.getInputStream())
            if (reponse == null || reponse.type != MqttCodec.CONNACK) {
                throw IOException("CONNACK attendu")
            }
            val code = reponse.payload.lastOrNull()?.toInt()?.and(0xFF) ?: 0xFF
            if (code != 0) throw IOException("connexion refusee par le broker (code " + code + ")")
            socket = flux
            sortie = sortieFlux
            true
        } catch (erreur: Exception) {
            Log.w(TAG, "connexion MQTT impossible : " + (erreur.message ?: erreur.javaClass.simpleName))
            _state.update { it.copy(error = erreur.message ?: erreur.javaClass.simpleName) }
            fermer()
            false
        }
    }

    private fun fermer() {
        val flux = socket
        val sortieFlux = sortie
        socket = null
        sortie = null
        try {
            sortieFlux?.write(MqttCodec.disconnect())
            sortieFlux?.flush()
        } catch (_: IOException) {
            // La socket est deja perdue : rien a signaler.
        }
        try {
            flux?.close()
        } catch (_: IOException) {
        }
    }

    // ---------------------------------------------------------------- charge

    private fun publierEtat(nouveau: String) {
        if (!actif) return
        val point = derniere ?: return
        dernierMs = SystemClock.elapsedRealtime()
        publier(point, nouveau)
    }

    private fun publier(point: Position, etatPublie: String) {
        val charge = LivePayload.encode(
            point.tMs,
            point.lat,
            point.lon,
            point.accuracyM,
            point.distanceM,
            point.paceSPerKm,
            point.heartRateBpm,
            point.batteryPercent,
            etatPublie,
            config.device,
            point.altitudeM,
        )
        val paquet = MqttCodec.publish(sujet, charge, config.retain)
        synchronized(verrou) {
            file.add(LiveQueue.Entry(point.tMs, paquet))
            verrou.notifyAll()
            // La file borne elle-meme sa taille : le compte des points
            // abandonnes remonte tel quel a l'ecran de reglages.
            _state.update { it.copy(dropped = file.dropped.toLong()) }
        }
    }

    private fun batteryPercent(): Int? {
        val application = contexte ?: return null
        return try {
            val gestionnaire = application.getSystemService(Context.BATTERY_SERVICE) as? BatteryManager
                ?: return null
            gestionnaire.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY).takeIf { it in 0..100 }
        } catch (_: Exception) {
            null
        }
    }

    private fun attendre(ms: Long) {
        try {
            Thread.sleep(ms)
        } catch (interrompu: InterruptedException) {
            Thread.currentThread().interrupt()
        }
    }

    /** Attend que la file se vide (bornee : jamais plus de [maxMs]). */
    private fun drainer(maxMs: Long) {
        val fin = SystemClock.elapsedRealtime() + maxMs
        while (SystemClock.elapsedRealtime() < fin) {
            val vide = synchronized(verrou) { file.isEmpty }
            if (vide) return
            attendre(50)
        }
    }
}

/** Etat du suivi en direct, observe par l'ecran de reglages. */
data class LiveState(
    val enabled: Boolean = false,
    val connected: Boolean = false,
    val url: String = "",
    val topic: String = "",
    val sent: Long = 0,
    val dropped: Long = 0,
    val error: String? = null,
    val lastPublishMs: Long = 0,
) {
    /** Resume d'une ligne pour l'ecran de reglages. */
    val resume: String
        get() = when {
            !enabled -> "Suivi en direct : inactif"
            connected -> "En direct : " + sent + " point(s) publie(s)"
            error != null -> "En attente : " + error
            else -> "Connexion au broker..."
        }
}
