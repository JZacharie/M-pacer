-- Courses de reference importees (export Strava ou Garmin) et commentaire de
-- seance. Idempotent : rejoue a chaque demarrage, comme les schemas precedents.

-- Une course importee est une course deja courue : elle porte l'origine du
-- fichier, ses temps et son denivele. Les colonnes sont ajoutees avec ADD COLUMN
-- IF NOT EXISTS pour que la migration reste rejouable sur une base existante.
ALTER TABLE races ADD COLUMN IF NOT EXISTS source TEXT;
ALTER TABLE races ADD COLUMN IF NOT EXISTS is_reference BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE races ADD COLUMN IF NOT EXISTS moving_time_s DOUBLE PRECISION;
ALTER TABLE races ADD COLUMN IF NOT EXISTS elapsed_time_s DOUBLE PRECISION;
ALTER TABLE races ADD COLUMN IF NOT EXISTS elevation_gain_m DOUBLE PRECISION;

-- Trace normalisee de la course importee (GPX 1.1) : elle permet de reexporter
-- le parcours et de relire un profil, sans conserver le fichier d'origine.
CREATE TABLE IF NOT EXISTS race_tracks (
    race_id       TEXT PRIMARY KEY REFERENCES races(id) ON DELETE CASCADE,
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    gpx           TEXT NOT NULL,
    points        INTEGER NOT NULL,
    created_at_ms BIGINT NOT NULL
);

-- Commentaire du coureur apres une seance. C'est une colonne a part et non un
-- champ du payload : la montre renvoie son resume a chaque synchronisation, et
-- ce renvoi ne doit jamais effacer le texte ecrit depuis le navigateur.
ALTER TABLE workouts ADD COLUMN IF NOT EXISTS comment TEXT;
