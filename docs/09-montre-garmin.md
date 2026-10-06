# 09 — Montre Garmin (Connect IQ, Monkey C)

> **État** : application écrite et **compilée** avec le SDK Connect IQ 9.2.0
> (`monkeyc -l 2`, `BUILD SUCCESSFUL`) ; simulation avec un profil d'appareil et
> validation terrain restent à faire. Code : [`garmin/`](../garmin/), mode
> d'emploi : [`garmin/README.md`](../garmin/README.md).

## 1. Pourquoi un portage et non une compilation croisée

Le cœur M-pacer est du Rust. Sur Wear OS il est compilé pour Android
(`aarch64-linux-android`) et appelé par JNI. Ce chemin n'existe pas sur Garmin :
une montre Garmin exécute des applications **Connect IQ** écrites en **Monkey C**,
un langage à machine virtuelle, sans bibliothèque native, sans système de fichiers
libre, sans synthèse vocale. Aucun `.so` ne peut y être embarqué.

Le portage suit donc la règle du dépôt — **le calcul métier ne vit pas dans le
shell** — en appliquant le même découpage que côté Rust/Kotlin :

```text
   Wear OS                                    Garmin
   ┌──────────────────────────┐               ┌──────────────────────────┐
   │ MainActivity / Service   │ shell         │ MpacerApp / MpacerView   │ shell
   │ (capteurs, service, TTS) │               │ (GPS, cardio, FIT, vibre)│
   └────────────┬─────────────┘               └────────────┬─────────────┘
                │ JNI (JSON)                               │ appels directs
   ┌────────────▼─────────────┐               ┌────────────▼─────────────┐
   │ mpacer-core (Rust)       │ cœur          │ Mpacer* (Monkey C)       │ cœur
   └──────────────────────────┘               └──────────────────────────┘
```

Chaque module Monkey C est le portage d'un module Rust, avec les mêmes seuils,
les mêmes formules et les mêmes cas limites.

## 2. Correspondance module par module

| Cœur Rust | Monkey C | Ce qui est conservé à l'identique |
|---|---|---|
| `units.rs` | `MpacerUnits.mc` | 1 609,344 m par mile, `--:--` sans allure, arrondi des secondes, distance courte « 350 m » |
| `geo.rs` | `MpacerGeo.mc` | Haversine, rayon 6 371 008,8 m |
| `gps.rs` | `MpacerGpsMonitor.mc` | seuils 10 m / 25 m, 3 échantillons bons pour le vert, rejet au-delà de 2 × 25 m, anti-saut (12 m/s + précisions) |
| `pace.rs` | `MpacerPaceEngine.mc` | fenêtre **120 s**, sondes 20 s, seuil de changement 15 %, allure max 30 min/km, points non monotones ignorés |
| `lap.rs` | `MpacerLapTracker.mc` | tour = 1 km ou 1 mile, durée du tour interpolée au franchissement, temps de course (pauses exclues), tour manuel |
| `workout.rs` | `MpacerWorkout.mc` | états `idle/armed/running/auto_paused/paused/finished`, auto-pause 10 s sous 0,7 m/s, reprise 3 s au-dessus de 1,4 m/s |
| `race_plan.rs` | `MpacerRacePlan.mc` | `a(f) = A(1 + r − 2rf)`, plan exact à l'arrivée, résolution du shadow runner par équation du second degré |
| `assistant.rs` | `MpacerAssistant.mc` | modes allure / finish estimé / temps visé, tolérance « sur le plan » = 5 m et 2 s |
| `best_distances.rs` | `MpacerTrack.mc` | 1 km, 1 mi, 5 km, 5 mi, 10 km, semi — mais **algorithme en O(n)** par fenêtre glissante (la version Rust en O(n²) serait trop lente sur une montre) |
| `cardio.rs` | `MpacerCardio.mc` | zones en pourcentage de la FC max (50-60-70-80-90-100 %), zone 0 sous la zone 1 |
| `voice.rs` | `MpacerCoach.mc` | **déclencheurs** identiques (départ, pause, reprise, arrêt, tour, périodique, écart au plan) — le rendu devient vibration + texte |
| `engine.rs` | `MpacerEngine.mc` | `step()` unique : auto-pause sur vitesse lissée, alimentation de l'allure seulement en course, tours, panneau, pauses, résumé final |
| `history.rs` | `MpacerEngine.summary()` | même structure de `WorkoutSummary` que le fichier `.pac` |
| shell Kotlin | `MpacerApp/View/Delegate.mc` | capteurs, écran rond, boutons, archivage, envoi |

