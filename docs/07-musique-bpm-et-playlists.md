# 07 - Musique, BPM et playlists de course

> **Statut** : contrat d'interface **gele** (v1). Les trois chantiers
> (coeur Rust, backend, montre/compagnon) s'implementent contre ce document.
> Toute modification d'interface doit etre reportee ici **avant** le code.

---

## 1. Objectif

Ajouter a la montre la **lecture de musique** et faire du **tempo (BPM)** un
outil de pacing :

1. recuperer des **playlists de course** (Spotify, ou fichiers personnels) ;
2. les **preparer sur la montre avant une course** (telechargement hors ligne) ;
3. pendant la course, **utiliser le BPM** comme consigne de rythme : le moteur
   choisit le morceau dont le tempo colle a l'allure cible, accelere la musique
   quand le coureur est en retard sur son plan, la calme quand il est en avance
   ou que le cardio s'emballe.

## 2. Ce qui est possible, ce qui ne l'est pas (contrainte Spotify)

Le projet est auto-heberge et sans DRM contourne. Il faut donc etre precis :

| Source | Lecture par M-pacer | Ce que fait M-pacer |
|---|---|---|
| **Fichiers personnels** (MP3/OGG/M4A possedes par le coureur) | **Oui**, hors ligne, via Media3/ExoPlayer | telechargement sur la montre avant la course |
| **Playlist Spotify** | **Non** (audio chiffre Widevine, reserve a l'app Spotify) | M-pacer **telecommande l'application Spotify installee sur la montre** (MediaSession/MediaController) et decide **quel** morceau jouer et **quand** passer au suivant |
| **Metadonnees Spotify** (titres, durees, BPM) | Oui, via l'API Web Spotify | import de playlist, calcul du tempo cible |

Consequence assumee pour l'utilisateur : pour une playlist Spotify, la montre
n'a besoin de telecharger **que la fiche** (titres + BPM, quelques Ko), la
musique restant geree hors ligne par l'app Spotify (fonction « Telecharger » de
l'app, abonnement Premium requis). Pour la musique personnelle, M-pacer
telecharge les **fichiers audio** sur la montre.

**Limite de l'API Spotify** : le 27 novembre 2024 Spotify a restreint son API
Web ; l'endpoint `GET /v1/audio-features` (qui fournissait `tempo`, donc le BPM)
n'est **plus disponible pour les nouvelles applications**. Le BPM suit donc une
cascade de sources, dans cet ordre :

1. `spotify` : `audio-features` si le compte Spotify du serveur y a encore acces ;
2. `tag` : balise du fichier (`TBPM` ID3v2, `BPM` Vorbis/FLAC, `tmpo` MP4) ;
3. `tap` : tap-tempo depuis l'interface (l'utilisateur tape en rythme) ;
4. `manual` : saisie directe du BPM.

