-- Optimisations PostgreSQL d'après le skill postgresql-table-design :
-- 1. Indexation systématique des clés étrangères (FK) pour accélérer les JOIN et les DELETE CASCADE
-- 2. Index d'expression insensible à la casse pour les recherches d'utilisateurs
-- 3. Index partiels pour les requêtes filtrées fréquentes

-- Clés étrangères de courses et tâches
CREATE INDEX IF NOT EXISTS idx_race_tasks_user_id ON race_tasks(user_id);
CREATE INDEX IF NOT EXISTS idx_race_tracks_user_id ON race_tracks(user_id);

-- Clés étrangères de musique
CREATE INDEX IF NOT EXISTS idx_music_tracks_user_id ON music_tracks(user_id);
CREATE INDEX IF NOT EXISTS idx_music_download_plans_playlist_id ON music_download_plans(playlist_id);
CREATE INDEX IF NOT EXISTS idx_music_download_plans_race_id ON music_download_plans(race_id) WHERE race_id IS NOT NULL;

-- Clés étrangères des tableaux de bord
CREATE INDEX IF NOT EXISTS idx_dashboard_widgets_user_id ON dashboard_widgets(user_id);

-- Clés étrangères et cibles d'amitiés
CREATE INDEX IF NOT EXISTS idx_friendships_friend_id ON friendships(friend_id);
CREATE INDEX IF NOT EXISTS idx_friend_invites_used_by ON friend_invites(used_by) WHERE used_by IS NOT NULL;

-- Recherche d'utilisateurs insensible à la casse (email & username)
CREATE INDEX IF NOT EXISTS idx_users_lower_email ON users(LOWER(email));
CREATE INDEX IF NOT EXISTS idx_users_lower_name ON users(LOWER(name)) WHERE name IS NOT NULL;