### Deux écarts assumés

1. **La précision GPS n'est pas en mètres.** `Position.Info.accuracy` renvoie une
   *qualité* (`NOT_AVAILABLE`, `LAST_KNOWN`, `POOR`, `USABLE`, `GOOD`). Le shell la
   traduit en précision estimée (5 / 18 / 60 / 999 m) avant d'appeler le filtre :
   le cœur, lui, ne connaît toujours que des mètres, et la logique de seuils reste
   celle de `gps.rs`.
2. **Les meilleures distances** utilisent une fenêtre glissante à pointeur unique
   (O(n) par distance cible) au lieu de la double boucle Rust : même résultat,
   coût compatible avec le processeur d'une montre.

## 3. Le shell Garmin

### Capteurs et enregistrement

- `Position.enableLocationEvents(Position.LOCATION_CONTINUOUS, …)` : positions à
  ~1 Hz, comme la montre Wear OS.
- `Sensor.enableSensorEvents(…)` : fréquence cardiaque (poignet ou ceinture) et
  cadence.
- `ActivityRecording.createSession({:sport => Activity.SPORT_RUNNING, …})` : la
  séance est écrite dans un **fichier FIT**, visible dans Garmin Connect, avec la
  trace GPS complète. Les tours calculés par le cœur sont ajoutés au FIT
  (`session.addLap()`), les bornes du FIT et de l'écran coïncident donc.
- Le FIT appartient à la montre : M-pacer n'a **pas** besoin de conserver la trace
  pour que la séance soit récupérable, ce qui est essentiel avec 128 Ko de
  stockage applicatif.

### Écran et commandes

| Bouton | Course | Autres écrans |
|---|---|---|
| START | démarrer / pause / reprise | — |
| LAP | tour manuel | — |
| haut / bas | écran course → sync → réglages | idem |
| MENU | annonce immédiate | appairer / envoyer |
| retour | arrêter | revenir |

L'écran affiche, de haut en bas : voyant GPS, **allure lissée** (la valeur
centrale), distance et temps de course, tour courant (distance + allure du tour
précédent), cardio et zone, panneau d'assistant (finish estimé, écart au shadow
runner, ou consigne de configuration manquante), puis l'alerte en cours.

### Alertes à la place de la voix

