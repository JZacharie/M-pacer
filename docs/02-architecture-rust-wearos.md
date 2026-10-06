# 02 - Architecture Rust / Wear OS

Objectif : **le calcul en Rust, la plateforme en Kotlin**. Ce document justifie ce
choix, decrit l'architecture livree et liste ce qui reste a faire cote montre.

![Vue d'ensemble de l'architecture M-pacer](images/architecture-mpacer.png)

---

## 1. Pourquoi Rust, et pourquoi pas du Rust partout

Le coeur de M-pacer est du calcul pur : filtrage GPS, moyennes glissantes, plans de
course, tours, detection d'efforts, formatage. En Rust il est :

- **testable sur PC** (77 tests, aucun emulateur, aucune montre) ;
- **deterministe** : pas de GC, pas de pause d'allocation pendant une seance ;
- **portable** : le meme code peut alimenter une app montre, une app telephone, un
  serveur de calcul ou un outil de ligne de commande ;
- **sans cout d'interopere** : une frontiere fine (une commande JSON, un etat JSON).

En revanche, **une UI 100 % Rust sur Wear OS n'est pas le bon choix en 2026** :

| Option etudiee | Etat | Verdict |
|---|---|---|
| `android-activity` (GameActivity/NativeActivity) | 0.6.1, MSRV 1.85, heberge une Activity native | Aucune prise en charge Wear OS documentee : on herite d'une Activity nue, sans integration systeme |
| Slint (backend Android) | 1.18.x | Pas de support montre / ecran rond identifie |
| egui / wgpu, Bevy | eframe a un support Android recent mais des bugs tactiles ouverts ; Bevy est lourd | Non adapte a une montre |
| Dioxus | rendu via WebView | Inadapte |
| **Kotlin/Wear Compose + coeur Rust (retenu)** | UniFFI 0.32.x mûr, `jni` 0.22.x | **Choix retenu** |

Raison decisive : **plusieurs fonctions indispensables ne sont accessibles qu'en
Kotlin**, quoi qu'on fasse.

- Le **service de premier plan** (`FOREGROUND_SERVICE_HEALTH`/`location`, obligatoire
  depuis Android 14) ne peut pas etre demarre depuis le NDK.
- **Health Services** (`ExerciseClient`, `MeasureClient`) n'a aucune liaison Rust :
  c'est une API Kotlin/coroutines.
- **TTS**, **focus audio**, **MediaSession** (boutons du casque en arriere-plan),
  **tuiles**, **complications**, **mode ambiant** : tout est androidx.

Le coeur Rust est donc la ou il apporte le plus (la logique), et Kotlin garde ce que
seule la plateforme sait faire. Le pont est volontairement **etroit** : une commande
JSON entre, un etat JSON sort.

## 2. Architecture livree

```text
┌─────────────────────────── Montre Wear OS (Kotlin) ───────────────────────────┐
│  MainActivity + Wear Compose (ecran rond)                                     │
│  TrackingService (foreground service) --> GPS / Health Services               │
│  VoiceCoach (TTS + focus audio)  <-- messages a prononcer                     │
│  MpacerCore.kt (JNI)             <-- etat JSON                                │
└───────────────────────────────────┬──────────────────────────────────────────┘
                                    │  libmpacer_ffi.so  (C ABI JSON)
┌───────────────────────────────────┴──────────────────────────────────────────┐
│  mpacer-ffi : Handle, Command (serde), Response (serde), exports extern "C"  │
└───────────────────────────────────┬──────────────────────────────────────────┘
                                    │  appels Rust directs
┌───────────────────────────────────┴──────────────────────────────────────────┐
│  mpacer-core (pur, sans E/S, sans Android)                                   │
│   gps (qualite du signal)   pace (allure lissee)   lap (tours)               │
│   workout (machine a etats, auto-pause)   assistant (4 modes)                │
│   race_plan (shadow runner + negative split)   voice (annonces FR/EN)        │
│   best_distances   history (.pac JSON)   gpx (export)   remote_race          │
│   engine : orchestrateur -> EngineOutput (une seule structure a afficher)    │
└───────────────────────────────────┬──────────────────────────────────────────┘
                                    │  meme crate, aucun changement
                          mpacer-sim (CLI de simulation)
```

### 2.1 Pourquoi un seul point d'entree `PacerEngine`

Le shell Android n'a que trois choses a faire : pousser des positions, envoyer des
commandes, afficher `EngineOutput`. Toute la coordination (ordre des mises a jour,
auto-pause, tours, annonces vocales) vit dans le moteur, donc dans les tests.

