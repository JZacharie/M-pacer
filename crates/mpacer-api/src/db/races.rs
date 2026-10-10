//! Acces aux courses (races), traces associees et taches de suivi.

use crate::models::{ImportedRaceMeta, Race, RaceInput, RaceTask};
use sqlx::{PgPool, Row};

/// Colonnes d'une course, dans l'ordre attendu par `bind_race_fields!`.
pub const RACE_COLUMNS: &str = "id, user_id, name, start_at_ms, distance_m, discipline, location, \
     start_location, bib_number, bib_pickup_at_ms, bib_pickup_location, live_url, \
     registration_url, website_url, latitude, longitude, hotel_name, hotel_address, hotel_phone, \
     hotel_url, hotel_booked, hotel_check_in_ms, hotel_check_out_ms, lodging_notes, \
     nutrition_notes, important_info, notes, goal_time_s, source, is_reference, moving_time_s, \
     elapsed_time_s, elevation_gain_m, created_at_ms, updated_at_ms";

/// Lie les 26 champs modifiables d'une course, dans l'ordre des colonnes.
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

/// Ajoute un element de suivi.
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
