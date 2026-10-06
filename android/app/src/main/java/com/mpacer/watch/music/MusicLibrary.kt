package com.mpacer.watch.music

import android.content.Context
import android.util.Log
import com.mpacer.watch.SyncClient
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.File
import java.io.FileOutputStream
import java.util.Locale
import java.util.concurrent.TimeUnit

/**
 * Bibliotheque musicale locale de la montre (docs/07 section 7.2).
 *
 * Responsabilites :
 *  1. lire la fiche des playlists du backend (jeton d'appareil, meme garde que
 *     /api/v1/workouts) ;
 *  2. telecharger les fichiers audio des playlists "upload" dans filesDir/music/ ;
 *  3. tenir un index JSON local (fiches + chemins des fichiers) ;
 *  4. recevoir les plans de telechargement (Data Layer ou GET /api/v1/music/prepare).
 *
 * Aucun calcul de course ici : le BPM affiche vient du backend ou du coeur Rust.
 */
object MusicLibrary {

    private const val TAG = "MusicLibrary"
    private const val DIRECTORY = "music"
    private const val INDEX_FILE = "index.json"
    private const val PLANS_FILE = "plans.json"
    private const val JSON_MEDIA = "application/json; charset=utf-8"

    /** Seuil de mise a jour de la progression (evite de repeindre a chaque bloc). */
    private const val PROGRESS_STEP_BYTES = 256L * 1024L

