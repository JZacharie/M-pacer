package com.mpacer.watch

import com.mpacer.watch.music.MusicConfig
import com.mpacer.watch.music.MusicPlaylist
import com.mpacer.watch.music.MusicState
import com.mpacer.watch.music.NowPlaying
import org.json.JSONArray
import org.json.JSONObject

/**
 * Pont vers le coeur Rust (libmpacer_ffi.so, expose par le shim JNI C).
 *
 * Regle : **aucun calcul de course cote Kotlin**. On pousse des positions, on envoie
 * des commandes, on lit [EngineOutput]. Toute la logique vit dans `mpacer-core`.
 */
class MpacerCore : AutoCloseable {

    private var handle: Long = nativeNew()

    // ------------------------------------------------------------------ reglages

    fun setAssistant(
        mode: AssistantMode,
        raceDistanceM: Double? = null,
        plannedTimeS: Double? = null,
        negativeSplitRatio: Double = 0.0,
    ): EngineOutput = EngineOutput(
        send(
            JSONObject()
                .put("cmd", "set_assistant")
                .put("mode", mode.wire)
                .put("race_distance_m", raceDistanceM ?: JSONObject.NULL)
                .put("planned_time_s", plannedTimeS ?: JSONObject.NULL)
                .put("negative_split_ratio", negativeSplitRatio)
        )
    )

    fun setVoice(config: VoiceConfig): EngineOutput = EngineOutput(
        send(
            JSONObject()
                .put("cmd", "set_voice")
                .put(
                    "config",
                    JSONObject()
                        .put("enabled", config.enabled)
                        .put("frequency", config.frequency.wire)
                        .put("language", config.language.wire)
                        .put("extended_lap_info", config.extendedLapInfo)
                        .put("short_forms", config.shortForms)
                        .put("music_policy", config.musicPolicy.wire)
                )
        )
    )

    // ----------------------------------------------------------------- musique

    /**
     * Reglages musique du moteur (docs/07 section 4.1). Le moteur valide les
     * bornes (min/max BPM) et publie l'etat dans `EngineOutput.music`.
     */
    fun setMusic(config: MusicConfig): EngineOutput = EngineOutput(
        send(JSONObject().put("cmd", "set_music").put("config", config.toJson()))
    )

    /** Playlist courante ; `null` la retire (le moteur repond alors NoPlaylist). */
    fun setMusicPlaylist(playlist: MusicPlaylist?): EngineOutput = EngineOutput(
        send(
            JSONObject()
                .put("cmd", "set_music_playlist")
                .put("playlist", playlist?.toJson() ?: JSONObject.NULL)
        )
    )

    /** Instantane de la piste jouee sur la montre ; `null` quand plus rien ne joue. */
    fun musicNowPlaying(now: NowPlaying?): EngineOutput = EngineOutput(
        send(
            JSONObject()
                .put("cmd", "music_now_playing")
                .put("now", now?.toJson() ?: JSONObject.NULL)
        )
    )

    /**
     * Cadence de pas mesuree (pas par minute) et son instant.
     * Aucun capteur de pas n'est branche en v1 : le tick du moteur estime la
     * cadence a partir de la vitesse (docs/07 sections 4.2 et 9).
     */
    fun onCadence(tMs: Long, spm: Double): EngineOutput = EngineOutput(
        send(JSONObject().put("cmd", "on_cadence").put("t_ms", tMs).put("spm", spm))
    )

    // ------------------------------------------------------------------ seance

    fun start(tMs: Long) = EngineOutput(send(JSONObject().put("cmd", "start").put("t_ms", tMs)))

    /** Appui long : la seance est prete, le chrono attend le premier mouvement. */
    fun arm(tMs: Long) = EngineOutput(send(JSONObject().put("cmd", "arm").put("t_ms", tMs)))

    fun pause(tMs: Long) = EngineOutput(send(JSONObject().put("cmd", "pause").put("t_ms", tMs)))

    fun resume(tMs: Long) = EngineOutput(send(JSONObject().put("cmd", "resume").put("t_ms", tMs)))

