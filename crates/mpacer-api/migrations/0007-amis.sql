-- Amis et partage de la position en direct.
-- Idempotent : rejoue a chaque demarrage (aucun outil de migration a installer).

-- Interrupteur de partage. Par defaut, un compte qui ajoute un ami partage sa
-- position avec lui : c'est l'objet de la fonctionnalite, et l'utilisateur peut
-- la couper a tout moment depuis la page Amis.
ALTER TABLE users ADD COLUMN IF NOT EXISTS share_live BOOLEAN NOT NULL DEFAULT TRUE;

-- Amities, stockees dans les deux sens. La lecture d'un cercle est alors un
-- simple SELECT, et retirer un ami efface les deux lignes d'un coup.
CREATE TABLE IF NOT EXISTS friendships (
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    friend_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at_ms BIGINT NOT NULL,
    PRIMARY KEY (user_id, friend_id)
);
CREATE INDEX IF NOT EXISTS idx_friendships_user ON friendships(user_id);

-- Invitations : un code court se transmet de la main a la main, comme le code
-- d'appairage de la montre. Usage unique, expiration courte.
CREATE TABLE IF NOT EXISTS friend_invites (
    code          TEXT PRIMARY KEY,
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at_ms BIGINT NOT NULL,
    expires_at_ms BIGINT NOT NULL,
    used_by       TEXT REFERENCES users(id) ON DELETE SET NULL,
    used_at_ms    BIGINT
);
CREATE INDEX IF NOT EXISTS idx_friend_invites_user ON friend_invites(user_id);
CREATE INDEX IF NOT EXISTS idx_friend_invites_live ON friend_invites(expires_at_ms) WHERE used_at_ms IS NULL;

-- Appareils qui publient en direct : le nom de montre du sujet MQTT
-- (<prefixe>/live/<montre>) est revendique par le compte qui lance la seance.
-- C'est ce qui permet de n'exposer une trace qu'aux amis de son proprietaire.
-- Le nom est unique : deux comptes ne peuvent pas revendiquer le meme, sinon
-- une position pourrait apparaitre dans le cercle de quelqu'un d'autre.
CREATE TABLE IF NOT EXISTS live_devices (
    device        TEXT PRIMARY KEY,
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    label         TEXT,
    first_seen_ms BIGINT NOT NULL,
    last_seen_ms  BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_live_devices_user ON live_devices(user_id);
