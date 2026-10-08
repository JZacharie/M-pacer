-- Deezer reste la seule source externe : les restes de Spotify sont retires.
-- Idempotent : rejoue a chaque demarrage, comme les migrations precedentes.
--
-- Les playlists importees de Spotify gardent leurs titres (elles se comportent
-- comme une playlist saisie a la main, sans source vivante) ; leur identifiant
-- Spotify et le compte OAuth associe sont oublies, faute de service utilisable.
--
-- Les valeurs sont migrees avant les suppressions de colonnes, et la table des
-- comptes disparait en premier.

DROP TABLE IF EXISTS spotify_accounts;

UPDATE music_playlists SET source = 'manual' WHERE source = 'spotify';
UPDATE music_tracks SET bpm_source = 'manual' WHERE bpm_source = 'spotify';

ALTER TABLE music_playlists DROP COLUMN IF EXISTS spotify_id;
ALTER TABLE music_tracks DROP COLUMN IF EXISTS spotify_uri;
