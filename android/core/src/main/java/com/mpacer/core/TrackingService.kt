package com.mpacer.core

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.location.Location
import android.os.Build
import android.os.IBinder
import android.os.SystemClock
import androidx.core.app.NotificationCompat
import com.google.android.gms.location.FusedLocationProviderClient
import com.google.android.gms.location.LocationCallback
import com.google.android.gms.location.LocationRequest
import com.google.android.gms.location.LocationResult
import com.google.android.gms.location.LocationServices
import com.google.android.gms.location.Priority
import com.mpacer.core.live.LiveTracker
import com.mpacer.core.music.MusicDirective
import com.mpacer.core.music.MusicPlayer
import com.mpacer.core.music.MusicSession
import com.mpacer.core.music.MusicState
import com.mpacer.core.music.NowPlaying
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Seance en cours : service de premier plan + boucle GPS 1 Hz.
 *
 * Le service ne calcule rien : il pousse les positions dans [MpacerCore] et publie
 * l'etat renvoye par le moteur ([state]). La notification evite que le systeme tue
 * la seance et rappelle a l'utilisateur qu'un enregistrement est en cours ; elle
 * porte aussi Pause/Reprendre et Stop, ce qui evite de sortir le telephone de sa
 * ceinture pendant la course.
 *
 * Le meme service sert la montre et le telephone : aucune ligne ne depend de
 * l'ecran qui l'a lance.
 */
class TrackingService : Service() {

    private lateinit var core: MpacerCore
    private lateinit var locations: FusedLocationProviderClient
    private lateinit var heart: HeartRateSensor
    private var startedAtMs: Long = 0L

    private val locationCallback = object : LocationCallback() {
        override fun onLocationResult(result: LocationResult) {
            result.lastLocation?.let(::onLocation)
        }
    }

    override fun onCreate() {
        super.onCreate()
        core = MpacerCore()
        // Le moteur n'existe que pendant la seance : l'ecran Musique lui transmet
        // ses reglages par ce pont (voir MusicSession).
        MusicSession.attach(core)
        SessionConfig.attach(core)
        // Prepare le MediaController de la montre (lecture des fichiers locaux).
        MusicPlayer.ensure(this)
        locations = LocationServices.getFusedLocationProviderClient(this)
        // Le capteur cardiaque est branche des la creation ; il n'ecoute vraiment
        // qu'entre le depart et l'arret de la seance.
        heart = HeartRateSensor(this, ::onHeartRate)
        createChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // Redemarrage par le systeme sans commande (apres un arret brutal) : il
        // n'y a plus de seance a tenir. Un service de premier plan qui ne
        // republierait pas sa notification serait tue par Android au bout de
        // cinq secondes : on ferme proprement et on ne demande pas de relance.
        if (intent?.action == null) {
            stopSelf()
            return START_NOT_STICKY
        }
        when (intent.action) {
            ACTION_START -> start(armed = false)
            ACTION_ARM -> start(armed = true)
            ACTION_PAUSE -> {
                publish(core.pause(now()))
                // En pause, plus besoin d'une position par seconde : on baisse la
                // cadence (l'auto-reprise continue de fonctionner, en 3 s).
                requestLocations(precisionMaximale = false)
            }
            ACTION_RESUME -> {
                publish(core.resume(now()))
                requestLocations(precisionMaximale = true)
            }
            ACTION_STOP -> stopWorkout()
            // Memes commandes que les boutons du casque sur la montre : elles
            // n'existaient que dans le moteur, aucun ecran ne les declenchait.
            ACTION_ANNOUNCE -> publish(core.announceNow(now()))
            ACTION_RESET_PACE -> publish(core.resetPaceWindow(now()))
        }
        return START_STICKY
    }

    private fun start(armed: Boolean) {
        startedAtMs = System.currentTimeMillis()
        derniereNotification = "Preparation GPS..."
        derniereNotificationMs = System.currentTimeMillis()
        // Reglages et playlist choisis avant la course : le moteur les recoit
        // avant le premier tick pour publier une consigne des le depart.
        MusicSession.apply(core)
        SessionConfig.apply(core)
        startForeground(NOTIFICATION_ID, notification("Preparation GPS...", enPause = false))
        publish(if (armed) core.arm(now()) else core.start(now()))
        // Suivi en direct : sans broker configure, LiveTracker.start ne fait
        // strictement rien (aucun fil, aucune connexion).
        LiveTracker.start(this, if (armed) "arm" else "run")
        requestLocations()
        startHeartSensors()
    }

    /**
     * Capteurs de frequence cardiaque : le capteur integre de l'appareil (montre,
     * ou rare telephone qui en possede un) et la source supplementaire declaree par
     * l'application ([HeartRateSources.external], la ceinture Bluetooth du
     * telephone). Aucune des deux n'est obligatoire : sans cardio la seance reste
     * complete, il manque seulement les zones et la derive cardiaque.
     */
    private fun startHeartSensors() {
        heart.start()
        HeartRateSources.external?.start(::onHeartRate)
    }

