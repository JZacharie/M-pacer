package com.mpacer.core

import android.util.Log

/**
 * Reglages de la seance (assistant de course et retour vocal) conserves hors
 * seance, exactement comme [com.mpacer.core.music.MusicSession] conserve ceux de
 * la musique.
 *
 * Le moteur Rust n'existe que pendant une seance : les ecrans de reglages
 * enregistrent donc ici ce que le moteur doit recevoir au depart. Chaque envoi
 * est isole : une commande refusee (version du coeur plus ancienne que le
 * contrat) ne doit jamais empecher une seance de demarrer.
 *
 * Aucun calcul ici : le coeur valide la configuration et signale les manques
 * (distance de course ou temps prevu absents) dans l'etat qu'il publie.
 */
object SessionConfig {

    private const val TAG = "SessionConfig"

    private var core: MpacerCore? = null

    /** Assistant courant (mode, distance de course, temps prevu). */
    var assistant: AssistantConfig = AssistantConfig()
        private set

    /** Retour vocal courant (activation, cadence, langue, politique musique). */
    var voice: VoiceConfig = VoiceConfig()
        private set

    fun attach(core: MpacerCore) {
        this.core = core
    }

    fun detach(core: MpacerCore) {
        if (this.core === core) this.core = null
    }

    /** Reapplique les reglages au moteur (debut de seance). */
    fun apply(core: MpacerCore) {
        envoyer("set_assistant") {
            core.setAssistant(
                mode = assistant.mode,
                raceDistanceM = assistant.raceDistanceM,
                plannedTimeS = assistant.plannedTimeS,
                negativeSplitRatio = assistant.negativeSplitRatio,
            )
        }
        envoyer("set_voice") { core.setVoice(voice) }
        // La voix est rendue par l'application : on lui transmet la meme config.
        VoiceCoach.configure(voice)
    }

    fun setAssistant(config: AssistantConfig) {
        assistant = config
        val moteur = core ?: return
        envoyer("set_assistant") {
            moteur.setAssistant(
                mode = config.mode,
                raceDistanceM = config.raceDistanceM,
                plannedTimeS = config.plannedTimeS,
                negativeSplitRatio = config.negativeSplitRatio,
            )
        }
    }

    fun setVoice(config: VoiceConfig) {
        voice = config
        VoiceCoach.configure(config)
        val moteur = core ?: return
        envoyer("set_voice") { moteur.setVoice(config) }
    }

    private inline fun envoyer(commande: String, action: () -> Unit) {
        try {
            action()
        } catch (error: Exception) {
            Log.w(TAG, commande + " ignoree : " + (error.message ?: error.javaClass.simpleName))
        }
    }
}

/**
 * Reglages de l'assistant de course.
 *
 * @param raceDistanceM distance de l'epreuve (m), requise par les modes qui
 *   prevoyent un temps de finish ou comparent a un shadow runner.
 * @param plannedTimeS temps vise au finish (s), pour "atteindre le temps prevu".
 * @param negativeSplitRatio part negative du plan (0.03 = 3 % plus rapide sur la
 *   seconde moitie).
 */
data class AssistantConfig(
    val mode: AssistantMode = AssistantMode.TRACK_PACE,
    val raceDistanceM: Double? = null,
    val plannedTimeS: Double? = null,
    val negativeSplitRatio: Double = 0.0,
)
