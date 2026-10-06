-- Tableaux de bord : des ecrans composes par l'utilisateur.
--
-- Un tableau appartient a un utilisateur. Chacun de ses widgets est une ligne
-- (et non un document JSON) : l'ordre d'affichage est explicite, le
-- reordonnancement et le retrait se font en SQL, et une cle de widget inconnue
-- reste visible en base au lieu d'etre silencieusement perdue.
-- Idempotent : rejoue a chaque demarrage, comme les schemas precedents.

CREATE TABLE IF NOT EXISTS dashboards (
    id            TEXT PRIMARY KEY,
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dashboards_user ON dashboards(user_id, created_at_ms);

CREATE TABLE IF NOT EXISTS dashboard_widgets (
    id            TEXT PRIMARY KEY,
    dashboard_id  TEXT NOT NULL REFERENCES dashboards(id) ON DELETE CASCADE,
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL,
    position      INTEGER NOT NULL,
    created_at_ms BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dashboard_widgets_dashboard
    ON dashboard_widgets(dashboard_id, position);