    private fun stopHeartSensors() {
        heart.stop()
        HeartRateSources.external?.stop()
    }

    /**
     * Demande de position. Pleine precision (1 Hz) pendant l'effort, cadence reduite
     * (3 s, precision equilibree) en pause : le GPS reste le premier poste de
     * consommation de la montre.
     */
    private fun requestLocations(precisionMaximale: Boolean = true) {
        val intervalle = if (precisionMaximale) 1000L else 3000L
        val priorite = if (precisionMaximale) {
            Priority.PRIORITY_HIGH_ACCURACY
        } else {
            Priority.PRIORITY_BALANCED_POWER_ACCURACY
        }
        val request = LocationRequest.Builder(priorite, intervalle)
            .setMinUpdateIntervalMillis(intervalle)
            .setMinUpdateDistanceMeters(0f)
            .build()
        try {
            locations.removeLocationUpdates(locationCallback)
            locations.requestLocationUpdates(request, locationCallback, mainLooper)
        } catch (security: SecurityException) {
            // Permission revoquee en cours de seance : on arrete proprement.
            stopWorkout()
        }
    }

    private fun onLocation(location: Location) {
        val tMs = location.time.takeIf { it > 0 } ?: System.currentTimeMillis()
        val output = core.gps(
            tMs = tMs,
            lat = location.latitude,
            lon = location.longitude,
            accuracyM = location.accuracy.toDouble(),
            altitudeM = location.altitude.takeIf { location.hasAltitude() },
            speedMps = location.speed.takeIf { location.hasSpeed() }?.toDouble(),
        )
        // Suivi en direct : la cadence, le filtre de precision et l'etat publie
        // sont decides dans LiveTracker (docs/10). Ici, on ne fait que tendre les
        // valeurs deja calculees par le moteur.
        LiveTracker.position(
            tMs = tMs,
            lat = location.latitude,
            lon = location.longitude,
            accuracyM = location.accuracy.toDouble(),
            distanceM = output.distanceM,
            paceSPerKm = output.currentPace,
            heartRateBpm = output.heartRateBpm,
            engineState = output.state,
            // L'altitude accompagne la position : elle alimente le profil de
            // denivele de la page /live. Absente, elle n'est pas publiee.
            altitudeM = location.altitude.takeIf { location.hasAltitude() && it.isFinite() },
        )
        publish(output, location)
    }

    /**
     * Mesure cardiaque : elle part dans le moteur, qui la rattache a la seance.
     * Aucun calcul cote montre, comme pour le GPS.
     */
    private fun onHeartRate(tMs: Long, bpm: Int) {
        publish(core.heartRate(tMs, bpm))
    }

    private fun publish(output: EngineOutput, location: Location? = null) {
        _state.value = SessionState(output, location?.accuracy?.toDouble())
        // Les annonces vocales sont deja redigees par le moteur : le service ne
        // fait que les transmettre.
        output.messages.forEach(VoiceCoach::speak)
        // Le coeur decide (directive), la montre execute sur ses fichiers locaux.
        applyMusic(output.music)
        pushNowPlaying()
        updateNotification(output)
    }

    /**
     * Applique la directive du directeur d'orchestre (docs/07 section 4.3) sur la
     * seule source possible en v2 : les fichiers importes par USB.
     */
    private fun applyMusic(music: MusicState?) {
        if (music == null) return
        val playlist = MusicSession.local
        if (!music.enabled || playlist == null) {
            MusicPlayer.pauseIfPlaying()
            return
        }
        when (music.directive) {
            MusicDirective.PLAY -> {
                val courante = MusicPlayer.state.value.track
                val demandee = music.nextTrackId
                when {
                    courante == null -> MusicPlayer.play(playlist, demandee)
                    demandee != null && demandee != courante.id -> MusicPlayer.skipTo(demandee)
                }
            }
            MusicDirective.SKIP_TO -> music.nextTrackId?.let(MusicPlayer::skipTo)
            MusicDirective.PAUSE -> MusicPlayer.pause()
            MusicDirective.RESUME -> MusicPlayer.resume()
            else -> Unit
        }
    }

    /**
     * Remonte la piste en cours au moteur (commande `music_now_playing`, docs/07
     * section 5). Le resultat n'est pas republie : le tick suivant le porte.
     */
    private fun pushNowPlaying() {
        val track = MusicPlayer.state.value.track
        if (track == null) {
            if (dernierePistePoussee != null) {
                MusicSession.nowPlaying(null)
                dernierePistePoussee = null
            }
            return
        }
        MusicSession.nowPlaying(
            NowPlaying(
                trackId = track.id,
                title = track.title,
                artist = track.artist,
                bpm = track.bpm,
                positionS = MusicPlayer.currentPositionS(),
            )
        )
        dernierePistePoussee = track.id
    }

