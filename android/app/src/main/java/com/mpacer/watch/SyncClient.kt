package com.mpacer.watch

import android.content.Context
import android.content.SharedPreferences
import android.util.Log
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.util.concurrent.TimeUnit

/**
 * Synchronisation des seances vers le backend auto-heberge.
 *
 * Deux responsabilites, volontairement separees du moteur Rust :
 *  1. appairer la montre avec le compte (device flow RFC 8628 : code utilisateur,
 *     puis sondage du jeton jusqu a approbation sur la page /link) ;
 *  2. envoyer les seances locales (WorkoutArchive, format .pac) via POST /api/v1/workouts.
 *
 * Regle du depot : aucun calcul de course ici. Le WorkoutSummary envoye est exactement
 * celui produit par mpacer-core (meme structure que le fichier .pac).
 *
 * Le jeton est stocke dans EncryptedSharedPreferences (cle AES256-GCM geree par le
 * Keystore Android). Aucun secret n est ecrit en dur dans le code.
 */
object SyncClient {

    private const val TAG = "SyncClient"
    private const val PREFS_FILE = "mpacer_sync"
    private const val KEY_TOKEN = "device_token"
    private const val KEY_BASE_URL = "base_url"
    private const val KEY_SYNCED_IDS = "synced_ids"

    private val JSON_MEDIA = "application/json; charset=utf-8".toMediaType()