Connect IQ n'expose **aucune synthèse vocale** : le planificateur d'annonces de
`voice.rs` est conservé, mais chaque annonce devient une vibration (courte,
double ou longue selon l'événement) et un texte affiché huit secondes. Les
Forerunner ne gèrent pas les motifs de vibration : le code se contente alors de
profils simples, et teste `Attention has :vibrate` avant tout appel.

## 4. Contraintes réelles, et comment elles sont traitées

| Contrainte Connect IQ | Conséquence dans M-pacer |
|---|---|
| `Application.Storage` : **8 Ko par clé, 128 Ko au total** | chaque séance en attente a sa propre clé `pending:<id>` ; la trace GPS n'y est **jamais** persistée ; l'historique des identifiants envoyés est borné à 200 |
| Pas de système de fichiers libre | l'archive locale se limite au résumé (tours, meilleures distances, pauses, cardio sous-échantillonnée) |
| Mémoire applicative limitée | trace en **tableaux parallèles** de nombres (pas un dictionnaire par point), plafonnée à 4 000 points, plus cardio sous-échantillonnée à 5 s |
| Requêtes BLE de taille inconnue mais faible (quelques kilo-octets) | l'envoi de la trace est **désactivé par défaut** (`syncTrace = false`) ; s'il est activé, la trace est décimée (300 points au plus) et la cardio plafonnée à 600 mesures |
| `makeWebRequest` : pas de timeout, pas d'annulation unitaire | les envois sont séquentiels, un échec laisse la séance dans l'archive et l'envoi reprend au prochain essai |
| HTTPS obligatoire hors serveur local, transport par le téléphone appairé (BLE) | le backend public du dépôt est en HTTPS ; l'appairage reste identique (device flow RFC 8628) |
| Pas de TTS, pas de lecture audio locale | voix remplacée par vibrations/texte ; musique non portée |

## 5. Réseau : le contrat reste celui du backend

L'application Garmin parle **exactement** l'API existante :

1. `POST /api/v1/device/code` avec `{"label": "Montre Garmin"}` → `device_code`,
   `user_code`, `verification_uri`, `interval`, `expires_in`.
2. Sondage de `POST /api/v1/device/token` toutes les `interval` secondes
   (Timer Connect IQ) jusqu'à l'approbation, l'expiration ou le refus.
3. `POST /api/v1/workouts` avec l'en-tête `Authorization: Bearer <jeton>` et un
   corps JSON identique au `WorkoutSummary` du cœur :

```json
{
  "id": "1760000000000",
  "started_at_ms": 1760000000000,
  "duration_s": 1500.0,
  "distance_m": 5000.0,
  "average_pace_s_per_km": 300.0,
  "laps": [ { "index": 1, "distance_m": 1000.0, "duration_s": 300.0, "pace_s_per_km": 300.0 } ],
  "best_efforts": [ { "label": "1 km", "distance_m": 1000.0, "time_s": 300.0, "start_dist_m": 0.0 } ],
  "track": [],
  "unit_system": "Metric",
  "elapsed_s": 1560.0,
  "pauses": [ { "at_s": 600.0, "at_distance_m": 2000.0, "duration_s": 60.0, "automatic": false } ],
  "heart_rate": [ { "t_ms": 0, "bpm": 140 } ],
  "plan": { "distance_m": 5000.0, "target_time_s": 1500.0, "negative_split": { "enabled": false, "ratio": 0.03 } }
}
```

L'envoi est **idempotent** comme celui de la montre Wear OS : un renvoi remplace
la séance côté backend au lieu de créer un doublon. `unit_system` reprend la
sérialisation du cœur Rust (`"Metric"` / `"Imperial"`).

## 6. Réglages

`resources/settings/settings.xml` et `resources/settings/properties.xml`
décrivent quinze réglages, éditables depuis Garmin Connect Mobile : assistant,
unités, détection de changement d'allure, pause automatique, alertes, plan de
course (distance, temps, negative split), FC max, adresse du backend et trace
GPS. Les listes stockent des **entiers** (contrainte du compilateur de
ressources Connect IQ) que `MpacerSettings.mc` traduit en modes ; une propriété
absente ou illisible retombe sur sa valeur par défaut.

## 7. Outillage et vérification

| Outil | Rôle |
|---|---|
| `garmin/build.ps1` | découvre (ou télécharge) le SDK, génère la clé de signature, compile, peut lancer le simulateur ou copier le `.prg` dans `GARMIN/APPS` |
| `garmin/preflight.ps1` | XML bien formés, cohérence `settings.xml` ↔ `properties.xml`, chaînes `Rez.Strings.*` déclarées, puis compilation |
| `garmin/tests.jungle` + `garmin/test/` | tests unitaires Monkey C : les valeurs attendues sont celles des tests Rust (formatage, Haversine, allure 2 min, tours, plan, shadow runner, meilleur km, zones FC) |
| `garmin/tools/sync-products.ps1` | aligne `<iq:products>` sur les appareils installés |
| `garmin/tools/make_icon.py` | régénère l'icône du lanceur |
| CI (`.github/workflows/ci.yml`, tâche `garmin`) | télécharge le SDK et exécute le pré-vol à chaque `push` |

Ce que la vérification couvre : syntaxe Monkey C, types (niveau *informative*),
appels d'API Connect IQ, manifeste, ressources (chaînes, icône, réglages), et la
compilation des tests unitaires (`-Test`).
Ce qu'elle ne couvre pas : le rendu écran et le comportement capteurs, qui
demandent un profil d'appareil (SDK Manager) puis une montre.

## 8. Suite

1. Installer les profils d'appareils (SDK Manager → **Devices**), compiler un
   `.prg` par modèle, lancer les tests unitaires
   (`pwsh ./garmin/build.ps1 -Test -Device <id>`) puis l'application dans le
   simulateur.
2. Valider sur le terrain : écart d'allure < 3 % avec une montre de référence,
   consommation < 25 %/h, comportement de la vibration sur Forerunner.
3. Ajouter un écran de réglages embarqué sur les modèles qui l'autorisent.
4. Étudier un envoi différé en arrière-plan (permission `Background`) pour les
   séances terminées hors de portée du téléphone.
5. Publier le `.iq` signé sur la Connect IQ Store (`garmin/build.ps1 -Package`).