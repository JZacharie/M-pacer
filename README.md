# M-pacer

**Contrôle d'allure auto-hébergé pour montres de course** : un cœur Rust unique, une
application Wear OS, un portage Garmin et un backend avec son site web. Inspiré
fonctionnellement de [Pace Control](https://pacecontrol.pbksoft.com/en/) (PBkSoft) —
allure fiable, shadow runner, retour vocal — mais sans publicité, sans compte tiers et
avec vos séances synchronisées.

> **Non affilié** à Pace Control : aucun code ni aucune ressource de l'application
> d'origine n'est utilisé.

| | |
|---|---|
| Documentation illustrée | <https://jzacharie.github.io/M-pacer/> (source [`site/`](site/)) |
| Documentation technique | [`docs/`](docs/) — index : [`docs/README.md`](docs/README.md) |
| Guide de déploiement | [`deploy/README.md`](deploy/README.md) |

---

## 1. Ce que fait le projet

| Composant | Rôle | État |
|---|---|---|
| **Cœur Rust** — [`crates/mpacer-core/`](crates/mpacer-core/) | Allure lissée 2 min, tours, machine à états de séance, assistant (4 modes) et shadow runner, cardio, voix FR/EN, analyse de séance, musique et tempo, GPX, format `.pac` | **Fait, testé** (182 tests) |
| **Montre Wear OS** — [`android/app/`](android/app/) | GPS 1 Hz, fréquence cardiaque, écran rond, service de premier plan, voix, archive locale, synchronisation | **APK construit** ; validation terrain à faire |
| **Course au téléphone** — [`android/phone/`](android/phone/) | Courir avec le téléphone seul : GPS 1 Hz, ceinture cardiaque Bluetooth LE, voix, musique, suivi MQTT, **onglet Amis (position des proches sur carte OpenStreetMap)**, archive et synchronisation — sur le même socle [`android/core/`](android/core/) que la montre | **APK construit** ; validation terrain à faire |
| **Application téléphone d'appoint** — [`android/companion/`](android/companion/) | Connexion au backend, liste et détail des séances, import, envoi vers la montre (Data Layer) | **APK construit** ; à valider sur appareils réels |
| **Montre Garmin** — [`garmin/`](garmin/) | Portage Connect IQ (Monkey C) du même cœur : GPS, cardio, FIT, vibrations, synchronisation identique | **Compilé** (SDK Connect IQ 9.2.0) ; à valider |
| **Backend et site** — [`crates/mpacer-api/`](crates/mpacer-api/) | API JSON, OAuth Google, appairage montre, interface web (séances, analyse, tableaux de bord, courses, musique, suivi en direct, **amis et partage de position sur carte OpenStreetMap**) | **Fait, testé** |
| **Outils** — `mpacer-sim`, `mpacer-music` | Simulateur de séance ; appariement des MP3 et copie sur la montre par USB | **Fait, testé** |
| **Déploiement** — [`charts/`](charts/), [`deploy/`](deploy/) | Chart Helm (application, PostgreSQL CloudNativePG, ingress, TLS), image conteneur, CI GitHub Actions | **Chart validé** ; déploiement sur jo3 en cours |

**Sur la montre**, le coureur voit l'allure lissée, la distance, le temps, le voyant GPS
et un panneau d'assistant (allure seule, temps de finish estimé, shadow runner, course à
distance) ; le cœur rédige les annonces vocales, et la séance est archivée localement
avant d'être envoyée au backend.

