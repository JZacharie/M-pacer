package com.mpacer.core.music

import android.content.Context
import android.os.StatFs
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
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.util.concurrent.TimeUnit

/**
 * Telechargement des MP3 deposes sur le serveur (docs/16).
 *
 * L'autre moitie du depot : la page /music garde les fichiers dans
 * MPACER_MEDIA_DIR le temps que l'appareil les recupere. Ce module interroge
 * l'API appareil avec le jeton d'appairage deja stocke par SyncClient, ecrit les
 * fichiers dans getExternalFilesDir("Music")/<playlist_id>/ avec leur
 * manifest.json, puis acquitte chaque piste : le serveur supprime alors sa copie.
 *
 * Aucun calcul : le manifeste est celui que le serveur a prepare (?files=1), et
 * MusicLibrary relit le dossier exactement comme apres un transfert USB.
 */
object MusicDownloader {

    private const val TAG = "MusicDownloader"

    /** Marge minimale laissee libre sur le disque avant d'accepter un fichier. */
    private const val FREE_MARGIN_BYTES = 32L * 1024 * 1024

    private val codec = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        explicitNulls = false
    }

    private val jsonMedia = "application/json; charset=utf-8".toMediaType()

    private val http: OkHttpClient by lazy {
        OkHttpClient.Builder()
            .connectTimeout(15, TimeUnit.SECONDS)
            // Un MP3 en Wi-Fi peut prendre du temps : la lecture n'est pas un appel court.
            .readTimeout(300, TimeUnit.SECONDS)
            .writeTimeout(60, TimeUnit.SECONDS)
            .build()
    }

    /**
     * Scope lie au processus : un telechargement lance depuis l'ecran Musique doit
     * survivre a sa fermeture (meme regle que SyncClient).
     */
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val _state = MutableStateFlow(MusicDownloadState())
    val state: StateFlow<MusicDownloadState> = _state.asStateFlow()

    /**
     * Lance le telechargement sans bloquer l'appelant. Toutes les playlists du
     * compte sont examinees : seules celles qui ont des fichiers stockes sur le
     * serveur sont ecrites.
     */
    fun syncInBackground(context: Context, playlistId: String? = null) {
        val application = context.applicationContext
        scope.launch {
            runCatching { sync(application, playlistId) }
                .onFailure { Log.w(TAG, "telechargement impossible : " + it.message) }
        }
    }

    /**
     * Recupere les MP3 stockes sur le serveur et recharge la bibliotheque.
     *
     * @return le nombre de playlists ecrites (0 si rien n'etait en attente).
     */
    suspend fun sync(context: Context, playlistId: String? = null): Int = withContext(Dispatchers.IO) {
        val application = context.applicationContext
        val token = SyncClient.token(application)
        if (token == null) {
            _state.update {
                it.copy(busy = false, message = "Appareil non appaire : impossible de telecharger")
            }
            return@withContext 0
        }
        _state.update {
            it.copy(busy = true, message = "Recherche des fichiers...", current = 0, total = 0, bytes = 0)
        }
        try {
            val racine = MusicLibrary.usbDirectory(application)
            if (racine == null) {
                _state.update { it.copy(busy = false, message = "Dossier Music introuvable") }
                return@withContext 0
            }
            val playlists = if (playlistId != null) {
                listOf(MusicPlaylistRef(playlistId, ""))
            } else {
                listPlaylists(application, token)
            }
            var ecrites = 0
            for (playlist in playlists) {
                ecrites += syncOne(application, token, racine, playlist)
            }
            if (ecrites > 0) {
                MusicLibrary.scan(application)
            }
            _state.update {
                it.copy(
                    busy = false,
                    message = if (ecrites == 0) "Aucun fichier a telecharger" else ecrites.toString() + " playlist(s) mise(s) a jour",
                )
            }
            ecrites
        } catch (error: Exception) {
            Log.w(TAG, "telechargement interrompu", error)
            _state.update {
                it.copy(
                    busy = false,
                    message = "Telechargement interrompu : " + (error.message ?: error.javaClass.simpleName),
                )
            }
            0
        }
    }

    /**
     * Telecharge les fichiers d'une playlist, ecrit le manifeste puis acquitte.
     *
     * Le manifeste n'est ecrit qu'apres les fichiers : MusicLibrary.scan ne voit
     * jamais une playlist a moitie telechargee.
     */
    private fun syncOne(
        context: Context,
        token: String,
        racine: File,
        playlist: MusicPlaylistRef,
    ): Int {
        val fichiers = listFiles(context, token, playlist.id)
        if (fichiers.isEmpty()) return 0
        val dossier = File(racine, playlist.id)
        if (!dossier.isDirectory && !dossier.mkdirs()) {
            throw IOException("dossier " + dossier.absolutePath + " impossible a creer")
        }
        val manifeste = fetchText(context, token, "/api/v1/music/playlists/" + playlist.id + "/manifest?files=1")
        val aAcquitter = mutableListOf<String>()
        var recus = 0
        fichiers.forEachIndexed { index, fichier ->
            _state.update {
                it.copy(playlist = playlist.name.ifBlank { playlist.id }, current = index + 1, total = fichiers.size)
            }
            val cible = File(dossier, fichier.fileName)
            if (cible.isFile && cible.length() == fichier.sizeBytes) {
                // Deja present (meme taille) : rien a faire, mais on l'acquitte
                // pour liberer la copie serveur.
                aAcquitter += fichier.trackId
                return@forEachIndexed
            }
            if (racine.freeSpace() < fichier.sizeBytes + FREE_MARGIN_BYTES) {
                throw IOException("espace insuffisant pour " + fichier.fileName)
            }
            val temporaire = File(dossier, fichier.fileName + ".part")
            download(
                context = context,
                token = token,
                chemin = "/api/v1/music/playlists/" + playlist.id + "/tracks/" + fichier.trackId + "/file",
                destination = temporaire,
            )
            if (!temporaire.renameTo(cible)) {
                temporaire.copyTo(cible, overwrite = true)
                temporaire.delete()
            }
            aAcquitter += fichier.trackId
            recus += 1
            _state.update { it.copy(bytes = it.bytes + fichier.sizeBytes) }
        }
        File(dossier, "manifest.json").writeText(manifeste)
        ack(context, token, playlist.id, aAcquitter)
        return 1
    }

    /** Ecrit un fichier, en reprenant apres un ".part" interrompu (Range). */
    private fun download(context: Context, token: String, chemin: String, destination: File) {
        val deja = if (destination.isFile) destination.length() else 0L
        val builder = Request.Builder()
            .url(SyncClient.baseUrl(context) + chemin)
            .header("Authorization", "Bearer " + token)
            .get()
        if (deja > 0) {
            builder.header("Range", "bytes=" + deja + "-")
        }
        http.newCall(builder.build()).execute().use { response ->
            if (!response.isSuccessful) {
                throw IOException("telechargement refuse (HTTP " + response.code + ")")
            }
            val corps = response.body ?: throw IOException("reponse vide")
            // Le serveur ne repond 206 que s'il a compris Range : sinon on repart
            // de zero plutot que de coller deux morceaux differents.
            val ajout = deja > 0 && response.code == 206
            destination.parentFile?.mkdirs()
            corps.byteStream().use { entree ->
                FileOutputStream(destination, ajout).use { sortie -> entree.copyTo(sortie) }
            }
        }
    }

    private fun listPlaylists(context: Context, token: String): List<MusicPlaylistRef> {
        val body = fetchText(context, token, "/api/v1/music/playlists")
        val response = runCatching { codec.decodeFromString(PlaylistsResponse.serializer(), body) }.getOrNull()
        return response?.playlists ?: emptyList()
    }

    private fun listFiles(context: Context, token: String, playlistId: String): List<MusicFileRow> {
        val chemin = "/api/v1/music/playlists/" + playlistId + "/files"
        val body = fetchText(context, token, chemin)
        val response = runCatching { codec.decodeFromString(MusicFilesResponse.serializer(), body) }.getOrNull()
        return response?.files ?: emptyList()
    }

    private fun ack(context: Context, token: String, playlistId: String, trackIds: List<String>) {
        if (trackIds.isEmpty()) return
        val payload = codec.encodeToString(MusicAckRequest.serializer(), MusicAckRequest(trackIds))
        val request = Request.Builder()
            .url(SyncClient.baseUrl(context) + "/api/v1/music/playlists/" + playlistId + "/ack")
            .header("Authorization", "Bearer " + token)
            .post(payload.toRequestBody(jsonMedia))
            .build()
        http.newCall(request).execute().use { response ->
            if (!response.isSuccessful) {
                // L'acquittement est un confort : le fichier est deja sur
                // l'appareil, il sera simplement renvoye au prochain passage.
                Log.w(TAG, "acquittement refuse (HTTP " + response.code + ")")
            }
        }
    }

    private fun fetchText(context: Context, token: String, chemin: String): String {
        val request = Request.Builder()
            .url(SyncClient.baseUrl(context) + chemin)
            .header("Authorization", "Bearer " + token)
            .get()
            .build()
        http.newCall(request).execute().use { response ->
            val body = response.body?.string().orEmpty()
            if (!response.isSuccessful) {
                throw IOException("lecture refusee (HTTP " + response.code + ") : " + chemin)
            }
            return body
        }
    }

    private fun File.freeSpace(): Long =
        runCatching { StatFs(absolutePath).availableBytes }.getOrDefault(0L)
}

