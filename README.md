# M-pacer

**Contrôle d'allure pour montre Android (Wear OS), écrit en Rust**, avec backend,
interface web et déploiement Kubernetes sur le cluster k3s **jo3**.

Inspiré fonctionnellement de [Pace Control](https://pacecontrol.pbksoft.com/en/)
(PBkSoft) : même cœur de métier — allure fiable, shadow runner, retour vocal —
mais auto-hébergé, sans publicité, sans compte tiers, et avec une vraie
synchronisation de vos séances.

> **Non affilié** à Pace Control ; aucun code ni aucune ressource de l'application
> d'origine n'est utilisé. Les fonctionnalités sont réimplémentées à partir de la
> documentation publique.

**Documentation illustrée** — la montre, le site web, l'architecture et le démarrage :
<https://jzacharie.github.io/M-pacer/>. Source dans [`site/`](site/) (Markdown + une mise en
page commune), publiée par [`.github/workflows/pages.yml`](.github/workflows/pages.yml) dès que
GitHub Pages est activé sur le dépôt (Settings → Pages → Source : GitHub Actions).

---

## 1. Ce que fait le projet

| Composant | Rôle | État |
|---|---|---|
| **Montre Wear OS** | Enregistre la séance (GPS 1 Hz, **fréquence cardiaque**), calcule l'allure, guide le coureur (voix, shadow runner), archive localement et synchronise | **Compilée** (APK montre et téléphone produits le 6 octobre 2026) ; validation terrain à faire |
| **Application téléphone** (`android/companion`) | Connexion au backend, liste et détail des séances, import d'un fichier de séance, envoi vers la montre | **Compilée** ; à valider sur appareils réels |
| **Cœur Rust** | Tous les algorithmes : allure lissée, tours, assistant, voix, GPX, historique, **analyse de séance**, **musique et tempo** | **Fait, testé** (141 tests) |
| **Backend Rust** | API de synchronisation, OAuth Google, interface web, PostgreSQL | **Fait, testé** (tests d'intégration exécutés contre PostgreSQL) |
| **Analyse de séance** | Plan de course contre réalisé, fréquence cardiaque et zones, temps de pause, temps d'accélération, allure ajustée à la pente | **Fait, testé** |
| **Tableaux de bord** | Écrans composés par l'utilisateur : neuf widgets (allure, résumé, carte GPS, tours, meilleures distances, cardio, historique, statistiques, courses) choisis, ordonnés et enregistrés, avec trois gabarits prêts à l'emploi | **Fait, testé** |
| **Courses à venir** | Fiches de course, planning des échéances, suivi des éléments à préparer, **import d'une ancienne course** (export Strava ou Garmin) comme course de référence | **Fait, testé** |
| **Musique et BPM** | Playlists Spotify (métadonnées), appariement des MP3 du disque et **copie sur la montre par USB**, tempo cible déduit de l'allure, Boost/Relax selon le plan, écran Musique sur la montre | **Fait, testé** (cœur, outil, API, tests d'intégration) ; compilation montre et téléphone validée |
| **PostgreSQL** | Stockage des séances, géré par CloudNativePG dans le cluster | **Déployé sur jo3** (PostgreSQL 18.6) |
| **Chart Helm** | Déploiement complet (app + base + ingress + TLS) | **Validé** (`helm lint` + `--dry-run=server` sur jo3) |

**L'analyse d'une séance.** Chaque séance synchronisée s'ouvre sur un écran
d'analyse complet : résumé (temps en mouvement, temps écoulé, temps de pause,
fréquence cardiaque), **graphique multi-courbes** allure / cardio / altitude,
**plan de course contre réalisé** avec l'écart cumulé kilomètre par kilomètre,
**zones de fréquence cardiaque** et dérive cardiaque, **temps de passage**
enrichis, et **chronologie** des pauses et des phases d'accélération. Chaque
séance accepte aussi un **commentaire de course** — sensations, météo, parcours,
matériel — conservé à part du résumé envoyé par la montre, donc jamais effacé par
une nouvelle synchronisation. Détail, formules et veille concurrente :
[docs/06](docs/06-analyse-seance.md).

**Les tableaux de bord.** L'interface web ne se limite pas à des pages figées :
chaque utilisateur compose ses propres écrans à partir d'un catalogue de neuf
widgets — allure, résumé, carte GPS, tours, meilleures distances, cardio,
historique, statistiques, courses — les ordonne, les renomme et les conserve.
Trois gabarits créent en un clic les écrans de référence : *Pace Control*,
*Analyse de séance* et *Historique*. Tout passe par des formulaires et des
redirections : le constructeur fonctionne sans JavaScript. Détail :
[docs/08](docs/08-tableaux-de-bord.md).

