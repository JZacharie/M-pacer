-- Courses a venir : fiche de course et suivi des elements importants.
-- Idempotent : rejoue a chaque demarrage, comme 0001_init.sql.

CREATE TABLE IF NOT EXISTS races (
    id                  TEXT PRIMARY KEY,
    user_id             TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name                TEXT NOT NULL,
    start_at_ms         BIGINT,
    distance_m          DOUBLE PRECISION,
    discipline          TEXT,
    location            TEXT,
    start_location      TEXT,
    bib_number          TEXT,
    bib_pickup_at_ms    BIGINT,
    bib_pickup_location TEXT,
    live_url            TEXT,
    registration_url    TEXT,
    website_url         TEXT,
    latitude            DOUBLE PRECISION,
    longitude           DOUBLE PRECISION,
    hotel_name          TEXT,
    hotel_address       TEXT,
    hotel_phone         TEXT,
    hotel_url           TEXT,
    hotel_booked        BOOLEAN NOT NULL DEFAULT FALSE,
    hotel_check_in_ms   BIGINT,
    hotel_check_out_ms  BIGINT,
    lodging_notes       TEXT,
    nutrition_notes     TEXT,
    important_info      TEXT,
    notes               TEXT,
    goal_time_s         DOUBLE PRECISION,
    created_at_ms       BIGINT NOT NULL,
    updated_at_ms       BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_races_user_start ON races(user_id, start_at_ms);

-- Suivi d'une course : un element important a preparer (dossard, hotel, materiel...).
CREATE TABLE IF NOT EXISTS race_tasks (
    id            TEXT PRIMARY KEY,
    race_id       TEXT NOT NULL REFERENCES races(id) ON DELETE CASCADE,
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    label         TEXT NOT NULL,
    due_at_ms     BIGINT,
    done          BOOLEAN NOT NULL DEFAULT FALSE,
    done_at_ms    BIGINT,
    position      INTEGER NOT NULL DEFAULT 0,
    created_at_ms BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_race_tasks_race ON race_tasks(race_id, position, created_at_ms);
