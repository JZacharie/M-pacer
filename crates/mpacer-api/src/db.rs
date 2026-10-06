//! Acces base de donnees (PostgreSQL) et initialisation du schema.
//!
//! Toutes les requetes utilisent des parametres numerotes ($1, $2...) : aucune
//! valeur n'est concatenee dans le SQL, donc aucune injection possible.

use crate::config::Config;
use crate::models::{ApiToken, Race, RaceInput, RaceTask, User, WorkoutRow};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use std::str::FromStr;
use std::time::Duration;

/// Schema initial, embarque dans le binaire (aucun fichier a deplacer).
const SCHEMA: &str = include_str!("../migrations/0001_init.sql");
/// Courses a venir et suivi : meme mecanisme, rejoue juste apres le schema initial.
const SCHEMA_RACES: &str = include_str!("../migrations/0002_races.sql");

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
        "SELECT id, started_at_ms, duration_s, distance_m, average_pace_s_per_km, unit_system, uploaded_at_ms
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
        "SELECT id, started_at_ms, duration_s, distance_m, average_pace_s_per_km, unit_system, uploaded_at_ms, payload
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
    };
    Ok(Some((workout, payload)))
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
        average_pace_s_per_km: if distance > 0.0 {
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
     nutrition_notes, important_info, notes, goal_time_s, created_at_ms, updated_at_ms";

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
        insert_task_row(&mut *tx, user_id, &id, label, None, now_ms).await?;
    }
    tx.commit().await?;

    get_race(pool, user_id, &id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
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
    let result = sqlx::query(
        "DELETE FROM race_tasks WHERE id = $1 AND race_id = $2 AND user_id = $3",
    )
    .bind(task_id)
    .bind(race_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
