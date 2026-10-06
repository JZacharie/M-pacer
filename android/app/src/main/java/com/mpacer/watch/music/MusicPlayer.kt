package com.mpacer.watch.music

import android.content.ComponentName
import android.content.Context
import android.net.Uri
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import java.io.File

/**
 * Lecteur de la montre : commande la session medias de [MusicPlaybackService].
 *
 * Contrat de responsabilites :
 *  - le coeur Rust decide *quelle* piste jouer (directive Play/Keep/Boost/Relax/
 *    SkipTo/Pause/Resume) ; cette classe ne fait que l'executer ;
 *  - la position et la piste en cours remontent au moteur via [MusicSession].
 *
 * Les fichiers joues sont locaux (filesDir/music) : la lecture reste possible
 * sans le telephone, en Wi-Fi coupe.
 */
object MusicPlayer {

    private val _state = MutableStateFlow(MusicPlayerState())
    val state: StateFlow<MusicPlayerState> = _state.asStateFlow()

    private var appContext: Context? = null
    private var controller: MediaController? = null
    private var connecting = false
    private val pending = mutableListOf<(MediaController) -> Unit>()

    private var queue: List<LocalTrack> = emptyList()
    private var loadedToken: String? = null

    // ---------------------------------------------------------------- connexion

    /** Ouvre (une fois) la connexion a la session de lecture de l'application. */
    fun ensure(context: Context): MediaController? {
        val application = context.applicationContext
        appContext = application
        controller?.let { return it }
        if (connecting) return null
        connecting = true
        val token = SessionToken(application, ComponentName(application, MusicPlaybackService::class.java))
        val future = MediaController.Builder(application, token).buildAsync()
        future.addListener({
            connecting = false
            runCatching { future.get() }
                .onSuccess { mediaController ->
                    controller = mediaController
                    mediaController.addListener(listener)
                    _state.update { it.copy(connected = true, message = null) }
                    val actions = pending.toList()
                    pending.clear()
                    actions.forEach { it(mediaController) }
                    refreshFrom(mediaController)
                }
                .onFailure { error ->
                    _state.update {
                        it.copy(connected = false, message = "Lecteur indisponible : " + (error.message ?: error.javaClass.simpleName))
                    }
                }
        }, application.mainExecutor)
        return null
    }

    fun isConnected(): Boolean = controller != null

    // ------------------------------------------------------------------ lecture

    /** Charge une playlist locale et demarre la lecture. */
    fun play(playlist: LocalPlaylist, startTrackId: String? = null) {
        val tracks = playlist.playable
        queue = tracks
        val token = playlist.id + ":" + tracks.size + ":" + (tracks.firstOrNull()?.id ?: "")
        _state.update {
            it.copy(
                playlistId = playlist.id,
                playlistName = playlist.name,
                source = playlist.source,
                count = tracks.size,
                message = if (tracks.isEmpty()) "Aucun fichier telecharge pour cette playlist" else null,
            )
        }
        withController { mediaController ->
            if (loadedToken != token) {
                mediaController.setMediaItems(tracks.map { track -> MediaItem.fromUri(Uri.fromFile(File(track.file!!))) })
                loadedToken = token
            }
            val index = tracks.indexOfFirst { it.id == startTrackId }.takeIf { it >= 0 } ?: 0
            mediaController.seekTo(index, 0L)
            mediaController.prepare()
            mediaController.play()
            refreshFrom(mediaController)
        }
    }

    /** Change de piste dans la file courante (directive SkipTo du moteur). */
    fun skipTo(trackId: String) {
        withController { mediaController ->
            val index = queue.indexOfFirst { it.id == trackId }
            if (index >= 0) {
                mediaController.seekTo(index, 0L)
                mediaController.play()
            } else {
                mediaController.seekToNextMediaItem()
            }
            refreshFrom(mediaController)
        }
    }

    fun resume() {
        withController { it.play() }
    }

    fun pause() {
        withController { it.pause() }
    }

    fun pauseIfPlaying() {
        withController { if (it.isPlaying) it.pause() }
    }

    fun toggle() {
        withController { if (it.isPlaying) it.pause() else it.play() }
    }

    fun next() {
        withController { it.seekToNextMediaItem() }
    }

    fun previous() {
        withController { it.seekToPreviousMediaItem() }
    }

    /** Position de lecture en secondes (0 si rien ne joue). */
    fun currentPositionS(): Double {
        val position = controller?.currentPosition ?: 0L
        return position / 1000.0
    }

    /** Piste suivante de la file locale (affichage). */
    fun nextTrack(): LocalTrack? {
        val index = controller?.currentMediaItemIndex ?: return null
        return queue.getOrNull(index + 1)
    }

    // ------------------------------------------------------------------- interne

    private fun withController(action: (MediaController) -> Unit) {
        val mediaController = controller
        if (mediaController != null) {
            action(mediaController)
            return
        }
        pending += action
        appContext?.let { ensure(it) }
    }

    private val listener = object : Player.Listener {
        override fun onIsPlayingChanged(isPlaying: Boolean) {
            _state.update { it.copy(playing = isPlaying) }
        }

        override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
            controller?.let(::refreshFrom)
        }

        override fun onPlaybackStateChanged(playbackState: Int) {
            controller?.let(::refreshFrom)
        }
    }

    private fun refreshFrom(mediaController: MediaController) {
        val index = mediaController.currentMediaItemIndex
        _state.update {
            it.copy(
                connected = true,
                playing = mediaController.isPlaying,
                track = queue.getOrNull(index),
                index = index,
                count = queue.size,
            )
        }
    }
}

/** Etat expose a l'ecran Musique. */
data class MusicPlayerState(
    val connected: Boolean = false,
    val playing: Boolean = false,
    val playlistId: String? = null,
    val playlistName: String? = null,
    val source: String = "upload",
    val track: LocalTrack? = null,
    val index: Int = 0,
    val count: Int = 0,
    val message: String? = null,
)