    private val codec = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        explicitNulls = false
    }

    private val http: OkHttpClient by lazy {
        OkHttpClient.Builder()
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(60, TimeUnit.SECONDS)
            .build()
    }

    private val _state = MutableStateFlow(MusicLibraryState())
    val state: StateFlow<MusicLibraryState> = _state.asStateFlow()

    // ------------------------------------------------------------------ index

    private fun directory(context: Context): File =
        File(context.filesDir, DIRECTORY).apply { mkdirs() }

    fun local(context: Context, playlistId: String): LocalPlaylist? =
        readIndex(context).playlists.firstOrNull { it.id == playlistId }

    private fun readIndex(context: Context): LocalIndex {
        val file = File(directory(context), INDEX_FILE)
        if (!file.exists()) return LocalIndex()
        return runCatching {
            codec.decodeFromString(LocalIndex.serializer(), file.readText())
        }.getOrElse {
            Log.w(TAG, "index musique illisible", it)
            LocalIndex()
        }
    }

    private fun writeIndex(context: Context, index: LocalIndex) {
        runCatching {
            File(directory(context), INDEX_FILE).writeText(
                codec.encodeToString(LocalIndex.serializer(), index)
            )
        }.onFailure { Log.w(TAG, "index musique non enregistre", it) }
        _state.update { it.copy(downloaded = index.playlists.associateBy { playlist -> playlist.id }) }
    }

    /** Recharge l'index local (appele a l'ouverture de l'ecran Musique). */
    fun reloadLocal(context: Context) {
        _state.update { it.copy(downloaded = readIndex(context).playlists.associateBy { playlist -> playlist.id }) }
    }

    /** Supprime une playlist locale et ses fichiers audio. */
    fun delete(context: Context, playlistId: String) {
        File(directory(context), safeName(playlistId)).deleteRecursively()
        val index = readIndex(context)
        writeIndex(context, index.copy(playlists = index.playlists.filter { it.id != playlistId }))
    }

    // ------------------------------------------------------------------- plans

    fun plans(context: Context): List<PreparePlan> = _state.value.plans

    private fun readPlans(context: Context): List<PreparePlan> {
        val file = File(directory(context), PLANS_FILE)
        if (!file.exists()) return emptyList()
        return runCatching {
            codec.decodeFromString(PendingPlans.serializer(), file.readText()).plans
        }.getOrElse { emptyList() }
    }

    private fun writePlans(context: Context, plans: List<PreparePlan>) {
        runCatching {
            File(directory(context), PLANS_FILE).writeText(
                codec.encodeToString(PendingPlans.serializer(), PendingPlans(plans))
            )
        }.onFailure { Log.w(TAG, "plans musique non enregistres", it) }
        _state.update { it.copy(plans = plans) }
    }

    /** Charge les plans recus precedemment (Data Layer) : survit au redemarrage. */
    fun reloadPlans(context: Context) {
        _state.update { it.copy(plans = readPlans(context)) }
    }

    private fun savePlan(context: Context, plan: PreparePlan) {
        val plans = readPlans(context).filter { it.id != plan.id } + plan
        writePlans(context, plans)
    }

    fun clearPlan(context: Context, planId: String) {
        writePlans(context, readPlans(context).filter { it.id != planId })
    }

    /**
     * Message recu du telephone sur /mpacer/music (docs/07 section 7.3).
     * Accepte le message du compagnon (`kind=music_plan`) ou un plan complet.
     */
    fun onPlanMessage(context: Context, text: String): PreparePlan? {
        val full = runCatching { codec.decodeFromString(PreparePlan.serializer(), text) }.getOrNull()
        if (full != null && full.playlistId.isNotEmpty()) {
            savePlan(context, full)
            return full
        }
        val message = runCatching { codec.decodeFromString(MusicPlanMessage.serializer(), text) }.getOrNull()
        if (message == null || message.playlistId.isEmpty()) {
            Log.w(TAG, "plan musique illisible")
            return null
        }
        val requested = message.requestedAtMs.takeIf { it > 0 } ?: System.currentTimeMillis()
        val plan = PreparePlan(
            id = "wear-" + requested,
            playlistId = message.playlistId,
            name = message.name,
            targetBpm = message.targetBpm,
            requestedAtMs = requested,
        )
        savePlan(context, plan)
        return plan
    }

    // ------------------------------------------------------------------- flash

    /** Liste les playlists du backend et les fusionne avec l'index local. */
    suspend fun refresh(context: Context) {
        if (SyncClient.token(context) == null) {
            _state.update { it.copy(message = "Montre non appairee : ouvrez Synchronisation") }
            return
        }
        _state.update { it.copy(busy = true, message = null) }
        try {
            val text = getText(context, "/api/v1/music/playlists")
            val list = codec.decodeFromString(ServerPlaylistList.serializer(), text)
            _state.update { it.copy(busy = false, playlists = list.playlists, message = null) }
        } catch (error: Exception) {
            _state.update {
                it.copy(busy = false, message = "Chargement impossible : " + (error.message ?: error.javaClass.simpleName))
            }
        }
        reloadLocal(context)
        reloadPlans(context)
    }

    /** Fiche detaillee d'une playlist (titres, BPM, tailles). */
    suspend fun open(context: Context, playlistId: String) {
        _state.update { it.copy(busy = true, message = null) }
        try {
            val text = getText(context, "/api/v1/music/playlists/" + playlistId)
            val detail = codec.decodeFromString(ServerPlaylistDetail.serializer(), text)
            _state.update { it.copy(busy = false, detail = detail) }
        } catch (error: Exception) {
            _state.update {
                it.copy(busy = false, message = "Fiche indisponible : " + (error.message ?: error.javaClass.simpleName))
            }
        }
    }

    /**
     * Prepare une playlist : telecharge la fiche, puis les fichiers audio quand le
     * backend en fournit un (`download_url`). Une playlist Spotify n'a que la fiche.
     *
     * @return true si la playlist est utilisable hors ligne.
     */
    suspend fun download(context: Context, playlistId: String): Boolean {
        if (SyncClient.token(context) == null) {
            _state.update { it.copy(message = "Montre non appairee : ouvrez Synchronisation") }
            return false
        }
        _state.update { it.copy(busy = true, progress = 0.0, message = null) }
        return try {
            val detail = codec.decodeFromString(
                ServerPlaylistDetail.serializer(),
                getText(context, "/api/v1/music/playlists/" + playlistId),
            )
            val folder = File(directory(context), safeName(playlistId)).apply { mkdirs() }
            val totalBytes = detail.tracks.sumOf { it.sizeBytes ?: 0L }.coerceAtLeast(1L)
            var copied = 0L
            var nextProgress = 0L
            val tracks = mutableListOf<LocalTrack>()
            val acknowledged = mutableListOf<String>()

            for (row in detail.tracks.sortedBy { it.position }) {
                val target = File(folder, fileNameFor(row))
                val url = row.downloadUrl
                if (url != null) {
                    downloadFile(context, absolute(context, url), target, row.sizeBytes) { chunk ->
                        copied += chunk
                        if (copied >= nextProgress) {
                            nextProgress = copied + PROGRESS_STEP_BYTES
                            _state.update { it.copy(progress = (copied.toDouble() / totalBytes).coerceIn(0.0, 1.0)) }
                        }
                    }
                    acknowledged += row.id
                }
                tracks += LocalTrack(
                    id = row.id,
                    title = row.title,
                    artist = row.artist,
                    durationS = row.durationS ?: 0.0,
                    bpm = row.bpm,
                    position = row.position,
                    file = target.takeIf { url != null && it.exists() }?.absolutePath,
                    sizeBytes = target.takeIf { it.exists() }?.length() ?: 0L,
                    spotifyUri = row.spotifyUri,
                )
            }

            val local = LocalPlaylist(
                id = detail.id,
                name = detail.name,
                source = detail.source,
                targetBpm = detail.targetBpm,
                tracks = tracks,
                downloadedAtMs = System.currentTimeMillis(),
            )
            val index = readIndex(context)
            writeIndex(context, index.copy(playlists = index.playlists.filter { it.id != local.id } + local))

            if (acknowledged.isNotEmpty()) {
                runCatching {
                    postJson(
                        context,
                        "/api/v1/music/playlists/" + detail.id + "/ack",
                        codec.encodeToString(PlaylistAckRequest.serializer(), PlaylistAckRequest(acknowledged)),
                    )
                }.onFailure { Log.w(TAG, "acquittement playlist impossible", it) }
            }

            _state.update {
                it.copy(
                    busy = false,
                    progress = 1.0,
                    detail = detail,
                    message = when {
                        acknowledged.isEmpty() -> detail.name + " : fiche prete (" + tracks.size + " titres)"
                        else -> detail.name + " : " + acknowledged.size + " fichier(s) telecharge(s)"
                    },
                )
            }
            true
        } catch (error: Exception) {
            _state.update {
                it.copy(busy = false, message = "Preparation impossible : " + (error.message ?: error.javaClass.simpleName))
            }
            false
        }
    }

    /** Recupere le dernier plan non acquitte du backend (docs/07 section 6.3). */
    suspend fun refreshPlan(context: Context) {
        if (SyncClient.token(context) == null) return
        try {
            val text = getText(context, "/api/v1/music/prepare")
            val response = codec.decodeFromString(PrepareResponse.serializer(), text)
            response.plan?.let { savePlan(context, it) }
        } catch (error: Exception) {
            Log.w(TAG, "plan de preparation indisponible", error)
        }
    }

    /** Acquitte le plan aupres du backend (le telephone ne le renverra plus). */
    suspend fun ackPlan(context: Context, planId: String) {
        clearPlan(context, planId)
        if (SyncClient.token(context) == null) return
        runCatching {
            postJson(
                context,
                "/api/v1/music/prepare/ack",
                codec.encodeToString(PrepareAck.serializer(), PrepareAck(planId)),
            )
        }.onFailure { Log.w(TAG, "acquittement du plan impossible", it) }
    }

    // ------------------------------------------------------------------ reseau

    private suspend fun getText(context: Context, path: String): String = withContext(Dispatchers.IO) {
        val token = SyncClient.token(context) ?: throw IllegalStateException("montre non appairee")
        val request = Request.Builder()
            .url(SyncClient.baseUrl(context) + path)
            .header("Authorization", "Bearer " + token)
            .get()
            .build()
        http.newCall(request).execute().use { response ->
            val body = response.body?.string().orEmpty()
            if (!response.isSuccessful) {
                throw IllegalStateException("HTTP " + response.code)
            }
            body
        }
    }

    private suspend fun postJson(context: Context, path: String, body: String): Unit = withContext(Dispatchers.IO) {
        val token = SyncClient.token(context) ?: throw IllegalStateException("montre non appairee")
        val request = Request.Builder()
            .url(SyncClient.baseUrl(context) + path)
            .header("Authorization", "Bearer " + token)
            .post(body.toRequestBody(JSON_MEDIA.toMediaType()))
            .build()
        http.newCall(request).execute().use { response ->
            if (!response.isSuccessful) {
                throw IllegalStateException("HTTP " + response.code)
            }
        }
    }

    /**
     * Telecharge un fichier audio. Reprend un telechargement interrompu avec
     * l'en-tete `Range` (le backend repond 206, docs/07 section 6.3) ; si le
     * serveur ignore la plage, on repart de zero.
     */
    private suspend fun downloadFile(
        context: Context,
        url: String,
        target: File,
        expected: Long?,
        onChunk: (Long) -> Unit,
    ) = withContext(Dispatchers.IO) {
        if (target.exists() && (expected == null || target.length() == expected)) return@withContext
        val token = SyncClient.token(context) ?: throw IllegalStateException("montre non appairee")
        val partial = File(target.parentFile, target.name + ".part")
        var offset = if (partial.exists()) partial.length() else 0L

        val builder = Request.Builder()
            .url(url)
            .header("Authorization", "Bearer " + token)
        if (offset > 0L) builder.header("Range", "bytes=" + offset + "-")
        http.newCall(builder.build()).execute().use { response ->
            if (response.code == 206) {
                // reprise acceptee
            } else if (response.isSuccessful) {
                offset = 0L
            } else {
                throw IllegalStateException("HTTP " + response.code)
            }
            val body = response.body ?: throw IllegalStateException("reponse vide")
            body.byteStream().use { input ->
                FileOutputStream(partial, offset > 0L).use { output ->
                    val buffer = ByteArray(64 * 1024)
                    while (true) {
                        val read = input.read(buffer)
                        if (read <= 0) break
                        output.write(buffer, 0, read)
                        onChunk(read.toLong())
                    }
                }
            }
        }
        if (expected != null && partial.length() != expected) {
            throw IllegalStateException("taille inattendue : " + partial.length() + " / " + expected)
        }
        if (!partial.renameTo(target)) {
            partial.copyTo(target, overwrite = true)
            partial.delete()
        }
    }

    private fun absolute(context: Context, url: String): String =
        if (url.startsWith("http://") || url.startsWith("https://")) url else SyncClient.baseUrl(context) + url

    // ------------------------------------------------------------------ utilitaires

    private fun safeName(value: String): String =
        value.replace(Regex("[^A-Za-z0-9._-]"), "_").take(64)

    private fun fileNameFor(track: ServerTrackRow): String = String.format(
        Locale.ROOT,
        "%03d-%s%s",
        track.position,
        safeName(track.id),
        extensionFor(track.mime),
    )

    private fun extensionFor(mime: String?): String = when (mime?.lowercase(Locale.ROOT)) {
        "audio/mpeg", "audio/mp3" -> ".mp3"
        "audio/ogg", "application/ogg" -> ".ogg"
        "audio/mp4", "audio/m4a", "audio/x-m4a" -> ".m4a"
        "audio/flac", "audio/x-flac" -> ".flac"
        "audio/wav", "audio/x-wav" -> ".wav"
        else -> ".audio"
    }
}