    private fun stopWorkout() {
        val output = core.stop(now())
        publish(output)
        // Dernier message retenu (etat "stop") puis fermeture de la liaison :
        // le suivi s'arrete avec la seance, il ne tourne pas en arriere-plan.
        LiveTracker.stop()
        // Resume pret pour l'historique / l'export
        val summary = core.summary(startedAtMs)
        WorkoutArchive.save(this, summary)
        locations.removeLocationUpdates(locationCallback)
        stopHeartSensors()
        // Envoi immediat de la seance terminee (le scope appartient au processus :
        // il n'est pas annule par le stopSelf() ci-dessous).
        SyncClient.syncInBackground(this)
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        LiveTracker.stop()
        locations.removeLocationUpdates(locationCallback)
        stopHeartSensors()
        MusicSession.detach(core)
        SessionConfig.detach(core)
        core.close()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    /** Dernier texte de notification publie, pour eviter les republications inutiles. */
    private var derniereNotification: String? = null
    private var derniereNotificationEnPause = false
    private var derniereNotificationMs = 0L

    /** Derniere piste remontee au moteur (evite d'envoyer un `null` a chaque tick). */
    private var dernierePistePoussee: String? = null

    private fun now(): Long = System.currentTimeMillis()

    // ------------------------------------------------------------- notification

    private fun createChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, "Seance", NotificationManager.IMPORTANCE_LOW)
        )
    }

    /**
     * Notification de seance : le contenu ouvre l'application, les deux actions
     * evitent d'avoir a la sortir de la poche ou de la ceinture pour mettre en
     * pause ou arreter.
     *
     * L'activite a ouvrir est celle que le systeme associe au paquet : le socle ne
     * connait aucune classe d'ecran, il sert la montre comme le telephone.
     */
    private fun notification(text: String, enPause: Boolean): Notification {
        val open = packageManager.getLaunchIntentForPackage(packageName)?.let { intention ->
            PendingIntent.getActivity(
                this,
                0,
                intention,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
        }
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("M-pacer")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_menu_mylocation)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(open)
            .addAction(0, if (enPause) "Reprendre" else "Pause", action(if (enPause) ACTION_RESUME else ACTION_PAUSE))
            .addAction(0, "Stop", action(ACTION_STOP))
            .build()
    }

    /** Action de notification : le service s'envoie la commande a lui-meme. */
    private fun action(name: String): PendingIntent = PendingIntent.getService(
        this,
        name.hashCode(),
        Intent(this, TrackingService::class.java).setAction(name),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    /**
     * Republier une notification a chaque position coute cher (aller-retour vers
     * system_server, bandeau retravaille). On ne la met a jour que si le texte change,
     * et au plus une fois toutes les 5 secondes.
     */
    private fun updateNotification(output: EngineOutput) {
        val texte = MpacerFormat.summaryLine(output)
        val enPause = output.isPaused
        val maintenant = System.currentTimeMillis()
        if (texte == derniereNotification && enPause == derniereNotificationEnPause) return
        if (maintenant - derniereNotificationMs < INTERVALLE_NOTIFICATION_MS) return
        derniereNotification = texte
        derniereNotificationEnPause = enPause
        derniereNotificationMs = maintenant
        getSystemService(NotificationManager::class.java).notify(NOTIFICATION_ID, notification(texte, enPause))
    }

    companion object {
        const val ACTION_START = "com.mpacer.core.START"
        const val ACTION_ARM = "com.mpacer.core.ARM"
        const val ACTION_PAUSE = "com.mpacer.core.PAUSE"
        const val ACTION_RESUME = "com.mpacer.core.RESUME"
        const val ACTION_STOP = "com.mpacer.core.STOP"
        const val ACTION_ANNOUNCE = "com.mpacer.core.ANNOUNCE"
        const val ACTION_RESET_PACE = "com.mpacer.core.RESET_PACE"

        private const val CHANNEL_ID = "mpacer.workout"
        private const val NOTIFICATION_ID = 42

        /** Cadence minimale de republication de la notification. */
        private const val INTERVALLE_NOTIFICATION_MS = 5000L

        private val _state = MutableStateFlow(SessionState.disconnected())
        val state: StateFlow<SessionState> = _state.asStateFlow()

        fun send(context: Context, action: String) {
            val intent = Intent(context, TrackingService::class.java).setAction(action)
            if (action == ACTION_START || action == ACTION_ARM) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }
    }
}

/** Etat de la seance expose a l'interface, montre comme telephone. */
data class SessionState(val output: EngineOutput?, val accuracyM: Double?) {
    companion object {
        fun disconnected() = SessionState(null, null)
    }
}
