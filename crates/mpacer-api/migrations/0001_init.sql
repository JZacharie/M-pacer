-- Schema M-pacer sur PostgreSQL : utilisateurs, jetons d'appareil et seances.
-- Idempotent : rejoue a chaque demarrage (aucun outil de migration a installer).

CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,
    google_sub    TEXT UNIQUE,
    email         TEXT NOT NULL,
    name          TEXT,
    picture_url   TEXT,
    created_at_ms BIGINT NOT NULL,
    last_seen_ms  BIGINT NOT NULL
);

-- Jetons d'appareil (montre) : seul le SHA-256 est stocke.
CREATE TABLE IF NOT EXISTS api_tokens (
    id            TEXT PRIMARY KEY,
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash    TEXT NOT NULL UNIQUE,
    label         TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    last_used_ms  BIGINT,
    revoked_at_ms BIGINT
);
CREATE INDEX IF NOT EXISTS idx_api_tokens_user ON api_tokens(user_id);

-- Flux d'appairage montre <-> navigateur (device authorization grant).
CREATE TABLE IF NOT EXISTS device_codes (
    device_code   TEXT PRIMARY KEY,
    user_code     TEXT NOT NULL UNIQUE,
    user_id       TEXT REFERENCES users(id) ON DELETE CASCADE,
    label         TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    expires_at_ms BIGINT NOT NULL,
    approved_at_ms BIGINT,
    consumed_at_ms BIGINT,
    last_poll_ms  BIGINT
);
CREATE INDEX IF NOT EXISTS idx_device_codes_user_code ON device_codes(user_code);

-- Seances synchronisees. `payload` contient le WorkoutSummary complet (JSON),
-- identique au format .pac produit par le coeur : la montre est la source de verite.
CREATE TABLE IF NOT EXISTS workouts (
    id                    TEXT NOT NULL,
    user_id               TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    started_at_ms         BIGINT NOT NULL,
    duration_s            DOUBLE PRECISION NOT NULL,
    distance_m            DOUBLE PRECISION NOT NULL,
    average_pace_s_per_km DOUBLE PRECISION NOT NULL,
    unit_system           TEXT NOT NULL,
    payload               TEXT NOT NULL,
    uploaded_at_ms        BIGINT NOT NULL,
    PRIMARY KEY (user_id, id)
);
CREATE INDEX IF NOT EXISTS idx_workouts_user_started ON workouts(user_id, started_at_ms DESC);

-- Codes d'etat OAuth (protection CSRF du callback Google).
CREATE TABLE IF NOT EXISTS oauth_states (
    state         TEXT PRIMARY KEY,
    pkce_verifier TEXT NOT NULL,
    redirect_to   TEXT,
    created_at_ms BIGINT NOT NULL,
    expires_at_ms BIGINT NOT NULL
);
