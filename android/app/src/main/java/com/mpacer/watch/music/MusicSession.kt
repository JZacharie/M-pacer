package com.mpacer.watch.music

import com.mpacer.watch.MpacerCore

/**
 * Pont entre l'interface musique et l'instance de [MpacerCore] detenue par
 * [com.mpacer.watch.TrackingService].
 *
 * Le moteur n'existe que pendant une seance. L'ecran Musique peut donc etre
 * utilise avant la course : les reglages et la playlist sont conserves ici et
 * reappliques au demarrage de la seance (voir TrackingService.start).
 *
 * Aucun calcul n'est fait ici : on ne fait que transporter le contrat FFI.
 */
object MusicSession {

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
        core.setMusic(config)
        core.setMusicPlaylist(playlist)
    }

    fun setConfig(config: MusicConfig) {
        this.config = config
        core?.setMusic(config)
    }

    fun setPlaylist(local: LocalPlaylist?) {
        this.local = local
        core?.setMusicPlaylist(playlist)
    }

    /** Instantane de la piste jouee (position en secondes). */
    fun nowPlaying(now: NowPlaying?) {
        core?.musicNowPlaying(now)
    }

    /** Cadence de pas mesuree (aucun capteur en v1 : le tick du moteur estime). */
    fun cadence(tMs: Long, spm: Double) {
        core?.onCadence(tMs, spm)
    }
}
