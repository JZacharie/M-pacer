//! Acces base de donnees (PostgreSQL) et initialisation du schema.
//!
//! Toutes les requetes utilisent des parametres numerotes ($1, $2...) : aucune
//! valeur n'est concatenee dans le SQL, donc aucune injection possible.

use crate::config::Config;
use crate::models::{
    ApiToken, DeezerAccount, ImportedRaceMeta, MusicPlaylist, MusicPlaylistInput,
    MusicPlaylistSummary, MusicTrack, MusicTrackInput, Race, RaceInput, RaceTask, SpotifyAccount,
    User, WorkoutRow,
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use std::str::FromStr;
use std::time::Duration;

/// Schema initial, embarque dans le binaire (aucun fichier a deplacer).
const SCHEMA: &str = include_str!("../migrations/0001_init.sql");
/// Courses a venir et suivi : meme mecanisme, rejoue juste apres le schema initial.
const SCHEMA_RACES: &str = include_str!("../migrations/0002_races.sql");
/// Musique : playlists, titres, plans de telechargement et comptes Spotify.
const SCHEMA_MUSIC: &str = include_str!("../migrations/0003_music.sql");
/// Courses de reference importees (Strava/Garmin) et commentaires de seance.
const SCHEMA_REFERENCES: &str = include_str!("../migrations/0004-references-et-commentaires.sql");
/// Tableaux de bord : ecrans composes par l'utilisateur (widgets ordonnes).
const SCHEMA_DASHBOARDS: &str = include_str!("../migrations/0005-tableaux-de-bord.sql");
/// Sources musicales multiples : compte Deezer et identifiant Deezer des playlists.
const SCHEMA_MUSIC_SOURCES: &str = include_str!("../migrations/0006-sources-musique.sql");

/// Ouvre le pool et applique le schema (idempotent).
pub async fn connect(config: &Config) -> anyhow::Result<PgPool> {
    let options = PgConnectOptions::from_str(&config.database.url())?;
    connect_with_options(options).await
}

/// Meme chose avec des options de connexion explicites (tests, reglages fins).
pub async fn connect_with_options(options: PgConnectOptions) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options)
        .await?;
    sqlx::raw_sql(SCHEMA).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_RACES).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_MUSIC).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_REFERENCES).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_DASHBOARDS).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_MUSIC_SOURCES).execute(&pool).await?;
    Ok(pool)
}

// ------------------------------------------------------------------ utilisateurs

