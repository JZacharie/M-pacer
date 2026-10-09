package com.mpacer.core

import android.util.Log
import org.json.JSONObject

/**
 * Analyse d'une seance terminee, calculee par le coeur Rust.
 *
 * Une fiche de seance affiche des zones cardiaques, une allure ajustee a la
 * pente, une derive cardiaque et une regularite : autant de calculs de course,
 * qui vivent dans mpacer-core, avec leurs tests. Cette passerelle ne fait que
 * transporter la seance (format .pac) et rapporter le resultat : aucun
 * algorithme n'est reecrit ici.
 *
 * Sans etat : contrairement au moteur de seance, l'analyse ne depend d'aucun
 * handle et peut donc tourner sans qu'une course soit en cours.
 */
object MpacerAnalysis {

    private const val TAG = "MpacerAnalysis"

    /** Reference par defaut des zones, celle du moteur (EngineConfig par defaut). */
    const val MAX_BPM_DEFAUT = 190

    init {
        // Le shim JNI depend de libmpacer_ffi.so, package dans le meme APK.
        System.loadLibrary("mpacer_jni")
    }

    /**
     * Rapport d'analyse d'une seance.
     *
     * @param summary seance archivee, telle que le moteur l'a ecrite (format .pac).
     * @param maxBpm frequence cardiaque maximale de l'utilisateur, reference des zones.
     * @param restingBpm frequence de repos ; la methode par reserve cardiaque
     *   (Karvonen) n'est utilisee que si elle est renseignee.
     * @return le rapport, ou un objet portant une cle "error" si la seance est
     *   illisible : l'historique doit s'ouvrir meme sur un fichier abime.
     */
    fun report(
        summary: JSONObject,
        maxBpm: Int = MAX_BPM_DEFAUT,
        restingBpm: Int? = null,
    ): JSONObject {
        val zones = JSONObject()
            .put("max_bpm", maxBpm)
            .put("resting_bpm", restingBpm ?: JSONObject.NULL)
            .put("method", if (restingBpm == null) "PercentMax" else "HeartRateReserve")
        val request = JSONObject()
            .put("summary", summary)
            .put("zones", zones)
        val raw = try {
            nativeReport(request.toString())
        } catch (error: Throwable) {
            // Un echec du pont (bibliotheque native absente, signature JNI
            // differente) doit se voir dans les journaux : sans trace, l'ecran
            // se contente d'un message generique.
            Log.w(TAG, "analyse impossible", error)
            return erreur(error.message ?: "analyse indisponible")
        }
        val rapport = runCatching { JSONObject(raw) }
            .getOrElse {
                Log.w(TAG, "rapport illisible : " + it.message)
                return erreur("rapport illisible : " + (it.message ?: "inconnu"))
            }
        rapport.optString("error").takeIf { it.isNotEmpty() }?.let { Log.w(TAG, "coeur : $it") }
        return rapport
    }

    private fun erreur(message: String): JSONObject = JSONObject().put("error", message)

    private external fun nativeReport(request: String): String
}