**Les courses à venir.** L'interface web ne sert pas qu'à relire le passé : elle
gère aussi ce que vous préparez. Chaque course a sa **fiche** — numéro de dossard,
horaire et lieu de départ, lien du live, hôtel réservé (nom, adresse, téléphone,
arrivée, départ), rendez-vous de prise de dossard, autres solutions pour dormir,
nutrition et ravitaillement, informations importantes, autres informations — un
**planning** qui met bout à bout toutes les échéances à venir, et un **suivi**
d'éléments à cocher (« dossard retiré », « hôtel réservé »…). Le passé s'y ajoute
aussi : un **export Strava ou Garmin** (GPX ou TCX) s'importe d'un fichier et
devient une **ancienne course de référence** — date, distance, temps en
mouvement, temps écoulé, dénivelé et trace réexportable. Détail complet :
[docs/05-courses-et-planning.md](docs/05-courses-et-planning.md).

**La musique.** La montre lit la musique pendant la course, et le tempo (BPM)
devient un outil de pacing : le moteur déduit un **BPM cible** de l'allure visée,
choisit le morceau qui colle à ce tempo, **accélère la musique** quand le coureur
est en retard sur son plan et la calme quand il est en avance ou que le cardio
s'emballe. Le parcours est simple : vous récupérez les **playlists Spotify** dans
l'interface web (`/music`) — titres, artistes, BPM, corrigeables au tap-tempo —
puis vous **téléchargez le manifeste**, et l'application locale
[`mpacer-music`](crates/mpacer-music/) apparie les MP3 de **votre** dossier à ce
manifeste et les **copie sur la montre par USB** (`adb push`). La montre joue
ensuite **en local, hors ligne**, sans téléphone ni serveur ; M-pacer ne pilote
pas Spotify et ne stocke aucun fichier audio. Détail et contrat d'interface :
[docs/07](docs/07-musique-bpm-et-playlists.md).

**La montre.** Elle enregistre la séance sans le téléphone (GPS 1 Hz dans un service de
premier plan), affiche l'allure lissée sur 2 minutes, le feu de statut GPS et un panneau
d'assistant à quatre modes — allure seule, temps de finish estimé, shadow runner, course à
distance — annonce les informations à la voix, conserve chaque séance dans son historique
local et l'envoie au backend à la fin, automatiquement si elle est appairée. Détail :
[`android/README.md`](android/README.md).

## 2. Architecture

```text
┌── Montre Wear OS ────────────────────────┐
│ Kotlin + Wear Compose                    │
│  GPS 1 Hz · TTS · écran rond · service   │
│  de premier plan (location|health)       │
└──────────────┬───────────────────────────┘
               │ JNI / C ABI (JSON)
┌──────────────▼───────────────────────────┐
│ mpacer-ffi  →  mpacer-core (Rust pur)    │
│  allure · tours · assistant · voix       │
└──────────────┬───────────────────────────┘
               │ HTTPS + jeton d'appareil
┌──────────────▼───────────────────────────┐        ┌──────────────────────┐
│ mpacer-api (Axum)                        │        │ CloudNativePG        │
│  /api/v1/*  synchronisation              │◄──────►│ Cluster mpacer-pg    │
│  /          interface web (maud)         │  SQL   │ PostgreSQL 18.6      │
│  /auth/google  OAuth 2.0 + PKCE          │        │ (namespace mpacer)   │
└──────────────┬───────────────────────────┘        └──────────────────────┘
               │ Ingress traefik (entrypoint websecure) + cert-manager
        Navigateur / téléphone
```

Un troisième module Android, l'application téléphone **compagnon**, partage le même cœur et
le même backend : elle liste les séances, en importe une et peut la renvoyer vers la montre
par le Data Layer Wear OS (message sous 90 Ko, sinon `DataClient` + `Asset`).

**Trois principes structurants**

1. **La montre est la source de vérité.** Elle enregistre et conserve ses séances ;
   la synchronisation est un confort, jamais un prérequis pour courir. Un envoi peut
   être rejoué sans créer de doublon.
2. **Le calcul vit dans le cœur Rust**, testable sans montre, sans GPS et sans Android ;
   le code Kotlin ne fait que piloter la plateforme (capteurs, service, TTS, écran).
3. **Aucun secret Google sur la montre** : appairage par code, jeton opaque révocable.

## 3. Carte du dépôt

### 3.1 Cœur métier — `crates/mpacer-core/src/`