Sources : [Spotify, changements d'API Web (27/11/2024)](https://developer.spotify.com/blog/2024-11-27-changes-to-the-web-api),
[TechCrunch, 27/11/2024](https://techcrunch.com/2024/11/27/spotify-cuts-developer-access-to-several-of-its-recommendation-features/).

## 3. Architecture

```text
                 +------------------- navigateur -------------------+
                 |  Page /music : playlists, import Spotify, upload |
                 |  fichiers, BPM par titre, tap-tempo,             |
                 |  « Envoyer sur la montre »                       |
                 +------------------+-------------------------------+
                                    | cookie de session
+-- Spotify Web API --+   +---------v----------------------------------------+
| /v1/me/playlists    |<--| mpacer-api  /music  + /api/v1/music/*            |
| /v1/search          |   |  spotify.rs . bpm.rs (balises) . music_download  |
| /v1/audio-features  |   |  fichiers audio sur disque (MPACER_MEDIA_DIR)    |
+---------------------+   +---------+------------------+-------------------+
                                    | jeton appareil   | jeton appareil
                                    | (manifeste)      | (fichiers + Range)
                          +---------v------------------v-------------------+
                          |  Montre Wear OS                              |
                          |  MusicLibrary (stockage local)               |
                          |  MusicPlayer (Media3/ExoPlayer)              |
                          |  SpotifyRemote (MediaController)             |
                          |  MusicScreen (ecran rond)                    |
                          +---------+------------------------------------+
                                    | mpacer-core (FFI JSON)
                          +---------v------------------------------------+
                          | music.rs : tempo cible, directeur            |
                          | d'orchestre (Keep/Boost/Relax/...),          |
                          | tap-tempo, lecture des balises               |
                          +----------------------------------------------+
```

## 4. Coeur Rust - `mpacer-core::music`

Nouveau module `crates/mpacer-core/src/music.rs` (pur, sans E/S), re-exporte
par `lib.rs`.

### 4.1 Modele

```rust
/// Piste d'une playlist, telle que le backend la publie.
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub duration_s: f64,
    pub bpm: Option<f64>,          // None = tempo inconnu : piste « neutre »
    pub position: u32,
}

pub struct Playlist {
    pub id: String,
    pub name: String,
    pub target_bpm: Option<f64>,   // consigne fixe decidee dans l'interface
    pub tracks: Vec<Track>,
}

/// Origine d'une valeur de BPM (affichage et confiance).
pub enum BpmSource { Spotify, Tag, Tap, Manual, Estimated }

/// Reglages musique du moteur (miroir de l'ecran Reglages de la montre).
pub struct MusicConfig {
    pub enabled: bool,
    /// Reference de calibration : « BPM de reference a l'allure de reference ».
    pub reference_bpm: f64,             // defaut 170.0
    pub reference_pace_s_per_km: f64,   // defaut 300.0 (5:00/km)
    /// Elasticite du tempo par rapport a l'allure (0 = tempo fixe).
    pub pace_elasticity: f64,           // defaut 0.35
    pub min_bpm: f64,                   // defaut 100.0
    pub max_bpm: f64,                   // defaut 200.0
    /// Ecart de BPM qui declenche un changement de morceau.
    pub switch_threshold_bpm: f64,      // defaut 8.0
    /// Gain applique quand le coureur est en retard sur le plan.
    pub boost_bpm: f64,                 // defaut 6.0
    /// Perte appliquee quand il est en avance (ou cardio trop haut).
    pub relax_bpm: f64,                 // defaut 6.0
    /// Annoncer les changements de consigne a la voix.
    pub announce: bool,                 // defaut true
    /// Ne pas rejouer une des N dernieres pistes lors d'une selection.
    pub avoid_last: usize,              // defaut 3
}

/// Instantane de la piste en cours, pousse par la montre.
pub struct NowPlaying {
    pub track_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub bpm: Option<f64>,
    pub position_s: f64,
}

pub enum MusicDirective { None, Play, Keep, Boost, Relax, SkipTo, Pause, Resume }
pub enum DirectiveReason {
    Disabled, NoPlaylist, Steady, OnPlan, BehindPlan, AheadOfPlan,
    PaceSlow, PaceFast, HeartRateHigh, Paused, Resumed, TrackBpmMismatch,
}

/// Resultat publie dans EngineOutput.
pub struct MusicState {
    pub enabled: bool,
    pub playlist_id: Option<String>,
    pub playlist_name: Option<String>,
    pub target_bpm: Option<f64>,
    pub target_cadence_spm: Option<f64>,
    pub cadence_spm: Option<f64>,
    pub current: Option<NowPlaying>,
    pub next_track_id: Option<String>,
    pub directive: MusicDirective,
    pub reason: DirectiveReason,
}
```

### 4.2 Loi tempo / allure (calibration)

Fonctions pures, testees :

```rust
/// BPM cible pour une allure cible (s/km).
/// target = clamp(reference_bpm * (reference_pace / pace)^elasticite, min, max)
pub fn target_bpm_for_pace(pace_s_per_km: f64, cfg: &MusicConfig) -> f64;

/// Cadence de pas visee. La musique doit « tenir » les appuis :
/// cadence = 60 * vitesse / foulee, avec une foulee de reference de 1.15 m
/// qui s'allonge legerement avec la vitesse :
/// foulee = 1.15 * (v / v_ref)^0.4   (v_ref = 1000 / reference_pace_s_per_km)
pub fn target_cadence_spm(pace_s_per_km: f64, cfg: &MusicConfig) -> f64;

/// Cadence estimee a partir de la vitesse (repli si aucun capteur de pas).
pub fn cadence_from_speed(speed_mps: f64) -> f64; // clamp 140..210
```

Exemples (defauts) : 5:00/km -> 170 BPM ; 4:00/km -> ~183.8 BPM ;
6:00/km -> ~159.5 BPM. Ces valeurs sont couvertes par des tests.

### 4.3 Directeur d'orchestre

```rust
pub struct MusicDirector { /* config, playlist, historique, etat */ }

impl MusicDirector {
    pub fn new(config: MusicConfig) -> Self;
    pub fn config(&self) -> MusicConfig;
    pub fn set_config(&mut self, config: MusicConfig);
    pub fn set_playlist(&mut self, playlist: Option<Playlist>);
    pub fn playlist(&self) -> Option<&Playlist>;
    pub fn on_now_playing(&mut self, now: Option<NowPlaying>);
    pub fn on_cadence(&mut self, spm: f64);
    pub fn reset(&mut self);

    /// Consigne du tick courant.
    pub fn evaluate(&mut self, input: MusicInput) -> MusicState;
}

pub struct MusicInput {
    pub t_ms: i64,
    pub state: WorkoutState,             // Idle/Armed/Running/Paused/AutoPaused/Finished
    pub target_pace_s_per_km: Option<f64>,
    pub current_pace_s_per_km: Option<f64>,
    pub shadow_delta_m: Option<f64>,     // + en avance, - en retard
    pub shadow_on_plan: bool,
    pub heart_rate_zone: Option<u8>,
    pub speed_mps: Option<f64>,
}
```

Regles de decision (ordre de priorite) :

1. `enabled == false` ou aucune playlist -> `MusicDirective::None`, raison
   `Disabled` / `NoPlaylist`.
2. seance `Paused`/`AutoPaused` -> `Pause` (raison `Paused`) ; retour en
   `Running` apres une pause -> `Resume` (raison `Resumed`).
3. **Boost** : en retard sur le plan de plus de **20 m**, ou allure courante
   plus lente que l'allure cible de plus de **5 s/km** ->
   `desired_bpm = cible + boost_bpm`, raison `BehindPlan` / `PaceSlow`.
4. **Relax** : en avance de plus de **20 m**, ou cardio en zone 5, ou allure
   plus rapide que la cible de plus de **5 s/km** ->
   `desired_bpm = cible - relax_bpm`, raison `AheadOfPlan` / `HeartRateHigh`
   / `PaceFast`.
5. sinon `desired_bpm = cible`, raison `OnPlan` (ou `Steady` sans plan).
6. Comparaison a la piste en cours :
   * aucune piste -> `Play` + `next_track_id` = meilleure piste ;
   * `|bpm_piste - desired_bpm| <= switch_threshold_bpm` -> `Keep` ;
   * sinon `SkipTo` + `next_track_id` (raison `TrackBpmMismatch`).
   * Le choix evite les `avoid_last` dernieres pistes ; une piste de BPM
     inconnu est neutre et n'est jamais choisie pour un changement de tempo.

L'etat est **stable** : deux evaluations consecutives avec les memes entrees
donnent le meme resultat (pas de tir aleatoire, pas d'horloge interne cachee).
Les directives `Pause`/`Resume` ne sont emises qu'**une fois** par transition.

### 4.4 Tap-tempo et balises BPM

```rust
/// BPM estime a partir d'instants de tap (ms). Median des intervalles,
/// au moins 4 taps, sinon None. Intervalles aberrants ignores.
pub fn tap_tempo(taps_ms: &[i64]) -> Option<f64>;

/// Extrait un BPM d'une balise binaire de fichier audio, sans dependance :
/// ID3v2 (TBPM), commentaire Vorbis/FLAC (BPM=), atome MP4 (tmpo).
pub fn bpm_from_tags(bytes: &[u8], filename: &str) -> Option<f64>;
```

### 4.5 Integration au moteur

* `EngineConfig` gagne `pub music: MusicConfig` (`Default` fourni, donc les
  litteraux existants avec `..Default::default()` continuent de compiler).
* `EngineOutput` gagne `pub music: MusicState` (toujours present, jamais
  `null`) et serialise en `"music"`.
* Nouvelles methodes : `set_music_config`, `set_music_playlist`,
  `on_now_playing`, `on_cadence` (le tick calcule la cadence estimee si aucun
  capteur n'en fournit).
* `voice.rs` : nouvelles annonces, uniquement si `MusicConfig::announce` et sur
  **transition** de directive :
  * `VoiceCue::MusicBoost` -> FR « Musique : on accelere. », EN « Music: pick it up. »
  * `VoiceCue::MusicRelax` -> FR « Musique : on ralentit. », EN « Music: ease off. »
  * `VoiceCue::MusicTempo` -> FR « Rythme {bpm}. », EN « Tempo {bpm}. »
    (une seule fois au demarrage de la seance si la musique est active).

## 5. Contrat FFI (JNI JSON)

Commandes ajoutees a `mpacer-ffi::Command` (toutes renvoient un `EngineOutput`) :

```json
{"cmd":"set_music","config":{ /* MusicConfig, champs snake_case */ }}
{"cmd":"set_music_playlist","playlist":null}
{"cmd":"set_music_playlist","playlist":{"id":"...","name":"Run 170","target_bpm":170.0,
  "tracks":[{"id":"t1","title":"...","artist":"...","duration_s":215.0,"bpm":172.0,"position":0}]}}
{"cmd":"music_now_playing","now":null}
{"cmd":"music_now_playing","now":{"track_id":"t1","title":"...","artist":"...","bpm":172.0,"position_s":42.5}}
{"cmd":"on_cadence","t_ms":1700000000000,"spm":174.0}
```

`EngineOutput.music` (extrait) :

```json
"music": {
  "enabled": true,
  "playlist_id": "8f...", "playlist_name": "Run 170",
  "target_bpm": 176.0, "target_cadence_spm": 176.0, "cadence_spm": 174.0,
  "current": {"track_id":"t1","title":"...","artist":"...","bpm":172.0,"position_s":42.5},
  "next_track_id": null,
  "directive": "Keep",
  "reason": "OnPlan"
}
```

Les variantes d'enumeration sont serialisees en `PascalCase` (comme
`AssistantMode`/`WorkoutState`) : `Keep`, `Boost`, `Relax`, `SkipTo`, `Pause`,
`Resume`, `Play`, `None` ; raisons `OnPlan`, `BehindPlan`, `AheadOfPlan`, etc.

## 6. Backend `mpacer-api`

### 6.1 Migration `migrations/0003_music.sql` (idempotente)

```sql
music_playlists(id PK, user_id FK users, name, source, spotify_id NULL,
                cover_url NULL, target_bpm NULL, created_at_ms, updated_at_ms)
music_tracks(id PK, playlist_id FK music_playlists ON DELETE CASCADE, user_id,
             position INT, title, artist NULL, album NULL, duration_s NULL,
             bpm NULL, bpm_source NULL, spotify_uri NULL, mime NULL,
             size_bytes NULL, storage_path NULL, created_at_ms)
music_download_plans(id PK, user_id FK users, playlist_id FK music_playlists,
                     race_id NULL FK races ON DELETE SET NULL, target_bpm NULL,
                     requested_at_ms, acked_at_ms NULL)
spotify_accounts(user_id PK FK users, spotify_user_id NULL, display_name NULL,
                 access_token, refresh_token NULL, expires_at_ms, scope NULL,
                 connected_at_ms)
```

Index : `music_tracks(playlist_id, position)`,
`music_playlists(user_id, updated_at_ms DESC)`,
`music_download_plans(user_id, acked_at_ms)`.
`source` dans `spotify|upload|manual` ; `bpm_source` dans `spotify|tag|tap|manual`.

### 6.2 Configuration (env)

| Variable | Defaut | Role |
|---|---|---|
| `MPACER_MEDIA_DIR` | `./media` | racine des fichiers audio televerses |
| `MPACER_SPOTIFY_CLIENT_ID` | - | OAuth Spotify (absent = fonctionnalite desactivee, page explicite) |
| `MPACER_SPOTIFY_CLIENT_SECRET` | - | idem |
| `MPACER_SPOTIFY_REDIRECT_URI` | `{public_url}/auth/spotify/callback` | a declarer dans la console Spotify |

### 6.3 API appareil (jeton `Bearer`, memes gardes que `/api/v1/workouts`)

| Methode | Chemin | Reponse |
|---|---|---|
| GET | `/api/v1/music/playlists` | `{"playlists":[{"id","name","source","target_bpm","track_count","total_bytes","ready_track_count","updated_at_ms"}]}` |
| GET | `/api/v1/music/playlists/{id}` | `{"id","name","source","target_bpm","tracks":[{"id","position","title","artist","album","duration_s","bpm","bpm_source","size_bytes","mime","spotify_uri","download_url"}]}` - `download_url` vaut `null` si la piste n'a pas de fichier |
| GET | `/api/v1/music/tracks/{id}/file` | octets audio, `Content-Type`, `Accept-Ranges: bytes`, `206` si `Range` |
| POST | `/api/v1/music/playlists/{id}/ack` | corps `{"track_ids":["..."]}` -> `{"playlist_id","downloaded"}` |
| GET | `/api/v1/music/prepare` | `{"plan": null}` ou `{"plan":{"id","playlist_id","name","target_bpm","race_id","race_name","requested_at_ms","tracks":[...]}}` (dernier plan non acquitte) |
| POST | `/api/v1/music/prepare/ack` | corps `{"plan_id":"..."}` -> `{"ok":true}` |

### 6.4 Interface web (cookie de session)

| Methode | Chemin | Role |
|---|---|---|
| GET | `/music` | page complete (voir section 7) |
| GET | `/auth/spotify` | demarre OAuth 2.0 + PKCE Spotify |
| GET | `/auth/spotify/callback` | echange le code, enregistre `spotify_accounts` |
| POST | `/music/spotify/disconnect` | supprime le compte lie |
| GET | `/music/search?q=running` | recherche de playlists Spotify (resultats dans la page) |
| POST | `/music/import` | `spotify_ref` (URL ou id) + `target_bpm` optionnel -> playlist `source=spotify` |
| POST | `/music/upload` | `multipart/form-data` : `playlist_name`, `files[]` -> playlist `source=upload`, BPM par balise |
| POST | `/music/playlists/{id}/track-bpm` | `track_id`, `bpm` (saisie manuelle ou tap-tempo) |
| POST | `/music/playlists/{id}/target` | `target_bpm` (vide = automatique) |
| POST | `/music/playlists/{id}/delete` | supprime la playlist et ses fichiers |
| POST | `/music/prepare` | `playlist_id`, `race_id` optionnel -> cree le plan de telechargement |
| POST | `/music/prepare/cancel` | annule le plan en attente |

Erreurs : memes codes que l'existant (`AppError`), message FR affiche dans la page.

### 6.5 Spotify (`src/spotify.rs`)

* `authorize_url(state, code_challenge)`, `exchange_code`, `refresh_token` ;
* `list_user_playlists`, `get_playlist(id)`, `search_playlists(q)` ;
* `audio_features(ids)` -> `Ok(None)` si l'API repond 403/404 (endpoint
  restreint) : **jamais d'echec bloquant**, le BPM reste `None` ;
* scopes : `playlist-read-private playlist-read-collaborative` ;
* les jetons sont stockes dans `spotify_accounts` (la base est la frontiere de
  confiance, au meme titre que les sessions) - a documenter dans le deploiement.

### 6.6 BPM (`src/bpm.rs`)

* `bpm_from_tags(bytes, filename)` : delegue a `mpacer_core::music` ;
* aucune analyse audio lourde n'est faite cote serveur (documente comme suite
  possible) : balises, tap-tempo et saisie manuelle couvrent le besoin.

## 7. Interface proposee

### 7.1 Page web `/music` (nouvelle entree de navigation « Musique »)

```text
+------------------------------------------------------------------------------+
| M-pacer   Tableau de bord  Seances  Courses  Planning  [ Musique ]  Reglages |
+------------------------------------------------------------------------------+
| 1. Source de musique                                                         |
| +------------------------------+  +-----------------------------------------+ |
| | Spotify : Connecte (Zach)    |  | Fichiers personnels                     | |
| | [Deconnecter]                |  | [ Choisir des fichiers MP3/OGG/M4A ]    | |
| | Rechercher : [ running    ]  |  | Nom de playlist : [ Ma course 10 km   ] | |
| |  - Running 170 BPM (18 titres)| | [ Importer sur le serveur ]            | |
| |  - Motivation 5 km (12)      |  +-----------------------------------------+ |
| +------------------------------+                                              |
|                                                                              |
| 2. Playlists preparees                                                       |
| +--------------------------------------------------------------------------+ |
| | Run 170      spotify  18 titres   0 Mo  BPM cible [170]  [Preparer >]     | |
| | Ma course 10km  upload  12 titres  86 Mo  BPM cible [auto] [Preparer >]   | |
| +--------------------------------------------------------------------------+ |
|                                                                              |
| 3. Titres (playlist selectionnee)                                            |
| +----+-----------------------+----------+------+-------+-----------------+   |
| | #  | Titre                 | Artiste  | Duree| BPM   |                 |   |
| | 1  | Wake me up            | Avicii   | 4:09 | 124   | [tapper][saisir]|   |
| +----+-----------------------+----------+------+-------+-----------------+   |
|                                                                              |
| 4. Preparation de la prochaine course                                        |
| +--------------------------------------------------------------------------+ |
| | Course : [ 10 km de Bordeaux v ]   Playlist : [ Run 170 v ]               | |
| | BPM cible [auto]   Taille 86 Mo   Etat : en attente de la montre          | |
| | [ Envoyer sur la montre ]                                                 | |
| +--------------------------------------------------------------------------+ |
+------------------------------------------------------------------------------+
```

« Envoyer sur la montre » **ne pousse pas** les octets depuis le navigateur :
cela cree un *plan de telechargement* que la montre (ou l'application
compagnon) recupere au prochain reveil, puis telecharge en Wi-Fi. C'est le
principe « la montre est la source de verite » deja applique aux seances.

### 7.2 Montre - ecran Musique

```text
   +---------------------------+        +---------------------------+
   | 1. Bibliotheque           |        | 2. Lecture                |
   |  Run 170      [>] 18 p.   |  --->  |      Wake me up           |
   |  Ma course 10km  [v] 12 p.|        |      Avicii  172 BPM      |
   |  (v = telechargee)        |        |  cible 176 BPM   cadence  |
   |  [Telecharger]  [Reglages]|        |  [<<]  [ Pause ]  [>>]    |
   +---------------------------+        +---------------------------+
```

* l'ecran principal affiche une **pastille musique** (BPM cible + fleche
  Boost/Relax) sous le panneau d'assistant ;
