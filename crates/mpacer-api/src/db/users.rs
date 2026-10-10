//! Gestion des utilisateurs et jetons d'appareil.

use crate::models::{ApiToken, User, UserCard};
use sqlx::PgPool;

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

/// Fiche minimale d'un compte, par son identifiant.
pub async fn user_card(pool: &PgPool, user_id: &str) -> Result<Option<UserCard>, sqlx::Error> {
    sqlx::query_as::<_, UserCard>("SELECT id, name, email, picture_url FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
}

/// Compte portant cette adresse (comparaison insensible a la casse).
pub async fn user_card_by_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<UserCard>, sqlx::Error> {
    sqlx::query_as::<_, UserCard>(
        "SELECT id, name, email, picture_url FROM users
          WHERE LOWER(email) = LOWER($1)
          ORDER BY created_at_ms LIMIT 1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

/// Recherche par adresse ou par nom, hors de son propre compte.
pub async fn search_users(
    pool: &PgPool,
    motif: &str,
    exclude_id: &str,
    limit: i64,
) -> Result<Vec<UserCard>, sqlx::Error> {
    sqlx::query_as::<_, UserCard>(
        "SELECT id, name, email, picture_url FROM users
          WHERE id <> $1
            AND (email ILIKE $2 ESCAPE '\\'
                 OR COALESCE(name, '') ILIKE $2 ESCAPE '\\')
          ORDER BY email
          LIMIT $3",
    )
    .bind(exclude_id)
    .bind(motif)
    .bind(limit)
    .fetch_all(pool)
    .await
}
