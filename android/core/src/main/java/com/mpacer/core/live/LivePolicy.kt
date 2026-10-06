package com.mpacer.core.live

/**
 * Politique de publication : c'est ici que se joue l'essentiel du cout du suivi
 * en direct (radio et batterie). Aucun envoi n'est declenche par le GPS : les
 * positions arrivent a 1 Hz et la politique n'en retient qu'une par periode,
 * et seulement si la precision est suffisante.
 */
internal object LivePolicy {

    /** Decision prise pour un echantillon GPS. */
    sealed interface Decision {
        /** Le point part sur le broker. */
        data object Publish : Decision

        /** Periode non ecoulee : le point est ignore sans aucun travail. */
        data object TooSoon : Decision

        /** Precision insuffisante : on ne publie pas une position approximative. */
        data object Imprecise : Decision

        /** Suivi desactive ou non configure. */
        data object Inactif : Decision
    }

    /**
     * Faut-il publier ce point ?
     *
     * @param maintenantMs horloge monotone (SystemClock.elapsedRealtime).
     * @param dernierMs instant de la derniere publication (0 = aucune).
     * @param enPause seance en pause (periode longue).
     * @param accuracyM precision annoncee par le GPS (null = inconnue).
     */
    fun decide(
        maintenantMs: Long,
        dernierMs: Long,
        enPause: Boolean,
        accuracyM: Double?,
        config: LiveConfig,
    ): Decision {
        if (!config.configured) return Decision.Inactif
        if (accuracyM != null && !accuracyM.isNaN() && accuracyM > config.minAccuracyM) {
            return Decision.Imprecise
        }
        // Premier point de la seance : on publie tout de suite, sans attendre une
        // periode complete.
        if (dernierMs <= 0L) return Decision.Publish
        return if (maintenantMs - dernierMs < config.intervalMs(enPause)) {
            Decision.TooSoon
        } else {
            Decision.Publish
        }
    }

    /**
     * Les changements d'etat (depart, pause, reprise, arret) partent tout de
     * suite : c'est ce qui donne son interet au suivi, et cela ne represente que
     * quelques messages par seance.
     */
    fun etatChange(ancien: String?, nouveau: String): Boolean = ancien != nouveau
}
