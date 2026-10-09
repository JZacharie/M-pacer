package com.mpacer.core.social

import android.content.Context
import android.util.Log
import com.mpacer.core.SyncClient
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.util.concurrent.TimeUnit

/**
 * Amis et partage de la position en direct, cote application.
 *
 * Le backend fait le travail : il garde le cercle, les codes d'invitation, les
 * revendications d'appareils et les positions publiees sur le broker MQTT (voir
 * `crates/mpacer-api/src/friends.rs`). Ici, on ne fait que parler son API avec
 * le jeton d'appareil deja utilise pour la synchronisation des seances.
 *
 * Rien n'est calcule ici : les distances, allures, zones et ages viennent du
 * serveur, exactement comme les valeurs de seance viennent du coeur Rust.
 */
object FriendsClient {

    private const val TAG = "FriendsClient"
    private const val JSON_MEDIA = "application/json; charset=utf-8"

    private val portee = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val codec = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        explicitNulls = false
    }

    private val http: OkHttpClient by lazy {
        OkHttpClient.Builder()
            .connectTimeout(10, TimeUnit.SECONDS)
            .readTimeout(20, TimeUnit.SECONDS)
            .build()
    }

    private val _state = MutableStateFlow(FriendsState())
    /** Etat observe par l'ecran Amis. */
    val state: StateFlow<FriendsState> = _state.asStateFlow()

    private val mediaJson = JSON_MEDIA.toMediaType()

    // ------------------------------------------------------------------ lecture

    /** Recharge le cercle : amis, partage et positions courantes. */
    suspend fun load(context: Context): Boolean = withContext(Dispatchers.IO) {
        val token = SyncClient.token(context)
        if (token == null) {
            _state.update {
                it.copy(
                    error = "Appairez d'abord l'appareil (onglet Synchronisation) pour utiliser les amis.",
                )
            }
            return@withContext false
        }
        val url = SyncClient.baseUrl(context) + "/api/v1/friends/live?trace=1"
        val reponse = try {
            http.newCall(
                Request.Builder()
                    .url(url)
                    .header("Authorization", "Bearer " + token)
                    .build()
            ).execute()
        } catch (erreur: Exception) {
            Log.w(TAG, "cercle injoignable", erreur)
            _state.update { it.copy(error = "Serveur injoignable : " + message(erreur), busy = false) }
            return@withContext false
        }
        reponse.use { resultat ->
            val corps = resultat.body?.string().orEmpty()
            if (!resultat.isSuccessful) {
                _state.update {
                    it.copy(error = erreurLisible(resultat.code, corps), busy = false)
                }
                return@withContext false
            }
            val cercle = runCatching { codec.decodeFromString(Circle.serializer(), corps) }
                .getOrElse { erreur ->
                    Log.w(TAG, "cercle illisible", erreur)
                    _state.update { it.copy(error = "Reponse illisible du serveur", busy = false) }
                    return@withContext false
                }
            _state.update {
                it.copy(
                    circle = cercle,
                    json = corps,
                    error = null,
                    busy = false,
                    charge = true,
                )
            }
            return@withContext true
        }
    }

    // --------------------------------------------------------------- partage

    /** Active ou coupe le partage de sa position. */
    suspend fun setShare(context: Context, partage: Boolean): Boolean {
        val envoye = poster(context, "/api/v1/friends/share", "{\"share_live\":$partage}", methode = "PUT")
        if (envoye) {
            _state.update {
                it.copy(
                    circle = it.circle?.copy(shareLive = partage),
                    message = if (partage) {
                        "Partage active : vos amis voient votre position pendant vos seances."
                    } else {
                        "Partage coupe : plus aucune position ne sort de votre compte."
                    },
                )
            }
        }
        return envoye
    }

    // ------------------------------------------------------------ invitations

    /** Demande un code d'invitation (le code en cours est reutilise). */
    suspend fun createInvite(context: Context, nouvelle: Boolean = false): Boolean {
        val corps = postLire(context, "/api/v1/friends/invite", "{\"nouvelle\":$nouvelle}")
        if (corps == null) return false
        val invitation = runCatching { codec.decodeFromString(Invite.serializer(), corps) }.getOrNull()
        if (invitation == null) {
            _state.update { it.copy(error = "Reponse illisible du serveur") }
            return false
        }
        _state.update { it.copy(invite = invitation, message = null, error = null) }
        return true
    }

    /** Ajoute un ami a partir de son code. */
    suspend fun accept(context: Context, code: String): Boolean {
        val propre = code.trim().uppercase()
        if (propre.isBlank()) return false
        val corps = codec.encodeToString(
            AcceptRequest.serializer(),
            AcceptRequest(propre),
        )
        val reponse = postLire(context, "/api/v1/friends/accept", corps)
        if (reponse == null) return false
        _state.update { it.copy(message = "Ami ajoute : vous partagez vos positions en direct.", error = null) }
        load(context)
        return true
    }

    /** Retire un ami. */
    suspend fun removeFriend(context: Context, id: String): Boolean {
        val envoye = poster(context, "/api/v1/friends/" + id, null, methode = "DELETE")
        if (envoye) {
            _state.update { it.copy(message = "Ami retire.", error = null) }
            load(context)
        }
        return envoye
    }

    // ---------------------------------------------------- demandes d'amitie

    /**
     * Recharge les demandes d'amitie recues et envoyees.
     *
     * L'identite est celle du compte M-pacer (adresse ou nom) : aucun nom de
     * montre n'entre dans une demande.
     */
    suspend fun loadRequests(context: Context): Boolean {
        val corps = lire(context, "/api/v1/friends/requests") ?: return false
        val demandes = runCatching { codec.decodeFromString(FriendRequests.serializer(), corps) }
            .getOrElse { erreur ->
                Log.w(TAG, "demandes illisibles", erreur)
                _state.update { it.copy(error = "Reponse illisible du serveur") }
                return false
            }
        // Une demande qui apparait entre deux chargements declenche une
        // notification systeme ; la comparaison des identifiants evite de
        // notifier deux fois la meme demande.
        val precedentes = _state.value.requests?.incoming?.map { it.id }?.toSet()
        _state.update { it.copy(requests = demandes, error = null) }
        if (precedentes != null) {
            val nouvelles = demandes.incoming.filter { it.id !in precedentes }
            if (nouvelles.isNotEmpty()) {
                FriendsNotifier.notifyNewRequests(context, nouvelles)
            }
        }
        return true
    }

    /** Cherche un compte M-pacer par son adresse ou son nom. */
    suspend fun search(context: Context, requete: String): Boolean {
        val texte = requete.trim()
        if (texte.length < 2) {
            _state.update { it.copy(search = emptyList(), searchQuery = texte, error = null) }
            return true
        }
        val corps = lire(context, "/api/v1/friends/search?q=" + encoder(texte)) ?: return false
        val reponse = runCatching { codec.decodeFromString(SearchResponse.serializer(), corps) }
            .getOrElse { erreur ->
                Log.w(TAG, "recherche illisible", erreur)
                _state.update { it.copy(error = "Reponse illisible du serveur") }
                return false
            }
        _state.update { it.copy(search = reponse.users, searchQuery = texte, error = null) }
        return true
    }

    /** Envoie une demande d'amitie a un compte (par son adresse). */
    suspend fun sendRequest(context: Context, email: String): Boolean {
        val adresse = email.trim()
        if (adresse.isBlank()) return false
        val corps = codec.encodeToString(
            RequestInput.serializer(),
            RequestInput(adresse),
        )
        if (postLire(context, "/api/v1/friends/requests", corps) == null) return false
        _state.update {
            it.copy(
                message = "Demande envoyee a " + adresse + " : elle devient une amitie quand l'autre accepte.",
                search = emptyList(),
                error = null,
            )
        }
        loadRequests(context)
        return true
    }

    /** Accepte une demande recue : l'amitie est creee dans les deux sens. */
    suspend fun acceptRequest(context: Context, id: String): Boolean {
        if (!poster(context, "/api/v1/friends/requests/" + id + "/accept", null)) return false
        _state.update { it.copy(message = "Demande acceptee : vous etes maintenant amis.", error = null) }
        load(context)
        loadRequests(context)
        return true
    }

    /** Refuse une demande recue : elle disparait, rien d'autre ne change. */
    suspend fun declineRequest(context: Context, id: String): Boolean {
        if (!poster(context, "/api/v1/friends/requests/" + id + "/decline", null)) return false
        _state.update { it.copy(message = "Demande refusee.", error = null) }
        loadRequests(context)
        return true
    }

    /** Annule une demande envoyee : elle disparait, rien d'autre ne change. */
    suspend fun cancelRequest(context: Context, id: String): Boolean {
        if (!poster(context, "/api/v1/friends/requests/" + id, null, methode = "DELETE")) return false
        _state.update { it.copy(message = "Demande annulee.", error = null) }
        loadRequests(context)
        return true
    }

    // -------------------------------------------------- revendication directe

    /**
     * Revendique le nom d'appareil aupres du backend, au depart d'une seance.
     *
     * C'est ce qui relie le sujet MQTT au compte : sans cette revendication,
     * aucune position ne sort du serveur. L'appel est fait au mieux : sans
     * jeton, sans reseau ou avec un nom deja pris, la seance continue — seul le
     * partage avec les amis est indisponible.
     */
    fun registerDevice(context: Context, device: String, label: String? = null) {
        if (device.isBlank()) return
        val application = context.applicationContext
        portee.launch {
            val token = SyncClient.token(application)
            if (token == null) {
                Log.i(TAG, "appareil non revendique : appareil non appaire")
                return@launch
            }
            val corps = codec.encodeToString(
                RegisterRequest.serializer(),
                RegisterRequest(device = device, label = label),
            )
            val envoye = poster(application, "/api/v1/live/register", corps, tokenForce = token)
            if (envoye) Log.i(TAG, "appareil revendique pour le partage : " + device)
        }
    }

    // ------------------------------------------------------------------ reseau

    private suspend fun poster(
        context: Context,
        chemin: String,
        corps: String?,
        methode: String = "POST",
        tokenForce: String? = null,
    ): Boolean = withContext(Dispatchers.IO) {
        val token = tokenForce ?: SyncClient.token(context) ?: return@withContext signaler("Appareil non appaire")
        val constructeur = Request.Builder()
            .url(SyncClient.baseUrl(context) + chemin)
            .header("Authorization", "Bearer " + token)
        val requete = when (methode) {
            "PUT" -> constructeur.put((corps ?: "{}").toRequestBody(mediaJson))
            "DELETE" -> constructeur.delete()
            else -> constructeur.post((corps ?: "{}").toRequestBody(mediaJson))
        }.build()
        try {
            http.newCall(requete).execute().use { reponse ->
                if (reponse.isSuccessful) {
                    _state.update { it.copy(error = null) }
                    true
                } else {
                    signaler(erreurLisible(reponse.code, reponse.body?.string().orEmpty()))
                }
            }
        } catch (erreur: Exception) {
            Log.w(TAG, "appel " + chemin + " impossible", erreur)
            signaler("Serveur injoignable : " + message(erreur))
        }
    }

    /** GET dont on veut lire la reponse (demandes, recherche). */
    private suspend fun lire(context: Context, chemin: String): String? =
        withContext(Dispatchers.IO) {
            val token = SyncClient.token(context) ?: return@withContext signalerLire(
                "Appairez d'abord l'appareil (onglet Synchronisation)."
            )
            val requete = Request.Builder()
                .url(SyncClient.baseUrl(context) + chemin)
                .header("Authorization", "Bearer " + token)
                .get()
                .build()
            try {
                http.newCall(requete).execute().use { reponse ->
                    val texte = reponse.body?.string().orEmpty()
                    if (reponse.isSuccessful) {
                        _state.update { it.copy(error = null) }
                        texte
                    } else {
                        signalerLire(erreurLisible(reponse.code, texte))
                    }
                }
            } catch (erreur: Exception) {
                Log.w(TAG, "appel " + chemin + " impossible", erreur)
                signalerLire("Serveur injoignable : " + message(erreur))
            }
        }

    /** Encodage d'une recherche dans une URL. */
    private fun encoder(texte: String): String =
        java.net.URLEncoder.encode(texte, "UTF-8")

    /** POST dont on veut lire la reponse (invitation, ajout d'ami). */
    private suspend fun postLire(context: Context, chemin: String, corps: String): String? =
        withContext(Dispatchers.IO) {
            val token = SyncClient.token(context) ?: return@withContext signalerLire(
                "Appairez d'abord l'appareil (onglet Synchronisation)."
            )
            val requete = Request.Builder()
                .url(SyncClient.baseUrl(context) + chemin)
                .header("Authorization", "Bearer " + token)
                .post(corps.toRequestBody(mediaJson))
                .build()
            try {
                http.newCall(requete).execute().use { reponse ->
                    val texte = reponse.body?.string().orEmpty()
                    if (reponse.isSuccessful) {
                        _state.update { it.copy(error = null) }
                        texte
                    } else {
                        signalerLire(erreurLisible(reponse.code, texte))
                    }
                }
            } catch (erreur: Exception) {
                Log.w(TAG, "appel " + chemin + " impossible", erreur)
                signalerLire("Serveur injoignable : " + message(erreur))
            }
        }

    private fun signaler(texte: String): Boolean {
        _state.update { it.copy(error = texte, busy = false) }
        return false
    }

    private fun signalerLire(texte: String): String? {
        _state.update { it.copy(error = texte, busy = false) }
        return null
    }

    /** Message du backend (`{"error": ..., "message": ...}`), sinon le code HTTP. */
    private fun erreurLisible(codeHttp: Int, corps: String): String = runCatching {
        val objet = org.json.JSONObject(corps)
        objet.optString("message").takeIf { it.isNotBlank() }
    }.getOrNull() ?: ("Erreur serveur (" + codeHttp + ")")

    private fun message(erreur: Exception): String =
        erreur.message ?: erreur.javaClass.simpleName
}

