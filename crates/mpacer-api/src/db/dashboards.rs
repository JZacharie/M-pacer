//! Acces aux tableaux de bord personnalises et leurs widgets.

use crate::models::{Dashboard, DashboardWidget};
use sqlx::PgPool;

/// Tableaux de bord de l'utilisateur, du plus recent au plus ancien.
pub async fn list_dashboards(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<Dashboard>, sqlx::Error> {
    sqlx::query_as::<_, Dashboard>(
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
) -> Result<Option<Dashboard>, sqlx::Error> {
    sqlx::query_as::<_, Dashboard>(
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
) -> Result<Vec<DashboardWidget>, sqlx::Error> {
    sqlx::query_as::<_, DashboardWidget>(
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
pub async fn insert_dashboard(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    kinds: &[String],
    now_ms: i64,
) -> Result<Dashboard, sqlx::Error> {
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
    Ok(Dashboard {
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
