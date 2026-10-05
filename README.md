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

---

## 1. Ce que fait le projet

| Composant | Rôle | État |
|---|---|---|
| **Montre Wear OS** | Enregistre la séance (GPS 1 Hz), calcule l'allure, guide le coureur (voix, shadow runner) | Squelette Kotlin, non compilé ici (pas de SDK Android) |
| **Cœur Rust** | Tous les algorithmes : allure lissée, tours, assistant, voix, GPX, historique | **Fait, testé** (71 tests) |
| **Backend Rust** | API de synchronisation, OAuth Google, interface web, PostgreSQL | **Fait, testé** (12 tests) |
| **PostgreSQL** | Stockage des séances, géré par CloudNativePG dans le cluster | **Déployé sur jo3** (PostgreSQL 18.6) |
| **Chart Helm** | Déploiement complet (app + base + ingress + TLS) | **Validé** (`helm lint` + `--dry-run=server` sur jo3) |

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
| `lap.rs` | Tours km/mile, allure du tour courant et du tour précédent |
| `workout.rs` | Machine à états de séance : démarrage suspendu, pause, auto-pause, reprise |
| `race_plan.rs` | Negative split et **shadow runner** (plan exact à l'arrivée) |
| `assistant.rs` | Les 4 modes : allure, temps estimé, plan de course, course à distance |
| `voice.rs` | Annonces vocales (planification + rédaction FR/EN) |
| `best_distances.rs` | Meilleurs 1/5/10 km et 1/5 mi dans une séance |
| `history.rs` | Format d'échange `.pac` (JSON versionné) |
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
| `src/routes/api.rs` | API `/api/v1/*` : ingestion, listes, stats, export GPX |
| `src/routes/web.rs` | Pages web (maud) : tableau de bord, détail, appairage, jetons |
| `src/routes/mod.rs` | Routeur global, sondes `/healthz` et `/readyz` |
| `src/assets.rs` + `static/` | CSS et JS embarqués dans le binaire |
| `migrations/0001_init.sql` | Schéma PostgreSQL (idempotent, rejoué au démarrage) |
| `tests/api.rs` | 10 tests d'intégration (flux complet, schéma dédié par test) |

### 3.3 Les autres crates

| Dossier | Contenu |
|---|---|
| `crates/mpacer-client/` | Client de synchronisation (appairage, envoi) utilisé par le simulateur et la montre |
| `crates/mpacer-ffi/` | Pont C ABI JSON exposé au shell Android (aucun panic ne traverse la frontière) |
| `crates/mpacer-sim/` | Simulateur : rejoue une course synthétique, écrit un GPX, synchronise vers le backend |

### 3.4 Déploiement

| Fichier | Contenu |
|---|---|
| `charts/mpacer/Chart.yaml` | Métadonnées du chart |
| `charts/mpacer/values.yaml` | Valeurs par défaut documentées |
| `charts/mpacer/values-jo3.yaml` | **Valeurs du cluster jo3** (traefik, regcred, CNPG, amd64) |
| `charts/mpacer/templates/postgresql.yaml` | `Cluster` CloudNativePG + sauvegardes planifiées (option) |
| `charts/mpacer/templates/deployment.yaml` | Application (uid 10001, rootfs read-only, sondes, env DB) |
| `charts/mpacer/templates/secret.yaml` | Secret de session généré et conservé entre upgrades + identifiants Google |
| `charts/mpacer/templates/configmap.yaml` | Configuration non sensible |
| `charts/mpacer/templates/ingress.yaml` | Ingress traefik (`websecure`) |
| `charts/mpacer/templates/ingress-cloudflare.yaml` | Ingress public via Cloudflare Tunnel (option) |
| `charts/mpacer/templates/certificate.yaml` | Certificat TLS cert-manager (DNS-01 Cloudflare) |
| `charts/mpacer/templates/pvc.yaml` | Volume applicatif (désactivé : PostgreSQL gère le stockage) |
| `charts/mpacer/templates/service.yaml` | Service ClusterIP |
| `charts/mpacer/templates/serviceaccount.yaml` | Compte de service sans jeton monté |
| `charts/mpacer/templates/_helpers.tpl` | Noms, labels, secret applicatif PostgreSQL |
| `deploy/Dockerfile` | Image multi-étapes (binaire seul, utilisateur non privilégié) |
| `deploy/build-image.ps1` | Construction/publication (podman, mono-arch ou multi-arch) |
| `deploy/README.md` | **Guide de déploiement** complet |
| `.github/workflows/ci.yml` | CI : format, clippy, tests, lint Helm, build d'image |

### 3.5 Application montre (à compiler sur une machine avec le SDK Android)

| Fichier | Contenu |
|---|---|
| `android/app/build.gradle.kts` | Module Wear OS, cargo-ndk, ABIs |
| `android/app/src/main/AndroidManifest.xml` | Permissions, service de premier plan `location|health` |
| `android/.../MpacerCore.kt` | Pont JNI, commandes JSON, lecture de `EngineOutput` |
| `android/.../TrackingService.kt` | Service de premier plan, boucle GPS 1 Hz, notification |
| `android/.../VoiceCoach.kt` | Synthèse vocale + focus audio (duck / pause / ignorer) |
| `android/.../WorkoutArchive.kt` | Historique local des séances |
| `android/.../ui/MainScreen.kt` | Écran rond : allure, distance, temps, feu GPS, panneau assistant |
| `android/.../ui/SettingsScreen.kt` | Mode d'assistant, unités, voix |
| `android/app/src/main/cpp/mpacer_jni.c` | Shim JNI (40 lignes) vers la C ABI Rust |
| `android/README.md` | Prérequis et compilation |

### 3.6 Documentation

| Fichier | Contenu |
|---|---|
| `docs/01-analyse-features.md` | Analyse des 37 fonctionnalités de Pace Control, priorisation, formules |
| `docs/02-architecture-rust-wearos.md` | Architecture Rust/Wear OS, pont FFI, algorithme d'allure, batterie, permissions |
| `docs/03-plan-action.md` | Plan de développement de l'application montre |
| `docs/04-backend-web-et-deploiement.md` | Backend, auth, API, modèle de données, exploitation |
| `docs/README.md` | Index des documents |

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

### Phase 4 — Application montre 🔲 *à faire*

| Tâche | Statut | Critère d'acceptation |
|---|---|---|
| Chaîne Android (JDK, SDK, NDK, cargo-ndk) | 🔲 | `mpacer_version()` affiché sur la montre |
| Écran principal + service GPS | 🔲 | 10 km enregistrés sans le téléphone |
| Voix, boutons du casque, mode ambiant | 🔲 | Séance guidée sans regarder l'écran |
| Synchronisation depuis la montre | 🔲 | Séance visible dans l'interface web |
| Validation terrain | 🔲 | Écart < 3 % avec une montre de référence, batterie < 25 %/h |

### Phase 5 — Exploitation et durcissement 🔲 *à faire*

| Tâche | Statut | Critère d'acceptation |
|---|---|---|
| Sauvegardes planifiées (`ScheduledBackup`) | 🔲 | Restauration testée |
| Supervision (métriques, alertes) | 🔲 | Alerte si `/readyz` échoue |
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
```

```text
[VERT ] t=  5:00 dist=0.98 km allure= 5:03 tour= 5:07 | ecart plan -2 m
    Allure 5:03 par kilometers. Distance 0.98 km. Temps 5:00. Vous etes sur le plan.
  tour  1 : 1.00 km en 5:06
  tour  2 : 1.00 km en 5:00
  tour  3 : 1.00 km en 4:53
```

### 5.2 Backend en local

```bash
# Un PostgreSQL local (conteneur) puis :
export MPACER_DATABASE_URL="postgresql://mpacer:mpacer@127.0.0.1:5432/mpacer?sslmode=disable"
export MPACER_DEV_AUTH=1 MPACER_PUBLIC_URL=http://localhost:8080
cargo run -p mpacer-api
# http://localhost:8080 → « Connexion développeur »
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
| `cargo test --workspace` | **89 tests** : 71 cœur, 6 FFI, 12 backend (dont 10 d'intégration exécutés contre PostgreSQL) |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 avertissement |
| `cargo fmt --all --check` | conforme |
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
- **Le shell Wear OS n'est pas compilé** dans cet environnement (ni JDK ni SDK/NDK) :
  il est livré comme squelette documenté.
- **TLS vers PostgreSQL désactivé** par défaut (réseau interne du cluster) ;
  `sslmode` est configurable pour un serveur externe.
- Les versions de Wear OS, permissions de santé et règles Play Store évoluent :
  à revalider sur la version ciblée avant publication.

## 10. Prochaines actions

1. Publier l'image : `pwsh deploy/build-image.ps1 -Push` (jeton GitHub).
2. Créer le client OAuth Google (redirect `https://mpacer.p.zacharie.org/auth/google/callback`).
3. `helm upgrade --install` puis vérifier `/readyz` et le certificat.
4. Ouvrir `/link`, appairer la montre (ou le simulateur) et valider un envoi réel.
5. Activer la sauvegarde CNPG (`postgresql.backup.enabled=true`) et l'accès public
   si la montre doit synchroniser hors du domicile.

## 11. Licence

MIT ou Apache-2.0, au choix (voir `Cargo.toml`).