**Sur le site**, chaque séance s'ouvre sur une analyse complète (résumé, courbes
allure / cardio / altitude, plan contre réalisé, zones de fréquence cardiaque,
chronologie des pauses), les écrans se composent à partir de **neuf widgets**, les
courses à venir ont leur fiche, leur planning et une **recherche dans le calendrier
Finishers** (la fiche d'une course trouvée arrive pré-remplie), et la page `/music` prépare la
bande-son (playlists Deezer par cookie `arl`, BPM cible, liste des MP3 à
télécharger, envoi dans la file **Deemix** et manifeste à copier sur la montre).

## 2. Architecture

```text
┌── Montre Wear OS ─────────────┐      ┌── Montre Garmin ────────────┐
│ Kotlin + Wear Compose         │      │ Connect IQ (Monkey C)       │
│ GPS · cardio · TTS · service  │      │ GPS · cardio · vibrations   │
└───────────────┬───────────────┘      └──────────────┬──────────────┘
                │ JNI / C ABI (JSON)                  │ HTTPS + jeton
┌───────────────▼───────────────┐                     │
│ mpacer-ffi → mpacer-core      │                     │
│ algorithmes Rust purs         │                     │
└───────────────┬───────────────┘                     │
                │ HTTPS + jeton d'appareil            │
┌───────────────▼─────────────────────────────────────▼──────────┐
│ mpacer-api (Axum)                                              │
│  /api/v1/*      synchronisation JSON                           │
│  /              interface web rendue par le serveur (maud)     │
│  /auth/google   OAuth 2.0 + PKCE                               │
└───────────────┬────────────────────────────────────────────────┘
                │ SQL
┌───────────────▼───────────────┐
│ PostgreSQL (CloudNativePG)    │
└───────────────────────────────┘
        ▲ Ingress traefik + cert-manager → navigateur / téléphone
```

**Trois principes structurants**

1. **La montre est la source de vérité** : elle enregistre et conserve ses séances ; la
   synchronisation est un confort, jamais un prérequis pour courir, et un envoi rejoué
   ne crée pas de doublon.
2. **Le calcul vit dans le cœur Rust**, testable sans montre, sans GPS et sans Android ;
   le code Kotlin et le code Monkey C ne font que piloter la plateforme (capteurs,
   service, TTS, écran).
3. **Aucun secret Google sur la montre** : appairage par code court, jeton opaque
   révocable.

## 3. État du projet

| Domaine | État |
|---|---|
| Cœur métier Rust | ✅ Terminé — algorithmes et tests |
| Backend et interface web | ✅ Terminé — API, authentification, pages, tests d'intégration |
| Applications montres | 🔶 Compilent — validation terrain à faire (écart < 3 % avec une montre de référence, batterie), boutons du casque non branchés |
| Déploiement sur jo3 | 🔶 Chart validé — reste à publier l'image, créer le client OAuth Google et lancer `helm upgrade` |
| Exploitation | 🔲 Sauvegardes planifiées, supervision, limitation de débit |
| Backlog | 🔲 Séances structurées (intervalles), Health Connect, course à distance, allure ajustée à la pente |

## 4. Carte du dépôt

| Chemin | Contenu |
|---|---|
| [`crates/mpacer-core/`](crates/mpacer-core/) | Cœur métier : unités, GPS, allure, tours, séance, assistant, voix, cardio, analyse, musique, GPX, `.pac` |
| [`crates/mpacer-api/`](crates/mpacer-api/) | Backend Axum : API `/api/v1/*`, pages web, OAuth Google, Deezer (cookie `arl` et OAuth), client Deemix, suivi MQTT, migrations SQL (0001 → 0009) |
| [`crates/mpacer-ffi/`](crates/mpacer-ffi/) | Pont C ABI JSON exposé au shell Android (aucun panic ne traverse la frontière) |
| [`crates/mpacer-client/`](crates/mpacer-client/) | Client de synchronisation utilisé par le simulateur |
| [`crates/mpacer-sim/`](crates/mpacer-sim/) | Simulateur : rejoue une course synthétique, écrit un GPX, synchronise vers le backend |
| [`crates/mpacer-music/`](crates/mpacer-music/) | Application locale de transfert des MP3 vers la montre (CLI + interface sur `127.0.0.1:8077`) |
| [`android/`](android/) | Socle partagé (`:core`), montre Wear OS (`:app`), course au téléphone (`:phone`) et application d'appoint (`:companion`) — voir [`android/README.md`](android/README.md) |
| [`garmin/`](garmin/) | Application Connect IQ en Monkey C — voir [`garmin/README.md`](garmin/README.md) |
| [`charts/`](charts/) | Chart Helm `mpacer` |
| [`deploy/`](deploy/) | Dockerfile, script de construction d'image, notes GitOps — voir [`deploy/README.md`](deploy/README.md) |
| [`docs/`](docs/) | Documentation technique (16 documents) — index : [`docs/README.md`](docs/README.md) |
| [`site/`](site/) | Documentation illustrée publiée sur GitHub Pages |
| [`examples/`](examples/) | Cas concrets reproductibles (pacer autour du parc de Parilly) |
| [`simulations/`](simulations/) | Traces GPS synthétiques prêtes à importer |
| [`.github/workflows/`](.github/workflows/) | CI : `ci.yml`, publication d'image `publish.yml`, Pages `pages.yml` |

## 5. Démarrage rapide

### Cœur et simulateur (aucune montre, aucun serveur)

```bash
cargo test --workspace

# Plan de course sur 10 km en 50 min, avec negative split et export GPX
cargo run -p mpacer-sim -- --mode plan --distance 10000 --time 3000 --split 0.03 --gpx trace.gpx

# Avec la musique : le moteur choisit le tempo, la simulation joue les pistes
cargo run -p mpacer-sim -- --mode plan --distance 10000 --time 3000 --music
```

```text
[VERT ] t=  5:00 dist=0.98 km allure= 5:03 tour= 5:07 | ecart plan -2 m
    Allure 5:03 par kilometers. Distance 0.98 km. Temps 5:00. Vous etes sur le plan.
  tour  1 : 1.00 km en 5:06
  tour  2 : 1.00 km en 5:00
```

### Backend en local

```bash
# Un PostgreSQL local (conteneur), puis :
export MPACER_DATABASE_URL="postgresql://mpacer:mpacer@127.0.0.1:5432/mpacer?sslmode=disable"
export MPACER_DEV_AUTH=1 MPACER_PUBLIC_URL=http://localhost:8080
cargo run -p mpacer-api
# http://localhost:8080 → « Continuer avec Google » ;
# avec MPACER_DEV_AUTH=1, POST /auth/dev-login ouvre une session de test.
```

Les secrets locaux (base, broker MQTT…) se rangent dans un `.env` **non versionné** :
copier le gabarit [`.env.example`](.env.example) puis renseigner les valeurs.

```bash
cp .env.example .env
set -a && . ./.env && set +a
```

**Synchroniser une séance** depuis le simulateur : ajouter `--api-url http://localhost:8080`,
saisir le code d'appairage affiché sur `http://localhost:8080/link`, la séance part.

**Tester le backend contre un PostgreSQL** (`MPACER_TEST_DATABASE_URL`) ou contre un vrai
broker MQTT (`MPACER_MQTT_TEST_URL`) : voir `crates/mpacer-api/tests/`.

### Montres et téléphone

```bash
pwsh ./local-ci.ps1 -Target all        # montre Wear OS + téléphone + application d'appoint
pwsh ./local-ci.ps1 -Target phone      # application de course du téléphone
pwsh ./local-ci.ps1 -Install           # compile et installe sur la montre branchée

adb install -r android/phone/build/outputs/apk/debug/phone-debug.apk
```

Pour la Garmin : `pwsh garmin/build.ps1` puis consignes de [`garmin/README.md`](garmin/README.md).

### Musique vers la montre

```bash
cargo run -p mpacer-music              # interface locale http://127.0.0.1:8077
```

## 6. Tests et qualité

| Vérification | Résultat |
|---|---|
| `cargo test --workspace` | **330 tests** : 182 cœur, 8 FFI, 139 service (API, musique, suivi MQTT), 1 test de documentation — 328 exécutés, 2 ignorés par défaut |
| `cd android && ./gradlew :core:testDebugUnitTest` | **27 tests** Kotlin (socle partagé) : codec MQTT octet par octet, politique de cadence, charge utile JSON, analyse d'adresse de broker |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 avertissement |
| `cargo fmt --all --check` | conforme |
| `helm lint charts/mpacer -f charts/mpacer/values-jo3.yaml` | 0 chart en échec (rendu `helm template` vérifié) |
| [`ci.yml`](.github/workflows/ci.yml) | fmt, clippy, tests, simulation bout en bout, lint Helm, pré-vol Connect IQ, construction de l'image |

Les tests d'intégration de l'API s'exécutent contre un vrai PostgreSQL (variable
`MPACER_TEST_DATABASE_URL`) et le test du broker MQTT est ignoré par défaut.