/** Etat expose a l'ecran Musique. */
data class MusicDownloadState(
    val busy: Boolean = false,
    /** Nom de la playlist en cours de traitement. */
    val playlist: String = "",
    val current: Int = 0,
    val total: Int = 0,
    /** Octets recus depuis le debut de l'operation. */
    val bytes: Long = 0,
    val message: String? = null,
)

@Serializable
internal data class PlaylistsResponse(val playlists: List<MusicPlaylistRef> = emptyList())

/** Resume de playlist renvoye par GET /api/v1/music/playlists. */
@Serializable
data class MusicPlaylistRef(
    val id: String,
    val name: String = "",
    val source: String = "manual",
    @SerialName("target_bpm") val targetBpm: Double? = null,
    @SerialName("track_count") val trackCount: Long = 0,
)

/** Reponse de GET /api/v1/music/playlists/{id}/files. */
@Serializable
data class MusicFilesResponse(
    @SerialName("playlist_id") val playlistId: String = "",
    val files: List<MusicFileRow> = emptyList(),
)

/** MP3 present sur le serveur, avec le nom exact attendu par la montre. */
@Serializable
data class MusicFileRow(
    @SerialName("track_id") val trackId: String,
    @SerialName("file_name") val fileName: String,
    @SerialName("size_bytes") val sizeBytes: Long = 0,
    val mime: String = "",
)

/** Corps de POST /api/v1/music/playlists/{id}/ack. */
@Serializable
internal data class MusicAckRequest(@SerialName("track_ids") val trackIds: List<String>)