// ------------------------------------------------------------------- modeles

/** Etat de l'ecran Amis. */
data class FriendsState(
    val circle: Circle? = null,
    /** Charge utile brute du serveur : la carte du telephone la rejoue telle quelle. */
    val json: String = "",
    val invite: Invite? = null,
    /** Demandes recues et envoyees. */
    val requests: FriendRequests? = null,
    /** Resultats de la derniere recherche de compte. */
    val search: List<UserSummary> = emptyList(),
    val searchQuery: String = "",
    val busy: Boolean = false,
    val charge: Boolean = false,
    val message: String? = null,
    val error: String? = null,
)

/** Cercle d'amis tel que le backend le renvoie. */
@Serializable
data class Circle(
    @SerialName("now_ms") val nowMs: Long = 0,
    @SerialName("public_url") val publicUrl: String = "",
    @SerialName("share_live") val shareLive: Boolean = true,
    /** Demandes d'amitie recues et pas encore validees. */
    @SerialName("pending_requests") val pendingRequests: Int = 0,
    val total: Int = 0,
    val live: Int = 0,
    val me: FriendPosition? = null,
    val friends: List<Friend> = emptyList(),
)

/** Un ami et sa position quand elle est partageable. */
@Serializable
data class Friend(
    val id: String,
    val name: String = "",
    val email: String = "",
    @SerialName("picture_url") val pictureUrl: String? = null,
    @SerialName("since_ms") val sinceMs: Long = 0,
    val sharing: Boolean = false,
    val live: FriendPosition? = null,
) {
    /** Nom affichable : le nom, sinon l'adresse. */
    val displayName: String get() = name.ifBlank { email }
}

