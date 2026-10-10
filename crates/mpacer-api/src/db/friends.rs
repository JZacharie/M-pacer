//! Acces aux relations d'amitie, demandes, invitations et appareils revendiques (live).

use crate::friends::{InviteOutcome, RequestOutcome, MAX_FRIENDS, MAX_PENDING_REQUESTS};
use crate::models::{FriendRequestRow, FriendRow, LiveDeviceRow, UserCard};
use sqlx::PgPool;

/// Selection commune d'une demande, avec la fiche des deux comptes.
pub const REQUEST_SELECT: &str = "SELECT r.id, r.from_user_id, r.to_user_id, r.message,
        r.created_at_ms,
        f.name AS from_name, f.email AS from_email,
        f.picture_url AS from_picture_url,
        t.name AS to_name, t.email AS to_email,
        t.picture_url AS to_picture_url
   FROM friend_requests r
   JOIN users f ON f.id = r.from_user_id
   JOIN users t ON t.id = r.to_user_id";

/// Amis d'un utilisateur, du plus recent au plus ancien.
pub async fn list_friends(pool: &PgPool, user_id: &str) -> Result<Vec<FriendRow>, sqlx::Error> {
    sqlx::query_as::<_, FriendRow>(
        "SELECT u.id, u.name, u.email, u.picture_url, u.share_live,
                f.created_at_ms AS since_ms
           FROM friendships f
           JOIN users u ON u.id = f.friend_id
          WHERE f.user_id = $1
          ORDER BY f.created_at_ms DESC, u.email",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Nombre d'amis (garde-fou avant d'en ajouter un).
pub async fn count_friends(pool: &PgPool, user_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*)::bigint FROM friendships WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
}

/// Vrai si les deux comptes sont amis.
pub async fn are_friends(pool: &PgPool, a: &str, b: &str) -> Result<bool, sqlx::Error> {
    let existe: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM friendships WHERE user_id = $1 AND friend_id = $2")
            .bind(a)
            .bind(b)
            .fetch_optional(pool)
            .await?;
    Ok(existe.is_some())
}

/// Cree l'amitie dans les deux sens (sans erreur si elle existe deja).
pub async fn add_friendship(
    pool: &PgPool,
    a: &str,
    b: &str,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO friendships (user_id, friend_id, created_at_ms)
         VALUES ($1, $2, $3), ($2, $1, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(a)
    .bind(b)
    .bind(now_ms)
    .execute(pool)
    .await?;
    Ok(())
}

/// Retire un ami : les deux sens d'un coup. `true` si un lien existait.
pub async fn remove_friendship(pool: &PgPool, a: &str, b: &str) -> Result<bool, sqlx::Error> {
    let resultat = sqlx::query(
        "DELETE FROM friendships
          WHERE (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)",
    )
    .bind(a)
    .bind(b)
    .execute(pool)
    .await?;
    Ok(resultat.rows_affected() > 0)
}

/// Regle l'interrupteur de partage d'un compte.
pub async fn set_share_live(
    pool: &PgPool,
    user_id: &str,
    partage: bool,
) -> Result<bool, sqlx::Error> {
    let resultat = sqlx::query("UPDATE users SET share_live = $1 WHERE id = $2")
        .bind(partage)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(resultat.rows_affected() > 0)
}

/// Enregistre un code d'invitation (forme normalisee, sans tiret).
pub async fn insert_friend_invite(
    pool: &PgPool,
    code: &str,
    user_id: &str,
    now_ms: i64,
    expires_ms: i64,
) -> Result<bool, sqlx::Error> {
    let resultat = sqlx::query(
        "INSERT INTO friend_invites (code, user_id, created_at_ms, expires_at_ms)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (code) DO NOTHING",
    )
    .bind(code)
    .bind(user_id)
    .bind(now_ms)
    .bind(expires_ms)
    .execute(pool)
    .await?;
    Ok(resultat.rows_affected() > 0)
}

/// Dernier code emis par un compte encore valide, s'il y en a un.
pub async fn current_friend_invite(
    pool: &PgPool,
    user_id: &str,
    now_ms: i64,
) -> Result<Option<(String, i64)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT code, expires_at_ms FROM friend_invites
          WHERE user_id = $1 AND used_at_ms IS NULL AND expires_at_ms > $2
          ORDER BY created_at_ms DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(now_ms)
    .fetch_optional(pool)
    .await
}

/// Accepte un code d'invitation : cree l'amitie dans les deux sens.
pub async fn accept_friend_invite(
    pool: &PgPool,
    code: &str,
    user_id: &str,
    now_ms: i64,
) -> Result<InviteOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let invitation: Option<(String, i64, Option<i64>)> = sqlx::query_as(
        "SELECT user_id, expires_at_ms, used_at_ms FROM friend_invites
          WHERE REPLACE(code, '-', '') = $1
          FOR UPDATE",
    )
    .bind(code)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((proprietaire, expiration, consomme)) = invitation else {
        tx.rollback().await?;
        return Ok(InviteOutcome::Unknown);
    };
    if consomme.is_some() {
        tx.rollback().await?;
        return Ok(InviteOutcome::Unknown);
    }
    if expiration <= now_ms {
        tx.rollback().await?;
        return Ok(InviteOutcome::Expired);
    }
    if proprietaire == user_id {
        tx.rollback().await?;
        return Ok(InviteOutcome::SelfInvite);
    }

    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM friendships WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
    if total >= MAX_FRIENDS {
        tx.rollback().await?;
        return Ok(InviteOutcome::TooMany);
    }

    sqlx::query("UPDATE friend_invites SET used_by = $1, used_at_ms = $2 WHERE code = $3")
        .bind(user_id)
        .bind(now_ms)
        .bind(code)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "INSERT INTO friendships (user_id, friend_id, created_at_ms)
         VALUES ($1, $2, $3), ($2, $1, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(&proprietaire)
    .bind(now_ms)
    .execute(&mut *tx)
    .await?;

    let ami = sqlx::query_as::<_, FriendRow>(
        "SELECT u.id, u.name, u.email, u.picture_url, u.share_live,
                f.created_at_ms AS since_ms
           FROM friendships f
           JOIN users u ON u.id = f.friend_id
          WHERE f.user_id = $1 AND f.friend_id = $2",
    )
    .bind(user_id)
    .bind(&proprietaire)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(InviteOutcome::Accepted(Box::new(ami)))
}

/// Demandes recues, de la plus recente a la plus ancienne.
pub async fn incoming_friend_requests(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<FriendRequestRow>, sqlx::Error> {
    sqlx::query_as::<_, FriendRequestRow>(&format!(
        "{REQUEST_SELECT} WHERE r.to_user_id = $1 ORDER BY r.created_at_ms DESC"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Demandes envoyees, en attente de reponse.
pub async fn outgoing_friend_requests(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<FriendRequestRow>, sqlx::Error> {
    sqlx::query_as::<_, FriendRequestRow>(&format!(
        "{REQUEST_SELECT} WHERE r.from_user_id = $1 ORDER BY r.created_at_ms DESC"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Nombre de demandes recues en attente (pastille de l'interface).
pub async fn count_incoming_friend_requests(
    pool: &PgPool,
    user_id: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*)::bigint FROM friend_requests WHERE to_user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
}

/// Vrai si une demande attend dans ce sens (de a vers b).
pub async fn friend_request_between(pool: &PgPool, a: &str, b: &str) -> Result<bool, sqlx::Error> {
    let existe: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM friend_requests WHERE from_user_id = $1 AND to_user_id = $2",
    )
    .bind(a)
    .bind(b)
    .fetch_optional(pool)
    .await?;
    Ok(existe.is_some())
}

/// Envoie une demande d'amitie : l'amitie n'existe qu'apres acceptation.
pub async fn send_friend_request(
    pool: &PgPool,
    from: &UserCard,
    to: &UserCard,
    message: Option<&str>,
    now_ms: i64,
) -> Result<RequestOutcome, sqlx::Error> {
    if from.id == to.id {
        return Ok(RequestOutcome::SelfRequest);
    }
    let mut tx = pool.begin().await?;

    let deja_amis: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM friendships WHERE user_id = $1 AND friend_id = $2")
            .bind(&from.id)
            .bind(&to.id)
            .fetch_optional(&mut *tx)
            .await?;
    if deja_amis.is_some() {
        tx.rollback().await?;
        return Ok(RequestOutcome::AlreadyFriends);
    }

    let identique: Option<String> = sqlx::query_scalar(
        "SELECT id FROM friend_requests WHERE from_user_id = $1 AND to_user_id = $2",
    )
    .bind(&from.id)
    .bind(&to.id)
    .fetch_optional(&mut *tx)
    .await?;
    if identique.is_some() {
        tx.rollback().await?;
        return Ok(RequestOutcome::AlreadySent);
    }

    // L'autre avait deja demande : demander en retour, c'est accepter.
    let reciproque: Option<String> = sqlx::query_scalar(
        "SELECT id FROM friend_requests WHERE from_user_id = $1 AND to_user_id = $2 FOR UPDATE",
    )
    .bind(&to.id)
    .bind(&from.id)
    .fetch_optional(&mut *tx)
    .await?;
    if reciproque.is_some() {
        add_friendship_tx(&mut tx, &from.id, &to.id, now_ms).await?;
        delete_friend_requests_tx(&mut tx, &from.id, &to.id).await?;
        let ami = friend_row_tx(&mut tx, &from.id, &to.id).await?;
        tx.commit().await?;
        return Ok(RequestOutcome::AlreadyIncoming(Box::new(ami)));
    }

    let total_expediteur: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM friendships WHERE user_id = $1")
            .bind(&from.id)
            .fetch_one(&mut *tx)
            .await?;
    if total_expediteur >= MAX_FRIENDS {
        tx.rollback().await?;
        return Ok(RequestOutcome::TooMany);
    }

    let total_destinataire: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM friend_requests WHERE to_user_id = $1")
            .bind(&to.id)
            .fetch_one(&mut *tx)
            .await?;
    if total_destinataire >= MAX_PENDING_REQUESTS {
        tx.rollback().await?;
        return Ok(RequestOutcome::TooManyPending);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let insere = sqlx::query(
        "INSERT INTO friend_requests (id, from_user_id, to_user_id, message, created_at_ms)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (from_user_id, to_user_id) DO NOTHING",
    )
    .bind(&id)
    .bind(&from.id)
    .bind(&to.id)
    .bind(message)
    .bind(now_ms)
    .execute(&mut *tx)
    .await?;
    if insere.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(RequestOutcome::AlreadySent);
    }
    tx.commit().await?;

    let row = FriendRequestRow {
        id,
        from_user_id: from.id.clone(),
        to_user_id: to.id.clone(),
        message: message.map(str::to_string),
        created_at_ms: now_ms,
        from_name: from.name.clone(),
        from_email: from.email.clone(),
        from_picture_url: from.picture_url.clone(),
        to_name: to.name.clone(),
        to_email: to.email.clone(),
        to_picture_url: to.picture_url.clone(),
    };
    Ok(RequestOutcome::Sent(Box::new(
        crate::friends::FriendRequestView::from_row(&row, &from.id),
    )))
}

/// Accepte une demande recue : l'amitie est creee dans les deux sens.
pub async fn accept_friend_request(
    pool: &PgPool,
    request_id: &str,
    user_id: &str,
    now_ms: i64,
) -> Result<Option<FriendRow>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let demandeur: Option<String> = sqlx::query_scalar(
        "SELECT from_user_id FROM friend_requests
          WHERE id = $1 AND to_user_id = $2
          FOR UPDATE",
    )
    .bind(request_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(demandeur) = demandeur else {
        tx.rollback().await?;
        return Ok(None);
    };

    add_friendship_tx(&mut tx, &demandeur, user_id, now_ms).await?;
    delete_friend_requests_tx(&mut tx, &demandeur, user_id).await?;
    let ami = friend_row_tx(&mut tx, user_id, &demandeur).await?;
    tx.commit().await?;
    Ok(Some(ami))
}

/// Refuse une demande recue : elle disparait, rien d'autre ne change.
pub async fn decline_friend_request(
    pool: &PgPool,
    request_id: &str,
    user_id: &str,
) -> Result<bool, sqlx::Error> {
    let resultat = sqlx::query("DELETE FROM friend_requests WHERE id = $1 AND to_user_id = $2")
        .bind(request_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(resultat.rows_affected() > 0)
}

/// Annule une demande envoyee : elle disparait, rien d'autre ne change.
pub async fn cancel_friend_request(
    pool: &PgPool,
    request_id: &str,
    user_id: &str,
) -> Result<bool, sqlx::Error> {
    let resultat = sqlx::query("DELETE FROM friend_requests WHERE id = $1 AND from_user_id = $2")
        .bind(request_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(resultat.rows_affected() > 0)
}

/// Amitie dans les deux sens, dans une transaction deja ouverte.
async fn add_friendship_tx(
    tx: &mut sqlx::PgConnection,
    a: &str,
    b: &str,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO friendships (user_id, friend_id, created_at_ms)
         VALUES ($1, $2, $3), ($2, $1, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(a)
    .bind(b)
    .bind(now_ms)
    .execute(tx)
    .await?;
    Ok(())
}

/// Efface les demandes en attente entre deux comptes, dans les deux sens.
async fn delete_friend_requests_tx(
    tx: &mut sqlx::PgConnection,
    a: &str,
    b: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM friend_requests
          WHERE (from_user_id = $1 AND to_user_id = $2)
             OR (from_user_id = $2 AND to_user_id = $1)",
    )
    .bind(a)
    .bind(b)
    .execute(tx)
    .await?;
    Ok(())
}

/// Fiche d'un ami vue par user_id, dans une transaction deja ouverte.
async fn friend_row_tx(
    tx: &mut sqlx::PgConnection,
    user_id: &str,
    friend_id: &str,
) -> Result<FriendRow, sqlx::Error> {
    sqlx::query_as::<_, FriendRow>(
        "SELECT u.id, u.name, u.email, u.picture_url, u.share_live,
                f.created_at_ms AS since_ms
           FROM friendships f
           JOIN users u ON u.id = f.friend_id
          WHERE f.user_id = $1 AND f.friend_id = $2",
    )
    .bind(user_id)
    .bind(friend_id)
    .fetch_one(tx)
    .await
}

/// Revendique un appareil pour le partage en direct.
pub async fn claim_live_device(
    pool: &PgPool,
    user_id: &str,
    device: &str,
    label: Option<&str>,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let resultat = sqlx::query(
        "INSERT INTO live_devices (device, user_id, label, first_seen_ms, last_seen_ms)
         VALUES ($1, $2, $3, $4, $4)
         ON CONFLICT (device) DO UPDATE
            SET last_seen_ms = EXCLUDED.last_seen_ms,
                label = COALESCE(EXCLUDED.label, live_devices.label)
          WHERE live_devices.user_id = EXCLUDED.user_id",
    )
    .bind(device)
    .bind(user_id)
    .bind(label)
    .bind(now_ms)
    .execute(pool)
    .await?;
    Ok(resultat.rows_affected() > 0)
}

/// Oublie les appareils muets depuis plus de [SESSION_TTL].
pub async fn prune_live_devices(pool: &PgPool, before_ms: i64) -> Result<u64, sqlx::Error> {
    let resultat = sqlx::query("DELETE FROM live_devices WHERE last_seen_ms < $1")
        .bind(before_ms)
        .execute(pool)
        .await?;
    Ok(resultat.rows_affected())
}

/// Appareils revendiques par une liste de comptes (amis d'un cercle).
pub async fn list_live_devices(
    pool: &PgPool,
    user_ids: &[String],
) -> Result<Vec<LiveDeviceRow>, sqlx::Error> {
    if user_ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, LiveDeviceRow>(
        "SELECT device, user_id, label, last_seen_ms
           FROM live_devices
          WHERE user_id = ANY($1)
          ORDER BY last_seen_ms DESC",
    )
    .bind(user_ids)
    .fetch_all(pool)
    .await
}
