-- Sources musicales multiples : compte Deezer en plus de Spotify.
-- Idempotent : rejoue a chaque demarrage, comme les migrations precedentes.
--
-- Deezer ne delivre pas de refresh_token : le jeton d'acces est conserve tel
-- quel, avec une date d'expiration (0 = duree inconnue, jeton sans expiration).

ALTER TABLE music_playlists ADD COLUMN IF NOT EXISTS deezer_id TEXT;

-- Retrouver la playlist deja importee depuis Deezer (pas de doublon a l'import).
CREATE INDEX IF NOT EXISTS idx_music_playlists_deezer
    ON music_playlists(user_id, deezer_id);

CREATE TABLE IF NOT EXISTS deezer_accounts (
    user_id         TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    deezer_user_id  TEXT,
    display_name    TEXT,
    access_token    TEXT NOT NULL,
    expires_at_ms   BIGINT NOT NULL,
    scope           TEXT,
    connected_at_ms BIGINT NOT NULL
);
