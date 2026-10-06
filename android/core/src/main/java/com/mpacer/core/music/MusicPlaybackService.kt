package com.mpacer.core.music

import android.content.Intent
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/**
 * Service de lecture musicale (Media3).
 *
 * Il porte la session medias de la montre : c'est elle que pilotent
 * [MusicPlayer], les boutons du casque et l'assistant. Le service est declare
 * en `foregroundServiceType="mediaPlayback"` ; Media3 le passe au premier plan
 * des que la lecture demarre (notification de lecture).
 *
 * Les fichiers joues sont ceux telecharges par [MusicLibrary] : aucune lecture
 * en flux, aucune donnee ne sort de la montre pendant la course.
 */
class MusicPlaybackService : MediaSessionService() {

    private var player: ExoPlayer? = null
    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        val exo = ExoPlayer.Builder(this).build()
        // Usage "media" : la montre baisse la musique quand le coach parle
        // (AudioFocusRequest de VoiceCoach) au lieu de la couper.
        exo.setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(C.USAGE_MEDIA)
                .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                .build(),
            true,
        )
        player = exo
        session = MediaSession.Builder(this, exo).build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    /** Retire la tache de la liste recente quand plus rien ne joue. */
    override fun onTaskRemoved(rootIntent: Intent?) {
        val exo = player
        if (exo == null || !exo.playWhenReady || exo.mediaItemCount == 0) {
            stopSelf()
        }
    }

    override fun onDestroy() {
        session?.release()
        session = null
        player?.release()
        player = null
        super.onDestroy()
    }
}
