package com.mpacer.watch.music

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.media.MediaMetadata
import android.media.session.MediaController
import android.media.session.MediaSessionManager
import android.media.session.PlaybackState
import android.provider.Settings
import androidx.core.app.NotificationManagerCompat
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/**
 * Telecommande de l'application Spotify installee sur la montre (docs/07 section 2).
 *
 * M-pacer ne lit jamais l'audio Spotify (chiffre Widevine) : il pilote la session
 * medias de l'application Spotify. Une session appartenant a une autre application
 * n'est visible qu'avec l'acces aux notifications : [MediaSessionAccessService] le
 * declare, l'utilisateur l'autorise depuis l'ecran Musique.
 *
 * Limite assumee : une session medias n'expose que lecture, pause, suivant et
 * precedent. Le choix precis du morceau reste fait dans l'application Spotify.
 */
object SpotifyRemote {

    const val SPOTIFY_PACKAGE = "com.spotify.music"

    private val _state = MutableStateFlow(SpotifyRemoteState())
    val state: StateFlow<SpotifyRemoteState> = _state.asStateFlow()

    private var controller: MediaController? = null
    private val callback = object : MediaController.Callback() {
        override fun onPlaybackStateChanged(state: PlaybackState?) {
            controller?.let(::refresh)
        }

        override fun onMetadataChanged(metadata: MediaMetadata?) {
            controller?.let(::refresh)
        }
    }

    /** L'acces aux notifications (donc aux sessions des autres applications) est-il accorde ? */
    fun accessGranted(context: Context): Boolean =
        NotificationManagerCompat.getEnabledListenerPackages(context).contains(context.packageName)

    /** Ouvre l'ecran systeme d'autorisation de l'acces aux notifications. */
    fun openAccessSettings(context: Context) {
        val intent = Intent(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        runCatching { context.startActivity(intent) }
            .onFailure { _state.update { current -> current.copy(message = "Reglages indisponibles") } }
    }

    /** Cherche la session medias de Spotify et s'y connecte (une fois). */
    fun connect(context: Context) {
        _state.update { it.copy(granted = accessGranted(context)) }
        if (controller != null) return
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
        controller = try {
            MediaController(context, spotify.sessionToken).also { active ->
                active.registerCallback(callback)
                refresh(active)
            }
        } catch (error: Exception) {
            _state.update {
                it.copy(available = false, message = "Spotify injoignable : " + (error.message ?: error.javaClass.simpleName))
            }
            null
        }
    }

    fun play(context: Context) {
        withController(context) { it.transportControls.play() }
    }

    fun pause(context: Context) {
        withController(context) { it.transportControls.pause() }
    }

    fun toggle(context: Context) {
        withController(context) { active ->
            if (active.playbackState?.state == PlaybackState.STATE_PLAYING) {
                active.transportControls.pause()
            } else {
                active.transportControls.play()
            }
        }
    }

    /** Passe au morceau suivant (l'application Spotify choisit lequel). */
    fun next(context: Context) {
        withController(context) { it.transportControls.skipToNext() }
    }

    fun previous(context: Context) {
        withController(context) { it.transportControls.skipToPrevious() }
    }

    private fun withController(context: Context, action: (MediaController) -> Unit) {
        connect(context)
        controller?.let(action)
    }

    private fun refresh(active: MediaController) {
        val metadata = active.metadata
        _state.update {
            it.copy(
                available = true,
                playing = active.playbackState?.state == PlaybackState.STATE_PLAYING,
                title = metadata?.getString(MediaMetadata.METADATA_KEY_TITLE),
                artist = metadata?.getString(MediaMetadata.METADATA_KEY_ARTIST),
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
