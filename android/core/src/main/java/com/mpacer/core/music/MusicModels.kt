package com.mpacer.core.music

import org.json.JSONArray
import org.json.JSONObject

/**
 * Modeles musique partages avec le coeur Rust (docs/07 section 4/5).
 *
 * Regle du depot : aucun calcul de course cote Kotlin. Ces classes ne font que
 * serialiser les commandes FFI et lire l'etat publie par le moteur.
 */

// ------------------------------------------------------------------ reglages

/**
 * Reglages musique du moteur (miroir de `mpacer_core::music::MusicConfig`).
 * Les valeurs par defaut sont celles du contrat (docs/07 section 4.1), avec la
 * musique ACTIVE par defaut (le coeur fait de meme) : l'utilisateur la coupe
 * depuis l'ecran Reglages s'il n'en veut pas.
 */
data class MusicConfig(
    val enabled: Boolean = true,
    val referenceBpm: Double = 170.0,
    val referencePaceSPerKm: Double = 300.0,
    val paceElasticity: Double = 0.35,
    val minBpm: Double = 100.0,
    val maxBpm: Double = 200.0,
    val switchThresholdBpm: Double = 8.0,
    val boostBpm: Double = 6.0,
    val relaxBpm: Double = 6.0,
    val announce: Boolean = true,
    val avoidLast: Int = 3,
) {
    /** Champs snake_case attendus par le coeur (docs/07 section 5). */
    fun toJson(): JSONObject = JSONObject()
        .put("enabled", enabled)
        .put("reference_bpm", referenceBpm)
        .put("reference_pace_s_per_km", referencePaceSPerKm)
        .put("pace_elasticity", paceElasticity)
        .put("min_bpm", minBpm)
        .put("max_bpm", maxBpm)
        .put("switch_threshold_bpm", switchThresholdBpm)
        .put("boost_bpm", boostBpm)
        .put("relax_bpm", relaxBpm)
        .put("announce", announce)
        .put("avoid_last", avoidLast)
}

// ------------------------------------------------------------------ playlist

/** Piste telle que le backend la publie (docs/07 section 4.1). */
data class MusicTrack(
    val id: String,
    val title: String,
    val artist: String? = null,
    val durationS: Double = 0.0,
    /** null = tempo inconnu : piste neutre, jamais choisie pour un changement. */
    val bpm: Double? = null,
    val position: Int = 0,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("id", id)
        .put("title", title)
        .put("artist", artist ?: JSONObject.NULL)
        .put("duration_s", durationS)
        .put("bpm", bpm ?: JSONObject.NULL)
        .put("position", position)
}

/** Playlist envoyee au moteur (`set_music_playlist`). */
data class MusicPlaylist(
    val id: String,
    val name: String,
    val targetBpm: Double? = null,
    val tracks: List<MusicTrack> = emptyList(),
) {
    fun toJson(): JSONObject {
        val array = JSONArray()
        tracks.forEach { array.put(it.toJson()) }
        return JSONObject()
            .put("id", id)
            .put("name", name)
            .put("target_bpm", targetBpm ?: JSONObject.NULL)
            .put("tracks", array)
    }
}

// ---------------------------------------------------------------- maintenant

/** Instantane de la piste en cours, pousse au moteur (`music_now_playing`). */
data class NowPlaying(
    val trackId: String,
    val title: String,
    val artist: String? = null,
    val bpm: Double? = null,
    val positionS: Double = 0.0,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("track_id", trackId)
        .put("title", title)
        .put("artist", artist ?: JSONObject.NULL)
        .put("bpm", bpm ?: JSONObject.NULL)
        .put("position_s", positionS)

    companion object {
        fun from(json: JSONObject): NowPlaying = NowPlaying(
            trackId = json.optString("track_id"),
            title = json.optString("title"),
            artist = json.stringOrNull("artist"),
            bpm = json.doubleOrNull("bpm"),
            positionS = json.optDouble("position_s", 0.0),
        )
    }
}

// --------------------------------------------------------------------- etat

/** Consigne du directeur d'orchestre, serialisee en PascalCase (docs/07 section 5). */
enum class MusicDirective(val wire: String) {
    NONE("None"),
    PLAY("Play"),
    KEEP("Keep"),
    BOOST("Boost"),
    RELAX("Relax"),
    SKIP_TO("SkipTo"),
    PAUSE("Pause"),
    RESUME("Resume"),
    ;

    companion object {
        fun from(wire: String?): MusicDirective =
            entries.firstOrNull { it.wire == wire } ?: NONE
    }
}

/** Bloc `music` de EngineOutput (docs/07 section 5). */
class MusicState(private val json: JSONObject) {

    /** La musique est active par defaut (meme defaut que MusicConfig). */
    val enabled: Boolean get() = json.optBoolean("enabled", true)

    val playlistId: String? get() = json.stringOrNull("playlist_id")
    val playlistName: String? get() = json.stringOrNull("playlist_name")

    /** BPM consigne pour l'allure cible (moteur), pas le BPM de la piste. */
    val targetBpm: Double? get() = json.doubleOrNull("target_bpm")
    val targetCadenceSpm: Double? get() = json.doubleOrNull("target_cadence_spm")
    val cadenceSpm: Double? get() = json.doubleOrNull("cadence_spm")

    val current: NowPlaying? get() = json.optJSONObject("current")?.let(NowPlaying::from)
    val nextTrackId: String? get() = json.stringOrNull("next_track_id")

    val directive: MusicDirective get() = MusicDirective.from(json.optString("directive", "None"))
    val reason: String get() = json.optString("reason", "Disabled")

    /** Piste suivante connue : utile pour l'affichage de la pastille. */
    val hasNext: Boolean get() = nextTrackId != null
}

// ------------------------------------------------------------------ lecture

internal fun JSONObject.stringOrNull(name: String): String? =
    if (!has(name) || isNull(name)) null else optString(name).takeIf { it.isNotEmpty() }

internal fun JSONObject.doubleOrNull(name: String): Double? =
    if (!has(name) || isNull(name)) null else optDouble(name).takeIf { !it.isNaN() }
