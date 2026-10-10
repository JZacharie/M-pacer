# 04 - Backend, interface web et déploiement

Objectif : la montre enregistre la séance (elle reste autonome), puis la
**synchronise** vers un backend Rust auto-hébergé où l'utilisateur consulte son
historique. Authentification Google, PostgreSQL géré par CloudNativePG, ingress
et chart Helm fournis.

---

## 1. Vue d'ensemble

```text
┌── Montre Wear OS (Kotlin + cœur Rust) ──┐
│  enregistre la séance hors ligne         │
│  jeton d'appareil (aucun secret Google)  │
└───────────────┬──────────────────────────┘
                │  POST /api/v1/workouts   (Authorization: Bearer …)
┌───────────────▼──────────────────────────┐        ┌────────────────────────┐
│  mpacer-api (Rust, Axum)                 │        │ Cluster CloudNativePG  │
│  ├─ /api/v1/*    API de synchronisation  │◄──────►│ mpacer-pg              │
│  ├─ /            interface web (maud)    │  SQL   │ PostgreSQL 18 (jo3)    │
│  └─ /auth/google OAuth 2.0 + PKCE        │        │ sauvegardes/HA gérées  │
└───────────────┬──────────────────────────┘        └────────────────────────┘
                │  HTTPS via traefik (ingress) + cert-manager
        Navigateur / téléphone
```

Principes de conception :

1. **La montre est la source de vérité.** Elle enregistre et conserve ses séances ;
   la synchronisation est un confort, jamais un prérequis pour courir. Un envoi peut
   être rejoué sans créer de doublon (clé primaire `(user_id, id)`).
2. **Aucun secret Google sur la montre.** Elle s'appaire par code et reçoit un jeton
   opaque révocable depuis l'interface web.
3. **Le même format de données partout.** Ce qui circule est le `WorkoutSummary` du
   cœur Rust, identique au fichier `.pac` : export GPX et relecture triviaux.
4. **L'application est sans état** : l'état vit dans PostgreSQL, géré par un
   opérateur. D'où un déploiement sans coupure et une montée en répliques possible.

## 2. Authentification

### 2.1 Navigateur : Google OAuth 2.0 Authorization Code + PKCE

```text
GET  /auth/google/start     -> crée state + verifier (PKCE), stocke en base, redirige vers Google
GET  /auth/google/callback  -> vérifie state, échange le code, valide l'id_token (JWKS),
                               crée/retrouve l'utilisateur, pose le cookie de session
POST /logout                -> supprime le cookie
```

- L'`id_token` est validé contre les clés publiques de Google (cache 1 h) : signature
  RS256, audience, émetteur, `email_verified`.
- La session est un JWT HS256 dans un cookie `HttpOnly`, `SameSite=Lax`, `Secure`.
- Aucun mot de passe, aucun jeton stocké dans le navigateur.

### 2.2 Montre : appairage par code (device authorization grant)

```text
1. POST /api/v1/device/code   {"label":"Pixel Watch 3"}
   <- {"user_code":"BCDF-GHJK","device_code":"…","verification_uri":"…/link","expires_in":600}
2. L'utilisateur ouvre /link (connecté avec Google) et saisit le code.
3. POST /api/v1/device/token  {"device_code":"…"}
   <- 400 {"error":"authorization_pending"}                 tant que non approuvé
   <- 200 {"access_token":"…","token_type":"Bearer"}         une fois approuvé
```

Le jeton n'est **stocké que haché** (SHA-256), peut être révoqué individuellement
(`/settings`) et n'expire pas tant qu'il n'est pas révoqué : une montre doit pouvoir
synchroniser des mois plus tard.

### 2.3 Mode développement

`MPACER_DEV_AUTH=1` active `POST /auth/dev-login` (session de test sans Google). Le
service refuse de démarrer en production sans identifiants Google et sans secret de
session d'au moins 32 caractères.

## 3. API de synchronisation

| Méthode | Chemin | Auth | Description |
|---|---|---|---|
| POST | `/api/v1/device/code` | — | demande un code d'appairage |
| POST | `/api/v1/device/token` | — | échange le code approuvé contre un jeton |
| GET | `/api/v1/me` | Bearer | utilisateur associé au jeton |
| POST | `/api/v1/workouts` | Bearer | envoie une séance (idempotent) |
| GET | `/api/v1/workouts?limit&offset` | Bearer | liste paginée |
| GET | `/api/v1/workouts/{id}` | Bearer | séance complète (avec la trace) |
| DELETE | `/api/v1/workouts/{id}` | Bearer | suppression |
| GET | `/api/v1/workouts/{id}/gpx` | Bearer | export GPX |
| GET | `/api/v1/races?upcoming` | Bearer | courses à venir (`upcoming=true`) ou toutes |
| POST | `/api/v1/races` | Bearer | crée une course (et son suivi par défaut) |
| GET | `/api/v1/races/{id}` | Bearer | fiche de course complète et son suivi |
| PUT | `/api/v1/races/{id}` | Bearer | met à jour la fiche de course |
| DELETE | `/api/v1/races/{id}` | Bearer | supprime la course et son suivi |
| GET | `/api/v1/stats?days=30` | Bearer | totaux sur une période |
| GET | `/healthz` `/readyz` | — | sondes Kubernetes |
| GET | `/metrics` | — | métriques Prometheus (docs/18, phase D) |

