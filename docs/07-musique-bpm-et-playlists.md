# 07 - Musique, BPM et transfert USB

> **Statut** : contrat d'interface **gele v2** (6 octobre 2026).
> La v1 (pilotage d'une application de streaming tierce et telechargement depuis
> le backend) est **abandonnee** : M-pacer ne pilote plus aucune application de
> lecture, et l'audio est **copie sur la montre par USB**, depuis un dossier
> choisi sur l'ordinateur.
> Toute modification d'interface doit etre reportee ici **avant** le code.

---

## 1. Principe retenu

1. **Deezer est la source de metadonnees**, pas un lecteur : on y recupere des
   playlists de course (titres, artistes, durees) ; le BPM vient des balises, du
   tap-tempo ou de la saisie (Deezer n'expose aucun tempo) ;
2. **les fichiers audio restent sur le disque de l'utilisateur** : l'application
   locale `mpacer-music` demande *ou* sont les MP3, apparie les fichiers aux pistes
   de la playlist, puis les **copie sur la montre par USB** (`adb push`) ;
3. **la montre joue en local**, hors ligne, sans telephone ni serveur, et le
   **BPM** sert de consigne de rythme pendant la course ;
4. le backend ne stocke **aucun** fichier audio : il publie les fiches de playlist
   et le **manifeste de transfert**.

### Ce qui est abandonne par rapport a la v1

| Point | v1 (abandonnee) | v2 (retenue) |
|---|---|---|
| Lecture par une application tierce | un lecteur externe etait pilote par `MediaController` | **supprime** (avec `MediaSessionAccessService.kt` et l'acces aux notifications) |
| Origine des fichiers | telecharges depuis le backend en Wi-Fi | **dossier local du PC**, choisi par l'utilisateur |
| Transport | HTTP (`/api/v1/music/tracks/{id}/file`) | **USB / adb push** par `mpacer-music` |
| Stockage serveur | `media.rs`, upload multipart, `MPACER_MEDIA_DIR` | **supprimes** |
| Plan de telechargement | `GET/POST /api/v1/music/prepare` | **supprime** ; le manifeste est exporte et consomme par l'outil local |
| Role du service de streaming | source **et** lecteur | source de **metadonnees** uniquement |

Rappel utile : Deezer n'expose **aucun tempo**. Le BPM vient donc, dans
l'ordre : **balises du fichier** (lues par l'outil local), puis **tap-tempo** ou
**saisie manuelle** dans la page `/music`.

## 2. Architecture

```text
  Navigateur                  PC de l'utilisateur                      Montre Wear OS
  +-----------+    +--------------------------------------+    +----------------------+
  | /music    |    |  mpacer-music (application locale)   |    |  MusicLibrary.kt     |
  | Deezer    |    |   - page locale 127.0.0.1:8077       |    |   scan du dossier    |
  | playlists |    |   - choix du dossier des MP3         |    |   Music/ (USB)       |
  | BPM, tap  |    |   - appariement fichiers <-> pistes  |    |  MusicPlayer.kt      |
  | manifeste |--->|   - lecture des balises (BPM)        |    |   Media3 ExoPlayer   |
  +-----+-----+    |   - adb push + manifest.json         |    |   hors ligne         |
        |          +------------------+-------------------+    +----------+-----------+
        | cookie                      | adb push (USB)                    |
  +-----v------------------+            +-----------------------------------+
  | mpacer-api             |                mpacer-core (FFI JSON)
  |  playlists, BPM,       |                music.rs : tempo cible,
  |  manifeste de transfert|                directeur d'orchestre,
  |  Deezer (arl ou OAuth) |                appariement, balises BPM
  +------------------------+
```

## 3. Coeur Rust - `mpacer-core::music`

Les sections 4.1 a 4.3 de la v1 sont **inchangees** : types `Track`, `Playlist`,
`MusicConfig`, `NowPlaying`, `MusicState`, `MusicDirective`, `DirectiveReason`,
`MusicInput`, `MusicDirector` ; loi de calibration
`target = clamp(reference_bpm * (reference_pace / pace)^elasticity, min, max)`
(defauts 170 BPM a 5:00/km, elasticite 0.35, bornes 100..200) ; cadence cible
bornee 140..=210 ; tap-tempo et lecture des balises
(`bpm_from_tags`: ID3v2 `TBPM`, Vorbis `BPM=`, MP4 `tmpo`).

### 3.1 Appariement fichiers <-> pistes (NOUVEAU, v2)

```rust
/// Fichier audio trouve sur le disque de l'utilisateur.
pub struct LocalFile {
    pub path: String,            // chemin complet
    pub file_name: String,
    pub size_bytes: u64,
    pub duration_s: Option<f64>,
    pub bpm: Option<f64>,        // lu dans les balises, sinon None
}

/// Piste attendue par un manifeste de transfert.
pub struct WantedTrack {
    pub id: String,
    /// Position **1-based** dans la playlist (01, 02, 03...) : c'est le prefixe
    /// du nom de fichier ecrit sur la montre (`01 - Artiste - Titre.mp3`).
    /// La base numerote les pistes depuis 0 ; le backend convertit
    /// (`models::manifest_position`).
    pub position: u32,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_s: Option<f64>,
    pub bpm: Option<f64>,
    pub file: Option<String>,      // rempli a l'ecriture sur la montre
    pub size_bytes: Option<u64>,   // idem
}

/// Manifeste de transfert (produit par le backend, ecrit sur la montre).
pub struct TransferManifest {
    pub version: u32,             // 1
    pub playlist_id: String,
    pub name: String,
    pub source: String,           // "deezer" | "manual"
    pub target_bpm: Option<f64>,
    pub tracks: Vec<WantedTrack>,
}

pub struct FileMatch {
    pub track_id: String,
    pub file: Option<LocalFile>,
    pub score: f64,               // 0.0 ..= 1.0
}

/// Normalise un libelle : minuscules, sans accents, ponctuation reduite a des
/// espaces, suffixes (feat. ..., remaster, official video, lyrics) retires,
/// numero de piste en tete retire.
pub fn normalize_label(text: &str) -> String;

/// Apparie des pistes a des fichiers. Deterministe : les couples
/// (piste, fichier) sont tries par score decroissant puis par position de piste ;
/// un fichier ne sert qu'a une piste ; seuil de validation 0.6.
pub fn match_tracks(wanted: &[WantedTrack], files: &[LocalFile]) -> Vec<FileMatch>;

/// Lit un manifeste JSON (format ci-dessus). None si illisible.
pub fn parse_manifest(json: &str) -> Option<TransferManifest>;

/// Nom de fichier propose pour la montre : "01 - Artiste - Titre.mp3"
/// (extension conservee a l'ecriture reelle).
pub fn suggested_file_name(track: &WantedTrack, extension: &str) -> String;
```

Regles d'appariement (testables, aucune E/S) :

* score de base **0.60** si le nom de fichier normalise contient le titre normalise ;
* **+0.25** si le nom contient aussi l'artiste normalise ;
* **+0.15** si la duree du fichier est connue et a moins de 3 s de celle de la piste ;
* un nom sans titre mais avec artiste et **duree a moins de 2 s** -> 0.55 (sous le seuil :
  on demande a l'utilisateur de renommer plutot que de copier le mauvais morceau) ;
* toute piste sous 0.60 reste **non appariee** (`file: None`) et est signalee ;
* l'extension reconnue est celle des fichiers audio du dossier (`.mp3`, `.m4a`,
  `.ogg`, `.opus`, `.flac`, `.wav`) ; un dossier vide ou un manifeste vide ne
  panique jamais.

## 4. Contrat FFI (inchange)

Les commandes de la v1 restent valables et sont **le seul** contrat entre la
montre et le coeur : `set_music`, `set_music_playlist`, `music_now_playing`,
`on_cadence`, plus le bloc `music` de `EngineOutput` (directive `Play`/`Keep`/
`Boost`/`Relax`/`SkipTo`/`Pause`/`Resume`, `target_bpm`, `target_cadence_spm`,
`cadence_spm`). La montre ne fait plus aucun appel reseau pour la musique.

## 5. Backend `mpacer-api` : metadonnees seulement

### 5.1 Base de donnees

La migration `0003_music.sql` est conservee telle quelle (idempotente, deja
appliquee). Les colonnes `storage_path`, `size_bytes` et `mime` de
`music_tracks` deviennent **inutilisees** (aucun audio stocke) : elles restent en
place pour ne pas casser un schema deja deploye. `music_download_plans` n'est
**plus lue ni ecrite** (conservee en base, sans code).

### 5.2 Configuration

| Variable | Role |
|---|---|
| `MPACER_DEEZER_ARL` | cookie `arl` du compte Deezer (voie recommandee) |
| `MPACER_DEEZER_APP_ID` / `_SECRET` | OAuth 2.0 Deezer (facultatif) |
| `MPACER_DEEZER_REDIRECT_URI` | defaut `{public_url}/auth/deezer/callback` |

`MPACER_MEDIA_DIR` disparait (plus de stockage audio).

### 5.3 API appareil (jeton `Bearer`)

| Methode | Chemin | Reponse |
|---|---|---|
| GET | `/api/v1/music/playlists` | `{"playlists":[{"id","name","source","target_bpm","track_count","duration_s","updated_at_ms"}]}` |
| GET | `/api/v1/music/playlists/{id}` | `{"id","name","source","target_bpm","manifest_url","tracks":[{"id","position","title","artist","album","duration_s","bpm","bpm_source"}]}` |
| GET | `/api/v1/music/playlists/{id}/manifest` | **manifeste de transfert** (JSON, schema de la section 3.1), en piece jointe `Content-Disposition: attachment` ; c'est ce fichier que `mpacer-music` consomme |

Les routes `tracks/{id}/file`, `playlists/{id}/ack`, `music/prepare`,
`music/prepare/ack` et `POST /api/v1/music/playlists` (multipart) sont
**supprimees**, ainsi que `src/media.rs`.

### 5.4 Interface web

| Methode | Chemin | Role |
|---|---|---|
| GET | `/music` | page complete (section 6.1) |
| GET | `/auth/deezer` + `/auth/deezer/callback` | OAuth 2.0 Deezer (repli) |
| POST | `/music/deezer/disconnect` | deconnecte le compte |
| GET | `/music/search?q=rock` | recherche de playlists Deezer |
| POST | `/music/import` | importe une playlist (`ref`, `target_bpm` optionnel) |
| POST | `/music/playlists/{id}/track-bpm` | BPM d'un titre (`bpm`, `source` = manual\|tap, `taps` optionnel) |
| POST | `/music/playlists/{id}/target` | BPM cible (vide = automatique) |
| POST | `/music/playlists/{id}/delete` | supprime la playlist (metadonnees) |
| GET | `/music/playlists/{id}/manifest` | **telecharge le manifeste** (session navigateur) |

Les routes `POST /music/upload`, `POST /music/prepare` et
`POST /music/prepare/cancel` sont **supprimees**.

### 5.5 Deezer

`src/deezer.rs` : OAuth 2.0 (repli) **et** API privee par cookie `arl`
(`arl_session`, `list_user_playlists_arl`, `search_playlists_arl`,
`get_playlist_arl`), plus `playlist_id_from_ref` et la detection des erreurs
applicatives renvoyees en HTTP 200. L'API Deezer n'expose aucun tempo : le BPM se
complete par les balises, le tap ou la saisie. Aucun appel reseau dans les tests
(les appels reels vivent dans `tests/deezer_live.rs`, ignores par defaut).

## 6. Interface

### 6.1 Page web `/music`

```text
+------------------------------------------------------------------------------+
| M-pacer   Seances  Courses  Planning  Statistiques  [ Musique ]  Appairer    |
+------------------------------------------------------------------------------+
| 1. Source Deezer (facultatif)                                                |
|  [ Connecter Deezer ]   recherche : [ rock              ]  [ Chercher ]      |
|   - Rock Workout (100 titres)                [ Importer ]                    |
| 2. Playlists preparees                                                       |
|  Run 170   deezer  18 titres  1:02:14  BPM cible [170]   [Manifeste] [x]     |
| 3. Titres (playlist selectionnee)                                            |
|  1  Wake me up   Avicii   4:09   124 bpm (balise)   [tapper][saisir]         |
| 4. Transfert vers la montre (USB)                                            |
|  1. [ Telecharger le manifeste ]  run-170.json                               |
|  2. Sur l'ordinateur :  mpacer-music transfer --manifest run-170.json \      |
|        --folder "D:\Musique\Course"                                          |
|  3. Brancher la montre en USB, puis lancer la commande.                      |
|  4. Sur la montre : Musique > [ Importer (USB) ].                            |
+------------------------------------------------------------------------------+
```

La page **ne televerse aucun fichier audio** et ne propose plus « Envoyer sur la
montre » : elle produit un manifeste JSON que l'outil local consomme.

### 6.2 Application locale `mpacer-music` (nouveau crate, interface web locale)

```text
+--- http://127.0.0.1:8077 ----------------------------------------------------+
| 1. Playlist      [ Choisir le manifeste... ] run-170.json                    |
|                  Run 170 - 18 titres - BPM cible 170                         |
| 2. Dossier des MP3   D:\Musique\Course            [ Parcourir ] [Analyser]   |
| 3. Montre         (o) Pixel Watch (adb)   libre 5,1 Go / 7,6 Go              |
| 4. Appariement    14/18 titres trouves   4 manquants   3 fichiers ignores    |
|    | # | Titre            | Fichier                       | BPM | Duree |      |
|    | 1 | Wake me up       | 01 - Avicii - Wake me up.mp3  | 124 | 4:09  |      |
|    | 5 | Levels           | -- manquant --                |     |       |      |
| 5. [ Transferer sur la montre ]   [ Simulation ]   [ Copier le rapport ]      |
+------------------------------------------------------------------------------+
```

CLI equivalente (meme code, meme resultat) :

```bash
mpacer-music                                  # interface locale, ouvre le navigateur
mpacer-music --port 9000 --no-browser
mpacer-music devices                          # montre(s) detectee(s) par adb
mpacer-music inspect  --manifest run-170.json --folder "D:\Musique\Course"
mpacer-music transfer --manifest run-170.json --folder "D:\Musique\Course" \
                      [--serial XXX] [--dry-run] [--prune] [--strict]
```

Codes de sortie : `0` succes ; `2` usage ; `3` adb introuvable ; `4` aucune
montre ; `5` espace insuffisant sur la montre ; `6` titres manquants avec
`--strict`.

Endpoints de l'interface locale :

| Methode | Chemin | Role |
|---|---|---|
| GET | `/` | page (section ci-dessus) |
| GET | `/api/browse?path=C:\...` | `{"path","parent","dirs":[{"name","path"}],"audio_count"}` (sans `path`, part des dossiers personnels) |
| GET | `/api/devices` | `{"adb":"chemin"\|"absent","devices":[{"serial","model","state","free_bytes","total_bytes"}]}` |
| POST | `/api/inspect` | corps `{"manifest_path"\|"manifest_json","folder"}` -> `{"playlist":{...},"matches":[{"track_id","position","title","artist","file","score","bpm","size_bytes","duration_s"}],"missing":[...],"unused_files":[...],"total_bytes":N}` |
| POST | `/api/transfer` | corps `{"manifest_path"\|"manifest_json","folder","serial","prune","dry_run"}` -> `{"job_id":"..."}` |
| GET | `/api/transfer/{job_id}` | `{"state":"running"\|"done"\|"failed"\|"cancelled","step","current","total","bytes_sent","error","logs":[...]}` |
| POST | `/api/transfer/{job_id}/cancel` | annule le transfert en cours |

L'interface et la CLI **appellent le meme planificateur** : l'appariement, le tri
par taille, le controle d'espace et la copie n'existent qu'une fois.

### 6.3 Ce que l'outil ecrit sur la montre

Cible adb : `context.getExternalFilesDir("Music")`, soit
`/sdcard/Android/data/com.mpacer.watch/files/Music/` (aucune permission
speciale, accessible par adb sur Android 11+).

```text
/sdcard/Android/data/com.mpacer.watch/files/Music/
  run-170/                       <- un dossier par playlist (id du manifeste)
    01 - Avicii - Wake me up.mp3
    ...
    manifest.json                <- manifeste + fichiers reellement copies
```

`manifest.json` = manifeste de la section 3.1, chaque piste portant en plus
`"file": "01 - Avicii - Wake me up.mp3"` et `"size_bytes": 4523112` (absents si
le titre n'a pas ete trouve). L'outil ecrit d'abord dans un dossier temporaire
puis pousse ; `--prune` supprime les fichiers du dossier montre qui ne sont plus
dans le manifeste.

### 6.4 Ecran Musique de la montre

```text
   +---------------------------+        +---------------------------+
   | 1. Bibliotheque (USB)     |        | 2. Lecture                |
   |  Run 170     18 p. 86 Mo  |  --->  |      Wake me up           |
   |  Ma course 10km 12 p.     |        |      Avicii  172 BPM      |
   |  [ Importer (USB) ]       |        |  cible 176 BPM   cadence  |
   |  libre 5,1 Go             |        |  [<<]  [ Pause ]  [>>]    |
   +---------------------------+        +---------------------------+
```

* aucune application de lecture tierce sur la montre (pas de session externe,
  pas d'acces aux notifications) ;
* « Importer (USB) » relit `getExternalFilesDir("Music")` : chaque sous-dossier
  contenant un `manifest.json` devient une playlist locale ; l'index est stocke
  dans `filesDir/music-index.json` ;
* la montre affiche le nombre de titres et l'espace utilise, et permet de
  supprimer une playlist importee ;
* l'ecran principal conserve la pastille BPM cible / Boost / Relax.

### 6.5 Application compagnon

L'onglet Musique devient **informatif** : liste des playlists du backend et
rappel que le transfert se fait par USB depuis l'ordinateur (`mpacer-music`).
Aucun televersement, aucun envoi de fichiers par le telephone.

## 7. Plan de verification

| Chantier | Preuve attendue |
|---|---|
| Coeur | `cargo test -p mpacer-core` : tests d'appariement (titre+artiste, duree, seuil 0.6, fichier unique, determinisme, accents/majuscules, `feat.`) |
| Outil | `cargo test -p mpacer-music` + `mpacer-music inspect` sur un dossier de test ; interface locale interrogee (browse/inspect) sans adb |
| Backend | `cargo test -p mpacer-api --test api` avec PostgreSQL : fiches, BPM, manifeste exporte, Deezer facultatif |
| Montre | `gradlew :app:assembleDebug :companion:assembleDebug` |
| Bout en bout | `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, relecture du diff par le Lead |

## 8. Hors perimetre

* lecture de l'audio Deezer par M-pacer (DRM) : **hors sujet en v2**, la montre
  ne joue que des fichiers presents sur son disque ;
* analyse audio pour deviner le BPM d'un MP3 sans balise (tap-tempo et saisie
  manuelle suffisent) ;
* synchronisation automatique a la connexion USB (l'utilisateur lance le
  transfert ; une surveillance `adb` continue est une suite possible) ;
* capteur de pas -> consigne de tempo (v2 du coeur).
## 9. Etat d'implementation (6 octobre 2026, v2)

| Chantier | Livre | Preuve |
|---|---|---|
| Coeur | appariement `LocalFile`/`WantedTrack`/`match_tracks`/`parse_manifest`/`normalize_label` dans `crates/mpacer-core/src/music.rs` | `cargo test --workspace` : coeur 164 tests ; clippy propre |
| Outil | nouveau crate `crates/mpacer-music` (CLI + interface locale 127.0.0.1:8077, planificateur unique, adb push, `--dry-run`, `--target-dir`) | `inspect` sur une fixture (3/4 appariees, scores 0.85, BPM lu dans les balises), `transfer --target-dir` verifie par le Lead : arborescence + `manifest.json` avec `file`/`size_bytes` |
| Backend | audio supprime (upload, stockage, service de fichiers, plans) ; `GET .../playlists/{id}/manifest` + `manifest_url` ; page `/music` en 4 blocs USB | `cargo test -p mpacer-api --test api` : **27 verts, 1 ignore, 0 echec** contre PostgreSQL ; page verifiee en service (`4. Transfert vers la montre (USB)`, plus aucun formulaire de televersement) |
| Montre | lecteur tiers et `MediaSessionAccessService.kt` supprimes ; `MusicLibrary` = scanner de `getExternalFilesDir("Music")` ; « Importer (USB) » ; lecture locale seule | `gradlew :app:assembleDebug :companion:assembleDebug` : BUILD SUCCESSFUL, trois `libmpacer_ffi.so` regeneres |
| Deploiement | plus de volume applicatif ni de `MPACER_MEDIA_DIR` ; cles Deezer dans le secret | `helm lint` + `helm template` (valeurs par defaut et jo3) |

Ecarts au contrat, tous documentes :

* `POST /api/v1/transfer` (interface locale) accepte un champ optionnel `target_dir`
  et la commande `serve` accepte `--target-dir` / `--adb` / `--port` / `--no-browser` :
  ajouts additifs, utilises par les tests et le mode simulation sans montre ;
* `GET /api/inspect` renvoie une entree par piste dans `matches` (`file: null` si non
  appariee) et la liste `missing` a part, pour suivre la maquette 6.2 ;
* la lecture des balises BPM par l'outil se limite aux 256 premiers Ko de chaque
  fichier (suffisant pour un tag ID3v2/Vorbis en tete) ;
* le transfert USB reel n'a pas pu etre exerce dans cette session (aucune montre
  branchee) : `adb push`, `df` et `--prune` sont couverts par les primitives, les
  tests et le mode `--target-dir`, pas par un branchement physique.
## 10. v3 - gestion de la musique, validation de la duree, reglages et identite visuelle

> Ajout au contrat v2 (tout le reste est inchange). Trois chantiers en parallele :
> coeur (calcul de couverture), backend/interface (page Musique, onglet Reglages),
> design (logo, icones, habillage des nouveaux blocs).

### 10.1 Coeur : couverture musicale d'une course

```rust
/// Marge de securite par defaut appliquee a la duree de course (5 %).
pub const MUSIC_MARGIN_RATIO: f64 = 0.05;

pub struct MusicCoverage {
    pub playlist_duration_s: f64,
    pub race_duration_s: Option<f64>,
    pub margin_s: Option<f64>,          // playlist - course (negatif si insuffisant)
    pub sufficient: Option<bool>,
    pub tracks_needed: Option<u32>,     // titres necessaires pour couvrir la course
    pub average_bpm: Option<f64>,
    pub target_bpm: Option<f64>,
    pub bpm_delta: Option<f64>,         // moyenne - cible
    pub tempo_ok: Option<bool>,
}

/// Somme des durees valides (finies et > 0) d'une playlist.
pub fn playlist_duration_s(tracks: &[Track]) -> f64;

/// Duree de course : le temps cible s'il est fourni (> 0), sinon distance x allure.
pub fn race_duration_s(
    distance_m: Option<f64>,
    target_time_s: Option<f64>,
    target_pace_s_per_km: Option<f64>,
) -> Option<f64>;

/// Validation : la playlist couvre-t-elle la course, et a quel tempo ?
pub fn music_coverage(
    tracks: &[Track],
    race_duration_s: Option<f64>,
    target_pace_s_per_km: Option<f64>,
    cfg: &MusicConfig,
    margin_ratio: f64,
) -> MusicCoverage;
```

Regles (testables) :

* `sufficient = playlist_duration_s >= race_duration_s * (1 + margin_ratio)` ;
  `None` si la duree de course est inconnue (le verdict n'est jamais invente) ;
* `tracks_needed = ceil(course * (1 + margin) / duree_moyenne_des_titres)`, `None` si
  la playlist est vide ou de duree moyenne nulle ;
* `average_bpm` = moyenne des BPM renseignes (`None` si aucun) ; `target_bpm` =
  `target_bpm_for_pace(pace)` si l'allure est connue ; `tempo_ok` = ecart inferieur ou
  egal a `MusicConfig::switch_threshold_bpm` (`None` si l'un des deux manque) ;
* une duree non finie, nulle ou negative est ignoree ; aucune division par zero, aucun
  panic sur une playlist vide.

### 10.2 Page `/music` : gestion et validation

La page passe a **cinq blocs** :

```text
+------------------------------------------------------------------------------+
| 1. Source Deezer (facultatif)                                                |
| 2. Playlists preparees   [ jouer ] [ renommer ] [ x ]                        |
| 3. Titres de la playlist selectionnee  (BPM : [tapper] [saisir])             |
| 4. Transfert vers la montre (USB)   [ Telecharger le manifeste ]             |
| 5. Assez de musique pour la course ?                                         |
|    Course [ 10 km de Bordeaux v ]  ou  distance [10] km  allure [5:00] /km   |
|    -> 1 h 12 min de musique pour 1 h 05 de course : OK, +7 min de marge      |
|       BPM moyen 148 vs cible 170 : tempo trop lent (il manque 22 bpm)        |
|       [ Valider ]                                                            |
+------------------------------------------------------------------------------+
```

Regles d'interface :

* le bloc 5 est un formulaire **GET** `?playlist=<id>&race=<id>&distance_km=&allure=&temps=`,
  pre-rempli depuis la fiche de course selectionnee (`distance_m`, `goal_time_s`) ;
  l'allure est acceptee en `m:ss` (par km) ou en secondes ; le temps en `h:mm:ss` ;
* la reponse affiche : duree de la playlist, duree estimee de la course, marge
  signee, verdict (`.coverage-ok` / `.coverage-warn` / `.coverage-bad`), nombre de
  titres manquants, BPM moyen et BPM cible ;
* la gestion des playlists s'ajoute : **renommer** (`POST /music/playlists/{id}/rename`,
  champ `name`) et **supprimer** (existant) ; le total des durees est affiche dans la
  liste ;
* classes CSS a utiliser exactement (fournies par le chantier design) : `.coverage`,
  `.coverage-verdict`, `.coverage-ok`, `.coverage-warn`, `.coverage-bad`,
  `.coverage-gauge`, `.coverage-gauge-fill`, `.coverage-metrics`, `.bpm-gauge`,
  `.bpm-gauge-marker`, `.illustration-usb`, `.logo`.

### 10.3 Menu : un onglet Reglages

* `NAV` gagne une entree `("reglages", "Reglages", "icon-settings", "/settings")` ;
* le menu deroulant `settings_menu` (capture d'ecran : « Appairer | Jetons |
  Deconnexion ») est **supprime** de l'en-tete ;
* `/settings` reunit desormais, dans cet ordre : profil, **appareils appaires**
  (jetons : liste + revocation), **appairer une montre** (code d'appairage, lien
  `/link`, rappel de la procedure), et **deconnexion** ;
* la route `/link` reste servie (la montre y renvoie l'utilisateur) mais ne figure
  plus dans la navigation ;
* la barre d'onglets mobile suit `NAV`, donc Reglages y apparait aussi.

### 10.4 Identite visuelle

Livrables du chantier design (fichiers) :

| Fichier | Contenu |
|---|---|
| `static/logo.svg` | logo complet : marque (montre + trace d'allure dans un carre arrondi orange #fc4c02) + mot-cle "M-pacer" |
| `static/logo-mark.svg` | la marque seule (carre arrondi), reutilisee en favicon et en petit format |
| `static/illustration-usb.svg` | schema `playlist -> cable USB -> montre` du bloc 4 |
| `static/app.css` | icones `icon-usb`, `icon-playlist`, `icon-tempo`, `icon-headphones`, `icon-check-circle` ; habillage `.coverage*`, `.bpm-gauge*`, `.logo`, `.illustration-usb` |

Regles : SVG sans dependance externe, `currentColor` pour les icones, contraste
verifie en theme clair et sombre (le site a les deux), aucune image bitmap.
`assets.rs` expose les nouveaux fichiers (`include_str!`) et `web.rs` les sert sur
`/static/logo.svg`, `/static/logo-mark.svg`, `/static/illustration-usb.svg`.
L'en-tete affiche `static/logo.svg` (hauteur ~28 px, lien vers `/`) et le favicon
du `layout` pointe sur `/static/logo-mark.svg`.
### 10.5 Etat d'implementation (v3, 6 octobre 2026)

| Chantier | Livre | Preuve |
|---|---|---|
| Coeur | `MUSIC_MARGIN_RATIO`, `MusicCoverage`, `playlist_duration_s`, `race_duration_s`, `music_coverage` | `cargo test -p mpacer-core` : **182 tests** (18 nouveaux) ; clippy et fmt propres |
| Page `/music` | bloc 5 « Assez de musique pour la course ? » (verdict + jauge + BPM), renommage, totaux de duree, illustrations | verifie en service : playlist de 3 titres (10:30) pour 10 km a 5:00/km -> « insuffisant, il manque 39 min », 15 titres necessaires, « BPM moyen 125 vs cible 170 » |
| Menu | `NAV` a 7 onglets avec **Reglages** ; menu deroulant supprime ; `/settings` = profil + appareils appaires + appairage + deconnexion | rendu verifie : Seances, Tableaux, Courses, Planning, Statistiques, Musique, Reglages |
| Identite visuelle | `static/logo.svg`, `logo-mark.svg`, `illustration-usb.svg`, 5 icones et habillage `.coverage*`/`.bpm-gauge*` en clair et sombre | 200 HTTP sur les 4 fichiers (tailles identiques), contraste WCAG >= 4.4, 28 SVG bien formes |
| Qualite | — | `cargo test --workspace` vert, `cargo clippy --workspace --all-targets -- -D warnings` propre, `cargo fmt --all --check` propre, `cargo test -p mpacer-api --test api` : 29 verts / 1 ignore |

## 11. v4 - sources multiples et synchro des MP3

> Le contrat v4 (7 octobre 2026) est decrit dans
> [11 - Playlists multi-sources et synchro des MP3](11-playlists-multi-sources-et-synchro-mp3.md) :
> **Deezer** comme deuxieme source de metadonnees, bloc **« Fichiers a preparer
> (MP3) »** (nom de fichier attendu par piste) et **push de MP3 dans
> l'application locale** avec statut de synchronisation par fichier. La page
> `/music` passe a **six blocs** (le transfert USB devient le bloc 5, la
> couverture le bloc 6). Le present document reste la reference du coeur, de
> l'appariement, du manifeste et de l'ecran de la montre.
