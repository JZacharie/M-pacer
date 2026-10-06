package com.mpacer.watch.music

import android.util.Log
import com.mpacer.watch.MpacerCore

/**
 * Pont entre l'interface musique et l'instance de [MpacerCore] detenue par
 * [com.mpacer.watch.TrackingService].
 *
 * Le moteur n'existe que pendant une seance. L'ecran Musique peut donc etre
 * utilise avant la course : les reglages et la playlist sont conserves ici et
 * reappliques au demarrage de la seance (voir TrackingService.start).
 *
 * Robustesse : une commande musique refusee par le moteur (version du coeur plus
 * ancienne que le contrat, playlist invalide) ne doit jamais interrompre la seance
 * en cours. Chaque envoi est donc isole ; l'erreur est journalisee.
 *
 * Aucun calcul n'est fait ici : on ne fait que transporter le contrat FFI.
 */
object MusicSession {

    private const val TAG = "MusicSession"

    private var core: MpacerCore? = null

    /** Reglages musique courants (miroir de l'ecran Reglages). */
    var config: MusicConfig = MusicConfig()
        private set

    /** Playlist choisie sur la montre, avec ses fichiers telecharges. */
    var local: LocalPlaylist? = null
        private set

    /** Vue envoyee au coeur (titres + BPM uniquement). */
    val playlist: MusicPlaylist? get() = local?.toCore()

    fun attach(core: MpacerCore) {
        this.core = core
    }

    fun detach(core: MpacerCore) {
        if (this.core === core) this.core = null
    }

    /** Reapplique la configuration et la playlist au moteur (debut de seance). */
    fun apply(core: MpacerCore) {
        envoyer("set_music") { core.setMusic(config) }
        envoyer("set_music_playlist") { core.setMusicPlaylist(playlist) }
    }

    fun setConfig(config: MusicConfig) {
        this.config = config
        val engine = core ?: return
        envoyer("set_music") { engine.setMusic(config) }
    }

    fun setPlaylist(local: LocalPlaylist?) {
        this.local = local
        val engine = core ?: return
        envoyer("set_music_playlist") { engine.setMusicPlaylist(playlist) }
    }

    /** Instantane de la piste jouee (position en secondes). */
    fun nowPlaying(now: NowPlaying?) {
        val engine = core ?: return
        envoyer("music_now_playing") { engine.musicNowPlaying(now) }
    }

    /** Cadence de pas mesuree (aucun capteur en v1 : le tick du moteur estime). */
    fun cadence(tMs: Long, spm: Double) {
        val engine = core ?: return
        envoyer("on_cadence") { engine.onCadence(tMs, spm) }
    }

    private inline fun envoyer(commande: String, action: () -> Unit) {
        try {
            action()
        } catch (error: Exception) {
            Log.w(TAG, commande + " ignoree : " + (error.message ?: error.javaClass.simpleName))
        }
    }
}
