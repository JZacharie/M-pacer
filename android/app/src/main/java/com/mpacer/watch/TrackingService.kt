package com.mpacer.watch

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
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Seance en cours : service de premier plan + boucle GPS 1 Hz.
 *
 * Le service ne calcule rien : il pousse les positions dans [MpacerCore] et publie
 * l'etat renvoye par le moteur ([state]). La notification evite que le systeme tue
 * la seance et rappelle a l'utilisateur qu'un enregistrement est en cours.
 */
class TrackingService : Service() {

    private lateinit var core: MpacerCore
    private lateinit var locations: FusedLocationProviderClient
    private var startedAtMs: Long = 0L

    private val locationCallback = object : LocationCallback() {
        override fun onLocationResult(result: LocationResult) {
            result.lastLocation?.let(::onLocation)
        }
    }

    override fun onCreate() {
        super.onCreate()
        core = MpacerCore()
        locations = LocationServices.getFusedLocationProviderClient(this)
        createChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> start(armed = false)
            ACTION_ARM -> start(armed = true)
            ACTION_PAUSE -> publish(core.pause(now()))
            ACTION_RESUME -> publish(core.resume(now()))
            ACTION_STOP -> stopWorkout()
        }
        return START_STICKY
    }

    private fun start(armed: Boolean) {
        startedAtMs = System.currentTimeMillis()
        startForeground(NOTIFICATION_ID, notification("Preparation GPS..."))
        publish(if (armed) core.arm(now()) else core.start(now()))
        requestLocations()
    }

    private fun requestLocations() {
        val request = LocationRequest.Builder(Priority.PRIORITY_HIGH_ACCURACY, 1000L)
            .setMinUpdateIntervalMillis(1000L)
            .setMinUpdateDistanceMeters(0f)
            .build()
        try {
            locations.requestLocationUpdates(request, locationCallback, mainLooper)
        } catch (security: SecurityException) {
            // Permission revoquee en cours de seance : on arrete proprement.
            stopWorkout()
        }
    }

    private fun onLocation(location: Location) {
        val output = core.gps(
            tMs = location.time.takeIf { it > 0 } ?: System.currentTimeMillis(),
            lat = location.latitude,
            lon = location.longitude,
            accuracyM = location.accuracy.toDouble(),
            altitudeM = location.altitude.takeIf { location.hasAltitude() },
            speedMps = location.speed.takeIf { location.hasSpeed() }?.toDouble(),
        )
        publish(output, location)
    }

    private fun publish(output: EngineOutput, location: Location? = null) {
        _state.value = WatchState(output, location?.accuracy?.toDouble())
        // Les annonces vocales sont deja redigees par le moteur : le service ne
        // fait que les transmettre.
        output.messages.forEach(VoiceCoach::speak)
        updateNotification(output)
    }

    private fun stopWorkout() {
        val output = core.stop(now())
        publish(output)
        // Resume pret pour l'historique / l'export
        val summary = core.summary(startedAtMs)
        WorkoutArchive.save(this, summary)
        locations.removeLocationUpdates(locationCallback)
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        locations.removeLocationUpdates(locationCallback)
        core.close()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun now(): Long = System.currentTimeMillis()

    // ------------------------------------------------------------- notification

    private fun createChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, "Seance", NotificationManager.IMPORTANCE_LOW)
        )
    }

    private fun notification(text: String): Notification {
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE,
        )
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("M-pacer")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_menu_mylocation)
            .setOngoing(true)
            .setContentIntent(open)
            .build()
    }

    private fun updateNotification(output: EngineOutput) {
        val manager = getSystemService(NotificationManager::class.java)
        manager.notify(NOTIFICATION_ID, notification(MpacerFormat.summaryLine(output)))
    }

    companion object {
        const val ACTION_START = "com.mpacer.watch.START"
        const val ACTION_ARM = "com.mpacer.watch.ARM"
        const val ACTION_PAUSE = "com.mpacer.watch.PAUSE"
        const val ACTION_RESUME = "com.mpacer.watch.RESUME"
        const val ACTION_STOP = "com.mpacer.watch.STOP"

        private const val CHANNEL_ID = "mpacer.workout"
        private const val NOTIFICATION_ID = 42

        private val _state = MutableStateFlow(WatchState.disconnected())
        val state: StateFlow<WatchState> = _state.asStateFlow()

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

/** Etat expose a l'interface. */
data class WatchState(val output: EngineOutput?, val accuracyM: Double?) {
    companion object {
        fun disconnected() = WatchState(null, null)
    }
}
