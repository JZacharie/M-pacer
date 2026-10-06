package com.mpacer.watch.music

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.media.session.MediaSessionManager
import android.provider.Settings
import androidx.core.app.NotificationManagerCompat
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/**
 * Telecommande de l'application Spotify installee sur la montre (docs/07 section 2).
 *
 * M-pacer ne lit jamais l'audio Spotify (chiffre Widevine) : il pilote la session
 * medias de l'application Spotify par MediaController. Le choix du morceau reste
 * donc limite a ce qu'expose une session medias : lecture, pause, suivant,
 * precedent. L'application Spotify garde la main sur sa file.
 *
 * Pour voir une session appartenant a une autre application, Android exige un
 * acces aux notifications : [MediaSessionAccessService] le declare, l'utilisateur
 * l'autorise depuis l'ecran Musique.
 */
object SpotifyRemote {

    const val SPOTIFY_PACKAGE = "com.spotify.music"

    private val _state = MutableStateFlow(SpotifyRemoteState())
    val state: StateFlow<SpotifyRemoteState> = _state.asStateFlow()

    private var controller: MediaController? = null
    private var connecting = false

    /** L'acces aux notifications (donc aux sessions des autres applications) est-il accorde ? */
    fun accessGranted(context: Context): Boolean =
        NotificationManagerCompat.getEnabledListenerPackages(context).contains(context.packageName)

    /** Ouvre l'ecran systeme d'autorisation de l'acces aux notifications. */
    fun openAccessSettings(context: Context) {
        val intent = Intent(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        runCatching { context.startActivity(intent) }
            .onFailure { _state.update { state -> state.copy(message = "Reglages indisponibles") } }
    }

    /** Cherche la session medias de Spotify et ouvre une telecommande. */
    fun connect(context: Context) {
        _state.update { it.copy(granted = accessGranted(context)) }
        if (controller != null || connecting) return
        if (!accessGranted(context)) {
            _state.update {
                it.copy(available = false, message = "Autorisez l acces aux notifications pour piloter Spotify")
            }
            return
        }
        val manager = context.getSystemService(Context.MEDIA_SESSION_SERVICE) as? MediaSessionManager
        if (manager == null) {
            _state.update { it.copy(available = false, message = "Sessions medias indisponibles") }
            return
        }
        val listener = ComponentName(context, MediaSessionAccessService::class.java)
        val spotify = runCatching { manager.getActiveSessions(listener) }
            .getOrDefault(emptyList())
            .firstOrNull { it.packageName == SPOTIFY_PACKAGE }
        if (spotify == null) {
            _state.update { it.copy(available = false, message = "Spotify ne joue pas sur la montre") }
            return
        }
        connecting = true
        val token = SessionToken(context, spotify.sessionToken)
        val future = MediaController.Builder(context, token).buildAsync()
        future.addListener({
            connecting = false
            runCatching { future.get() }
                .onSuccess { mediaController ->
                    controller = mediaController
                    mediaController.addListener(object : Player.Listener {
                        override fun onIsPlayingChanged(isPlaying: Boolean) {
                            refresh(mediaController)
                        }

                        override fun onMediaMetadataChanged(mediaMetadata: MediaMetadata) {
                            refresh(mediaController)
                        }

                        override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
                            refresh(mediaController)
                        }
                    })
                    _state.update { it.copy(available = true, message = null) }
                    refresh(mediaController)
                }
                .onFailure { error ->
                    _state.update {
                        it.copy(available = false, message = "Spotify injoignable : " + (error.message ?: error.javaClass.simpleName))
                    }
                }
        }, context.mainExecutor)
    }

    fun play(context: Context) {
        withController(context) { it.play() }
    }

    fun pause(context: Context) {
        withController(context) { it.pause() }
    }

    fun toggle(context: Context) {
        withController(context) { if (it.isPlaying) it.pause() else it.play() }
    }

    /** Passe au morceau suivant (l'application Spotify choisit lequel). */
    fun next(context: Context) {
        withController(context) { it.seekToNextMediaItem() }
    }

    fun previous(context: Context) {
        withController(context) { it.seekToPreviousMediaItem() }
    }

    private fun withController(context: Context, action: (MediaController) -> Unit) {
        connect(context)
        controller?.let(action)
    }

    private fun refresh(mediaController: MediaController) {
        val metadata = mediaController.mediaMetadata
        _state.update {
            it.copy(
                available = true,
                playing = mediaController.isPlaying,
                title = metadata.title?.toString(),
                artist = metadata.artist?.toString(),
            )
        }
    }
}

/** Etat expose a l'ecran Musique pour la source Spotify. */
data class SpotifyRemoteState(
    val granted: Boolean = false,
    val available: Boolean = false,
    val playing: Boolean = false,
    val title: String? = null,
    val artist: String? = null,
    val message: String? = null,
)
