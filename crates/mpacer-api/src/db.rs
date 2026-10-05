//! Acces base de donnees (PostgreSQL) et initialisation du schema.
//!
//! Toutes les requetes utilisent des parametres numerotes ($1, $2...) : aucune
//! valeur n'est concatenee dans le SQL, donc aucune injection possible.

use crate::config::Config;
use crate::models::{ApiToken, User, WorkoutRow};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use std::str::FromStr;
use std::time::Duration;

/// Schema initial, embarque dans le binaire (aucun fichier a deplacer).
const SCHEMA: &str = include_str!("../migrations/0001_init.sql");

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

pub async fn list_workouts(
    pool: &PgPool,
    user_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<WorkoutRow>, sqlx::Error> {
    sqlx::query_as::<_, WorkoutRow>(
        "SELECT id, started_at_ms, duration_s, distance_m, average_pace_s_per_km, unit_system, uploaded_at_ms
           FROM workouts WHERE user_id = $1 ORDER BY started_at_ms DESC LIMIT $2 OFFSET $3",
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn count_workouts(pool: &PgPool, user_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*)::bigint FROM workouts WHERE user_id = $1")
        .bind(user_id)
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
