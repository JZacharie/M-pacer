package com.mpacer.core.music

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * Service de premier plan du telechargement des MP3 (docs/16).
 *
 * [MusicDownloader] fait le travail ; ce service lui donne le droit de survivre a
 * la fermeture de l'ecran Musique et d'etre interrompu proprement. Type
 * dataSync : Android 14 exige une raison declaree pour un service de premier
 * plan, et c'est exactement la notre (recuperer des fichiers).
 *
 * Le service s'arrete de lui-meme quand le telechargement est fini ; relancer
 * l'ecran Musique relance les fichiers qui restent.
 */
class MusicDownloadService : Service() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var travail: Job? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        creerCanal()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // Un seul telechargement a la fois : le bouton et la reprise se croisent.
        if (travail?.isActive == true) return START_NOT_STICKY
        val playlistId = intent?.getStringExtra(EXTRA_PLAYLIST)
        demarrerPremierPlan(notification("Preparation du telechargement...", 0, 0))
        val application = applicationContext
        travail = scope.launch {
            val suivi = launch {
                MusicDownloader.state.collect { etat -> majNotification(etat) }
            }
            try {
                MusicDownloader.sync(application, playlistId)
            } finally {
                suivi.cancel()
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
            }
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private fun demarrerPremierPlan(premiere: Notification) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(ID_NOTIFICATION, premiere, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        } else {
            startForeground(ID_NOTIFICATION, premiere)
        }
    }

    private fun notification(texte: String, current: Int, total: Int): Notification =
        NotificationCompat.Builder(this, CANAL)
            .setContentTitle("M-pacer - musique")
            .setContentText(texte)
            .setSmallIcon(android.R.drawable.stat_sys_download)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setProgress(if (total > 0) total else 0, current, total == 0)
            .build()

    private fun majNotification(etat: MusicDownloadState) {
        if (!etat.busy) return
        val texte = if (etat.total > 0) {
            etat.playlist.ifBlank { "Playlist" } + " : " + etat.current + "/" + etat.total
        } else {
            etat.message ?: "Telechargement..."
        }
        val manager = getSystemService(NotificationManager::class.java) ?: return
        manager.notify(ID_NOTIFICATION, notification(texte, etat.current, etat.total))
    }

    private fun creerCanal() {
        val manager = getSystemService(NotificationManager::class.java) ?: return
        val canal = NotificationChannel(
            CANAL,
            "Telechargement de musique",
            NotificationManager.IMPORTANCE_LOW,
        ).apply { description = "Recupere les MP3 deposes sur M-pacer" }
        manager.createNotificationChannel(canal)
    }

    companion object {
        private const val CANAL = "mpacer_music_download"
        private const val ID_NOTIFICATION = 41
        private const val EXTRA_PLAYLIST = "playlist_id"

        /**
         * Demarre le telechargement en premier plan (ou rejoint celui en cours).
         * Sans playlist precise, toutes celles qui ont des fichiers en attente
         * sont examinees.
         */
        fun start(context: Context, playlistId: String? = null) {
            val intent = Intent(context, MusicDownloadService::class.java).apply {
                if (playlistId != null) putExtra(EXTRA_PLAYLIST, playlistId)
            }
            ContextCompat.startForegroundService(context, intent)
        }
    }
}
