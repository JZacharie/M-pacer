-- Musique : playlists de course, titres et plans de telechargement.
-- Idempotent : rejoue a chaque demarrage, comme 0001_init.sql et 0002_races.sql.
--
-- Deezer est la seule source externe (voir 0006 et 0009) : le serveur ne
-- conserve que des metadonnees, jamais d'audio.

CREATE TABLE IF NOT EXISTS music_playlists (
    id            TEXT PRIMARY KEY,
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    -- `deezer` | `manual`
    source        TEXT NOT NULL,
    cover_url     TEXT,
    -- Consigne fixe decidee dans l'interface ; NULL = tempo automatique.
    target_bpm    DOUBLE PRECISION,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_music_playlists_user
    ON music_playlists(user_id, updated_at_ms DESC);

CREATE TABLE IF NOT EXISTS music_tracks (
    id               TEXT PRIMARY KEY,
    playlist_id      TEXT NOT NULL REFERENCES music_playlists(id) ON DELETE CASCADE,
    user_id          TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    position         INTEGER NOT NULL DEFAULT 0,
    title            TEXT NOT NULL,
    artist           TEXT,
    album            TEXT,
    duration_s       DOUBLE PRECISION,
    bpm              DOUBLE PRECISION,
    -- `tag` | `tap` | `manual` ; NULL = tempo inconnu.
    bpm_source       TEXT,
    mime             TEXT,
    size_bytes       BIGINT,
    -- Chemin relatif a MPACER_MEDIA_DIR ; NULL = aucun octet sur le serveur.
    storage_path     TEXT,
    created_at_ms    BIGINT NOT NULL,
    -- Accuse de la montre (POST /api/v1/music/playlists/{id}/ack) : NULL tant
    -- que la montre n'a pas confirme avoir recupere la piste.
    downloaded_at_ms BIGINT
);
CREATE INDEX IF NOT EXISTS idx_music_tracks_playlist
    ON music_tracks(playlist_id, position);

CREATE TABLE IF NOT EXISTS music_download_plans (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    playlist_id     TEXT NOT NULL REFERENCES music_playlists(id) ON DELETE CASCADE,
    race_id         TEXT REFERENCES races(id) ON DELETE SET NULL,
    target_bpm      DOUBLE PRECISION,
    requested_at_ms BIGINT NOT NULL,
    acked_at_ms     BIGINT
);
-- Index partiel du contrat : (user_id, acked_at_ms) sert a retrouver le dernier
-- plan en attente sans balayer l'historique.
CREATE INDEX IF NOT EXISTS idx_music_download_plans_user
    ON music_download_plans(user_id, acked_at_ms);

