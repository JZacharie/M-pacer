package com.mpacer.core

import android.content.Context
import android.content.SharedPreferences
import android.util.Log
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.net.URLEncoder
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Synchronisation des seances vers le backend auto-heberge.
 *
 * Deux responsabilites, volontairement separees du moteur Rust :
 *  1. appairer l'appareil avec le compte (device flow RFC 8628 : code utilisateur,
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

    /**
     * Scope lie au processus, et non a un ecran : l'envoi lance a la fin d'une seance
     * doit survivre a l'arret du service de suivi (qui appelle stopSelf()).
     */
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private const val KEY_TOKEN = "device_token"
    private const val KEY_BASE_URL = "base_url"
    private const val KEY_SYNCED_IDS = "synced_ids"

    /** Nombre de seances examinees a chaque reprise : au-dela, on reprendra plus tard. */
    private const val PULL_LIMIT = 200

    /**
     * Delai minimal entre deux reprises automatiques (ms).
     *
     * L'application reprend a chaque retour au premier plan : sans ce garde-fou,
     * passer d'un ecran a l'autre interrogerait le backend sans arret.
     */
    private const val PULL_MIN_INTERVAL_MS = 20_000L

    /** Echecs consecutifs au-dela desquels la reprise s'arrete : le reseau est tombe. */
    private const val MAX_PULL_FAILURES = 3

    /** Une seule reprise a la fois : le bouton manuel et la reprise auto se croisent. */
    private val pullLock = AtomicBoolean(false)

    @Volatile
    private var lastPullAttemptMs = 0L

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

    /**
     * Libelle envoye au backend pendant l'appairage. Chaque application pose le
     * sien au demarrage ("Montre M-pacer", "Telephone M-pacer") : le socle ne
     * connait aucun nom d'appareil.
     */
    @Volatile
    var pairingLabel: String = "M-pacer"

    /** Etat observe par l'ecran de synchronisation. */
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
        _state.update { it.copy(paired = false, pairing = null, phase = SyncPhase.Idle, message = "Appareil deconnecte") }
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
            val code = requestDeviceCode(context, pairingLabel)
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
                            message = "Appareil appaire",
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
    /**
     * Envoie les seances en attente sans bloquer l'appelant : utilise a la fin d'une
     * seance pour que l'utilisateur n'ait pas a ouvrir l'ecran de synchronisation.
     * Si l'appareil n'est pas appaire, l'archive reste locale (aucun echec).
     */
    fun syncInBackground(context: Context) {
        if (!isPaired(context)) {
            Log.i(TAG, "seance conservee localement : appareil non appaire")
            return
        }
        val application = context.applicationContext
        scope.launch {
            runCatching { syncPending(application) }
                .onSuccess { Log.i(TAG, "seances envoyees automatiquement : " + it) }
                .onFailure { Log.w(TAG, "envoi automatique impossible : " + it.message) }
        }
    }

    suspend fun syncPending(context: Context): Int {
        val token = token(context)
        if (token == null) {
            fail("Appareil non appaire")
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

    // ------------------------------------------------------- reprise des seances

    /**
     * Reprise automatique, sans bloquer l'appelant.
     *
     * Appelee au lancement de l'application et a l'ouverture de l'historique :
     * c'est ce qui fait arriver sur le telephone une seance courue avec la
     * montre, sans que personne ait rien a demander. Ne fait rien si l'appareil
     * n'est pas appaire, ou si une reprise vient d'avoir lieu.
     */
    fun pullInBackground(context: Context) {
        val application = context.applicationContext
        scope.launch {
            // L'etat affiche se recalcule ici : l'historique ouvre l'application
            // sans passer par l'ecran de synchronisation, et c'est pourtant lui
            // qui annonce si l'appareil est appaire. Tout se fait sur le fil IO :
            // relire l'archive et les preferences n'a rien a faire sur l'ecran.
            refresh(application)
            if (!isPaired(application)) {
                signalerReprise("Backend non appaire : les seances de la montre restent en ligne")
                return@launch
            }
            val maintenant = System.currentTimeMillis()
            if (maintenant - lastPullAttemptMs < PULL_MIN_INTERVAL_MS) return@launch
            lastPullAttemptMs = maintenant
            runCatching { pullWorkouts(application) }
                .onSuccess { Log.i(TAG, "reprise automatique : " + it + " seance(s)") }
                .onFailure { Log.w(TAG, "reprise automatique impossible : " + it.message) }
        }
    }

    /**
     * Recupere les seances du compte presentes sur le backend et absentes du
     * telephone, puis les range dans l'archive locale.
     *
     * C'est l'autre moitie de la synchronisation : la montre envoie sa seance au
     * backend des la fin de la course ([syncInBackground]), le telephone la
     * reprend ici. Aucun recalcul : le resume repris est celui qu'a produit
     * mpacer-core, trace GPS et frequence cardiaque comprises.
     *
     * @return le nombre de seances reprises pendant cet appel.
     */
    suspend fun pullWorkouts(context: Context): Int {
        val token = token(context)
        if (token == null) {
            signalerReprise("Backend non appaire : les seances de la montre restent en ligne")
            return 0
        }
        if (!pullLock.compareAndSet(false, true)) return 0
        _state.update {
            it.copy(pulling = true, pullMessage = "Recherche des seances de la montre...")
        }
        try {
            val distantes = try {
                workoutIds(get(context, token, "/api/v1/workouts?limit=" + PULL_LIMIT))
            } catch (error: Exception) {
                signalerReprise("Backend injoignable : " + (error.message ?: error.javaClass.simpleName))
                return 0
            }

            val manquantes = missingIds(distantes, localWorkoutIds(context))
            if (manquantes.isEmpty()) {
                signalerReprise("Aucune nouvelle seance")
                return 0
            }

            val dejaEnvoyees = prefs(context).getStringSet(KEY_SYNCED_IDS, emptySet())
                ?.toMutableSet() ?: mutableSetOf()
            var reprises = 0
            var echecs = 0
            var echecsDeSuite = 0
            for (id in manquantes) {
                val resume = try {
                    downloadSummary(context, token, id)
                } catch (error: Exception) {
                    // Un echec isole (seance supprimee, coupure passagere) ne doit
                    // pas priver l'utilisateur du reste du lot ; trois echecs
                    // d'affilee, en revanche, signent une panne : on s'arrete.
                    echecs += 1
                    echecsDeSuite += 1
                    Log.w(TAG, "seance " + id + " non reprise : " + error.message)
                    if (echecsDeSuite >= MAX_PULL_FAILURES) break
                    continue
                } ?: continue
                echecsDeSuite = 0
                WorkoutArchive.save(context, resume)
                // Une seance reprise vient du backend : la marquer comme deja
                // envoyee, sinon l'envoi suivant la renverrait et les deux
                // appareils se repondraient sans fin.
                val identifiant = resume.optString("id").ifBlank {
                    resume.optLong("started_at_ms").toString()
                }
                dejaEnvoyees += identifiant.ifBlank { id }
                reprises += 1
            }
            prefs(context).edit().putStringSet(KEY_SYNCED_IDS, dejaEnvoyees).apply()
            signalerReprise(
                text = when {
                    reprises == 0 && echecs == 0 -> "Aucune nouvelle seance"
                    echecs == 0 -> reprises.toString() + " seance(s) reprise(s)"
                    else -> reprises.toString() + " reprise(s), " + echecs + " en echec"
                },
                recues = reprises,
                pending = pendingCount(context),
            )
            return reprises
        } finally {
            pullLock.set(false)
            _state.update { it.copy(pulling = false) }
        }
    }

    /**
     * Identifiants des seances presentes sur le backend.
     *
     * Reponse attendue de GET /api/v1/workouts : {"total": n, "items": [{"id": ...}]}.
     * Un corps illisible ne ramene rien plutot que de faire echouer la reprise.
     */
    internal fun workoutIds(body: String): List<String> {
        val racine = runCatching { codec.parseToJsonElement(body) }.getOrNull() as? JsonObject
            ?: return emptyList()
        val items = racine["items"] as? JsonArray ?: return emptyList()
        return items.mapNotNull { element ->
            (element as? JsonObject)?.get("id")?.jsonPrimitive?.contentOrNull?.takeIf { it.isNotBlank() }
        }
    }

    /** Identifiants distants absents de l'archive locale, dans l'ordre recu. */
    internal fun missingIds(remote: List<String>, local: Set<String>): List<String> =
        remote.filter { it.isNotBlank() && it !in local }

    /** Identifiants des seances deja presentes sur le telephone. */
    private fun localWorkoutIds(context: Context): Set<String> =
        WorkoutArchive.list(context).mapNotNull { summary ->
            summary.optString("id").ifBlank { summary.optLong("started_at_ms").toString() }
                .takeIf { it.isNotBlank() }
        }.toSet()

    /** Resume complet d'une seance distante : le champ "summary" de sa fiche. */
    private suspend fun downloadSummary(context: Context, token: String, id: String): JSONObject? {
        val chemin = "/api/v1/workouts/" + URLEncoder.encode(id, "UTF-8").replace("+", "%20")
        return JSONObject(get(context, token, chemin)).optJSONObject("summary")
    }

    private suspend fun get(context: Context, token: String, chemin: String): String =
        withContext(Dispatchers.IO) {
            val request = Request.Builder()
                .url(baseUrl(context) + chemin)
                .header("Authorization", "Bearer " + token)
                .get()
                .build()
            http.newCall(request).execute().use { response ->
                val body = response.body?.string().orEmpty()
                if (!response.isSuccessful) {
                    throw ApiException(response.code, errorCode(body), "lecture refusee par le backend")
                }
                body
            }
        }

    /** Publie le compte rendu de reprise, sans toucher a l'ecran d'appairage. */
    private fun signalerReprise(
        text: String,
        recues: Int? = null,
        pending: Int? = null,
    ) {
        _state.update {
            it.copy(
                pullMessage = text,
                received = recues ?: it.received,
                lastPullMs = System.currentTimeMillis(),
                pending = pending ?: it.pending,
            )
        }
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
    /** Vrai pendant une reprise de seances. */
    val pulling: Boolean = false,
    /** Instant de la derniere tentative de reprise, aboutie ou non. */
    val lastPullMs: Long? = null,
    /** Seances reprises lors de la derniere tentative. */
    val received: Int = 0,
    /**
     * Compte rendu de la reprise, separe de [message] : la reprise tourne toute
     * seule en arriere-plan et ne doit pas ecraser ce qu'affiche l'ecran
     * d'appairage.
     */
    val pullMessage: String? = null,
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