/** Position en direct d'un ami ou de soi-meme. */
@Serializable
data class FriendPosition(
    val device: String = "",
    val state: String = "run",
    val lat: Double = 0.0,
    val lon: Double = 0.0,
    @SerialName("accuracy_m") val accuracyM: Double? = null,
    @SerialName("last_ms") val lastMs: Long = 0,
    @SerialName("age_s") val ageS: Long = 0,
    @SerialName("distance_m") val distanceM: Double? = null,
    @SerialName("pace_s_per_km") val paceSPerKm: Double? = null,
    @SerialName("heart_rate_bpm") val heartRateBpm: Int? = null,
    @SerialName("battery_percent") val batteryPercent: Int? = null,
    val lap: Int? = null,
    val trace: List<List<Double>> = emptyList(),
) {
    /** Vrai si la seance est en cours (l'ami court encore). */
    val enCourse: Boolean get() = state != "stop" && state != "pause"
}

/** Code d'invitation renvoye par le backend. */
@Serializable
data class Invite(
    val code: String,
    @SerialName("expires_at_ms") val expiresAtMs: Long = 0,
    @SerialName("expires_in_s") val expiresInS: Long = 0,
    val url: String = "",
)

/** Fiche minimale d'un compte M-pacer (recherche, demande). */
@Serializable
data class UserSummary(
    val id: String = "",
    val name: String? = null,
    val email: String = "",
    @SerialName("picture_url") val pictureUrl: String? = null,
) {
    /** Nom affichable : le nom, sinon l'adresse. */
    val displayName: String get() = name?.takeIf { it.isNotBlank() } ?: email
}

/** Demande d'amitie en attente de reponse. */
@Serializable
data class FriendRequest(
    val id: String = "",
    /** "in" (recue, a valider) ou "out" (envoyee). */
    val direction: String = "in",
    val from: UserSummary = UserSummary(),
    val to: UserSummary = UserSummary(),
    val message: String? = null,
    @SerialName("created_at_ms") val createdAtMs: Long = 0,
) {
    val recue: Boolean get() = direction == "in"
    /** L'autre bout de la demande : celui a qui on repond, ou qui doit repondre. */
    val autre: UserSummary get() = if (recue) from else to
}

/** Reponse de la liste des demandes. */
@Serializable
data class FriendRequests(
    val incoming: List<FriendRequest> = emptyList(),
    val outgoing: List<FriendRequest> = emptyList(),
    val pending: Int = 0,
)

/** Reponse de la recherche de comptes. */
@Serializable
private data class SearchResponse(
    val query: String = "",
    val users: List<UserSummary> = emptyList(),
)

/** Corps d'une demande d'amitie. */
@Serializable
private data class RequestInput(val email: String)

@Serializable
private data class RegisterRequest(val device: String, val label: String? = null)

@Serializable
private data class AcceptRequest(val code: String)