/// Insere ou met a jour l'utilisateur identifie par son `sub` Google.
pub async fn upsert_user(
    pool: &PgPool,
    google_sub: Option<&str>,
    email: &str,
    name: Option<&str>,
    picture_url: Option<&str>,
    now_ms: i64,
) -> Result<User, sqlx::Error> {
    // Un `sub` Google stable sert de cle fonctionnelle ; en mode dev, l'email suffit.
    let existing: Option<User> = match google_sub {
        Some(sub) => {
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE google_sub = $1")
                .bind(sub)
                .fetch_optional(pool)
                .await?
        }
        None => {
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1 AND google_sub IS NULL")
                .bind(email)
                .fetch_optional(pool)
                .await?
        }
    };

    if let Some(user) = existing {
        sqlx::query(
            "UPDATE users
                SET email = $1,
                    name = COALESCE($2, name),
                    picture_url = COALESCE($3, picture_url),
                    last_seen_ms = $4
              WHERE id = $5",
        )
        .bind(email)
        .bind(name)
        .bind(picture_url)
        .bind(now_ms)
        .bind(&user.id)
        .execute(pool)
        .await?;
        return find_user_by_id(pool, &user.id)
            .await?
            .ok_or(sqlx::Error::RowNotFound);
    }

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO users (id, google_sub, email, name, picture_url, created_at_ms, last_seen_ms)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(&id)
    .bind(google_sub)
    .bind(email)
    .bind(name)
    .bind(picture_url)
    .bind(now_ms)
    .bind(now_ms)
    .execute(pool)
    .await?;

    find_user_by_id(pool, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn find_user_by_id(pool: &PgPool, id: &str) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

// ------------------------------------------------------------------ jetons

/// Enregistre un jeton d'appareil (le hash seul est conserve).
pub async fn insert_api_token(
    pool: &PgPool,
    user_id: &str,
    token_hash: &str,
    label: &str,
    now_ms: i64,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO api_tokens (id, user_id, token_hash, label, created_at_ms) VALUES ($1, $2, $3, $4, $5)")
        .bind(&id)
        .bind(user_id)
        .bind(token_hash)
        .bind(label)
        .bind(now_ms)
        .execute(pool)
        .await?;
    Ok(id)
}

/// Retrouve l'utilisateur proprietaire d'un jeton d'appareil actif.
pub async fn user_for_token_hash(
    pool: &PgPool,
    token_hash: &str,
    now_ms: i64,
) -> Result<Option<User>, sqlx::Error> {
    let user = sqlx::query_as::<_, User>(
        "SELECT u.* FROM api_tokens t
           JOIN users u ON u.id = t.user_id
          WHERE t.token_hash = $1 AND t.revoked_at_ms IS NULL",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;

    if user.is_some() {
        sqlx::query("UPDATE api_tokens SET last_used_ms = $1 WHERE token_hash = $2")
            .bind(now_ms)
            .bind(token_hash)
            .execute(pool)
            .await?;
    }
    Ok(user)
}

pub async fn list_tokens(pool: &PgPool, user_id: &str) -> Result<Vec<ApiToken>, sqlx::Error> {
    sqlx::query_as::<_, ApiToken>(
        "SELECT id, label, created_at_ms, last_used_ms, revoked_at_ms
           FROM api_tokens WHERE user_id = $1 ORDER BY created_at_ms DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn revoke_token(
    pool: &PgPool,
    user_id: &str,
    token_id: &str,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE api_tokens SET revoked_at_ms = $1
          WHERE id = $2 AND user_id = $3 AND revoked_at_ms IS NULL",
    )
    .bind(now_ms)
    .bind(token_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

// ------------------------------------------------------------------ seances

/// Insere ou remplace une seance. Renvoie `true` si elle remplace l'existante.
pub async fn upsert_workout(
    pool: &PgPool,
    user_id: &str,
    upload: &crate::models::WorkoutUpload,
    payload: &str,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let existing: Option<String> =
        sqlx::query_scalar("SELECT id FROM workouts WHERE user_id = $1 AND id = $2")
            .bind(user_id)
            .bind(&upload.id)
            .fetch_optional(pool)
            .await?;

    let units = serde_json::to_string(&upload.units()).unwrap_or_else(|_| "\"Metric\"".into());
    sqlx::query(
        "INSERT INTO workouts (id, user_id, started_at_ms, duration_s, distance_m, average_pace_s_per_km, unit_system, payload, uploaded_at_ms)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (user_id, id) DO UPDATE SET
             duration_s = EXCLUDED.duration_s,
             distance_m = EXCLUDED.distance_m,
             average_pace_s_per_km = EXCLUDED.average_pace_s_per_km,
             unit_system = EXCLUDED.unit_system,
             payload = EXCLUDED.payload,
             uploaded_at_ms = EXCLUDED.uploaded_at_ms",
    )
    .bind(&upload.id)
    .bind(user_id)
    .bind(upload.started_at_ms)
    .bind(upload.duration_s)
    .bind(upload.distance_m)
    .bind(upload.average_pace_s_per_km)
    .bind(units.trim_matches('"'))
    .bind(payload)
    .bind(now_ms)
    .execute(pool)
    .await?;

    Ok(existing.is_some())
}

/// Filtre de liste : pagination et plage de dates (bornes incluses, en ms).
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkoutFilter {
    pub limit: i64,
    pub offset: i64,
    pub from_ms: Option<i64>,
    pub to_ms: Option<i64>,
}

pub async fn list_workouts(
    pool: &PgPool,
    user_id: &str,
    filter: &WorkoutFilter,
) -> Result<Vec<WorkoutRow>, sqlx::Error> {
    sqlx::query_as::<_, WorkoutRow>(
        "SELECT id, started_at_ms, duration_s, distance_m, average_pace_s_per_km, unit_system, uploaded_at_ms, comment
           FROM workouts
          WHERE user_id = $1
            AND ($2::bigint IS NULL OR started_at_ms >= $2)
            AND ($3::bigint IS NULL OR started_at_ms <= $3)
          ORDER BY started_at_ms DESC
          LIMIT $4 OFFSET $5",
    )
    .bind(user_id)
    .bind(filter.from_ms)
    .bind(filter.to_ms)
    .bind(filter.limit.max(1))
    .bind(filter.offset.max(0))
    .fetch_all(pool)
    .await
}

/// Toutes les seances d'un utilisateur, sous forme de `WorkoutSummary` JSON.
///
/// Sert a l'export `.pac` : le format est exactement celui produit par le coeur
/// Rust, donc reimportable par une montre ou par un autre backend.
pub async fn all_workout_payloads(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT payload FROM workouts WHERE user_id = $1 ORDER BY started_at_ms")
        .bind(user_id)
        .fetch_all(pool)
        .await
}

/// Cumul par semaine, pour la page de statistiques.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct WeekTotal {
    /// Etiquette courte de la semaine (ex. "06/10").
    pub label: String,
    pub workout_count: i64,
    pub distance_m: f64,
    pub duration_s: f64,
}

pub async fn weekly_totals(
    pool: &PgPool,
    user_id: &str,
    since_ms: i64,
) -> Result<Vec<WeekTotal>, sqlx::Error> {
    sqlx::query_as::<_, WeekTotal>(
        "SELECT to_char(date_trunc('week', to_timestamp(started_at_ms / 1000.0)), 'DD/MM') AS label,
                COUNT(*)::bigint AS workout_count,
                COALESCE(SUM(distance_m), 0)::double precision AS distance_m,
                COALESCE(SUM(duration_s), 0)::double precision AS duration_s
           FROM workouts
          WHERE user_id = $1 AND started_at_ms >= $2
          GROUP BY 1
          ORDER BY 1",
    )
    .bind(user_id)
    .bind(since_ms)
    .fetch_all(pool)
    .await
}

pub async fn count_workouts(
    pool: &PgPool,
    user_id: &str,
    filter: &WorkoutFilter,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM workouts
          WHERE user_id = $1
            AND ($2::bigint IS NULL OR started_at_ms >= $2)
            AND ($3::bigint IS NULL OR started_at_ms <= $3)",
    )
    .bind(user_id)
    .bind(filter.from_ms)
    .bind(filter.to_ms)
    .fetch_one(pool)
    .await
}

pub async fn get_workout(
    pool: &PgPool,
    user_id: &str,
    id: &str,
) -> Result<Option<(WorkoutRow, String)>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, started_at_ms, duration_s, distance_m, average_pace_s_per_km, unit_system, uploaded_at_ms, comment, payload
           FROM workouts WHERE user_id = $1 AND id = $2",
    )
    .bind(user_id)
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else { return Ok(None) };
    let payload: String = row.try_get("payload")?;
    let workout = WorkoutRow {
        id: row.try_get("id")?,
        started_at_ms: row.try_get("started_at_ms")?,
        duration_s: row.try_get("duration_s")?,
        distance_m: row.try_get("distance_m")?,
        average_pace_s_per_km: row.try_get("average_pace_s_per_km")?,
        unit_system: row.try_get("unit_system")?,
        uploaded_at_ms: row.try_get("uploaded_at_ms")?,
        comment: row.try_get("comment")?,
    };
    Ok(Some((workout, payload)))
}

/// Enregistre (ou efface) le commentaire d'une seance.
///
/// Le commentaire vit dans sa propre colonne : une nouvelle synchronisation de
/// la montre remplace le `payload` mais ne touche pas au texte du coureur.
pub async fn set_workout_comment(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    comment: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE workouts SET comment = $1 WHERE user_id = $2 AND id = $3")
        .bind(comment)
        .bind(user_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_workout(pool: &PgPool, user_id: &str, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM workouts WHERE user_id = $1 AND id = $2")
        .bind(user_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Statistiques agregees sur les `days` derniers jours.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Stats {
    pub workout_count: i64,
    pub total_distance_m: f64,
    pub total_duration_s: f64,
    pub average_pace_s_per_km: Option<f64>,
    pub longest_distance_m: f64,
}

pub async fn stats(
    pool: &PgPool,
    user_id: &str,
    days: i64,
    now_ms: i64,
) -> Result<Stats, sqlx::Error> {
    let since = now_ms - days * 24 * 3600 * 1000;
    // CAST explicites : sqlx attend bigint et double precision, y compris sur un
    // historique vide (PostgreSQL renvoie des types entiers pour les litteraux).
    let row = sqlx::query(
        "SELECT COUNT(*)::bigint AS c,
                COALESCE(SUM(distance_m), 0)::double precision AS d,
                COALESCE(SUM(duration_s), 0)::double precision AS t,
                COALESCE(MAX(distance_m), 0)::double precision AS m
           FROM workouts WHERE user_id = $1 AND started_at_ms >= $2",
    )
    .bind(user_id)
    .bind(since)
    .fetch_one(pool)
    .await?;

    let count: i64 = row.try_get("c")?;
    let distance: f64 = row.try_get("d")?;
    let duration: f64 = row.try_get("t")?;
    let longest: f64 = row.try_get("m")?;

    Ok(Stats {
        workout_count: count,
        total_distance_m: distance,
        total_duration_s: duration,
        // Sous 100 m cumules, l'allure moyenne n'a aucun sens (une seance sans
        // GPS donnait « 579:14 /km ») : on ne l'affiche pas plutot que d'écrire
        // un chiffre absurde.
        average_pace_s_per_km: if distance >= 100.0 {
            Some(duration / (distance / 1000.0))
        } else {
            None
        },
        longest_distance_m: longest,
    })
}

// ------------------------------------------------------------------ courses

/// Colonnes d'une course, dans l'ordre attendu par `bind_race_fields!`.
const RACE_COLUMNS: &str = "id, user_id, name, start_at_ms, distance_m, discipline, location, \
     start_location, bib_number, bib_pickup_at_ms, bib_pickup_location, live_url, \
     registration_url, website_url, latitude, longitude, hotel_name, hotel_address, hotel_phone, \
     hotel_url, hotel_booked, hotel_check_in_ms, hotel_check_out_ms, lodging_notes, \
     nutrition_notes, important_info, notes, goal_time_s, source, is_reference, moving_time_s, \
     elapsed_time_s, elevation_gain_m, created_at_ms, updated_at_ms";

/// Lie les 26 champs modifiables d'une course, dans l'ordre des colonnes.
///
/// Un seul endroit a corriger si une colonne s'ajoute : l'insertion et la mise
/// a jour restent strictement alignees, donc aucun decalage silencieux.
macro_rules! bind_race_fields {
    ($query:expr, $input:expr) => {
        $query
            .bind($input.name.as_str())
            .bind($input.start_at_ms)
            .bind($input.distance_m)
            .bind($input.discipline.as_deref())
            .bind($input.location.as_deref())
            .bind($input.start_location.as_deref())
            .bind($input.bib_number.as_deref())
            .bind($input.bib_pickup_at_ms)
            .bind($input.bib_pickup_location.as_deref())
            .bind($input.live_url.as_deref())
            .bind($input.registration_url.as_deref())
            .bind($input.website_url.as_deref())
            .bind($input.latitude)
            .bind($input.longitude)
            .bind($input.hotel_name.as_deref())
            .bind($input.hotel_address.as_deref())
            .bind($input.hotel_phone.as_deref())
            .bind($input.hotel_url.as_deref())
            .bind($input.hotel_booked)
            .bind($input.hotel_check_in_ms)
            .bind($input.hotel_check_out_ms)
            .bind($input.lodging_notes.as_deref())
            .bind($input.nutrition_notes.as_deref())
            .bind($input.important_info.as_deref())
            .bind($input.notes.as_deref())
            .bind($input.goal_time_s)
    };
}

/// Toutes les courses de l'utilisateur, sans date en dernier.
pub async fn list_races(pool: &PgPool, user_id: &str) -> Result<Vec<Race>, sqlx::Error> {
    sqlx::query_as::<_, Race>(&format!(
        "SELECT {RACE_COLUMNS} FROM races WHERE user_id = $1
          ORDER BY start_at_ms ASC NULLS LAST, created_at_ms ASC"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Courses a venir (ou sans date connue), de la plus proche a la plus lointaine.
pub async fn list_upcoming_races(
    pool: &PgPool,
    user_id: &str,
    now_ms: i64,
) -> Result<Vec<Race>, sqlx::Error> {
    sqlx::query_as::<_, Race>(&format!(
        "SELECT {RACE_COLUMNS} FROM races
          WHERE user_id = $1 AND (start_at_ms IS NULL OR start_at_ms >= $2)
          ORDER BY start_at_ms ASC NULLS LAST, created_at_ms ASC"
    ))
    .bind(user_id)
    .bind(now_ms)
    .fetch_all(pool)
    .await
}

/// Courses deja courues, de la plus recente a la plus ancienne.
pub async fn list_past_races(
    pool: &PgPool,
    user_id: &str,
    now_ms: i64,
) -> Result<Vec<Race>, sqlx::Error> {
    sqlx::query_as::<_, Race>(&format!(
        "SELECT {RACE_COLUMNS} FROM races
          WHERE user_id = $1 AND start_at_ms IS NOT NULL AND start_at_ms < $2
          ORDER BY start_at_ms DESC"
    ))
    .bind(user_id)
    .bind(now_ms)
    .fetch_all(pool)
    .await
}

/// Une course precise, verifiee comme appartenant a l'utilisateur.
pub async fn get_race(pool: &PgPool, user_id: &str, id: &str) -> Result<Option<Race>, sqlx::Error> {
    sqlx::query_as::<_, Race>(&format!(
        "SELECT {RACE_COLUMNS} FROM races WHERE user_id = $1 AND id = $2"
    ))
    .bind(user_id)
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Cree une course et ses elements de suivi par defaut, en une transaction.
pub async fn insert_race(
    pool: &PgPool,
    user_id: &str,
    input: &RaceInput,
    now_ms: i64,
) -> Result<Race, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = pool.begin().await?;
    bind_race_fields!(
        sqlx::query(
            "INSERT INTO races (
                 id, name, start_at_ms, distance_m, discipline, location, start_location,
                 bib_number, bib_pickup_at_ms, bib_pickup_location, live_url, registration_url,
                 website_url, latitude, longitude, hotel_name, hotel_address, hotel_phone,
                 hotel_url, hotel_booked, hotel_check_in_ms, hotel_check_out_ms, lodging_notes,
                 nutrition_notes, important_info, notes, goal_time_s,
                 user_id, created_at_ms, updated_at_ms
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                       $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30)"
        )
        .bind(&id),
        input
    )
    .bind(user_id)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut *tx)
    .await?;

    for label in crate::models::DEFAULT_RACE_TASKS {
        insert_task_row(&mut tx, user_id, &id, label, None, now_ms).await?;
    }
    tx.commit().await?;

    get_race(pool, user_id, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Cree une course de reference a partir d'un export Strava ou Garmin.
///
/// Deux differences avec `insert_race` : les metadonnees de l'import sont
/// enregistrees avec la fiche, et la trace normalisee part dans `race_tracks`
/// dans la meme transaction. Aucun element de suivi n'est cree : la course est
/// deja courue, il n'y a plus rien a preparer.
pub async fn insert_imported_race(
    pool: &PgPool,
    user_id: &str,
    input: &RaceInput,
    meta: &ImportedRaceMeta,
    now_ms: i64,
) -> Result<Race, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = pool.begin().await?;
    bind_race_fields!(
        sqlx::query(
            "INSERT INTO races (
                 id, name, start_at_ms, distance_m, discipline, location, start_location,
                 bib_number, bib_pickup_at_ms, bib_pickup_location, live_url, registration_url,
                 website_url, latitude, longitude, hotel_name, hotel_address, hotel_phone,
                 hotel_url, hotel_booked, hotel_check_in_ms, hotel_check_out_ms, lodging_notes,
                 nutrition_notes, important_info, notes, goal_time_s,
                 source, is_reference, moving_time_s, elapsed_time_s, elevation_gain_m,
                 user_id, created_at_ms, updated_at_ms
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                       $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27,
                       $28, $29, $30, $31, $32, $33, $34, $35)"
        )
        .bind(&id),
        input
    )
    .bind(&meta.source)
    .bind(true)
    .bind(meta.moving_time_s)
    .bind(meta.elapsed_time_s)
    .bind(meta.elevation_gain_m)
    .bind(user_id)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO race_tracks (race_id, user_id, gpx, points, created_at_ms)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(&meta.gpx)
    .bind(meta.points)
    .bind(now_ms)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    get_race(pool, user_id, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Trace conservee d'une course importee : contenu GPX et nombre de points.
pub async fn get_race_track(
    pool: &PgPool,
    user_id: &str,
    race_id: &str,
) -> Result<Option<(String, i32)>, sqlx::Error> {
    let row =
        sqlx::query("SELECT gpx, points FROM race_tracks WHERE user_id = $1 AND race_id = $2")
            .bind(user_id)
            .bind(race_id)
            .fetch_optional(pool)
            .await?;
    match row {
        Some(row) => Ok(Some((row.try_get("gpx")?, row.try_get("points")?))),
        None => Ok(None),
    }
}

/// Met a jour une course. Renvoie `None` si elle n'appartient pas a l'utilisateur.
pub async fn update_race(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    input: &RaceInput,
    now_ms: i64,
) -> Result<Option<Race>, sqlx::Error> {
    let result = bind_race_fields!(
        sqlx::query(
            "UPDATE races SET
                 name = $1, start_at_ms = $2, distance_m = $3, discipline = $4, location = $5,
                 start_location = $6, bib_number = $7, bib_pickup_at_ms = $8,
                 bib_pickup_location = $9, live_url = $10, registration_url = $11,
                 website_url = $12, latitude = $13, longitude = $14, hotel_name = $15,
                 hotel_address = $16, hotel_phone = $17, hotel_url = $18, hotel_booked = $19,
                 hotel_check_in_ms = $20, hotel_check_out_ms = $21, lodging_notes = $22,
                 nutrition_notes = $23, important_info = $24, notes = $25, goal_time_s = $26,
                 updated_at_ms = $27
             WHERE id = $28 AND user_id = $29"
        ),
        input
    )
    .bind(now_ms)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }
    get_race(pool, user_id, id).await
}

/// Supprime une course (ses elements de suivi suivent par cascade).
pub async fn delete_race(pool: &PgPool, user_id: &str, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM races WHERE user_id = $1 AND id = $2")
        .bind(user_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// La course appartient-elle bien a cet utilisateur ?
pub async fn race_belongs_to(
    pool: &PgPool,
    user_id: &str,
    race_id: &str,
) -> Result<bool, sqlx::Error> {
    let found: Option<String> =
        sqlx::query_scalar("SELECT id FROM races WHERE user_id = $1 AND id = $2")
            .bind(user_id)
            .bind(race_id)
            .fetch_optional(pool)
            .await?;
    Ok(found.is_some())
}

/// Elements de suivi d'une course, dans l'ordre d'affichage.
pub async fn list_race_tasks(
    pool: &PgPool,
    user_id: &str,
    race_id: &str,
) -> Result<Vec<RaceTask>, sqlx::Error> {
    sqlx::query_as::<_, RaceTask>(
        "SELECT id, race_id, user_id, label, due_at_ms, done, done_at_ms, position, created_at_ms
           FROM race_tasks
          WHERE user_id = $1 AND race_id = $2
          ORDER BY position ASC, created_at_ms ASC",
    )
    .bind(user_id)
    .bind(race_id)
    .fetch_all(pool)
    .await
}

/// Insere une ligne de suivi ; la position suit l'ordre d'ajout.
async fn insert_task_row(
    executor: &mut sqlx::PgConnection,
    user_id: &str,
    race_id: &str,
    label: &str,
    due_at_ms: Option<i64>,
    now_ms: i64,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO race_tasks (id, race_id, user_id, label, due_at_ms, done, position, created_at_ms)
         VALUES ($1, $2, $3, $4, $5, FALSE,
                 COALESCE((SELECT MAX(position) + 1 FROM race_tasks WHERE race_id = $2), 0), $6)",
    )
    .bind(&id)
    .bind(race_id)
    .bind(user_id)
    .bind(label)
    .bind(due_at_ms)
    .bind(now_ms)
    .execute(executor)
    .await?;
    Ok(id)
}

/// Ajoute un element de suivi. Renvoie son identifiant, ou `None` si la course
/// n'appartient pas a l'utilisateur.
pub async fn insert_race_task(
    pool: &PgPool,
    user_id: &str,
    race_id: &str,
    label: &str,
    due_at_ms: Option<i64>,
    now_ms: i64,
) -> Result<Option<String>, sqlx::Error> {
    if !race_belongs_to(pool, user_id, race_id).await? {
        return Ok(None);
    }
    let mut tx = pool.begin().await?;
    let id = insert_task_row(&mut tx, user_id, race_id, label, due_at_ms, now_ms).await?;
    tx.commit().await?;
    Ok(Some(id))
}

/// Coche ou decoche un element de suivi.
pub async fn set_race_task_done(
    pool: &PgPool,
    user_id: &str,
    race_id: &str,
    task_id: &str,
    done: bool,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE race_tasks
            SET done = $1,
                done_at_ms = CASE WHEN $1 THEN $2 ELSE NULL END
          WHERE id = $3 AND race_id = $4 AND user_id = $5",
    )
    .bind(done)
    .bind(now_ms)
    .bind(task_id)
    .bind(race_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Supprime un element de suivi.
pub async fn delete_race_task(
    pool: &PgPool,
    user_id: &str,
    race_id: &str,
    task_id: &str,
) -> Result<bool, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM race_tasks WHERE id = $1 AND race_id = $2 AND user_id = $3")
            .bind(task_id)
            .bind(race_id)
            .bind(user_id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

// ------------------------------------------------------------------ musique

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
             (id, user_id, name, source, spotify_id, deezer_id, cover_url, target_bpm,
              created_at_ms, updated_at_ms)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(&input.name)
    .bind(&input.source)
    .bind(input.spotify_id.as_deref())
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

/// Playlist deja importee depuis Spotify (un import deux fois ne cree pas de doublon).
pub async fn find_music_playlist_by_spotify(
    pool: &PgPool,
    user_id: &str,
    spotify_id: &str,
) -> Result<Option<MusicPlaylist>, sqlx::Error> {
    sqlx::query_as::<_, MusicPlaylist>(
        "SELECT * FROM music_playlists WHERE user_id = $1 AND spotify_id = $2",
    )
    .bind(user_id)
    .bind(spotify_id)
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
///
/// Un LEFT JOIN suffit : une playlist sans titre renvoie 0 partout, jamais NULL.
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
///
/// Renvoie `false` si la playlist n'appartient pas a l'utilisateur.
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

/// Supprime une playlist (metadonnees seules : ses titres partent par cascade).
///
/// Renvoie `false` si la playlist n'appartient pas a l'utilisateur.
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

// ------------------------------------------------------------------ titres

/// Insere un titre ; la position est fournie par l'appelant (ordre de playlist).
pub async fn insert_music_track(
    pool: &PgPool,
    user_id: &str,
    playlist_id: &str,
    input: &MusicTrackInput,
    now_ms: i64,
) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    // Aucun audio n'est stocke (v2) : les colonnes mime, size_bytes et
    // storage_path restent a NULL en base.
    sqlx::query(
        "INSERT INTO music_tracks
             (id, playlist_id, user_id, position, title, artist, album, duration_s, bpm,
              bpm_source, spotify_uri, created_at_ms)
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
    .bind(input.spotify_uri.as_deref())
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

/// Enregistre un BPM (tap-tempo ou saisie manuelle) pour un titre de la playlist.
///
/// La source fait partie du vocabulaire gele ; une source inconnue est ignoree
/// plutot que stockee.
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

// La table music_download_plans (plan de telechargement v1) n'est plus lue ni
// ecrite : le transfert passe par le manifeste et l'outil local mpacer-music.

// ------------------------------------------------------------------ comptes Spotify

/// Enregistre (ou remplace) le compte Spotify lie a l'utilisateur.
///
/// Un rafraichissement de jeton ne renvoie pas toujours de `refresh_token` :
/// l'ancien est alors conserve, sinon la liaison deviendrait inutilisable.
pub async fn upsert_spotify_account(
    pool: &PgPool,
    account: &SpotifyAccount,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO spotify_accounts
             (user_id, spotify_user_id, display_name, access_token, refresh_token,
              expires_at_ms, scope, connected_at_ms)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (user_id) DO UPDATE SET
             spotify_user_id = EXCLUDED.spotify_user_id,
             display_name = COALESCE(EXCLUDED.display_name, spotify_accounts.display_name),
             access_token = EXCLUDED.access_token,
             refresh_token = COALESCE(EXCLUDED.refresh_token, spotify_accounts.refresh_token),
             expires_at_ms = EXCLUDED.expires_at_ms,
             scope = COALESCE(EXCLUDED.scope, spotify_accounts.scope),
             connected_at_ms = EXCLUDED.connected_at_ms",
    )
    .bind(&account.user_id)
    .bind(account.spotify_user_id.as_deref())
    .bind(account.display_name.as_deref())
    .bind(&account.access_token)
    .bind(account.refresh_token.as_deref())
    .bind(account.expires_at_ms)
    .bind(account.scope.as_deref())
    .bind(account.connected_at_ms)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_spotify_account(
    pool: &PgPool,
    user_id: &str,
) -> Result<Option<SpotifyAccount>, sqlx::Error> {
    sqlx::query_as::<_, SpotifyAccount>("SELECT * FROM spotify_accounts WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
}

/// Met a jour le seul jeton d'acces (rafraichissement OAuth).
pub async fn update_spotify_access_token(
    pool: &PgPool,
    user_id: &str,
    access_token: &str,
    expires_at_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE spotify_accounts SET access_token = $1, expires_at_ms = $2 WHERE user_id = $3",
    )
    .bind(access_token)
    .bind(expires_at_ms)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Supprime la liaison Spotify (les jetons sont oublies).
pub async fn delete_spotify_account(pool: &PgPool, user_id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM spotify_accounts WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// ------------------------------------------------------------------ comptes Deezer

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

// ------------------------------------------------------------ tableaux de bord

/// Tableaux de bord de l'utilisateur, du plus recent au plus ancien.
pub async fn list_dashboards(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<crate::models::Dashboard>, sqlx::Error> {
    sqlx::query_as::<_, crate::models::Dashboard>(
        "SELECT id, user_id, name, created_at_ms, updated_at_ms
           FROM dashboards
          WHERE user_id = $1
          ORDER BY created_at_ms DESC, name ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Un tableau precis, verifie comme appartenant a l'utilisateur.
pub async fn get_dashboard(
    pool: &PgPool,
    user_id: &str,
    id: &str,
) -> Result<Option<crate::models::Dashboard>, sqlx::Error> {
    sqlx::query_as::<_, crate::models::Dashboard>(
        "SELECT id, user_id, name, created_at_ms, updated_at_ms
           FROM dashboards
          WHERE user_id = $1 AND id = $2",
    )
    .bind(user_id)
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Widgets d'un tableau, dans l'ordre d'affichage.
pub async fn list_dashboard_widgets(
    pool: &PgPool,
    user_id: &str,
    dashboard_id: &str,
) -> Result<Vec<crate::models::DashboardWidget>, sqlx::Error> {
    sqlx::query_as::<_, crate::models::DashboardWidget>(
        "SELECT id, dashboard_id, user_id, kind, position, created_at_ms
           FROM dashboard_widgets
          WHERE user_id = $1 AND dashboard_id = $2
          ORDER BY position ASC, created_at_ms ASC",
    )
    .bind(user_id)
    .bind(dashboard_id)
    .fetch_all(pool)
    .await
}

/// Nombre de widgets par tableau : une seule requete pour la page de liste.
pub async fn dashboard_widget_counts(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<(String, i64)>, sqlx::Error> {
    sqlx::query_as::<_, (String, i64)>(
        "SELECT dashboard_id, COUNT(*)::bigint
           FROM dashboard_widgets
          WHERE user_id = $1
          GROUP BY dashboard_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Cree un tableau et ses widgets, en une transaction.
///
/// `name` est deja nettoye par l'appelant (`clean_dashboard_name`) et `kinds`
/// ne contient que des cles du catalogue.
pub async fn insert_dashboard(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    kinds: &[String],
    now_ms: i64,
) -> Result<crate::models::Dashboard, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO dashboards (id, user_id, name, created_at_ms, updated_at_ms)
         VALUES ($1, $2, $3, $4, $4)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(name)
    .bind(now_ms)
    .execute(&mut *tx)
    .await?;
    for (position, kind) in kinds.iter().enumerate() {
        sqlx::query(
            "INSERT INTO dashboard_widgets (id, dashboard_id, user_id, kind, position, created_at_ms)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&id)
        .bind(user_id)
        .bind(kind)
        .bind(position as i32)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(crate::models::Dashboard {
        id,
        user_id: user_id.to_string(),
        name: name.to_string(),
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
    })
}

/// Renomme un tableau. `false` s'il n'appartient pas a l'utilisateur.
pub async fn rename_dashboard(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    name: &str,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE dashboards SET name = $1, updated_at_ms = $2 WHERE user_id = $3 AND id = $4",
    )
    .bind(name)
    .bind(now_ms)
    .bind(user_id)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Supprime un tableau ; ses widgets partent avec lui (ON DELETE CASCADE).
pub async fn delete_dashboard(pool: &PgPool, user_id: &str, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM dashboards WHERE user_id = $1 AND id = $2")
        .bind(user_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Ajoute un widget en fin de tableau. `None` si le tableau n'est pas au coureur.
pub async fn add_dashboard_widget(
    pool: &PgPool,
    user_id: &str,
    dashboard_id: &str,
    kind: &str,
    now_ms: i64,
) -> Result<Option<String>, sqlx::Error> {
    if get_dashboard(pool, user_id, dashboard_id).await?.is_none() {
        return Ok(None);
    }
    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO dashboard_widgets (id, dashboard_id, user_id, kind, position, created_at_ms)
         VALUES ($1, $2, $3, $4,
                 COALESCE((SELECT MAX(position) + 1 FROM dashboard_widgets WHERE dashboard_id = $2), 0),
                 $5)",
    )
    .bind(&id)
    .bind(dashboard_id)
    .bind(user_id)
    .bind(kind)
    .bind(now_ms)
    .execute(&mut *tx)
    .await?;
    touch_dashboard(&mut tx, user_id, dashboard_id, now_ms).await?;
    tx.commit().await?;
    Ok(Some(id))
}

/// Retire un widget ; les suivants remontent d'un cran pour combler le trou.
pub async fn remove_dashboard_widget(
    pool: &PgPool,
    user_id: &str,
    dashboard_id: &str,
    widget_id: &str,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let position: Option<i32> = sqlx::query_scalar(
        "SELECT position FROM dashboard_widgets
          WHERE id = $1 AND dashboard_id = $2 AND user_id = $3",
    )
    .bind(widget_id)
    .bind(dashboard_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(position) = position else {
        tx.rollback().await?;
        return Ok(false);
    };
    sqlx::query("DELETE FROM dashboard_widgets WHERE id = $1 AND user_id = $2")
        .bind(widget_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE dashboard_widgets SET position = position - 1
          WHERE dashboard_id = $1 AND user_id = $2 AND position > $3",
    )
    .bind(dashboard_id)
    .bind(user_id)
    .bind(position)
    .execute(&mut *tx)
    .await?;
    touch_dashboard(&mut tx, user_id, dashboard_id, now_ms).await?;
    tx.commit().await?;
    Ok(true)
}

/// Deplace un widget d'un cran (haut ou bas). `false` s'il est deja au bord.
pub async fn move_dashboard_widget(
    pool: &PgPool,
    user_id: &str,
    dashboard_id: &str,
    widget_id: &str,
    up: bool,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let current: Option<i32> = sqlx::query_scalar(
        "SELECT position FROM dashboard_widgets
          WHERE id = $1 AND dashboard_id = $2 AND user_id = $3",
    )
    .bind(widget_id)
    .bind(dashboard_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(current) = current else {
        tx.rollback().await?;
        return Ok(false);
    };
    let neighbour: Option<(String, i32)> = if up {
        sqlx::query_as(
            "SELECT id, position FROM dashboard_widgets
              WHERE dashboard_id = $1 AND user_id = $2 AND position < $3
              ORDER BY position DESC LIMIT 1",
        )
        .bind(dashboard_id)
        .bind(user_id)
        .bind(current)
        .fetch_optional(&mut *tx)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, position FROM dashboard_widgets
              WHERE dashboard_id = $1 AND user_id = $2 AND position > $3
              ORDER BY position ASC LIMIT 1",
        )
        .bind(dashboard_id)
        .bind(user_id)
        .bind(current)
        .fetch_optional(&mut *tx)
        .await?
    };
    let Some((neighbour_id, neighbour_position)) = neighbour else {
        tx.rollback().await?;
        return Ok(false);
    };
    sqlx::query("UPDATE dashboard_widgets SET position = $1 WHERE id = $2 AND user_id = $3")
        .bind(neighbour_position)
        .bind(widget_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE dashboard_widgets SET position = $1 WHERE id = $2 AND user_id = $3")
        .bind(current)
        .bind(neighbour_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    touch_dashboard(&mut tx, user_id, dashboard_id, now_ms).await?;
    tx.commit().await?;
    Ok(true)
}

/// Marque un tableau comme modifie ; silencieux si le tableau n'existe plus.
async fn touch_dashboard(
    tx: &mut sqlx::PgConnection,
    user_id: &str,
    dashboard_id: &str,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE dashboards SET updated_at_ms = $1 WHERE id = $2 AND user_id = $3")
        .bind(now_ms)
        .bind(dashboard_id)
        .bind(user_id)
        .execute(tx)
        .await?;
    Ok(())
}