```rust
let mut engine = PacerEngine::new(EngineConfig::default());
engine.set_assistant_config(AssistantConfig { /* mode, distance, temps cible */ });
let output = engine.on_gps(sample);      // 1 Hz
let output = engine.start(t_ms);
```

`EngineOutput` contient : etat GPS + couleur du voyant, etat de seance, duree,
distance, allure courante / du tour courant / du tour precedent, vitesse lissee,
panneau d'assistant (temps de finish estime, ecart au shadow runner), tour franchi,
evenements de seance et **messages vocaux deja rediges**.

### 2.2 Le pont FFI

- `mpacer_new()` / `mpacer_free()` : cycle de vie d'un moteur (un par seance).
- `mpacer_command(handle, "{\"cmd\":...}")` : **une seule fonction** pour toutes les
  commandes (`start`, `arm`, `pause`, `resume`, `stop`, `gps`, `tick`,
  `reset_pace_window`, `announce_now`, `configure`, `set_assistant`, `set_voice`,
  `summary`, `version`, `reset`).
- `mpacer_string_free()` : liberation explicite des chaines.
- Toute panique est interceptee (`catch_unwind`) et transformee en
  `{"error": "..."}` : **un panic Rust ne traverse jamais la frontiere FFI**.
- Les enums Rust sont serialises en clair (`"AchievePlannedTime"`,
  `"Every2Minutes"`) : le code Kotlin reste lisible.

Cote Kotlin, `MpacerCore.kt` charge `libmpacer_ffi.so` et expose des fonctions
typees ; `mpacer_jni.c` (30 lignes) fait le lien JNI -> C ABI, ce qui evite d'ajouter
la dependance `jni` au crate Rust.

**Alternative recommandee a moyen terme** : UniFFI 0.32.x, qui genere directement des
liaisons Kotlin typees a partir d'un fichier UDL (objets, enums), au prix d'une
dependance de build supplementaire. La C ABI actuelle est un choix "zero dependance",
plus simple a integrer et deja testee sur PC.

### 2.3 Ou vit quel etat

| Etat | Emplacement | Justification |
|---|---|---|
| Trace GPS, distance, duree, tours | Rust (`PacerEngine`) | Source de verite unique, testable |
| Allure lissee, fenetres de detection | Rust | Algorithmes |
| Reglages (unites, modes, voix) | Rust (serialises) + `DataStore` Kotlin pour la persistance | Le shell reste mince |
| Historique des seances | JSON cote stockage Android (ou `rusqlite`) | Volume faible (une seance = 50-100 ko) |
| Etat GPS brut, capteurs, TTS | Kotlin | API plateforme |

## 3. Algorithme d'allure (le point critique)

`pace::PaceEngine` :

1. **Fenetre de 2 minutes**, mais dont le debut `s` est mobile : par defaut
   `s = t - 120 s`.
2. **Detection de changement d'allure** (option) : a chaque point, on compare la
   vitesse des 20 dernieres secondes a celle des 20 secondes precedentes ; si l'ecart
   relatif depasse 15 %, la fenetre est **redemarree** a `t - 20 s`. C'est ce qui rend
   le fractionne exploitable sans faire remonter deux minutes d'historique parasite.
3. **Garde-fous** : une fenetre doit couvrir au moins 5 m et 5 s (sinon `--:--`),
   allure plafonnee a 30 min/km, horodatages non monotones ignores.
4. **Filtre d'entree** : precision horizontale rejetee au-dela de 50 m, saut
   implausible rejete (vitesse max 12 m/s + marges de precision des deux points).

Ce dernier point est un bug evade : la premiere version poussait aussi un point a
chaque rafraichissement d'ecran, ce qui ecrasait l'allure moyenne par des points
immobiles. Le moteur n'accepte desormais une evolution d'allure (et une decision
d'auto-pause) **que sur une position reellement recue** ; `tick()` ne sert qu'a
rafraichir l'affichage.

## 4. Interface sur montre ronde

Priorites d'affichage, du plus grand au plus petit (ecran `450x450` typique) :

1. **Allure courante** en tres grand (c'est la seule information qu'on lit en courant).
2. **Distance** et **temps** en dessous.
3. **Panneau d'assistant** : soit le temps de finish estime, soit l'ecart au shadow
   runner (texte + couleur : en avance = vert, sur le plan = neutre, en retard = ambre).
4. **Feu de statut GPS** en haut (4 pixels de couleur suffisent).
5. **Ecran ambiant** : uniquement allure + distance, sans secondes, rafraichi au
   minimum (1 Hz maximum, souvent moins).

Interaction : couronne rotative pour changer d'ecran de donnees, appui long sur le
bouton physique pour pause/stop, et surtout **retour vocal** pour ne pas dependre de
l'ecran. Les commandes du casque Bluetooth sont reprises de Pace Control (simple,
double, triple clic).