## 7. Déploiement et exploitation

```bash
# 1. Image (ghcr.io)
pwsh deploy/build-image.ps1 -Push

# 2. Déploiement (client OAuth Google de type « Application Web »)
helm upgrade --install mpacer charts/mpacer -n mpacer --create-namespace \
  -f charts/mpacer/values-jo3.yaml \
  --set auth.googleClientId=... --set auth.googleClientSecret=...

# 3. Vérifications
kubectl -n mpacer get cluster,pods,ingress,certificate
curl -s https://mpacer.p.zacharie.org/readyz
```

Le suivi en direct s'active avec un broker MQTT (optionnel) :
`--set config.mqttUrl=mqtt://mosquitto.mpacer.svc:1883`.

| Sujet | Commande |
|---|---|
| État de la base | `kubectl -n mpacer get cluster mpacer-pg` |
| Se connecter à la base | `kubectl -n mpacer exec -it mpacer-pg-1 -- psql -U postgres -d mpacer` |
| Sauvegarde / restauration | `pg_dump -Fc` puis `pg_restore --clean` via `kubectl exec` |
| Mise à jour / retour arrière | `helm upgrade … --set image.tag=…` / `helm rollback mpacer -n mpacer` |
| Journal de l'application | `kubectl -n mpacer logs deploy/mpacer -f` |

