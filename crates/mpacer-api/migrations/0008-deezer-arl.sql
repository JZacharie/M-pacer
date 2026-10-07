-- Deezer par cookie `arl` : lecture des playlists et liens de telechargement.
-- Idempotent : rejoue a chaque demarrage, comme les migrations precedentes.
--
-- Le cookie `arl` lui-meme n'est jamais stocke en base : il vit dans
-- l'environnement du service (MPACER_DEEZER_ARL). Ce qui est conserve ici, c'est
-- l'identifiant Deezer d'une piste, qui permet de construire son lien Deemix
-- ({deemix}/#/track/{id}) dans la liste des MP3 a telecharger.

ALTER TABLE music_tracks ADD COLUMN IF NOT EXISTS deezer_track_id TEXT;
