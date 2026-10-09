-- Demandes d'amitie : on ne s'ajoute plus tout seul, on demande, et l'autre valide.
-- Idempotent : rejoue a chaque demarrage (aucun outil de migration a installer).
--
-- L'amitie n'existe plus des qu'un code est saisi : elle nait quand la personne
-- visee accepte la demande. Un compte se designe par son identite M-pacer
-- (adresse du compte), jamais par un identifiant de montre ou d'appareil.

CREATE TABLE IF NOT EXISTS friend_requests (
    id            TEXT PRIMARY KEY,
    from_user_id  TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    to_user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message       TEXT,
    created_at_ms BIGINT NOT NULL,
    -- Une seule demande en attente par couple et par sens : renvoyer la meme
    -- demande ne cree pas de doublon, elle est simplement deja en attente.
    UNIQUE (from_user_id, to_user_id),
    CHECK (from_user_id <> to_user_id)
);
CREATE INDEX IF NOT EXISTS idx_friend_requests_to ON friend_requests(to_user_id, created_at_ms DESC);
CREATE INDEX IF NOT EXISTS idx_friend_requests_from ON friend_requests(from_user_id, created_at_ms DESC);
