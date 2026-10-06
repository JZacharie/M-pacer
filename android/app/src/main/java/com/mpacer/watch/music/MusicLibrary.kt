package com.mpacer.watch.music

import android.content.Context
import android.os.StatFs
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import java.io.File

/**
 * Bibliotheque musicale locale de la montre (docs/07 v2, sections 6.3 et 6.4).
 *
 * La montre ne telecharge plus rien : les fichiers audio et leur `manifest.json`
 * sont copies par USB par l'outil PC `mpacer-music` dans
 * `context.getExternalFilesDir("Music")` :
 *
 * ```text
 * /sdcard/Android/data/com.mpacer.watch/files/Music/
 *   run-170/                      <- un dossier par playlist
 *     01 - Avicii - Wake me up.mp3
 *     manifest.json               <- manifeste + fichier et taille par piste
 * ```
 *
 * « Importer (USB) » relit ce dossier et reconstruit l'index local
 * (filesDir/music-index.json). Aucun acces reseau, aucun acces aux notifications.
 */
object MusicLibrary {

    private const val TAG = "MusicLibrary"
    private const val USB_DIRECTORY = "Music"
    private const val MANIFEST_FILE = "manifest.json"
    private const val INDEX_FILE = "music-index.json"

    private val codec = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        explicitNulls = false
    }

    private val _state = MutableStateFlow(MusicLibraryState())
    val state: StateFlow<MusicLibraryState> = _state.asStateFlow()

    /** Dossier ou l'outil USB depose les playlists (.../files/Music). */
    fun usbDirectory(context: Context): File? = context.getExternalFilesDir(USB_DIRECTORY)

    // ------------------------------------------------------------------ index

    private fun indexFile(context: Context): File = File(context.filesDir, INDEX_FILE)

    fun readIndex(context: Context): LocalIndex {
        val file = indexFile(context)
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
            indexFile(context).writeText(codec.encodeToString(LocalIndex.serializer(), index))
        }.onFailure { Log.w(TAG, "index musique non enregistre", it) }
    }

    /** Recharge l'index local et les espaces (ouverture de l'ecran Musique). */
    fun reload(context: Context) {
        _state.update { it.copy(playlists = readIndex(context).playlists, message = null) }
        refreshSpaces(context)
    }

    // -------------------------------------------------------------- import USB

    /**
     * « Importer (USB) » : relit `getExternalFilesDir("Music")`. Chaque
     * sous-dossier contenant un `manifest.json` devient une playlist locale.
     *
     * @return le nombre de playlists importees.
     */
    suspend fun scan(context: Context): Int = withContext(Dispatchers.IO) {
        _state.update { it.copy(busy = true, message = null) }
        val root = usbDirectory(context)
        if (root == null || !root.isDirectory) {
            _state.update {
                it.copy(busy = false, usbAvailable = false, message = "Dossier Music introuvable sur la montre")
            }
            return@withContext 0
        }

        val playlists = mutableListOf<LocalPlaylist>()
        val folders = root.listFiles()?.filter { it.isDirectory }?.sortedBy { it.name } ?: emptyList()
        for (folder in folders) {
            val manifestFile = File(folder, MANIFEST_FILE)
            if (!manifestFile.isFile) continue
            val manifest = runCatching {
                codec.decodeFromString(TransferManifest.serializer(), manifestFile.readText())
            }.getOrNull()
            if (manifest == null || manifest.playlistId.isBlank()) {
                Log.w(TAG, "manifeste illisible : " + manifestFile.absolutePath)
                continue
            }
            playlists += toLocalPlaylist(folder, manifest, manifestFile.lastModified())
        }

        val triees = playlists.sortedByDescending { it.importedAtMs }
        writeIndex(context, LocalIndex(playlists = triees))
        _state.update {
            it.copy(
                busy = false,
                playlists = triees,
                usbAvailable = true,
                message = if (triees.isEmpty()) {
                    "Aucune playlist dans Music/ : lancez mpacer-music sur l'ordinateur"
                } else {
                    triees.size.toString() + " playlist(s) importee(s)"
                },
            )
        }
        refreshSpaces(context)
        triees.size
    }

    private fun toLocalPlaylist(folder: File, manifest: TransferManifest, importedAtMs: Long): LocalPlaylist {
        val tracks = manifest.tracks
            .sortedBy { it.position }
            .map { track ->
                val audio = track.file?.let { name -> File(folder, name).takeIf { it.isFile } }
                LocalTrack(
                    id = track.id,
                    title = track.title,
                    artist = track.artist,
                    album = track.album,
                    durationS = track.durationS ?: 0.0,
                    bpm = track.bpm,
                    position = track.position,
                    file = audio?.absolutePath,
                    sizeBytes = audio?.length() ?: track.sizeBytes ?: 0L,
                )
            }
        return LocalPlaylist(
            id = manifest.playlistId,
            name = manifest.name.ifBlank { manifest.playlistId },
            source = manifest.source,
            targetBpm = manifest.targetBpm,
            tracks = tracks,
            folder = folder.name,
            importedAtMs = importedAtMs,
        )
    }

    /**
     * Supprime une playlist importee : son entree d'index et les fichiers copies
     * sur la montre (le dossier appartient a l'application, aucune permission
     * speciale n'est requise). Le transfert USB peut la recreer.
     */
    suspend fun delete(context: Context, playlistId: String) = withContext(Dispatchers.IO) {
        val index = readIndex(context)
        val cible = index.playlists.firstOrNull { it.id == playlistId } ?: return@withContext
        val root = usbDirectory(context)
        val dossier = cible.folder?.let { name -> root?.let { File(it, name) } }
        if (dossier != null && root != null && dossier.canonicalPath.startsWith(root.canonicalPath)) {
            runCatching { dossier.deleteRecursively() }
                .onFailure { Log.w(TAG, "suppression des fichiers impossible", it) }
        }
        writeIndex(context, index.copy(playlists = index.playlists.filter { it.id != playlistId }))
        _state.update {
            it.copy(
                playlists = it.playlists.filter { playlist -> playlist.id != playlistId },
                message = "Playlist supprimee : " + cible.name,
            )
        }
        refreshSpaces(context)
    }

    // ------------------------------------------------------------------ espace

    private fun refreshSpaces(context: Context) {
        val root = usbDirectory(context)
        val used = _state.value.playlists.sumOf { it.sizeBytes }
        val free = runCatching {
            StatFs((root ?: context.filesDir).absolutePath).availableBytes
        }.getOrDefault(0L)
        _state.update {
            it.copy(usedBytes = used, freeBytes = free, usbAvailable = root?.isDirectory == true)
        }
    }
}

/** Etat expose a l'ecran Musique. */
data class MusicLibraryState(
    val playlists: List<LocalPlaylist> = emptyList(),
    /** Octets utilises par les fichiers importes. */
    val usedBytes: Long = 0,
    /** Octets libres sur le volume qui porte le dossier Music. */
    val freeBytes: Long = 0,
    val usbAvailable: Boolean = false,
    val busy: Boolean = false,
    val message: String? = null,
)