    fun stop(tMs: Long) = EngineOutput(send(JSONObject().put("cmd", "stop").put("t_ms", tMs)))

    fun reset() = EngineOutput(send(JSONObject().put("cmd", "reset")))

    /** Rafraichissement d'affichage : sans position GPS, donc sans effet sur l'allure. */
    fun tick(tMs: Long) = EngineOutput(send(JSONObject().put("cmd", "tick").put("t_ms", tMs)))

    /** Triple clic sur le bouton du casque : redemarre la fenetre d'allure. */
    fun resetPaceWindow(tMs: Long) =
        EngineOutput(send(JSONObject().put("cmd", "reset_pace_window").put("t_ms", tMs)))

    /** Double clic : annonce vocale immediate. */
    fun announceNow(tMs: Long) =
        EngineOutput(send(JSONObject().put("cmd", "announce_now").put("t_ms", tMs)))

    /**
     * Mesure de frequence cardiaque (bpm), a l'instant donne.
     *
     * Le moteur la rattache a la seance : zones, derive cardiaque, graphique et
     * export GPX. Les mesures sont conservees meme pendant une pause (frequence
     * de recuperation), mais elles ne comptent pas dans l'allure.
     */
    fun heartRate(tMs: Long, bpm: Int): EngineOutput = EngineOutput(
        send(JSONObject().put("cmd", "heart_rate").put("t_ms", tMs).put("bpm", bpm))
    )

    /** Position GPS (1 Hz). Seuls de vrais echantillons font evoluer l'allure. */
    fun gps(
        tMs: Long,
        lat: Double,
        lon: Double,
        accuracyM: Double,
        altitudeM: Double? = null,
        speedMps: Double? = null,
    ): EngineOutput = EngineOutput(
        send(
            JSONObject()
                .put("cmd", "gps")
                .put("t_ms", tMs)
                .put("lat", lat)
                .put("lon", lon)
                .put("accuracy_m", accuracyM)
                .put("altitude_m", altitudeM ?: JSONObject.NULL)
                .put("speed_mps", speedMps ?: JSONObject.NULL)
        )
    )

    /** Resume complet de la seance : historique, export GPX, export .pac. */
    fun summary(startedAtMs: Long): JSONObject =
        send(JSONObject().put("cmd", "summary").put("started_at_ms", startedAtMs))

    override fun close() {
        if (handle != 0L) {
            nativeFree(handle)
            handle = 0L
        }
    }

    private fun send(command: JSONObject): JSONObject {
        check(handle != 0L) { "MpacerCore est ferme" }
        val raw = nativeCommand(handle, command.toString())
        val json = JSONObject(raw)
        json.optString("error").takeIf { it.isNotEmpty() }?.let { error ->
            throw IllegalStateException("mpacer-core : $error")
        }
        return json
    }

    companion object {
        init {
            // Le shim JNI depend de libmpacer_ffi.so, package dans le meme APK.
            System.loadLibrary("mpacer_jni")
        }

        val version: String get() = nativeVersion()

        @JvmStatic private external fun nativeNew(): Long
        @JvmStatic private external fun nativeFree(handle: Long)
        @JvmStatic private external fun nativeCommand(handle: Long, command: String): String
        @JvmStatic private external fun nativeVersion(): String
    }
}

// ---------------------------------------------------------------------- enums

enum class AssistantMode(val wire: String) {
    TRACK_PACE("TrackPace"),
    PREDICT_FINISH_TIME("PredictFinishTime"),
    ACHIEVE_PLANNED_TIME("AchievePlannedTime"),
    REMOTE_RACE("RemoteRace"),
}

enum class VoiceFrequency(val wire: String) {
    OFF("Off"),
    EVERY_MINUTE("EveryMinute"),
    EVERY_2_MINUTES("Every2Minutes"),
    EVERY_5_MINUTES("Every5Minutes"),
    EVERY_LAP("EveryLap"),
    MANUAL_ONLY("ManualOnly"),
}