    /** Codec commun : tolerant aux champs ajoutes par une version plus recente du backend. */
    private val codec = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        explicitNulls = false
    }

    private val http: OkHttpClient by lazy {
        OkHttpClient.Builder()
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
            .writeTimeout(60, TimeUnit.SECONDS)
            .build()
    }

    private val _state = MutableStateFlow(SyncState())

    /** Etat observe par [com.mpacer.watch.ui.SyncScreen]. */
    val state: StateFlow<SyncState> = _state.asStateFlow()

    // ------------------------------------------------------------- preferences

    private fun prefs(context: Context): SharedPreferences = EncryptedSharedPreferences.create(
        context.applicationContext,
        PREFS_FILE,
        MasterKey.Builder(context.applicationContext)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build(),
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )

    /** URL du backend : reglage utilisateur, sinon valeur de compilation. */
    fun baseUrl(context: Context): String {
        val stored = prefs(context).getString(KEY_BASE_URL, null)
        return normalise(stored?.takeIf { it.isNotBlank() } ?: BuildConfig.DEFAULT_API_URL)
    }

    fun setBaseUrl(context: Context, url: String) {
        prefs(context).edit().putString(KEY_BASE_URL, normalise(url)).apply()
        refresh(context)
    }

    fun token(context: Context): String? =
        prefs(context).getString(KEY_TOKEN, null)?.takeIf { it.isNotBlank() }

    fun isPaired(context: Context): Boolean = token(context) != null

    /** Oublie le jeton local ; les seances deja envoyees restent marquees comme telles. */
    fun disconnect(context: Context) {
        prefs(context).edit().remove(KEY_TOKEN).apply()
        _state.update { it.copy(paired = false, pairing = null, phase = SyncPhase.Idle, message = "Montre deconnectee") }
    }

    private fun normalise(url: String): String = url.trim().trimEnd('/')

    // ------------------------------------------------------------------- etat

    /** Recalcule l etat affiche : connexion et nombre de seances en attente. */
    fun refresh(context: Context) {
        _state.update {
            it.copy(
                baseUrl = baseUrl(context),
                paired = isPaired(context),
                pending = pendingCount(context),
            )
        }
    }

    /** Seances presentes sur la montre et pas encore acceptees par le backend. */
    fun pendingCount(context: Context): Int {
        val synced = prefs(context).getStringSet(KEY_SYNCED_IDS, emptySet()) ?: emptySet()
        return WorkoutArchive.list(context).count { summary ->
            val id = summary.optString("id").ifBlank { summary.optLong("started_at_ms").toString() }
            id !in synced
        }
    }

    // ---------------------------------------------------------------- appairage

    /**
     * Demande un code d appairage. Le code affiche doit etre saisi sur /link.
     * @return le code, ou null si le backend est injoignable.
     */
    suspend fun startPairing(context: Context): DeviceCode? {
        _state.update { it.copy(phase = SyncPhase.RequestingCode, pairing = null, message = null) }
        return try {
            val code = requestDeviceCode(context, BuildConfig.PAIRING_LABEL)
            _state.update { it.copy(phase = SyncPhase.WaitingApproval, pairing = code, message = null) }
            code
        } catch (error: Exception) {
            fail("Appairage impossible : " + (error.message ?: error.javaClass.simpleName))
            null
        }
    }

    /**
     * Sonde POST /api/v1/device/token jusqu a approbation, expiration ou refus.
     * @return true si un jeton a ete enregistre.
     */
    suspend fun awaitApproval(context: Context, code: DeviceCode): Boolean {
        val deadline = System.currentTimeMillis() + code.expiresIn * 1000L
        val intervalMs = maxOf(2L, code.interval) * 1000L
        while (System.currentTimeMillis() < deadline) {
            when (val result = pollToken(context, code.deviceCode)) {
                is TokenPoll.Approved -> {
                    prefs(context).edit().putString(KEY_TOKEN, result.token.accessToken).apply()
                    _state.update {
                        it.copy(
                            paired = true,
                            pairing = null,
                            phase = SyncPhase.Done,
                            pending = pendingCount(context),
                            message = "Montre appairee",
                        )
                    }
                    return true
                }
                TokenPoll.Pending -> delay(intervalMs)
                is TokenPoll.Failed -> {
                    fail("Appairage refuse : " + result.reason)
                    return false
                }
            }
        }
        fail("Code expire : relancez l appairage")
        return false
    }

    // ------------------------------------------------------------- envoi seances

    /**
     * Envoie toutes les seances en attente. Un echec reseau interrompt la boucle
     * (les seances deja acceptees restent marquees, les autres seront renvoyees).
     * @return le nombre de seances envoyees pendant cet appel.
     */
    suspend fun syncPending(context: Context): Int {
        val token = token(context)
        if (token == null) {
            fail("Montre non appairee")
            return 0
        }
        val synced = prefs(context).getStringSet(KEY_SYNCED_IDS, emptySet())?.toMutableSet() ?: mutableSetOf()
        val workouts = WorkoutArchive.list(context)
        var uploaded = 0
        _state.update { it.copy(phase = SyncPhase.Syncing, message = "Envoi de " + workouts.size + " seance(s)...") }

        for (summary in workouts) {
            val id = summary.optString("id").ifBlank { summary.optLong("started_at_ms").toString() }
            if (id in synced) continue
            try {
                upload(context, token, summary)
                synced += id
                uploaded += 1
                prefs(context).edit().putStringSet(KEY_SYNCED_IDS, synced).apply()
            } catch (error: Exception) {
                fail("Envoi interrompu : " + (error.message ?: error.javaClass.simpleName))
                refresh(context)
                return uploaded
            }
        }

        _state.update {
            it.copy(
                phase = SyncPhase.Done,
                pending = pendingCount(context),
                message = if (uploaded == 0) "Aucune seance en attente" else uploaded.toString() + " seance(s) envoyee(s)",
            )
        }
        return uploaded
    }

    // -------------------------------------------------------------------- reseau

    private suspend fun requestDeviceCode(context: Context, label: String): DeviceCode =
        withContext(Dispatchers.IO) {
            val payload = codec.encodeToString(DeviceCodeRequest.serializer(), DeviceCodeRequest(label))
            val request = Request.Builder()
                .url(baseUrl(context) + "/api/v1/device/code")
                .post(payload.toRequestBody(JSON_MEDIA))
                .build()
            http.newCall(request).execute().use { response ->
                val body = response.body?.string().orEmpty()
                if (!response.isSuccessful) {
                    throw ApiException(response.code, errorCode(body), "demande de code refusee")
                }
                codec.decodeFromString(DeviceCode.serializer(), body)
            }
        }

    private suspend fun pollToken(context: Context, deviceCode: String): TokenPoll =
        withContext(Dispatchers.IO) {
            val payload = codec.encodeToString(DeviceTokenRequest.serializer(), DeviceTokenRequest(deviceCode))
            val request = Request.Builder()
                .url(baseUrl(context) + "/api/v1/device/token")
                .post(payload.toRequestBody(JSON_MEDIA))
                .build()
            http.newCall(request).execute().use { response ->
                val body = response.body?.string().orEmpty()
                when {
                    response.isSuccessful -> TokenPoll.Approved(codec.decodeFromString(DeviceToken.serializer(), body))
                    errorCode(body) == "authorization_pending" -> TokenPoll.Pending
                    else -> TokenPoll.Failed(errorCode(body) ?: ("HTTP " + response.code))
                }
            }
        }

    private suspend fun upload(context: Context, token: String, summary: JSONObject) =
        withContext(Dispatchers.IO) {
            // Relecture stricte du resume produit par mpacer-core avant envoi.
            val model = codec.decodeFromString(WorkoutSummary.serializer(), summary.toString())
            val payload = codec.encodeToString(WorkoutSummary.serializer(), model)
            val request = Request.Builder()
                .url(baseUrl(context) + "/api/v1/workouts")
                .header("Authorization", "Bearer " + token)
                .post(payload.toRequestBody(JSON_MEDIA))
                .build()
            http.newCall(request).execute().use { response ->
                val body = response.body?.string().orEmpty()
                if (!response.isSuccessful) {
                    throw ApiException(response.code, errorCode(body), "seance refusee par le backend")
                }
            }
        }

    private fun errorCode(body: String): String? = runCatching {
        JSONObject(body).optString("error").takeIf { it.isNotBlank() }
    }.getOrNull()

    private fun fail(message: String) {
        Log.w(TAG, message)
        _state.update { it.copy(phase = SyncPhase.Error, message = message) }
    }
}

