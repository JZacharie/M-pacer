package com.mpacer.companion

import android.app.Application
import android.net.Uri
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json

enum class PairingState { Idle, Requesting, Waiting, Paired, Failed }

/** Etat complet de l interface telephone. */
data class UiState(
    val baseUrl: String = "",
    val paired: Boolean = false,
    val pairing: DeviceCodeResponse? = null,
    val pairingState: PairingState = PairingState.Idle,
    val me: MeResponse? = null,
    val workouts: List<WorkoutRow> = emptyList(),
    val total: Int = 0,
    val stats: Stats? = null,
    val detail: WorkoutDetail? = null,
    val busy: Boolean = false,
    val message: String? = null,
    val syncLog: List<String> = emptyList(),
    val watchNodes: Int = 0,
)

/**
 * Detient l etat de l application telephone : appairage, liste des seances,
 * detail, import d un fichier .pac/JSON et envoi vers la montre.
 *
 * Aucun calcul de course ici : les valeurs affichees viennent du backend, qui les
 * tient du coeur Rust.
 */
class AppViewModel(application: Application) : AndroidViewModel(application) {

    private val store = TokenStore(application)
    private val codec = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        explicitNulls = false
    }

    private val _state = MutableStateFlow(
        UiState(baseUrl = store.baseUrl, paired = store.token != null)
    )
    val state: StateFlow<UiState> = _state.asStateFlow()

    private var pairingJob: Job? = null

    init {
        countWatchNodes()
        if (store.token != null) refresh()
    }

    private fun api(): MpacerApi = MpacerApi(store.baseUrl)

    // -------------------------------------------------------------- reglages

    fun setBaseUrl(url: String) {
        store.baseUrl = url
        _state.update { it.copy(baseUrl = store.baseUrl) }
    }

    fun clearMessage() {
        _state.update { it.copy(message = null) }
    }

    // ------------------------------------------------------------- appairage

    /** Ouvre un flux d appairage et sonde le jeton jusqu a validation sur /link. */
    fun startPairing() {
        if (pairingJob?.isActive == true) return
        pairingJob = viewModelScope.launch {
            _state.update { it.copy(pairingState = PairingState.Requesting, message = null) }
            try {
                val code = api().requestDeviceCode(BuildConfig.DEVICE_LABEL)
                _state.update { it.copy(pairing = code, pairingState = PairingState.Waiting) }
                val deadline = System.currentTimeMillis() + code.expiresIn * 1000L
                val interval = maxOf(2L, code.interval) * 1000L
                while (System.currentTimeMillis() < deadline) {
                    when (val result = api().pollDeviceToken(code.deviceCode)) {
                        is DeviceTokenResult.Approved -> {
                            store.token = result.response.accessToken
                            _state.update {
                                it.copy(
                                    paired = true,
                                    pairing = null,
                                    pairingState = PairingState.Paired,
                                    message = "Connexion reussie",
                                )
                            }
                            refresh()
                            return@launch
                        }
                        DeviceTokenResult.Pending -> delay(interval)
                        is DeviceTokenResult.Failed -> {
                            _state.update {
                                it.copy(
                                    pairingState = PairingState.Failed,
                                    message = "Appairage refuse : " + result.reason,
                                )
                            }
                            return@launch
                        }
                    }
                }
                _state.update { it.copy(pairingState = PairingState.Failed, message = "Code expire") }
            } catch (error: Exception) {
                _state.update {
                    it.copy(
                        pairingState = PairingState.Failed,
                        message = "Backend injoignable : " + (error.message ?: error.javaClass.simpleName),
                    )
                }
            }
        }
    }

    fun cancelPairing() {
        pairingJob?.cancel()
        pairingJob = null
        _state.update { it.copy(pairing = null, pairingState = PairingState.Idle, message = null) }
    }

    fun disconnect() {
        store.clearToken()
        _state.update {
            it.copy(
                paired = false,
                me = null,
                workouts = emptyList(),
                stats = null,
                detail = null,
                pairing = null,
                pairingState = PairingState.Idle,
                message = "Deconnecte",
            )
        }
    }

    // ---------------------------------------------------------------- seances

    fun refresh() {
        val token = store.token ?: return
        viewModelScope.launch {
            _state.update { it.copy(busy = true, message = null) }
            try {
                val list = api().listWorkouts(token)
                val stats = api().stats(token)
                val me = runCatching { api().me(token) }.getOrNull()
                _state.update {
                    it.copy(
                        busy = false,
                        workouts = list.items,
                        total = list.total,
                        stats = stats,
                        me = me,
                    )
                }
            } catch (error: Exception) {
                _state.update {
                    it.copy(
                        busy = false,
                        message = "Chargement impossible : " + (error.message ?: error.javaClass.simpleName),
                    )
                }
            }
        }
    }

    fun openDetail(id: String) {
        val token = store.token ?: return
        viewModelScope.launch {
            _state.update { it.copy(busy = true, detail = null) }
            try {
                val detail = api().getWorkout(token, id)
                _state.update { it.copy(busy = false, detail = detail) }
            } catch (error: Exception) {
                _state.update {
                    it.copy(
                        busy = false,
                        message = "Detail indisponible : " + (error.message ?: error.javaClass.simpleName),
                    )
                }
            }
        }
    }

    fun clearDetail() {
        _state.update { it.copy(detail = null) }
    }

    // ------------------------------------------------------------- import local

    /** Importe un fichier .pac, un tableau JSON ou un WorkoutSummary unique. */
    fun importFile(uri: Uri) {
        val token = store.token ?: return
        viewModelScope.launch {
            _state.update { it.copy(busy = true, message = null, syncLog = emptyList()) }
            try {
                val text = readText(uri) ?: throw IllegalStateException("fichier illisible")
                val workouts = parseWorkouts(text)
                if (workouts.isEmpty()) throw IllegalStateException("aucune seance reconnue")
                var sent = 0
                for (workout in workouts) {
                    api().uploadWorkout(token, workout)
                    sent += 1
                    log("Envoyee : " + workout.id)
                }
                _state.update { it.copy(busy = false, message = sent.toString() + " seance(s) envoyee(s)") }
                refresh()
            } catch (error: Exception) {
                _state.update {
                    it.copy(
                        busy = false,
                        message = "Import impossible : " + (error.message ?: error.javaClass.simpleName),
                    )
                }
            }
        }
    }

    private fun readText(uri: Uri): String? =
        getApplication<Application>().contentResolver.openInputStream(uri)
            ?.use { stream -> stream.bufferedReader().readText() }

    private fun parseWorkouts(text: String): List<WorkoutSummary> {
        runCatching { codec.decodeFromString(PacFile.serializer(), text) }
            .getOrNull()
            ?.takeIf { it.workouts.isNotEmpty() }
            ?.let { return it.workouts }
        runCatching { codec.decodeFromString(ListSerializer(WorkoutSummary.serializer()), text) }
            .getOrNull()
            ?.let { return it }
        runCatching { codec.decodeFromString(WorkoutSummary.serializer(), text) }
            .getOrNull()
            ?.let { return listOf(it) }
        return emptyList()
    }

    // --------------------------------------------------------------- montre

    fun countWatchNodes() {
        viewModelScope.launch {
            val nodes = withContext(Dispatchers.IO) {
                runCatching { WearSync.reachableNodeIds(getApplication()) }.getOrDefault(emptyList())
            }
            _state.update { it.copy(watchNodes = nodes.size) }
        }
    }

    /** Envoie la seance actuellement affichee vers la montre. */
    fun sendToWatch() {
        val summary = _state.value.detail?.summary
        if (summary == null) {
            _state.update { it.copy(message = "Aucun resume a envoyer") }
            return
        }
        viewModelScope.launch {
            _state.update { it.copy(busy = true, message = null) }
            val payload = codec
                .encodeToString(WorkoutSummary.serializer(), summary)
                .toByteArray(Charsets.UTF_8)
            val outcome = withContext(Dispatchers.IO) {
                WearSync.sendPayload(getApplication(), payload)
            }
            val message = when (outcome) {
                is WearSync.Outcome.Sent ->
                    "Envoye a la montre (" + outcome.nodes + " appareil(s), " + payload.size + " octets)"
                WearSync.Outcome.NoDevice -> "Aucune montre connectee"
                is WearSync.Outcome.Failed -> "Envoi impossible : " + outcome.reason
            }
            _state.update { it.copy(busy = false, message = message) }
        }
    }

    private fun log(entry: String) {
        _state.update { it.copy(syncLog = it.syncLog + entry) }
    }
}
