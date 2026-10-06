package com.mpacer.companion

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.MediaType.Companion.toMediaTypeOrNull
import okhttp3.MultipartBody
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.util.concurrent.TimeUnit

/**
 * Client HTTP du backend mpacer-api.
 *
 * Toutes les methodes sont suspendues et executent l appel hors du thread principal.
 * L authentification se fait par en-tete `Authorization: Bearer <jeton d appareil>`.
 * Les erreurs serveur respectent le format {"error": "code", "message": "..."}.
 */
class MpacerApi(private val baseUrl: String) {

    private val codec = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        explicitNulls = false
    }

    private val jsonMedia = "application/json; charset=utf-8".toMediaType()

    private val http: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .writeTimeout(60, TimeUnit.SECONDS)
        .build()

    private fun url(path: String): String = baseUrl.trimEnd('/') + path

    // ------------------------------------------------------------- appairage

    suspend fun requestDeviceCode(label: String): DeviceCodeResponse = post(
        path = "/api/v1/device/code",
        body = codec.encodeToString(DeviceCodeRequest.serializer(), DeviceCodeRequest(label)),
        token = null,
        deserializer = DeviceCodeResponse.serializer(),
    )

    /**
     * Sonde le flux d appairage. Un HTTP 400 authorization_pending est le cas normal
     * tant que l utilisateur n a pas valide le code sur la page /link.
     */
    suspend fun pollDeviceToken(deviceCode: String): DeviceTokenResult = withContext(Dispatchers.IO) {
        val body = codec.encodeToString(DeviceTokenRequest.serializer(), DeviceTokenRequest(deviceCode))
        val request = Request.Builder()
            .url(url("/api/v1/device/token"))
            .post(body.toRequestBody(jsonMedia))
            .build()
        http.newCall(request).execute().use { response ->
            val text = response.body?.string().orEmpty()
            when {
                response.isSuccessful -> DeviceTokenResult.Approved(
                    codec.decodeFromString(DeviceTokenResponse.serializer(), text)
                )
                errorCode(text) == "authorization_pending" -> DeviceTokenResult.Pending
                else -> DeviceTokenResult.Failed(errorCode(text) ?: ("HTTP " + response.code))
            }
        }
    }

    suspend fun me(token: String): MeResponse =
        get("/api/v1/me", token, MeResponse.serializer())

    // ---------------------------------------------------------------- seances

    suspend fun listWorkouts(token: String, limit: Int = 50, offset: Int = 0): ListResponse =
        get("/api/v1/workouts?limit=" + limit + "&offset=" + offset, token, ListResponse.serializer())

    suspend fun getWorkout(token: String, id: String): WorkoutDetail =
        get("/api/v1/workouts/" + id, token, WorkoutDetail.serializer())

    suspend fun uploadWorkout(token: String, summary: WorkoutSummary): UploadResponse = post(
        path = "/api/v1/workouts",
        body = codec.encodeToString(WorkoutSummary.serializer(), summary),
        token = token,
        deserializer = UploadResponse.serializer(),
    )

    suspend fun stats(token: String, days: Int = 30): Stats =
        get("/api/v1/stats?days=" + days, token, Stats.serializer())

    // ---------------------------------------------------------------- musique

    /** Playlists preparees cote serveur (docs/07 section 6.3). */
    suspend fun listMusicPlaylists(token: String): MusicPlaylistListResponse =
        get("/api/v1/music/playlists", token, MusicPlaylistListResponse.serializer())

    /**
     * Televerse des fichiers audio (endpoint appareil).
     *
     * `POST /api/v1/music/playlists` : multipart avec le champ `name` et un champ
     * `files` repete par fichier, jeton d'appareil en Bearer, memes gardes que
     * /api/v1/workouts. Reponse :
     * {playlist_id, name, track_count, total_bytes, ready_track_count}.
     */
    suspend fun uploadMusic(token: String, playlistName: String, files: List<UploadFile>): MusicUploadResponse =
        withContext(Dispatchers.IO) {
            val body = MultipartBody.Builder()
                .setType(MultipartBody.FORM)
                .addFormDataPart("name", playlistName)
            files.forEach { file ->
                body.addFormDataPart(
                    "files",
                    file.name,
                    file.bytes.toRequestBody(file.mime?.toMediaTypeOrNull()),
                )
            }
            val request = Request.Builder()
                .url(url("/api/v1/music/playlists"))
                .header("Authorization", "Bearer " + token)
                .post(body.build())
                .build()
            http.newCall(request).execute().use { response ->
                val text = response.body?.string().orEmpty()
                if (!response.isSuccessful) {
                    throw ApiException(response.code, errorCode(text), "televersement refuse")
                }
                codec.decodeFromString(MusicUploadResponse.serializer(), text)
            }
        }

    // ------------------------------------------------------------------ outils

    private suspend fun <T> get(
        path: String,
        token: String,
        deserializer: DeserializationStrategy<T>,
    ): T = withContext(Dispatchers.IO) {
        val request = Request.Builder()
            .url(url(path))
            .header("Authorization", "Bearer " + token)
            .get()
            .build()
        http.newCall(request).execute().use { response ->
            val text = response.body?.string().orEmpty()
            if (!response.isSuccessful) {
                throw ApiException(response.code, errorCode(text), "requete refusee")
            }
            codec.decodeFromString(deserializer, text)
        }
    }

    private suspend fun <T> post(
        path: String,
        body: String,
        token: String?,
        deserializer: DeserializationStrategy<T>,
    ): T = withContext(Dispatchers.IO) {
        val builder = Request.Builder()
            .url(url(path))
            .post(body.toRequestBody(jsonMedia))
        if (token != null) builder.header("Authorization", "Bearer " + token)
        http.newCall(builder.build()).execute().use { response ->
            val text = response.body?.string().orEmpty()
            if (!response.isSuccessful) {
                throw ApiException(response.code, errorCode(text), "requete refusee")
            }
            codec.decodeFromString(deserializer, text)
        }
    }

    private fun errorCode(body: String): String? = runCatching {
        JSONObject(body).optString("error").takeIf { it.isNotBlank() }
    }.getOrNull()
}

sealed interface DeviceTokenResult {
    data class Approved(val response: DeviceTokenResponse) : DeviceTokenResult
    object Pending : DeviceTokenResult
    data class Failed(val reason: String) : DeviceTokenResult
}

/** Erreur HTTP du backend : `statusCode` est le code machine de la reponse. */
class ApiException(val statusCode: Int, val errorCode: String?, message: String) : Exception(message)