// -------------------------------------------------------------------- modeles

/** Etat observable par l ecran d appairage. */
data class SyncState(
    val baseUrl: String = "",
    val paired: Boolean = false,
    val pending: Int = 0,
    val pairing: DeviceCode? = null,
    val phase: SyncPhase = SyncPhase.Idle,
    val message: String? = null,
)

enum class SyncPhase { Idle, RequestingCode, WaitingApproval, Syncing, Done, Error }

@Serializable
data class DeviceCodeRequest(val label: String)

@Serializable
data class DeviceCode(
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
data class DeviceToken(
    @SerialName("access_token") val accessToken: String,
    @SerialName("token_type") val tokenType: String = "Bearer",
    @SerialName("expires_in") val expiresIn: Long? = null,
    val label: String = "",
)

/**
 * Miroir de mpacer_core::history::WorkoutSummary (memes noms de champs JSON).
 * laps / best_efforts / track restent des JsonArray : la montre ne les interprete pas,
 * elle les transporte tels que le coeur les a produits.
 */
@Serializable
data class WorkoutSummary(
    val id: String,
    @SerialName("started_at_ms") val startedAtMs: Long,
    @SerialName("duration_s") val durationS: Double,
    @SerialName("distance_m") val distanceM: Double,
    @SerialName("average_pace_s_per_km") val averagePaceSPerKm: Double,
    val laps: JsonArray = JsonArray(emptyList()),
    @SerialName("best_efforts") val bestEfforts: JsonArray = JsonArray(emptyList()),
    val track: JsonArray = JsonArray(emptyList()),
    @SerialName("unit_system") val unitSystem: String? = null,
)

sealed interface TokenPoll {
    data class Approved(val token: DeviceToken) : TokenPoll
    object Pending : TokenPoll
    data class Failed(val reason: String) : TokenPoll
}

/** Erreur HTTP du backend : `statusCode` est le code machine de la reponse. */
class ApiException(val statusCode: Int, val errorCode: String?, message: String) : Exception(message)