enum class MusicPolicy(val wire: String) {
    DUCK("Duck"),
    PAUSE("Pause"),
    IGNORE_AND_SPEAK("IgnoreAndSpeak"),
}

enum class VoiceLanguage(val wire: String) {
    FR("Fr"),
    EN("En"),
}

data class VoiceConfig(
    val enabled: Boolean = true,
    val frequency: VoiceFrequency = VoiceFrequency.EVERY_2_MINUTES,
    val language: VoiceLanguage = VoiceLanguage.FR,
    val extendedLapInfo: Boolean = false,
    val shortForms: Boolean = false,
    val musicPolicy: MusicPolicy = MusicPolicy.DUCK,
)

// ---------------------------------------------------------------------- etat

private fun JSONObject.doubleOrNull(name: String): Double? =
    if (isNull(name)) null else optDouble(name).takeIf { !it.isNaN() }

/** Etat complet restitue par le moteur apres chaque mise a jour. */
data class EngineOutput(val json: JSONObject) {
    val light: String get() = json.optString("light", "Orange")
    val state: String get() = json.optString("state", "Idle")
    val elapsedS: Double get() = json.optDouble("elapsed_s", 0.0)
    val distanceM: Double get() = json.optDouble("distance_m", 0.0)
    val currentPace: Double? get() = json.doubleOrNull("current_pace")
    val currentLapPace: Double? get() = json.doubleOrNull("current_lap_pace")
    val previousLapPace: Double? get() = json.doubleOrNull("previous_lap_pace")
    val currentLapDistanceM: Double get() = json.optDouble("current_lap_distance_m", 0.0)
    val speedMps: Double? get() = json.doubleOrNull("speed_mps")
    val isRunning: Boolean get() = state == "Running"

    /** Derniere frequence cardiaque recue (bpm), null sans capteur. */
    val heartRateBpm: Int? get() = json.optInt("heart_rate_bpm", 0).takeIf { it > 0 }

    /** Zone de la derniere frequence (1 a 5), null sous la zone 1 ou sans mesure. */
    val heartRateZone: Int? get() = json.optInt("heart_rate_zone", 0).takeIf { it > 0 }
    val isPaused: Boolean get() = state == "Paused" || state == "AutoPaused"

    val estimatedFinishS: Double?
        get() = json.optJSONObject("panel")?.doubleOrNull("estimated_finish_s")

    val remainingM: Double?
        get() = json.optJSONObject("panel")?.doubleOrNull("remaining_m")

    /** Comparaison au shadow runner (mode "atteindre le temps prevu"). */
    val shadow: Shadow? get() = json.optJSONObject("panel")?.optJSONObject("shadow")?.let { Shadow(it) }

    /** Annonces vocales a prononcer, deja redigees par le coeur (FR/EN). */
    val messages: List<String>
        get() = json.optJSONArray("messages").mapObjects { it.optString("text") }

    /** Evenements de seance ("Armed", "AutoPaused"...) : des chaines JSON cote coeur. */
    val events: List<String>
        get() = json.optJSONArray("events").mapObjects { it.optString("kind") }

    /**
     * Bloc musique publie par le moteur (docs/07 section 5). Toujours present cote
     * coeur ; null seulement si le coeur est plus ancien que le contrat.
     */
    val music: MusicState? get() = json.optJSONObject("music")?.let(::MusicState)
}

data class Shadow(val json: JSONObject) {
    val distanceDeltaM: Double get() = json.optDouble("distance_delta_m", 0.0)
    val timeDeltaS: Double get() = json.optDouble("time_delta_s", 0.0)
    val ahead: Boolean get() = json.optBoolean("ahead", false)
    val onPlan: Boolean get() = json.optBoolean("on_plan", false)
}

private fun JSONArray?.mapObjects(transform: (JSONObject) -> String): List<String> {
    if (this == null) return emptyList()
    // Les evenements sont des chaines, les messages des objets : on accepte les deux.
    return (0 until length()).mapNotNull { index ->
        when (val value = opt(index)) {
            is JSONObject -> transform(value)
            is String -> value
            else -> null
        }
    }
}