* l'acces Musique est un bouton de l'ecran principal a l'arret, et une entree
  « Musique » des reglages ;
* avant la course : « Preparer » liste les playlists du backend, telecharge la
  fiche (Spotify) et les fichiers (upload), affiche la progression et l'espace
  utilise.

### 7.3 Application compagnon

Onglet « Musique » : liste des playlists preparees cote serveur, bouton
**« Envoyer sur la montre »** (met le plan en file via Data Layer et reveille la
montre), bouton « Televerser des fichiers » (memes endpoints web que le
navigateur, avec le jeton de session du telephone).

## 8. Plan de verification

| Chantier | Preuve attendue |
|---|---|
| Coeur | `cargo test -p mpacer-core` (nouveaux tests `music::*`), `cargo clippy --workspace --all-targets -- -D warnings` |
| FFI | `cargo test -p mpacer-ffi` (aller-retour des nouvelles commandes) |
| Backend | `cargo test -p mpacer-api` + `MPACER_TEST_DATABASE_URL=... cargo test -p mpacer-api --test api` (18 tests existants + nouveaux) |
| Montre / compagnon | `android/gradlew.bat :app:assembleDebug :companion:assembleDebug` |
| Bout en bout | `cargo test --workspace` et relecture du diff complet par le Lead |

## 9. Hors perimetre (v1)

* decodage/analyse audio serveur pour estimer le BPM d'un MP3 (balises, tap et
  saisie manuelle suffisent ; suite possible : analyse par autocorrelation) ;
* lecture de l'audio Spotify par M-pacer (impossible sans contourner le DRM) ;
* synchronisation du BPM avec la cadence mesuree par la montre (v2 : boucle
  fermee capteur de pas -> consigne de tempo).