Les erreurs sont homogènes : `{"error":"code_stable","message":"explication"}`.
`authorization_pending` est le seul cas où la montre doit simplement repoller.

Validation à l'ingestion : identifiant présent, durée < 48 h, distance < 500 km,
fréquence cardiaque entre 20 et 250 bpm, pauses et temps écoulé cohérents.
Une séance invraisemblable est refusée avec 400 (protection contre un bug de calcul
côté montre plutôt que contre un utilisateur malveillant).

## 4. Interface web

Rendue côté serveur en Rust (**maud**), sans chaîne JavaScript ni dépendance front :

| Page | Contenu |
|---|---|
| `/` | tableau de bord : totaux 30 jours, montres appairées, historique cliquable |
| `/workouts/{id}` | **analyse de séance** : résumé (mouvement, écoulé, pauses), graphique allure / cardio / altitude, plan de course contre réalisé, zones de fréquence cardiaque et dérive cardiaque, temps de passage, chronologie des pauses et phases d'accélération, meilleures distances, export GPX, suppression |
| `/courses` | **cartes des courses** : compte à rebours, dossard, distance, hôtel, progression du suivi, courses déjà courues |
| `/courses/planning` | **planning** : agenda des échéances à venir (départ, prise de dossard, hôtel, éléments de suivi datés) par mois |
| `/courses/{id}` | **fiche de course** : dossard, horaires, lieux, live, hébergement, nutrition, informations importantes, suivi à cocher |
| `/courses/nouvelle`, `/courses/{id}/modifier` | création et modification d'une fiche de course |
| `/link` | saisie du code affiché par la montre |
| `/settings` | fiche du compte (photo Google, nom, adresse) et jetons d'appareil : création, dernier envoi, révocation |
| `/login` | bouton « Continuer avec Google » |

