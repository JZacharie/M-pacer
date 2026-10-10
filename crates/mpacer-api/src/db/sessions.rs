//! Acces aux donnees des seances de course (workouts).

use crate::models::WorkoutRow;
use sqlx::{PgPool, Row};

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
        average_pace_s_per_km: if distance >= 100.0 {
            Some(duration / (distance / 1000.0))
        } else {
            None
        },
        longest_distance_m: longest,
    })
}