Détail complet (image, secrets, TLS, sauvegardes, dépannage) :
[`deploy/README.md`](deploy/README.md).

### Publier une version

Un tag `vX.Y.Z` déclenche [`release.yml`](.github/workflows/release.yml) : la CI
construit les APK des trois applications et les outils Rust (Linux et Windows),
valide la compilation Garmin, puis crée la publication GitHub avec les empreintes
SHA-256 ; l'image conteneur part en parallèle sur `ghcr.io`. Secrets de
signature, variables et procédure complète : [`docs/15-releases.md`](docs/15-releases.md).

## 8. Limites et points ouverts

**Choix assumés**

- **La montre est la source de vérité** ; la synchronisation est rejouable sans doublon.
- **PostgreSQL géré par CloudNativePG** : sauvegardes, réplication et montées de version
  sont déléguées à l'opérateur. Une instance suffit pour un usage personnel ; passer à 3
  pour du HA.
- **Interface web rendue côté serveur** (maud) : une seule image, aucun jeton exposé au
  JavaScript. L'API reste consommable par une SPA si le besoin apparaît.
- **Un seul secret de session** et une application sans état : `RollingUpdate` sans
  coupure et montée en répliques possible.
- **TLS vers PostgreSQL désactivé** par défaut (réseau interne du cluster) ; `sslmode`
  est configurable pour un serveur externe.
- **Aucun fichier audio stocké côté serveur** : les MP3 du disque sont copiés sur la
  montre par USB ; seules les fiches de playlist vivent en base.
- **La « carte » d'une course est un lien** OpenStreetMap, pas une carte interactive : ni
  script tiers, ni donnée envoyée à un service de cartographie. Les liens saisis dans une
  fiche sont limités à `http://` et `https://`.

**Points ouverts** (revue de l'arbre de travail, non corrigés)

1. **Localisation incomplète** — `android/.../MainActivity.kt` ne demande que
   `ACCESS_FINE_LOCATION` ; depuis Android 12, il faut aussi `ACCESS_COARSE_LOCATION`,
   sinon la demande est ignorée et le service de suivi s'arrête.
2. **Réglages sans effet** — `android/.../ui/SettingsScreen.kt` modifie un état local
   jamais poussé au moteur : `setAssistant`, `setVoice` et `VoiceCoach.configure` ne
   sont appelés nulle part.
3. **Champs perdus à l'envoi** — `android/.../SyncClient.kt` ré-encode un résumé partiel :
   `elapsed_s`, `pauses`, `heart_rate` et `plan` n'arrivent pas au backend.
4. **`/readyz` répond toujours 200** — [`src/routes/mod.rs`](crates/mpacer-api/src/routes/mod.rs)
   signale l'état de la base dans le corps JSON, mais ne renvoie jamais de code d'échec.
5. **Erreurs détaillées exposées** — [`src/error.rs`](crates/mpacer-api/src/error.rs) place
   `self.to_string()` dans le champ `message` (texte brut des erreurs SQL et internes).
6. **Pas de jeton anti-CSRF** — [`src/routes/web.rs`](crates/mpacer-api/src/routes/web.rs) :
   les formulaires POST ne sont protégés que par le cookie `SameSite=Lax`.
7. **État du moteur** — `engine.rs::reset()` ne vide pas les tours ;
   `gps.rs` ne fait pas retomber le voyant quand un échantillon est rejeté.
8. **Deux versions de `.pac`** — le cœur écrit la version 2
   ([`history.rs`](crates/mpacer-core/src/history.rs)), l'archive Android annonce encore
   `version: 1` (`WorkoutArchive.kt`).

## 9. Licence

MIT ou Apache-2.0, au choix (voir [`Cargo.toml`](Cargo.toml)).
