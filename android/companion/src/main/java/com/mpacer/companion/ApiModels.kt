package com.mpacer.companion

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Miroirs Kotlin des modeles exposes par mpacer-api. Les noms de champs JSON sont
// ceux de serde (snake_case) ; aucune conversion cote serveur n est necessaire.

// ------------------------------------------------------------------ appairage

@Serializable
data class DeviceCodeRequest(val label: String)

@Serializable
data class DeviceCodeResponse(
    @SerialName("device_code") val deviceCode: String,
    @SerialName("user_code") val userCode: String,
    @SerialName("verification_uri") val verificationUri: String,
    @SerialName("verification_uri_complete") val verificationUriComplete: String,
    @SerialName("expires_in") val expiresIn: Long = 600,
    val interval: Long = 5,
)

@Serializable
data class DeviceTokenRequest(@SerialName("device_code") val deviceCode: String)

@Serializable
data class DeviceTokenResponse(
    @SerialName("access_token") val accessToken: String,
    @SerialName("token_type") val tokenType: String = "Bearer",
    @SerialName("expires_in") val expiresIn: Long? = null,
    val label: String = "",
)

@Serializable
data class MeResponse(
    val id: String,
    val email: String,
    val name: String? = null,
    val picture: String? = null,
)

// --------------------------------------------------------------- seances

/** Version legere renvoyee par GET /api/v1/workouts. */
@Serializable
data class WorkoutRow(
    val id: String,
    @SerialName("started_at_ms") val startedAtMs: Long,
    @SerialName("duration_s") val durationS: Double,
    @SerialName("distance_m") val distanceM: Double,
    @SerialName("average_pace_s_per_km") val averagePaceSPerKm: Double,
    @SerialName("unit_system") val unitSystem: String? = null,
    @SerialName("uploaded_at_ms") val uploadedAtMs: Long? = null,
)

@Serializable
data class ListResponse(
    val total: Int = 0,
    val items: List<WorkoutRow> = emptyList(),
)

/** Tour enregistre par mpacer-core (1 km ou 1 mile). */
@Serializable
data class Lap(
    val index: Int = 0,
    @SerialName("distance_m") val distanceM: Double = 0.0,
    @SerialName("duration_s") val durationS: Double = 0.0,
    @SerialName("pace_s_per_km") val paceSPerKm: Double = 0.0,
)

@Serializable
data class BestEffort(
    val label: String = "",
    @SerialName("distance_m") val distanceM: Double = 0.0,
    @SerialName("time_s") val timeS: Double = 0.0,
    @SerialName("start_dist_m") val startDistM: Double = 0.0,
)

@Serializable
data class TrackPoint(
    @SerialName("t_ms") val tMs: Long = 0,
    @SerialName("dist_m") val distM: Double = 0.0,
    val lat: Double = 0.0,
    val lon: Double = 0.0,
    @SerialName("elevation_m") val elevationM: Double? = null,
)

/**
 * Miroir de mpacer_core::history::WorkoutSummary : meme structure que le fichier .pac.
 * C est ce JSON qui part vers POST /api/v1/workouts et vers la montre.
 */
@Serializable
data class WorkoutSummary(
    val id: String,
    @SerialName("started_at_ms") val startedAtMs: Long,
    @SerialName("duration_s") val durationS: Double,
    @SerialName("distance_m") val distanceM: Double,
    @SerialName("average_pace_s_per_km") val averagePaceSPerKm: Double,
    val laps: List<Lap> = emptyList(),
    @SerialName("best_efforts") val bestEfforts: List<BestEffort> = emptyList(),
    val track: List<TrackPoint> = emptyList(),
    @SerialName("unit_system") val unitSystem: String? = null,
)

/** Reponse de GET /api/v1/workouts/{id} : liste + resume complet. */
@Serializable
data class WorkoutDetail(
    val workout: WorkoutRow,
    val summary: WorkoutSummary? = null,
)

/** Fichier d echange .pac (mpacer_core::history::PacFile). */
@Serializable
data class PacFile(
    val format: String = "mpacer.pac",
    val version: Int = 1,
    val workouts: List<WorkoutSummary> = emptyList(),
)

/** Statistiques agregees (GET /api/v1/stats?days=30). */
@Serializable
data class Stats(
    @SerialName("workout_count") val workoutCount: Int = 0,
    @SerialName("total_distance_m") val totalDistanceM: Double = 0.0,
    @SerialName("total_duration_s") val totalDurationS: Double = 0.0,
    @SerialName("average_pace_s_per_km") val averagePaceSPerKm: Double? = null,
    @SerialName("longest_distance_m") val longestDistanceM: Double = 0.0,
)

@Serializable
data class UploadResponse(val id: String = "", val replaced: Boolean = false)