// ---------------------------------------------------------------------- etat

/** Une playlist telle que l'ecran Musique l'affiche : fiche serveur + etat local. */
data class MusicEntry(
    val id: String,
    val name: String,
    val source: String,
    val targetBpm: Double?,
    val trackCount: Int,
    val totalBytes: Long,
    val local: LocalPlaylist?,
) {
    val downloaded: Boolean get() = local != null
    val playable: Boolean get() = (local?.playable?.isNotEmpty() == true) || source == "spotify"
}

/** Etat expose a l'ecran Musique. */
data class MusicLibraryState(
    val playlists: List<ServerPlaylistRow> = emptyList(),
    val downloaded: Map<String, LocalPlaylist> = emptyMap(),
    val detail: ServerPlaylistDetail? = null,
    val plans: List<PreparePlan> = emptyList(),
    val busy: Boolean = false,
    val progress: Double = 0.0,
    val message: String? = null,
) {
    /** Playlists du serveur, completees par celles deja presentes sur la montre. */
    val entries: List<MusicEntry>
        get() {
            val rows = playlists.map { row ->
                MusicEntry(
                    id = row.id,
                    name = row.name,
                    source = row.source,
                    targetBpm = row.targetBpm,
                    trackCount = row.trackCount,
                    totalBytes = row.totalBytes,
                    local = downloaded[row.id],
                )
            }
            val known = rows.map { it.id }.toSet()
            val extra = downloaded.values
                .filter { it.id !in known }
                .map { MusicEntry(it.id, it.name, it.source, it.targetBpm, it.trackCount, 0L, it) }
            return rows + extra
        }

    /** Dernier plan de preparation en attente. */
    val pendingPlan: PreparePlan? get() = plans.lastOrNull()
}