## 5. Batterie et performance

| Poste | Mesure / decision |
|---|---|
| GPS | 1 Hz suffit pour une allure lissee 2 min ; ne pas demander plus. `ExerciseClient` agrege les donnees en mode ambiant. |
| Affichage | Mode ambiant obligatoire pendant une seance ; pas d'animation continue. |
| TTS | Uniquement aux moments planifies par le moteur (le coach vocal decide *quand*, le shell se contente de parler). |
| Calcul | Negligeable : le moteur traite quelques dizaines de points par seconde au plus. Le cout est dans les capteurs et l'ecran. |
| Reseau | Aucun en seance (le remote race, phase 4, sera le seul consommateur). |

Le simulateur permet de verifier ce budget : `mpacer-sim` rejoue 45 minutes de course
en moins d'une seconde de CPU.

## 6. Permissions et publication (a verifier avant sortie)

- `ACCESS_FINE_LOCATION` (allure et trace).
- `FOREGROUND_SERVICE` + `FOREGROUND_SERVICE_HEALTH` (seance de premier plan).
- `ACTIVITY_RECOGNITION` si detection d'activite.
- `POST_NOTIFICATIONS` (notification de seance en cours).
- Cardio : a partir des versions recentes de Wear OS, l'acces passe par
  `android.permission.health` (`READ_HEART_RATE`, `READ_HEALTH_DATA_IN_BACKGROUND`)
  plutot que `BODY_SENSORS` ; le Play Store exige une declaration "application de
  sante" et une revue des permissions sensibles.

Ces points sont des exigences de plateforme **a revalider sur la version de Wear OS
ciblee** avant la sortie : ils evoluent a chaque version majeure.

## 7. Choix de dependances

Le coeur ne depend que de `serde` / `serde_json` - volontairement :

- geodesie : Haversine implemente (0,5 % d'erreur, suffisant) plutot que `geo` ;
- export GPX : ecrit a la main (30 lignes) plutot que `gpx` (peu maintenu) ;
- dates : conversion ISO 8601 implementee (algorithme civil-from-days) plutot que
  `chrono`/`time` ;
- filtrage : moyenne glissante + porte de plausibilite plutot que `adskalman`
  (Kalman `no_std` disponible si l'on veut affiner plus tard) ;
- historique : JSON versionne plutot que SQLite - a revoir si l'historique depasse
  quelques milliers de seances (`rusqlite` 0.40.x serait le candidat).

Regle : chaque dependance doit gagner plus qu'elle ne coute en taille de binaire, en
temps de compilation et en surface de maintenance. Un coeur sans dependance se teste,
s'embarque et se porte sans mauvaise surprise.

## 8. Construction (build)

```bash
# Coeur + simulateur, sur PC (aucun Android requis)
cargo test --workspace
cargo run -p mpacer-sim -- --mode plan --distance 10000 --time 3000 --gpx trace.gpx

# Bibliotheque Android (necessite le NDK)
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o android/app/src/main/jniLibs build --release -p mpacer-ffi

# Application montre (necessite le SDK Android)
cd android && ./gradlew :app:assembleDebug
```

L'outillage retenu est `cargo-ndk` + le plugin Gradle `org.mozilla.rust-android-gradle`
(la voie maintenue en 2026) ; `cargo-apk` n'a plus de version depuis 2023.

## 9. Ce qui n'a pas pu etre verifie dans cet environnement

- **Aucun JDK, aucun SDK/NDK Android n'est installe ici** : `android/` est un
  squelette ecrit avec soin mais **non compile**. Le coeur Rust, lui, est compile,
  teste et execute (77 tests, clippy sans avertissement).
- Les versions de Wear OS, les noms exacts des permissions et les seuils de
  politique Play Store evoluent : ils sont donnes comme points de depart a
  revalider, pas comme une verite figee.
- Le comportement materiel (precision GPS reelle sur montre, latence des boutons
  Bluetooth, duree de batterie) ne peut etre mesure que sur une montre physique.
