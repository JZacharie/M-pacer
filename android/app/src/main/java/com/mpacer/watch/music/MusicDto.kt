package com.mpacer.watch.music

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Modeles JSON echanges avec le backend (docs/07 sections 6.3 et 7.2) et index
 * local de la bibliotheque de la montre.
 *
 * Les noms de champs sont ceux de serde (snake_case) : aucune conversion n'est
 * faite cote serveur.
 */

// ------------------------------------------------------------- API appareil

/** Ligne de GET /api/v1/music/playlists. */
@Serializable
data class ServerPlaylistRow(
    val id: String,
    val name: String,
    val source: String = "upload",
    @SerialName("target_bpm") val targetBpm: Double? = null,
    @SerialName("track_count") val trackCount: Int = 0,
    @SerialName("total_bytes") val totalBytes: Long = 0,
    @SerialName("ready_track_count") val readyTrackCount: Int = 0,
    @SerialName("updated_at_ms") val updatedAtMs: Long = 0,
)

@Serializable
data class ServerPlaylistList(val playlists: List<ServerPlaylistRow> = emptyList())

/** Piste de GET /api/v1/music/playlists/{id}. */
@Serializable
data class ServerTrackRow(
    val id: String,
    val position: Int = 0,
    val title: String = "",
    val artist: String? = null,
    val album: String? = null,
    @SerialName("duration_s") val durationS: Double? = null,
    val bpm: Double? = null,
    @SerialName("bpm_source") val bpmSource: String? = null,
    @SerialName("size_bytes") val sizeBytes: Long? = null,
    val mime: String? = null,
    @SerialName("spotify_uri") val spotifyUri: String? = null,
    /** null quand la piste n'a pas de fichier (playlist Spotify : fiche seule). */
    @SerialName("download_url") val downloadUrl: String? = null,
)

@Serializable
data class ServerPlaylistDetail(
    val id: String,
    val name: String,
    val source: String = "upload",
    @SerialName("target_bpm") val targetBpm: Double? = null,
    val tracks: List<ServerTrackRow> = emptyList(),
)

/** Plan de telechargement en attente (GET /api/v1/music/prepare). */
@Serializable
data class PreparePlan(
    val id: String,
    @SerialName("playlist_id") val playlistId: String,
    val name: String = "",
    @SerialName("target_bpm") val targetBpm: Double? = null,
    @SerialName("race_id") val raceId: String? = null,
    @SerialName("race_name") val raceName: String? = null,
    @SerialName("requested_at_ms") val requestedAtMs: Long = 0,
    val tracks: List<PrepareTrack> = emptyList(),
)

@Serializable
data class PrepareTrack(
    val id: String,
    val position: Int = 0,
    val title: String = "",
    val artist: String? = null,
    @SerialName("duration_s") val durationS: Double? = null,
    val bpm: Double? = null,
    @SerialName("size_bytes") val sizeBytes: Long? = null,
    @SerialName("download_url") val downloadUrl: String? = null,
)

@Serializable
data class PrepareResponse(val plan: PreparePlan? = null)

@Serializable
data class PrepareAck(@SerialName("plan_id") val planId: String)

@Serializable
data class PlaylistAckRequest(
    @SerialName("track_ids") val trackIds: List<String> = emptyList(),
)

// ------------------------------------------------------------- index local

/** Piste effectivement presente sur la montre (fichier audio telecharge). */
@Serializable
data class LocalTrack(
    val id: String,
    val title: String,
    val artist: String? = null,
    @SerialName("duration_s") val durationS: Double = 0.0,
    val bpm: Double? = null,
    val position: Int = 0,
    /** Chemin absolu du fichier audio, ou null pour une fiche Spotify. */
    val file: String? = null,
    @SerialName("size_bytes") val sizeBytes: Long = 0,
    @SerialName("spotify_uri") val spotifyUri: String? = null,
)

/** Playlist locale : fiche + fichiers audios deja telecharges. */
@Serializable
data class LocalPlaylist(
    val id: String,
    val name: String,
    val source: String = "upload",
    @SerialName("target_bpm") val targetBpm: Double? = null,
    val tracks: List<LocalTrack> = emptyList(),
    @SerialName("downloaded_at_ms") val downloadedAtMs: Long = 0,
) {
    val playable: List<LocalTrack> get() = tracks.filter { it.file != null }
    val trackCount: Int get() = tracks.size

    /** Vue envoyee au coeur Rust : le moteur ne connait que titres et BPM. */
    fun toCore(): MusicPlaylist = MusicPlaylist(
        id = id,
        name = name,
        targetBpm = targetBpm,
        tracks = tracks.map {
            MusicTrack(
                id = it.id,
                title = it.title,
                artist = it.artist,
                durationS = it.durationS,
                bpm = it.bpm,
                position = it.position,
            )
        },
    )
}

/** Contenu de filesDir/music/index.json. */
@Serializable
data class LocalIndex(
    val playlists: List<LocalPlaylist> = emptyList(),
)

/** Contenu de filesDir/music/plans.json : plans recus par le Data Layer. */
@Serializable
data class PendingPlans(
    val plans: List<PreparePlan> = emptyList(),
)

/** Message envoye par le compagnon sur le chemin Data Layer /mpacer/music. */
@Serializable
data class MusicPlanMessage(
    val kind: String = "music_plan",
    @SerialName("playlist_id") val playlistId: String,
    val name: String = "",
    @SerialName("target_bpm") val targetBpm: Double? = null,
    @SerialName("requested_at_ms") val requestedAtMs: Long = 0,
)