| Fichier | Contenu |
|---|---|
| `lib.rs` | Racine du crate, modules et ré-exports, version |
| `units.rs` | Unités métrique/impérial, formatage allure/durée/distance |
| `geo.rs` | Distance Haversine, détection de saut GPS |
| `gps.rs` | Qualité du signal (feu rouge/orange/jaune/vert), filtre anti-aberration |
| `pace.rs` | **Cœur du produit** : allure moyennée 2 min, détection de changement d'allure |
| `lap.rs` | Tours km/mile, allure du tour courant et du tour précédent (temps de course, pauses exclues) |
| `analysis.rs` | Analyse d'une séance : temps de passage, plan contre réalisé, pauses, accélération, allure ajustée à la pente |
| `cardio.rs` | Fréquence cardiaque : zones (% FC max et réserve de FC), bilan, dérive cardiaque |
| `workout.rs` | Machine à états de séance : démarrage suspendu, pause, auto-pause, reprise |
| `race_plan.rs` | Negative split et **shadow runner** (plan exact à l'arrivée) |
| `assistant.rs` | Les 4 modes : allure, temps estimé, plan de course, course à distance |
| `voice.rs` | Annonces vocales (planification + rédaction FR/EN), annonces musique (Boost/Relax/Tempo) |
| `music.rs` | Musique de course : **calibration tempo/allure**, directeur d'orchestre (Keep/Boost/Relax/SkipTo), tap-tempo, lecture des balises BPM (ID3v2, Vorbis, MP4) |
| `best_distances.rs` | Meilleurs 1/5/10 km et 1/5 mi dans une séance |
| `history.rs` | Format d'échange `.pac` version 2 (tours, cardio, pauses, plan, trace) |
| `gpx.rs` | Export GPX 1.1 |
| `remote_race.rs` | Protocole et classement de course à distance |
| `engine.rs` | **Orchestrateur** : une seule structure `EngineOutput` à afficher |

### 3.2 Backend et interface web — `crates/mpacer-api/`

| Fichier | Contenu |
|---|---|
| `src/main.rs` | Démarrage, journalisation, arrêt propre (SIGTERM) |
| `src/lib.rs` | Racine du crate, documentation d'ensemble |
| `src/config.rs` | Configuration par variables d'environnement, `DatabaseConfig` |
| `src/db.rs` | Accès PostgreSQL (requêtes paramétrées `$1`), schéma, statistiques |
| `src/models.rs` | Modèles persistés et validation des séances reçues |
| `src/error.rs` | Erreurs HTTP homogènes (`{"error": "code", "message": ...}`) |
| `src/state.rs` | État partagé (pool PostgreSQL, configuration, fournisseur OIDC) |
| `src/auth/mod.rs` | Sessions (JWT en cookie), extraction de l'utilisateur, hachage des jetons |
| `src/auth/google.rs` | OAuth 2.0 + PKCE, vérification de l'`id_token` via JWKS |
| `src/auth/device.rs` | Appairage montre ↔ navigateur (device authorization grant) |
| `src/avatar.rs` | Photo de profil Google : `GET /avatar` (proxy avec cache mémoire, hôtes `*.googleusercontent.com` uniquement) et pastille SVG d'initiales en repli |
| `src/routes/api.rs` | API `/api/v1/*` : appairage (`device/code`, `device/token`), `me`, ingestion et lecture des séances, export GPX, courses, statistiques, export `.pac`, **musique** (playlists, fiche, fichier audio avec `Range`, accusé, plan de préparation), version |
| `src/spotify.rs` | OAuth 2.0 + PKCE Spotify et API Web : playlists de l'utilisateur, recherche, import ; `audio-features` en *best effort* (endpoint restreint par Spotify depuis le 27/11/2024) |
| `src/bpm.rs` | BPM d'une piste : balises du fichier (délégué au cœur) et tap-tempo |
| `src/media.rs` | Réception `multipart` (navigateur et application téléphone), écriture sur disque, nettoyage en cas d'échec |
| `src/routes/web.rs` | Pages web (maud) : accueil, tableau de bord, **analyse de séance**, statistiques, appairage, jetons, **fiches de course, planning et suivi**, **page `/music`** (playlists, import Spotify, téléversement, tap-tempo, envoi vers la montre), connexion OAuth Google et Spotify, **pastille de compte** (photo Google) dans l'en-tête |
| `src/dashboards.rs` | **Tableaux de bord** : catalogue des widgets, gabarits (Pace Control, Analyse, Historique), chargement des données et rendu des écrans composés |
| `src/routes/mod.rs` | Routeur global, sondes `/healthz` et `/readyz` |
| `src/assets.rs` + `static/` | CSS et JS embarqués dans le binaire |
| `migrations/0001_init.sql` | Schéma initial : utilisateurs, jetons d'appareil, codes d'appairage, séances, états OAuth (idempotent, rejoué au démarrage) |
| `migrations/0002_races.sql` | Courses à venir et suivi de préparation (idempotent aussi) |
| `migrations/0003_music.sql` | Musique : playlists, pistes (BPM, origine, fichier), plans de téléchargement vers la montre, comptes Spotify liés (idempotent aussi) |
| `tests/api.rs` | **24 tests d'intégration** (flux complet, musique, Spotify optionnel, avatar, schéma dédié par test) ; sans `MPACER_TEST_DATABASE_URL`, ils affichent un message et s'arrêtent. Un test supplémentaire, ignoré par défaut (accès réseau à `googleusercontent.com`), vérifie le proxy de la photo Google : `cargo test -p mpacer-api --test api -- --ignored` |

### 3.3 Les autres crates

| Dossier | Contenu |
|---|---|
| `crates/mpacer-client/` | Client de synchronisation (appairage, envoi) utilisé par le simulateur ; la montre embarque son propre client Kotlin (`SyncClient.kt`) |
| `crates/mpacer-ffi/` | Pont C ABI JSON exposé au shell Android (aucun panic ne traverse la frontière) |
| `crates/mpacer-sim/` | Simulateur : rejoue une course synthétique, écrit un GPX, synchronise vers le backend, option `--music` |
| `crates/mpacer-music/` | **Application locale** de transfert : choisit le dossier des MP3, apparie les fichiers au manifeste, copie sur la montre par USB (`adb push`). CLI + interface web locale (127.0.0.1:8077) |

### 3.4 Déploiement

| Fichier | Contenu |
|---|---|
| `charts/mpacer/Chart.yaml` | Métadonnées du chart |
| `charts/mpacer/values.yaml` | Valeurs par défaut documentées |
| `charts/mpacer/values-jo3.yaml` | **Valeurs du cluster jo3** (traefik, regcred, CNPG, amd64) |
| `charts/mpacer/templates/postgresql.yaml` | `Cluster` CloudNativePG + sauvegardes planifiées (option) |
| `charts/mpacer/templates/deployment.yaml` | Application (uid 10001, rootfs read-only, sondes, env DB) |
| `charts/mpacer/templates/secret.yaml` | Secret de session généré et conservé entre upgrades + identifiants Google (et Spotify si activé) |
| `charts/mpacer/templates/configmap.yaml` | Configuration non sensible, dont `MPACER_MEDIA_DIR` et l'URI de redirection Spotify |
| `charts/mpacer/templates/ingress.yaml` | Ingress traefik (`websecure`) |
| `charts/mpacer/templates/ingress-cloudflare.yaml` | Ingress public via Cloudflare Tunnel (option) |
| `charts/mpacer/templates/certificate.yaml` | Certificat TLS cert-manager (DNS-01 Cloudflare) |
| `charts/mpacer/templates/pvc.yaml` | Volume applicatif : **fichiers audio téléversés** (`/data/media`) ; sans PVC, un `emptyDir` est monté à la place |
| `charts/mpacer/templates/service.yaml` | Service ClusterIP |
| `charts/mpacer/templates/serviceaccount.yaml` | Compte de service sans jeton monté |
| `charts/mpacer/templates/externalsecret.yaml` | ExternalSecret (option) : secret applicatif fourni par un `ClusterSecretStore` (Vault) au lieu d'être créé par le chart |
| `charts/mpacer/templates/NOTES.txt` | Instructions affichées après `helm install` (sondes, OAuth, appairage, simulateur) |
| `charts/mpacer/templates/_helpers.tpl` | Noms, labels, secret applicatif PostgreSQL |
| `deploy/Dockerfile` | Image multi-étapes (binaire seul, utilisateur non privilégié) |
| `deploy/build-image.ps1` | Construction/publication (podman, mono-arch ou multi-arch) |
| `deploy/README.md` | **Guide de déploiement** complet |
| `.github/workflows/ci.yml` | CI : format, clippy, tests, lint Helm, build d'image |

### 3.5 Application montre (à compiler sur une machine avec le SDK Android)

| Fichier | Contenu |
|---|---|
| `local-ci.ps1` | Diagnostic de l'environnement (JDK, SDK, NDK, cargo-ndk), compilation du cœur pour les trois ABI, assemblage Gradle, installation, veille de la montre de test |
| `android/app/build.gradle.kts` | Module Wear OS, cargo-ndk, ABIs, `DEFAULT_API_URL` |
| `android/app/src/main/AndroidManifest.xml` | Permissions, services de premier plan `location|health` et `mediaPlayback`, listener Data Layer |
| `android/.../MpacerCore.kt` | Pont JNI, commandes JSON, lecture de `EngineOutput` |
| `android/.../MainActivity.kt` | Navigation entre course, réglages et synchronisation, demande de permissions |
| `android/.../TrackingService.kt` | Service de premier plan, boucle GPS 1 Hz (3 s en pause), notification, archivage et envoi en fin de séance |
| `android/.../VoiceCoach.kt` | Synthèse vocale + focus audio (duck / pause / ignorer) |
| `android/.../music/MusicLibrary.kt` | Bibliothèque hors ligne : fiche et fichiers téléchargés depuis le backend, index local, progression |
| `android/.../music/MusicPlayer.kt` + `MusicPlaybackService.kt` | Lecture locale Media3/ExoPlayer dans un service de premier plan `mediaPlayback` |
| `android/.../music/SpotifyRemote.kt` | Télécommande de l'application Spotify de la montre (session média tierce) : aucun octet audio téléchargé |
| `android/.../music/MusicSession.kt` | Pont vers le cœur : réglages, playlist, piste en cours, cadence |
| `android/.../ui/MusicScreen.kt` | Écran rond Musique : bibliothèque, préparation avant course, lecture (titre, BPM, tempo cible) |
| `android/.../WorkoutArchive.kt` | Historique local des séances (stockage privé de l'application) |
| `android/.../SyncClient.kt` | Appairage par code, envoi des séances, jeton chiffré, identifiants déjà envoyés |
| `android/.../WearSyncListener.kt` | Réception des séances envoyées par le téléphone (Data Layer) |
| `android/.../MpacerFormat.kt` | Formatage local (allure, durée, distance) ; aucun calcul de course |
| `android/.../ui/MainScreen.kt` | Écran rond : allure, distance, temps, feu GPS, panneau assistant, pastille musique (BPM cible, Boost/Relax), commandes |
| `android/.../ui/SettingsScreen.kt` | Mode d'assistant, unités, voix, musique (activation, annonces, BPM de référence) |
| `android/.../ui/SyncScreen.kt` | Appairage, état de connexion, séances en attente |
| `android/.../ui/Theme.kt` | Palette et voyant de statut GPS |
| `android/app/src/main/cpp/mpacer_jni.c` | Shim JNI (une quarantaine de lignes) vers la C ABI Rust |
| `android/companion/...` | Application téléphone : `AppViewModel`, `MpacerApi`, `TokenStore`, `WearSync`, écrans Compose (connexion, liste, détail, envoi) |
| `android/README.md` | Prérequis, commandes de compilation, appairage et dépannage |

### 3.6 Documentation

| Fichier | Contenu |
|---|---|
| `docs/01-analyse-features.md` | Analyse des 37 fonctionnalités de Pace Control, priorisation, formules |
| `docs/02-architecture-rust-wearos.md` | Architecture Rust/Wear OS, pont FFI, algorithme d'allure, batterie, permissions |
| `docs/03-plan-action.md` | Plan de développement de l'application montre |
| `docs/04-backend-web-et-deploiement.md` | Backend, auth, API, modèle de données, exploitation |
| `docs/05-courses-et-planning.md` | Courses à venir : fiches, planning, suivi, API et modèle de données |
| `docs/05-plan-action-jo3.md` | Plan d'action GitOps et déploiement sur le cluster jo3 (lots, ordre d'exécution, points bloquants) |
| `docs/06-analyse-seance.md` | Analyse d'une séance : veille concurrente (Strava, Garmin, Polar…), écrans, formules des zones FC, du découplage et de l'accélération |
| `docs/07-musique-bpm-et-playlists.md` | Musique et BPM : playlists Spotify ou fichiers personnels, calibration tempo / allure, directeur d'orchestre, contrat FFI, API et écrans — **implémenté** |
| `docs/README.md` | Index des documents |
| [`site/`](site/) | **Documentation en ligne** (GitHub Pages) : présentation illustrée de la montre et du site web, architecture, démarrage |

## 4. Plan d'action complet

### Phase 0 — Cadrage et analyse ✅ *terminée*

| Tâche | Statut | Critère d'acceptation |
|---|---|---|
| Analyse fonctionnelle de Pace Control | ✅ | 37 fonctionnalités inventoriées et priorisées |
| Choix d'architecture (Rust + plateforme) | ✅ | Documenté et justifié (`docs/02`) |
| Plan de développement | ✅ | Phases, tâches, risques (`docs/03`) |

### Phase 1 — Cœur métier Rust ✅ *terminée*

| Tâche | Statut | Critère d'acceptation |
|---|---|---|
| Allure lissée 2 min + détection de changement | ✅ | 71 tests verts, exemple du manuel reproduit (5:52 → 5:30) |
| Tours, séance, auto-pause, démarrage suspendu | ✅ | Machine à états testée |
| Assistant (4 modes), shadow runner, negative split | ✅ | Plan exact à l'arrivée (erreur < 1e-6) |
| Voix FR/EN, meilleures distances, GPX, `.pac` | ✅ | Rendu et exports testés |
| Simulateur de séance | ✅ | Séance de 21 km rejouée, GPX produit |

### Phase 2 — Backend et interface web ✅ *terminée*

| Tâche | Statut | Critère d'acceptation |
|---|---|---|
| API de synchronisation (idempotente) | ✅ | Renvoi d'une séance = remplacement, pas de doublon |
| OAuth Google (code + PKCE + JWKS) | ✅ | Flux complet testé avec un fournisseur simulé |
| Appairage montre (code court) | ✅ | `BCDF-GHJK` → jeton d'appareil révocable |
| Interface web (tableau de bord, détail, GPX) | ✅ | Pages rendues et vérifiées de bout en bout |
| PostgreSQL + schéma versionné | ✅ | **Tests exécutés contre le PostgreSQL 18.6 de jo3** |

### Phase 3 — Déploiement sur jo3 🔶 *en cours*

| Tâche | Statut | Critère d'acceptation |
|---|---|---|
| Chart Helm (app, base, ingress, TLS) | ✅ | `helm lint` + `--dry-run=server` acceptés par l'API server |
| Cluster PostgreSQL sur jo3 | ✅ | `mpacer-pg` : pod 1/1, PVC lié, secret applicatif créé |
| Tests contre la base du cluster | ✅ | 12 tests verts via `kubectl port-forward` |
| Image conteneur | ✅ | Construite (102 Mo), conteneur démarré, sondes OK |
| **Publication de l'image sur ghcr.io** | ⏳ *à faire* | `pwsh deploy/build-image.ps1 -Push` (jeton GitHub requis) |
| **Client OAuth Google** | ⏳ *à faire* | Console Google Cloud, redirect `…/auth/google/callback` |
| **`helm upgrade --install`** | ⏳ *à faire* | Pod prêt, certificat émis, `/readyz` = ready |
| Activation de l'accès public (montre hors domicile) | ⏳ *option* | `cloudflareIngress.enabled=true` |

### Phase 4 — Application montre 🔶 *en cours*

| Tâche | Statut | Critère d'acceptation |
|---|---|---|
| Chaîne Android (JDK, SDK, NDK, cargo-ndk) | ✅ | `pwsh ./local-ci.ps1 -Target all` : APK montre (36,4 Mo) et téléphone (11,5 Mo) produits, aucune erreur Kotlin |
| Écran principal + service GPS | ✅ | Écran rond, service de premier plan `location|health`, GPS 1 Hz ; 10 km sans le téléphone à valider |
| Voix | ✅ | Annonces rédigées par le cœur, prononcées par le TTS, focus audio (duck / pause / ignorer) |
| Synchronisation depuis la montre | ✅ | Appairage par code, archive locale, envoi automatique en fin de séance, idempotent |
| Application téléphone (compagnon) | ✅ | Liste et détail des séances, import, envoi vers la montre par le Data Layer |
| Capteur de fréquence cardiaque | ✅ | la montre écoute `TYPE_HEART_RATE` et pousse chaque mesure au moteur (`heart_rate`) |
| Boutons du casque, mode ambiant | 🔲 | Séance guidée sans regarder l'écran : les commandes existent dans le cœur, aucun `MediaSession` ne les déclenche encore |
| Validation terrain | 🔲 | Écart < 3 % avec une montre de référence, batterie < 25 %/h |

### Phase 5 — Exploitation et durcissement 🔲 *à faire*

| Tâche | Statut | Critère d'acceptation |
|---|---|---|
| Sauvegardes planifiées (`ScheduledBackup`) | 🔲 | Restauration testée |
| Supervision (métriques, alertes) | 🔲 | Alerte sur l'état rapporté par `/readyz` (le corps JSON porte `database: true|false` ; le code HTTP reste 200) |
| Limitation de débit sur l'API | 🔲 | 429 au-delà du seuil |
| CI : publication d'image + déploiement | 🔲 | Pipeline vert de bout en bout |

### Phase 6 — Différenciation 🔲 *backlog*

Cardio (zones, alertes FC) · allure ajustée à la pente (GAP) · séances structurées
(échauffement / intervalles / retour au calme) · Health Connect · course à distance
avec backend temps réel.

## 5. Démarrage rapide

### 5.1 Cœur et simulateur (aucune montre, aucun serveur)

```bash
cargo test --workspace

cargo run -p mpacer-sim -- --mode plan --distance 10000 --time 3000 --split 0.03 --gpx trace.gpx

# Avec la musique : le moteur choisit le tempo, la simulation joue les pistes
cargo run -p mpacer-sim -- --mode plan --distance 10000 --time 3000 --music
```

```text
[VERT ] t=  5:00 dist=0.98 km allure= 5:03 tour= 5:07 | ecart plan -2 m
    Allure 5:03 par kilometers. Distance 0.98 km. Temps 5:00. Vous etes sur le plan.
  tour  1 : 1.00 km en 5:06
  tour  2 : 1.00 km en 5:00
  tour  3 : 1.00 km en 4:53
```

Le mode `--music` ajoute la consigne de tempo à chaque ligne et fait jouer les
pistes par la simulation, exactement comme la montre le fera :

```text
    >> lecture : Rythme 170 (170 bpm)
[VERT ] t=  2:00 dist=0.51 km allure= 3:51 | ecart plan +114 m | musique 164 bpm cad=203 stable (en avance sur le plan) - Rythme 170
    >> lecture : Tempo 176 (176 bpm)
[VERT ] t=  4:20 dist=1.14 km allure= 5:30 | ecart plan +278 m | musique 176 bpm cad=164 stable (allure trop lente) - Tempo 176
```

### 5.2 Backend en local

```bash
# Un PostgreSQL local (conteneur) puis :
export MPACER_DATABASE_URL="postgresql://mpacer:mpacer@127.0.0.1:5432/mpacer?sslmode=disable"
export MPACER_DEV_AUTH=1 MPACER_PUBLIC_URL=http://localhost:8080
cargo run -p mpacer-api
# http://localhost:8080 → « Continuer avec Google » ; avec MPACER_DEV_AUTH=1,
# POST /auth/dev-login ouvre une session de test (aucun bouton sur /login)
```

### 5.3 Synchroniser une séance depuis le simulateur

```bash
cargo run -p mpacer-sim -- --mode plan --distance 5000 --time 1500 \
  --api-url http://localhost:8080
```

Le simulateur affiche un code d'appairage (`BCDF-GHJK`) : saisissez-le sur
`http://localhost:8080/link` (ou sur le backend déployé) et la séance part.

### 5.4 Tests du backend contre un PostgreSQL

```bash
MPACER_TEST_DATABASE_URL="postgresql://mpacer:motdepasse@127.0.0.1:5432/mpacer?sslmode=disable" \
  cargo test -p mpacer-api
```

Sans cette variable, les tests d'intégration sont ignorés (message explicite).

## 6. Déploiement sur jo3

```bash
# 1. Image
pwsh deploy/build-image.ps1 -Push

# 2. Déploiement (identifiants Google : client OAuth de type « Application Web »)
helm upgrade --install mpacer charts/mpacer -n mpacer --create-namespace \
  -f charts/mpacer/values-jo3.yaml \
  --set auth.googleClientId=... --set auth.googleClientSecret=...

# 3. Vérifications
kubectl -n mpacer get cluster,pods,ingress,certificate
curl -s https://mpacer.p.zacharie.org/readyz
```

Détail complet, dépannage et sauvegardes : [deploy/README.md](deploy/README.md).

## 7. Exploitation

| Sujet | Commande |
|---|---|
| État de la base | `kubectl -n mpacer get cluster mpacer-pg` (⚠ préfixer par `clusters.postgresql.cnpg.io` si KubeBlocks est installé) |
| Se connecter à la base | `kubectl -n mpacer exec -it mpacer-pg-1 -- psql -U postgres -d mpacer` |
| Sauvegarde logique | `kubectl -n mpacer exec mpacer-pg-1 -- pg_dump -U postgres -Fc mpacer > mpacer.dump` |
| Restauration | `cat mpacer.dump | kubectl -n mpacer exec -i mpacer-pg-1 -- pg_restore -U postgres -d mpacer --clean` |
| Mise à jour applicative | `helm upgrade … --set image.tag=nouveau` (sans coupure : `RollingUpdate`) |
| Retour arrière | `helm rollback mpacer -n mpacer` |
| Journal de l'application | `kubectl -n mpacer logs deploy/mpacer -f` |

## 8. Qualité et tests

| Vérification | Résultat |
|---|---|
| `cargo test --workspace` | **204 tests** : 141 cœur, 8 FFI, 54 backend (dont 24 d'intégration exécutés contre PostgreSQL, 1 ignoré faute de réseau) et 1 test de documentation |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 avertissement |
| `cargo fmt --all --check` | conforme sur l'état commité ; l'arbre de travail en cours peut présenter des écarts |
| `helm lint` / `helm template` | 0 échec |
| `helm install --dry-run=server` sur jo3 | accepté (CRD cert-manager et CNPG validées) |
| Image conteneur | construite, conteneur démarré, `/healthz` et `/readyz` OK |
| Bout en bout | appairage → envoi d'une séance par le simulateur → visible dans l'API, le tableau de bord et la page détail |

## 9. Décisions et limites assumées

- **PostgreSQL géré par CloudNativePG** (présent sur jo3) plutôt qu'un StatefulSet
  maison : sauvegardes, réplication et mises à jour de version sont gérées par
  l'opérateur. Une instance suffit pour un usage personnel ; passer à 3 pour du HA.
- **Interface web rendue côté serveur (maud)** plutôt qu'une SPA WebAssembly : une
  seule image, aucun jeton exposé au JavaScript. L'API reste consommable par une SPA
  si le besoin apparaît.
- **Un seul secret de session partagé** : l'application est sans état, donc
  `RollingUpdate` sans coupure et montée en répliques possible.
- **Les applications Android compilent** (JDK 17, SDK 35, NDK r27, cargo-ndk, via
  `local-ci.ps1`), mais n'ont pas encore d'usage terrain documenté : l'écart avec une
  montre de référence et la consommation réelle restent à mesurer.
- **TLS vers PostgreSQL désactivé** par défaut (réseau interne du cluster) ;
  `sslmode` est configurable pour un serveur externe.
- Les versions de Wear OS, permissions de santé et règles Play Store évoluent :
  à revalider sur la version ciblée avant publication.
- **La « carte » d'une course est un lien**, pas une carte interactive : elle ouvre
  OpenStreetMap sur les coordonnées si elles sont renseignées, sinon sur le nom du
  lieu. Aucun script tiers, aucune donnée envoyée à un service de cartographie.
- **Les liens saisis dans une fiche de course sont vérifiés** : seuls `http://` et
  `https://` sont acceptés, pour qu'un lien stocké ne puisse pas devenir un script
  exécutable dans la page.

## 10. Points ouverts (revue de code du 6 octobre 2026)

Revue de l'arbre de travail, du plus grave au plus anodin. Chaque point donne le fichier
concerné ; aucun n'a été corrigé dans cette passe de documentation.

1. **Permission de localisation incomplète** — `android/.../MainActivity.kt` ne demande que
   `ACCESS_FINE_LOCATION`. Depuis Android 12, une demande de position précise non accompagnée
   de `ACCESS_COARSE_LOCATION` (pourtant déclarée au manifeste) est ignorée par le système :
   le service de suivi reçoit une `SecurityException` et arrête la séance.
2. **Réglages sans effet** — `android/.../ui/SettingsScreen.kt` modifie un état local que
   `MainActivity.kt` ne pousse jamais au moteur : `MpacerCore.setAssistant` et `setVoice`
   ne sont appelés nulle part, pas plus que `VoiceCoach.configure`. Mode d'assistant, unités
   et voix sont décoratifs en l'état.
3. **Champs perdus à l'envoi** — `android/.../SyncClient.kt` décode le résumé du cœur dans un
   modèle Kotlin partiel, puis le ré-encode : `elapsed_s`, `pauses`, `heart_rate` et `plan`
   n'arrivent pas au backend. L'analyse cardio et la chronologie des pauses sont donc vides
   pour les séances venues de la montre.
4. **`/readyz` répond toujours 200** — `crates/mpacer-api/src/routes/mod.rs` place l'état de la
   base dans le corps JSON mais ne renvoie jamais un code d'échec : une sonde ne peut pas
   alerter sur une base injoignable.
5. **Détail des erreurs exposé** — `crates/mpacer-api/src/error.rs` met `self.to_string()` dans
   le champ `message` de la réponse, donc le texte brut des erreurs SQL et internes.
6. **Pas de jeton anti-CSRF** — `crates/mpacer-api/src/routes/web.rs` : les formulaires POST
   (déconnexion, appairage, révocation, suppression, suivi) ne sont protégés que par le cookie
   `SameSite=Lax`.
7. **État du moteur** — `crates/mpacer-core/src/engine.rs` : `reset()` ne vide pas les tours
   (des tours périmés restent exposés jusqu'au prochain `start()`) ; `crates/mpacer-core/src/gps.rs` :
   un échantillon rejeté pour précision insuffisante ne fait pas retomber le voyant.
8. **Deux versions de `.pac`** — le cœur écrit la version 2
   (`crates/mpacer-core/src/history.rs`) alors que l'export de l'archive Android annonce encore
   `version: 1` (`android/.../WorkoutArchive.kt`).
9. **Formatage** — `cargo fmt --all --check` signale des écarts dans plusieurs fichiers de
   l'arbre de travail ; `cargo clippy --workspace --all-targets -- -D warnings` passe.

## 11. Prochaines actions

1. Publier l'image : `pwsh deploy/build-image.ps1 -Push` (jeton GitHub).
2. Créer le client OAuth Google (redirect `https://mpacer.p.zacharie.org/auth/google/callback`).
3. `helm upgrade --install` puis vérifier `/readyz` et le certificat.
4. Ouvrir `/link`, appairer la montre (ou le simulateur) et valider un envoi réel.
5. Activer la sauvegarde CNPG (`postgresql.backup.enabled=true`) et l'accès public
   si la montre doit synchroniser hors du domicile.

## 12. Licence

MIT ou Apache-2.0, au choix (voir `Cargo.toml`).
