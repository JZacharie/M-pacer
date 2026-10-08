package com.mpacer.core.music

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Modeles locaux de la musique de la montre (docs/07 v2, section 6.3).
 *
 * La montre ne fait plus aucun appel reseau pour la musique : les fichiers et le
 * manifeste sont copies par USB par l'outil PC `mpacer-music`, puis relus dans
 * context.getExternalFilesDir("Music").
 */

// ---------------------------------------------------------- manifeste USB

/**
 * Piste d'un manifeste de transfert (schema docs/07 section 3.1), enrichie par
 * l'outil PC avec `file` et `size_bytes` quand le fichier a ete apparie.
 */
@Serializable
data class ManifestTrack(
    val id: String,
    val position: Int = 0,
    val title: String = "",
    val artist: String? = null,
    val album: String? = null,
    @SerialName("duration_s") val durationS: Double? = null,
    val bpm: Double? = null,
    /** Nom du fichier dans le dossier de la playlist, absent si non apparie. */
    val file: String? = null,
    @SerialName("size_bytes") val sizeBytes: Long? = null,
)

/** Manifeste ecrit par mpacer-music dans <Music>/<playlist_id>/manifest.json. */
@Serializable
data class TransferManifest(
    val version: Int = 1,
    @SerialName("playlist_id") val playlistId: String = "",
    val name: String = "",
    /** "deezer" ou "manual" : origine des metadonnees, pas un mode de lecture. */
    val source: String = "manual",
    @SerialName("target_bpm") val targetBpm: Double? = null,
    val tracks: List<ManifestTrack> = emptyList(),
)

// ------------------------------------------------------------- index local

/** Piste presente sur la montre (fichier audio copie par USB). */
@Serializable
data class LocalTrack(
    val id: String,
    val title: String,
    val artist: String? = null,
    val album: String? = null,
    @SerialName("duration_s") val durationS: Double = 0.0,
    val bpm: Double? = null,
    val position: Int = 0,
    /** Chemin absolu du fichier audio, ou null si la piste n'a pas ete appariee. */
    val file: String? = null,
    @SerialName("size_bytes") val sizeBytes: Long = 0,
)

/** Playlist importee : manifeste + fichiers reellement presents sur la montre. */
@Serializable
data class LocalPlaylist(
    val id: String,
    val name: String,
    val source: String = "manual",
    @SerialName("target_bpm") val targetBpm: Double? = null,
    val tracks: List<LocalTrack> = emptyList(),
    /** Nom du sous-dossier de <Music> qui porte le manifeste. */
    val folder: String? = null,
    @SerialName("imported_at_ms") val importedAtMs: Long = 0,
) {
    /** Pistes reellement jouables (fichier present sur la montre). */
    val playable: List<LocalTrack> get() = tracks.filter { it.file != null }

    val trackCount: Int get() = tracks.size

    val sizeBytes: Long get() = playable.sumOf { it.sizeBytes }

    /**
     * Vue envoyee au coeur Rust. Seules les pistes jouables y figurent : le moteur
     * ne doit jamais choisir un morceau absent du disque.
     */
    fun toCore(): MusicPlaylist = MusicPlaylist(
        id = id,
        name = name,
        targetBpm = targetBpm,
        tracks = playable.map { track ->
            MusicTrack(
                id = track.id,
                title = track.title,
                artist = track.artist,
                durationS = track.durationS,
                bpm = track.bpm,
                position = track.position,
            )
        },
    )
}

/** Contenu de filesDir/music-index.json. */
@Serializable
data class LocalIndex(
    val playlists: List<LocalPlaylist> = emptyList(),
)