**Photo de profil.** À la connexion, Google fournit une photo (claim `picture` de
l'`id_token`), conservée dans `users.picture_url`. La route `GET /avatar` la sert :
le service la télécharge lui-même (hôtes `*.googleusercontent.com` uniquement), la
garde six heures en mémoire, puis la renvoie avec `Cache-Control: private`. Le
navigateur ne contacte donc jamais Google, et l'avatar reste affiché même si le poste
ne peut pas joindre `lh3.googleusercontent.com`. Sans photo — ou si Google ne répond
pas — une pastille SVG aux initiales du compte (teinte stable dérivée de son
identifiant) prend le relais. La photo apparaît dans l'en-tête à côté du nom, et en
grand sur `/settings`.

Ce choix (SSR plutôt qu'une SPA WebAssembly) est délibéré : une seule image à
déployer, aucun jeton exposé au JavaScript, fonctionne sans build front. Si une
expérience plus interactive devient nécessaire, une SPA Leptos/Yew pourra consommer
la même API sans rien changer côté serveur.

## 5. Modèle de données (PostgreSQL)

```text
users(id, google_sub, email, name, picture_url, created_at_ms, last_seen_ms)
api_tokens(id, user_id, token_hash, label, created_at_ms, last_used_ms, revoked_at_ms)
device_codes(device_code, user_code, user_id, label, created_at_ms, expires_at_ms,
             approved_at_ms, consumed_at_ms, last_poll_ms)
workouts(id, user_id, started_at_ms, duration_s, distance_m, average_pace_s_per_km,
         unit_system, payload, uploaded_at_ms)   PRIMARY KEY (user_id, id)
oauth_states(state, pkce_verifier, redirect_to, created_at_ms, expires_at_ms)

races(id, user_id, name, start_at_ms, distance_m, discipline, location, start_location,
      bib_number, bib_pickup_at_ms, bib_pickup_location, live_url, registration_url,
      website_url, latitude, longitude, hotel_name, hotel_address, hotel_phone,
      hotel_url, hotel_booked, hotel_check_in_ms, hotel_check_out_ms, lodging_notes,
      nutrition_notes, important_info, notes, goal_time_s, created_at_ms, updated_at_ms)

race_tasks(id, race_id, user_id, label, due_at_ms, done, done_at_ms, position, created_at_ms)
```

- Les horodatages sont des `BIGINT` (millisecondes UNIX) et les mesures des
  `DOUBLE PRECISION` : aucun problème de fuseau horaire, aucune perte de précision.
- `payload` contient le `WorkoutSummary` complet en JSON (tours, meilleures distances,
  trace GPS) : relecture et export GPX avec le même code que la montre. La version 2
  du format `.pac` y ajoute la **fréquence cardiaque**, les **pauses**, le **temps
  écoulé** et le **plan de course** ; une séance version 1 reste lisible, les champs
  absents retombant sur leur valeur par défaut.
- Le schéma est **idempotent** (`CREATE TABLE IF NOT EXISTS`) et appliqué à chaque
  démarrage : aucun outil de migration à installer.
- Toutes les requêtes utilisent des **paramètres numérotés** (`$1`, `$2`…), jamais de
  concaténation : pas d'injection SQL possible.

### 5.1 Configuration de la connexion

Deux formes acceptées :

```bash
# 1. URL complète (tests, serveur externe)
MPACER_DATABASE_URL="postgresql://user:motdepasse@host:5432/mpacer?sslmode=require"

# 2. Variables séparées — exactement les clés du secret applicatif CloudNativePG
MPACER_DB_HOST=mpacer-pg-rw   MPACER_DB_PORT=5432   MPACER_DB_NAME=mpacer
MPACER_DB_USER=mpacer         MPACER_DB_PASSWORD=…   MPACER_DB_SSLMODE=disable
```

La seconde forme est utilisée par le chart : le mot de passe est lu directement dans
le secret généré par l'opérateur, ce qui évite tout problème d'encodage dans une URL.

## 6. Sécurité

| Sujet | Mise en œuvre |
|---|---|
| Secrets | identifiants Google et secret de session dans un Secret Kubernetes (`mpacer-credentials`), jamais dans l'image ni dans le ConfigMap |
| Jeton de base | généré par CloudNativePG, monté par référence (`secretKeyRef`), jamais en clair dans les manifestes |
| Jetons d'appareil | 32 octets aléatoires, stockés hachés (SHA-256), révocables |
| Sessions | JWT HS256, cookie `HttpOnly` + `SameSite=Lax` + `Secure` |
| CSRF | flux OAuth protégé par `state` + PKCE ; les formulaires web exigent la session |
| Cloisonnement | chaque requête filtre par `user_id` (aucun accès croisé entre comptes) |
| Conteneur | utilisateur non privilégié (uid 10001), racine en lecture seule, `/tmp` en emptyDir, capabilities supprimées |
| Réseau base | TLS désactivé par défaut (réseau interne du cluster) ; `sslmode` configurable |

## 7. Déploiement sur jo3

Le détail opérationnel est dans [deploy/README.md](../deploy/README.md). En résumé :

```bash
pwsh deploy/build-image.ps1 -Push          # 1. image

# 2. client OAuth Google (type « Application Web »)
#    URI de redirection : https://mpacer.p.zacharie.org/auth/google/callback

helm upgrade --install mpacer charts/mpacer -n mpacer --create-namespace \
  -f charts/mpacer/values-jo3.yaml \
  --set auth.googleClientId=… --set auth.googleClientSecret=…   # 3. déploiement
```

Le chart crée **le cluster PostgreSQL lui-même** (`Cluster` CloudNativePG), l'ingress
traefik avec l'entrypoint `websecure`, le certificat TLS cert-manager et lit les
identifiants de la base dans le secret applicatif généré par l'opérateur.

## 8. Exploitation

| Sujet | Commande |
|---|---|
| État du cluster | `kubectl -n mpacer get clusters.postgresql.cnpg.io mpacer-pg` |
| Se connecter | `kubectl -n mpacer exec -it mpacer-pg-1 -- psql -U postgres -d mpacer` |
| Sauvegarde logique | `kubectl -n mpacer exec mpacer-pg-1 -- pg_dump -U postgres -Fc mpacer > mpacer.dump` |
| Restauration | `kubectl -n mpacer exec -i mpacer-pg-1 -- pg_restore -U postgres -d mpacer --clean < mpacer.dump` |
| Sauvegardes continues | `postgresql.backup.enabled=true` (objet S3 + `ScheduledBackup`) |
| Mise à jour applicative | `helm upgrade … --set image.tag=…` — sans coupure (`RollingUpdate`) |
| Retour arrière | `helm rollback mpacer -n mpacer` |
| Sondes | `/healthz` (vivacité), `/readyz` (base joignable) |
| Métriques | `/metrics` (Prometheus), port `metrics` du Service pour le `ServiceMonitor` |
| Journal | `RUST_LOG` (défaut `info,mpacer_api=info,tower_http=warn`) |

## 9. Limites connues

- **Une instance PostgreSQL** par défaut : suffisant pour un usage personnel.
  Passer `postgresql.instances` à 3 pour du HA (l'opérateur gère le basculement).
- **TLS vers PostgreSQL désactivé** par défaut : le trafic reste dans le réseau du
  cluster. Pour un serveur externe, activer `sslmode=require` et monter le CA.
- **Politique de mot de passe** : gérée par l'opérateur ; ne pas modifier le secret
  applicatif à la main.
- Le mode `MPACER_DEV_AUTH` ne doit **jamais** être activé en production
  (avertissement journalisé au démarrage, valeur par défaut `false`).
- Les permissions de santé et les règles Google Play évoluent : à revalider avant
  toute publication de l'application montre.
