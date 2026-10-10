package com.mpacer.core.music

import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioManager
import android.net.Uri
import androidx.core.content.ContextCompat
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
 * Les fichiers joues sont ceux copies par USB dans le dossier Music/ de la montre
 * (docs/07 v2) : la lecture reste possible sans telephone, sans serveur, hors
 * ligne. Aucune source distante n'est pilotee.
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

    /**
     * Ordre de lecture choisi sur l'ecran Musique : melange, ou ordre de la
     * playlist. Il ne concerne que la file locale ; le choix de piste du moteur
     * (docs/07) reste pilote par le tempo. Le choix est retenu entre deux
     * lancements.
     */
    @Volatile
    private var shuffle = false

    /** Nom des preferences du lecteur (ordre de lecture). */
    private const val PREFERENCES = "mpacer-musique-lecture"
    private const val CLE_SHUFFLE = "aleatoire"

    /** Vrai si la file locale est jouee dans un ordre melange. */
    fun shuffleEnabled(): Boolean = shuffle

    /** Regle l'ordre de lecture et le retient pour les prochaines seances. */
    fun setShuffle(enabled: Boolean) {
        shuffle = enabled
        _state.update { it.copy(shuffle = enabled) }
        appContext
            ?.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
            ?.edit()
            ?.putBoolean(CLE_SHUFFLE, enabled)
            ?.apply()
    }

    // ---------------------------------------------------------------- connexion

    /**
     * Prepare le lecteur sans exposer la session Media3 : c'est le point d'entree
     * des interfaces (montre, telephone), qui n'ont pas Media3 sur leur classpath.
     */
    fun prepare(context: Context) {
        // L'ordre de lecture est un choix de l'utilisateur : il survit a la
        // fermeture de l'application.
        val application = context.applicationContext
        shuffle = application
            .getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
            .getBoolean(CLE_SHUFFLE, false)
        _state.update { it.copy(shuffle = shuffle) }
        ensure(context)
    }

    /** Ouvre (une fois) la connexion a la session de lecture de l'application. */
    internal fun ensure(context: Context): MediaController? {
        val application = context.applicationContext
        appContext = application
        controller?.let { return it }
        if (connecting) return null
        connecting = true
        // Le volume media est reglable meme quand aucune piste ne joue : la vue
        // Musique en course doit pouvoir le montrer des l'ouverture.
        suivreVolume(application)
        refreshVolume()
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
        val tracks = ordreLecture(playlist.playable, shuffle)
        queue = tracks
        // Le jeton porte l'ordre complet : relancer une playlist en mode melange
        // doit remplacer la file Media3, meme si la premiere piste est la meme.
        val token = playlist.id + ":" + tracks.joinToString(",") { track -> track.id }
        _state.update {
            it.copy(
                playlistId = playlist.id,
                playlistName = playlist.name,
                source = playlist.source,
                count = tracks.size,
                message = if (tracks.isEmpty()) "Aucun fichier sur la montre pour cette playlist" else null,
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

    /**
     * Coupe la lecture si elle est en cours. N'empile aucune commande : appelee a
     * chaque tick quand la musique est coupee, elle ne doit pas remplir la file.
     */
    fun pauseIfPlaying() {
        val mediaController = controller ?: return
        if (mediaController.isPlaying) mediaController.pause()
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

    // ------------------------------------------------------------------ volume

    /** Pas d'un appui sur les commandes de volume (10 % du maximum du flux media). */
    private const val PAS_VOLUME = 10

    private var volumeReceiver: BroadcastReceiver? = null

    private fun audio(): AudioManager? =
        appContext?.getSystemService(Context.AUDIO_SERVICE) as? AudioManager

    /**
     * Volume media courant, en pourcentage du maximum (0 a 100).
     *
     * C'est le volume du flux STREAM_MUSIC, celui que baissent et montent les
     * boutons de la montre : la valeur affichee correspond donc au son entendu,
     * que la lecture soit pilotee par M-pacer ou par le systeme.
     */
    fun volumePercent(): Int {
        val gestionnaire = audio() ?: return _state.value.volumePercent
        val maximum = gestionnaire.getStreamMaxVolume(AudioManager.STREAM_MUSIC)
        if (maximum <= 0) return 0
        return (gestionnaire.getStreamVolume(AudioManager.STREAM_MUSIC) * 100 + maximum / 2) / maximum
    }

    /** Relit le volume du systeme (boutons de la montre, autre application). */
    fun refreshVolume() {
        val gestionnaire = audio() ?: return
        val maximum = gestionnaire.getStreamMaxVolume(AudioManager.STREAM_MUSIC)
        _state.update { it.copy(volumePercent = volumePercent(), volumeMax = maximum) }
    }

    fun volumeUp() = changerVolume(PAS_VOLUME)

    fun volumeDown() = changerVolume(-PAS_VOLUME)

    /**
     * Regle le volume media a un pourcentage precis (0 a 100).
     *
     * C'est le point d'entree du curseur de l'application telephone : la montre,
     * elle, n'a que les deux ronds + et -.
     */
    fun setVolumePercent(percent: Int) {
        val gestionnaire = audio() ?: return
        val maximum = gestionnaire.getStreamMaxVolume(AudioManager.STREAM_MUSIC)
        if (maximum <= 0) return
        val cible = (percent.coerceIn(0, 100) * maximum + 50) / 100
        // Un appareil sans flux media ne doit jamais faire tomber l'ecran : le
        // reglage est un confort, pas une condition de la seance.
        runCatching { gestionnaire.setStreamVolume(AudioManager.STREAM_MUSIC, cible, 0) }
        refreshVolume()
    }

    private fun changerVolume(pas: Int) = setVolumePercent(volumePercent() + pas)

    /**
     * Garde l'affichage juste quand le volume change ailleurs : boutons
     * physiques de la montre, ou fin de la lecture d'une autre application.
     * Un seul recepteur pour la duree de vie du processus.
     */
    private fun suivreVolume(application: Context) {
        if (volumeReceiver != null) return
        val recepteur = object : BroadcastReceiver() {
            override fun onReceive(contexte: Context?, intention: Intent?) {
                when (intention?.action) {
                    ACTION_VOLUME_CHANGED -> {
                        val flux = intention.getIntExtra(EXTRA_VOLUME_STREAM_TYPE, -1)
                        if (flux == AudioManager.STREAM_MUSIC) refreshVolume()
                    }
                    ACTION_STREAM_MUTE_CHANGED -> refreshVolume()
                }
            }
        }
        val filtre = IntentFilter().apply {
            addAction(ACTION_VOLUME_CHANGED)
            addAction(ACTION_STREAM_MUTE_CHANGED)
        }
        // Diffusions du systeme : le drapeau NOT_EXPORTED evite d'exposer le
        // recepteur, comme l'exige Android 14 pour tout enregistrement dynamique.
        runCatching {
            ContextCompat.registerReceiver(
                application,
                recepteur,
                filtre,
                ContextCompat.RECEIVER_NOT_EXPORTED,
            )
        }.onSuccess { volumeReceiver = recepteur }
    }

    /**
     * Diffusions du systeme qui informent d'un changement de volume.
     *
     * Les constantes [AudioManager] correspondantes ne sont pas dans l'API
     * publique (annotations @hide) : les valeurs sont celles du framework,
     * stables depuis Android 5.
     */
    private const val ACTION_VOLUME_CHANGED = "android.media.VOLUME_CHANGED_ACTION"
    private const val ACTION_STREAM_MUTE_CHANGED = "android.media.STREAM_MUTE_CHANGED_ACTION"
    private const val EXTRA_VOLUME_STREAM_TYPE = "android.media.EXTRA_VOLUME_STREAM_TYPE"

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
    /** File locale melangee (ecran Musique) ou jouee dans l'ordre de la playlist. */
    val shuffle: Boolean = false,
    val playlistId: String? = null,
    val playlistName: String? = null,
    val source: String = "manual",
    val track: LocalTrack? = null,
    val index: Int = 0,
    val count: Int = 0,
    val message: String? = null,
    /** Volume media du systeme, en pourcentage (0 a 100). */
    val volumePercent: Int = 0,
    /** Maximum du flux media, pour l'echelle des commandes. */
    val volumeMax: Int = 0,
)
