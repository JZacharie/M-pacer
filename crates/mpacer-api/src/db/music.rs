//! Acces aux playlists, titres, quotas de stockage et comptes Deezer.

use crate::models::{
    DeezerAccount, MusicPlaylist, MusicPlaylistInput, MusicPlaylistSummary, MusicTrack,
    MusicTrackInput,
};
use sqlx::PgPool;

/// Cree une playlist (vide) et renvoie sa ligne.
pub async fn insert_music_playlist(
    pool: &PgPool,
    user_id: &str,
    input: &MusicPlaylistInput,
    now_ms: i64,
) -> Result<MusicPlaylist, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO music_playlists
             (id, user_id, name, source, deezer_id, cover_url, target_bpm,
              created_at_ms, updated_at_ms)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(&input.name)
    .bind(&input.source)
    .bind(input.deezer_id.as_deref())
    .bind(input.cover_url.as_deref())
    .bind(input.target_bpm)
    .bind(now_ms)
    .bind(now_ms)
    .execute(pool)
    .await?;

    get_music_playlist(pool, user_id, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn get_music_playlist(
    pool: &PgPool,
    user_id: &str,
    id: &str,
) -> Result<Option<MusicPlaylist>, sqlx::Error> {
    sqlx::query_as::<_, MusicPlaylist>(
        "SELECT * FROM music_playlists WHERE user_id = $1 AND id = $2",
    )
    .bind(user_id)
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Playlist deja importee depuis Deezer (un import deux fois ne cree pas de doublon).
pub async fn find_music_playlist_by_deezer(
    pool: &PgPool,
    user_id: &str,
    deezer_id: &str,
) -> Result<Option<MusicPlaylist>, sqlx::Error> {
    sqlx::query_as::<_, MusicPlaylist>(
        "SELECT * FROM music_playlists WHERE user_id = $1 AND deezer_id = $2",
    )
    .bind(user_id)
    .bind(deezer_id)
    .fetch_optional(pool)
    .await
}

/// Toutes les playlists, de la plus recemment modifiee a la plus ancienne.
pub async fn list_music_playlists(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<MusicPlaylist>, sqlx::Error> {
    sqlx::query_as::<_, MusicPlaylist>(
        "SELECT * FROM music_playlists WHERE user_id = $1 ORDER BY updated_at_ms DESC, name ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Playlists avec leurs compteurs (nombre de titres et duree cumulee).
pub async fn list_music_playlist_summaries(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<MusicPlaylistSummary>, sqlx::Error> {
    sqlx::query_as::<_, MusicPlaylistSummary>(
        "SELECT p.id, p.name, p.source, p.target_bpm, p.updated_at_ms,
                COUNT(t.id)::bigint AS track_count,
                COALESCE(SUM(t.duration_s), 0)::double precision AS duration_s
           FROM music_playlists p
           LEFT JOIN music_tracks t ON t.playlist_id = p.id AND t.user_id = p.user_id
          WHERE p.user_id = $1
          GROUP BY p.id
          ORDER BY p.updated_at_ms DESC, p.name ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Consigne de tempo d'une playlist (vide = automatique).
pub async fn set_music_playlist_target(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    target_bpm: Option<f64>,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE music_playlists SET target_bpm = $1, updated_at_ms = $2 WHERE id = $3 AND user_id = $4",
    )
    .bind(target_bpm)
    .bind(now_ms)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Renomme une playlist (le nom est nettoye par l'appelant).
pub async fn rename_music_playlist(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    name: &str,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE music_playlists SET name = $1, updated_at_ms = $2 WHERE id = $3 AND user_id = $4",
    )
    .bind(name)
    .bind(now_ms)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Marque une playlist comme modifiee (tri de la liste).
pub async fn touch_music_playlist(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE music_playlists SET updated_at_ms = $1 WHERE id = $2 AND user_id = $3")
        .bind(now_ms)
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Supprime une playlist (ses titres partent par cascade).
pub async fn delete_music_playlist(
    pool: &PgPool,
    user_id: &str,
    id: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM music_playlists WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Insere un titre ; la position est fournie par l'appelant (ordre de playlist).
pub async fn insert_music_track(
    pool: &PgPool,
    user_id: &str,
    playlist_id: &str,
    input: &MusicTrackInput,
    now_ms: i64,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO music_tracks
             (id, playlist_id, user_id, position, title, artist, album, duration_s, bpm,
              bpm_source, deezer_track_id, created_at_ms)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(&id)
    .bind(playlist_id)
    .bind(user_id)
    .bind(input.position)
    .bind(&input.title)
    .bind(input.artist.as_deref())
    .bind(input.album.as_deref())
    .bind(input.duration_s)
    .bind(input.bpm)
    .bind(input.bpm_source.as_deref())
    .bind(input.deezer_track_id.as_deref())
    .bind(now_ms)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn list_music_tracks(
    pool: &PgPool,
    user_id: &str,
    playlist_id: &str,
) -> Result<Vec<MusicTrack>, sqlx::Error> {
    sqlx::query_as::<_, MusicTrack>(
        "SELECT * FROM music_tracks
          WHERE user_id = $1 AND playlist_id = $2
          ORDER BY position ASC, created_at_ms ASC, id ASC",
    )
    .bind(user_id)
    .bind(playlist_id)
    .fetch_all(pool)
    .await
}

/// Enregistre un BPM pour un titre de la playlist.
pub async fn set_music_track_bpm(
    pool: &PgPool,
    user_id: &str,
    playlist_id: &str,
    track_id: &str,
    bpm: Option<f64>,
    bpm_source: Option<&str>,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE music_tracks SET bpm = $1, bpm_source = $2
          WHERE id = $3 AND user_id = $4 AND playlist_id = $5",
    )
    .bind(bpm)
    .bind(bpm_source)
    .bind(track_id)
    .bind(user_id)
    .bind(playlist_id)
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 {
        touch_music_playlist(pool, user_id, playlist_id, now_ms).await?;
    }
    Ok(result.rows_affected() > 0)
}

/// Octets audio stockes par un compte : base du quota.
pub async fn music_storage_bytes(pool: &PgPool, user_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(size_bytes), 0)::bigint FROM music_tracks
          WHERE user_id = $1 AND storage_path IS NOT NULL",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Enregistre les octets d'un MP3 televerse sur un titre.
#[allow(clippy::too_many_arguments)]
pub async fn set_music_track_storage(
    pool: &PgPool,
    user_id: &str,
    playlist_id: &str,
    track_id: &str,
    mime: &str,
    size_bytes: i64,
    storage_path: &str,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE music_tracks
            SET mime = $1, size_bytes = $2, storage_path = $3, downloaded_at_ms = NULL
          WHERE id = $4 AND user_id = $5 AND playlist_id = $6",
    )
    .bind(mime)
    .bind(size_bytes)
    .bind(storage_path)
    .bind(track_id)
    .bind(user_id)
    .bind(playlist_id)
    .execute(pool)
    .await?;
    if result.rows_affected() > 0 {
        touch_music_playlist(pool, user_id, playlist_id, now_ms).await?;
    }
    Ok(result.rows_affected() > 0)
}

/// Titres d'une playlist dont les octets sont presents sur le serveur.
pub async fn list_stored_music_tracks(
    pool: &PgPool,
    user_id: &str,
    playlist_id: &str,
) -> Result<Vec<MusicTrack>, sqlx::Error> {
    sqlx::query_as::<_, MusicTrack>(
        "SELECT * FROM music_tracks
          WHERE user_id = $1 AND playlist_id = $2 AND storage_path IS NOT NULL
          ORDER BY position ASC, created_at_ms ASC, id ASC",
    )
    .bind(user_id)
    .bind(playlist_id)
    .fetch_all(pool)
    .await
}

/// Titres stockes parmi une liste d'identifiants (prepare l'acquittement).
pub async fn stored_music_tracks_by_ids(
    pool: &PgPool,
    user_id: &str,
    playlist_id: &str,
    track_ids: &[String],
) -> Result<Vec<MusicTrack>, sqlx::Error> {
    sqlx::query_as::<_, MusicTrack>(
        "SELECT * FROM music_tracks
          WHERE user_id = $1 AND playlist_id = $2 AND id = ANY($3)
            AND storage_path IS NOT NULL",
    )
    .bind(user_id)
    .bind(playlist_id)
    .bind(track_ids)
    .fetch_all(pool)
    .await
}

/// Acquitte des titres : le fichier peut partir, la fiche reste.
pub async fn ack_music_tracks(
    pool: &PgPool,
    user_id: &str,
    playlist_id: &str,
    track_ids: &[String],
    now_ms: i64,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE music_tracks
            SET mime = NULL, size_bytes = NULL, storage_path = NULL, downloaded_at_ms = $1
          WHERE user_id = $2 AND playlist_id = $3 AND id = ANY($4)
            AND storage_path IS NOT NULL",
    )
    .bind(now_ms)
    .bind(user_id)
    .bind(playlist_id)
    .bind(track_ids)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Enregistre (ou remplace) le compte Deezer lie a l'utilisateur.
pub async fn upsert_deezer_account(
    pool: &PgPool,
    account: &DeezerAccount,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO deezer_accounts
             (user_id, deezer_user_id, display_name, access_token, expires_at_ms, scope,
              connected_at_ms)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (user_id) DO UPDATE SET
             deezer_user_id = EXCLUDED.deezer_user_id,
             display_name = COALESCE(EXCLUDED.display_name, deezer_accounts.display_name),
             access_token = EXCLUDED.access_token,
             expires_at_ms = EXCLUDED.expires_at_ms,
             scope = COALESCE(EXCLUDED.scope, deezer_accounts.scope),
             connected_at_ms = EXCLUDED.connected_at_ms",
    )
    .bind(&account.user_id)
    .bind(account.deezer_user_id.as_deref())
    .bind(account.display_name.as_deref())
    .bind(&account.access_token)
    .bind(account.expires_at_ms)
    .bind(account.scope.as_deref())
    .bind(account.connected_at_ms)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_deezer_account(
    pool: &PgPool,
    user_id: &str,
) -> Result<Option<DeezerAccount>, sqlx::Error> {
    sqlx::query_as::<_, DeezerAccount>("SELECT * FROM deezer_accounts WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
}

/// Supprime la liaison Deezer (le jeton est oublie).
pub async fn delete_deezer_account(pool: &PgPool, user_id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM deezer_accounts WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
