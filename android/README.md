# Android : socle partage (`:core`), montre (`:app`), course au telephone (`:phone`) et compagnon (`:companion`)

Ce dossier contient **quatre modules Gradle** qui partagent le meme coeur Rust et le
meme backend auto-heberge :

| Module | Identifiant | Role |
|---|---|---|
| `:core` | `com.mpacer.core` | Bibliotheque commune : pont JNI vers `mpacer-core`, service de seance (GPS 1 Hz), voix, archive locale, synchronisation backend, suivi MQTT, musique, amis et partage de position. Aucune interface. |
| `:app` | `com.mpacer.watch` | Application Wear OS : ecrans ronds, six vues de course (allure, tour, cardio, objectif, **musique avec volume et changement de piste**, **heure**), capteur cardiaque de la montre, Data Layer (reception des seances envoyees par le telephone). |
| `:phone` | `com.mpacer.phone` | Application telephone (Android 8+) pour **courir avec le telephone** : ecrans Material 3, ceinture cardiaque Bluetooth LE, historique, synchronisation, MQTT, musique, onglet Amis (carte OpenStreetMap des proches). |
| `:companion` | `com.mpacer.companion` | Application telephone d'appoint : connexion au backend, liste et detail des seances, import de fichier `.pac`/JSON, envoi vers la montre. **Ne fait pas de seance.** |

> **Verifie le 6 octobre 2026** : les quatre modules compilent (`.gradlew.bat
> assembleDebug`), les trois applications produisent un APK, les bibliotheques
> natives sont empaquetees et les 27 tests JVM du socle passent. La seance n'a en
> revanche **pas encore ete courue** avec le telephone : voir
> [Points a verifier](#points-a-verifier).

## Arborescence

```
android/
  gradlew, gradlew.bat, gradle/wrapper/     wrapper Gradle 8.11.1 (jar inclus)
  gradle/libs.versions.toml                  catalogue de versions unique
  settings.gradle.kts                        include(":core") + :app + :phone + :companion
  build.gradle.kts                           plugins declares en apply false
  gradle.properties                          reglages communs
  .gitignore                                 secrets et sorties locales
  core/                                      SOCLE PARTAGE (bibliotheque Android)
    build.gradle.kts                         cargo-ndk, ABI, buildConfig, signature des .so
    proguard-rules.pro                       regles R8 transmises aux applications
    src/main/AndroidManifest.xml             permissions et services fusionnes dans les applications
    src/main/cpp/CMakeLists.txt, mpacer_jni.c  shim JNI vers la C ABI Rust (Java_com_mpacer_core_*)
    src/main/jniLibs/<abi>/libmpacer_ffi.so  coeur Rust, produit par cargo-ndk (non versionne)
    src/main/java/com/mpacer/core/
      MpacerCore.kt        pont JNI + data classes d etat
      TrackingService.kt   service de premier plan GPS + notification (Pause / Stop)
      VoiceCoach.kt        TTS + focus audio
      WorkoutArchive.kt    historique local .pac (lister, supprimer, exporter)
      MpacerFormat.kt      formatage (allure, duree, distance, notification)
      SyncClient.kt        device flow, envoi des seances, jeton chiffre
      SessionConfig.kt     reglages assistant + voix gardes hors seance
      HeartRateSensor.kt   capteur integre + interface HeartRateSource (ceinture BLE)
      social/FriendsClient.kt  amis et partage de position (API du backend)
      live/                MQTT : config, politique de cadence, charge utile, codec, tracker, test, persistance ;
                           parcours planifie (lecture GPX, stockage, publication)
      music/               Media3 : bibliotheque USB, lecteur, session, modeles du contrat docs/07 v2
      ui/Palette.kt        palette et voyant GPS partages par les deux interfaces
    src/test/java/com/mpacer/core/live/      tests unitaires JVM (paquets, cadence, charge utile)
  app/                                       module montre
    build.gradle.kts                         Wear OS, ABI, signature
    proguard-rules.pro
    src/main/AndroidManifest.xml             permissions propres a la montre + listener Data Layer
    src/main/java/com/mpacer/watch/
      MainActivity.kt      navigation course / reglages / synchronisation / musique
      WearSyncListener.kt  reception des seances envoyees par le telephone
      ui/                  MainScreen, SettingsScreen, SyncScreen, MusicScreen, LiveSettingsScreen
  phone/                                     module telephone (courir avec le telephone)
    build.gradle.kts                         Material 3, ABI, signature
    proguard-rules.pro
    src/main/AndroidManifest.xml             GPS, cardio, Bluetooth LE, ceinture cardiaque
    src/main/java/com/mpacer/phone/
      MainActivity.kt      permissions + navigation a cinq onglets
      PhoneSettings.kt     reglages persistants (assistant, voix, musique, cardio, ecran)
      hr/BleHeartRate.kt   ceinture cardiaque Bluetooth LE (0x180D / 0x2A37) + recherche
      ui/                  Theme, RunScreen (parcours planifie GPX), FriendsScreen
                           (carte OSM), HistoryScreen,
                           MusicScreen (lecteur a icones et volume), SyncScreen,
                           SettingsScreen, PhoneIcons (jeu d icones maison, sans
                           material-icons-extended)
  companion/                                 module telephone
    build.gradle.kts                         Material 3, Compose, OkHttp, Wearable
    proguard-rules.pro
    src/main/AndroidManifest.xml             INTERNET + <queries> Play/Wear
    src/main/java/com/mpacer/companion/
      MainActivity.kt      hote Compose + navigation
      AppViewModel.kt      etat de l application
      MpacerApi.kt         client HTTP du backend
      ApiModels.kt         modeles JSON (serde snake_case)
      TokenStore.kt        jeton dans EncryptedSharedPreferences
      WearSync.kt          MessageClient / DataClient + installation montre
      ui/                  Theme, Format, Login, WorkoutList, WorkoutDetail, Send, Music
```

## Prerequis

- **JDK 17** (AGP 8.7 exige Java 17 ; un JDK 21 fonctionne aussi).
- **Android SDK 35** : `platforms;android-35`, `build-tools;35.0.0`, `platform-tools`.
- **NDK r27** (ou r26+) : `ndk;27.2.12479018` par exemple, et **CMake 3.22.1** (fourni par le SDK).
- **Gradle** : inutile d en installer un, le wrapper telecharge 8.11.1.
- **Rust + cargo-ndk** pour la montre :
  - `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`
  - `cargo install cargo-ndk`
- **Wear OS** : une montre ou un emulateur Wear OS 3+ pour `:app`.

Declarer le SDK, au choix :

```powershell
$env:ANDROID_HOME = "C:\Users\<vous>\AppData\Local\Android\Sdk"
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
# ou creer android/local.properties (non versionne) :
# sdk.dir=C\:\\Users\\<vous>\\AppData\\Local\\Android\\Sdk
```

## Commandes de build

### 0. Tout en une commande : `local-ci.ps1`

Le script [local-ci.ps1](../local-ci.ps1) (a la racine du depot) enchaine
verification de l'environnement, compilation du coeur Rust pour les trois ABI,
assemblage Gradle et installation eventuelle sur la montre :

```powershell
pwsh ./local-ci.ps1 -Check                 # diagnostic seul (code de sortie 1 si un prerequis manque)
pwsh ./local-ci.ps1                        # montre, debug
pwsh ./local-ci.ps1 -Target all            # montre + les deux applications telephone
pwsh ./local-ci.ps1 -Target phone          # application de course du telephone
pwsh ./local-ci.ps1 -Release               # APK release (si android/keystore.properties)
pwsh ./local-ci.ps1 -ApiUrl http://192.168.0.152:8080 -Install
pwsh ./local-ci.ps1 -Bootstrap             # installe cibles rustup, cargo-ndk et paquets du SDK
pwsh ./local-ci.ps1 -Clean -Test           # nettoyage + tests du coeur Rust avant build
```

Le script detecte le JDK (y compris le JBR embarque par Android Studio via
`-JavaHome`), le SDK et le NDK, et affiche pour chaque manque la commande exacte
a lancer. Le coeur Rust est compile par `cargo ndk` puis Gradle est appele avec
`-Pmpacer.buildRust=false` : la chaine Rust n'est pas executee deux fois.

### Commandes manuelles

Toutes les commandes se lancent depuis `android/`. Sous Windows, utiliser
`gradlew.bat` (ou `./gradlew` si un shell POSIX est disponible).

### 1. Montre (`:app`)

```powershell
cd android
./gradlew :app:assembleDebug
```

La tache `cargoNdkBuild` (branchee sur `preBuild`) compile automatiquement le coeur
Rust pour chaque ABI :

```powershell
# equivalente manuelle de ce que fait la tache (le module :core porte le coeur) :
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o android/core/src/main/jniLibs build --release -p mpacer-ffi
```

Options utiles :

```powershell
./gradlew :app:assembleDebug -Pmpacer.buildRust=false      # reutilise les .so deja presents
./gradlew :app:assembleDebug -Pmpacer.cargoProfile=debug   # profil cargo debug
./gradlew :app:installDebug                                # installe sur la montre/emulateur
```

APK : `app/build/outputs/apk/debug/app-debug.apk`.

### 2. Telephone — courir avec le telephone (`:phone`)

```powershell
./gradlew :phone:assembleDebug
./gradlew :phone:installDebug
```

APK : `phone/build/outputs/apk/debug/phone-debug.apk` (~20 Mo, trois ABI :
arm64-v8a, armeabi-v7a, x86_64).

### 3. Telephone — application d'appoint (`:companion`)

```powershell
./gradlew :companion:assembleDebug
./gradlew :companion:installDebug
```

APK : `companion/build/outputs/apk/debug/companion-debug.apk`.

### 4. Les quatre modules

```powershell
./gradlew assembleDebug        # :core + :app + :phone + :companion
./gradlew :core:testDebugUnitTest   # tests JVM du socle (27 tests)
```

### 5. APK release signe (a deployer sur la montre)

Le build release exige une cle : sans `android/keystore.properties`,
`assembleRelease` produit un APK non signe que la montre refuse. Le magasin de
cles local (`android/mpacer-release.jks`, RSA 4096, validite 10 ans) et son mot de
passe vivent dans ces deux fichiers **non versionnes** (`android/.gitignore`) :
sauvegardez-les, ils signeront toutes les mises a jour.

```powershell
# Montre seule, avec l URL du backend inscrite dans l APK :
pwsh ./local-ci.ps1 -Release -ApiUrl https://mpacer.p.zacharie.org

# Equivalent direct, en reutilisant les .so Rust deja construits :
cd android
.gradlew.bat :app:assembleRelease '-Pmpacer.buildRust=false' '-Pmpacer.apiUrl=https://mpacer.p.zacharie.org'
```

APK : `app/build/outputs/apk/release/app-release.apk` (~33 Mo, trois ABI :
arm64-v8a, armeabi-v7a, x86_64).

Installation (USB ou adb sans fil) :

```powershell
adb install -r android/app/build/outputs/apk/release/app-release.apk
# Si une version de debug (autre cle) est deja installee, la desinstaller d abord :
adb uninstall com.mpacer.watch
```

Le module telephone se signe avec **la meme cle** (`:companion:assembleRelease`) :
c est la condition du Data Layer Wear OS. Une montre en release et un telephone en
debug ne communiquent pas. L URL du backend reste surchargeable au lancement, sans
recompiler :

```powershell
adb shell am start -n com.mpacer.watch/.MainActivity --es api_url http://192.168.0.152:8080
```

Aucun secret n entre dans l APK : l adresse du broker MQTT se saisit sur la montre
(menu Broker MQTT) et le mot de passe est chiffre sur l appareil, pas dans le binaire.

Sans SDK Android ni cargo-ndk, **aucune de ces commandes n a ete executee ici**.

## Application telephone : courir avec le telephone (`:phone`)

Le module `:phone` reprend **toutes** les fonctions de la montre, sur le meme socle
`:core` : le coeur Rust ne change pas d une ligne, seule la plateforme differe.
Detail complet : [docs/12](../docs/12-course-telephone.md).

| Fonction | Montre (`:app`) | Telephone (`:phone`) |
|---|---|---|
| Calcul de la seance | `mpacer-core` (JNI) | `mpacer-core` (JNI) — le meme |
| Position | FusedLocation 1 Hz, service de premier plan | identique (type `location`) |
| Cardio | capteur integre | ceinture Bluetooth LE (`0x180D` / `0x2A37`) |
| Voix, annonces, tours | TTS + focus audio | identique |
| Assistant, shadow runner | ecran rond | tuiles Material 3 |
| Historique local | `.pac` dans `filesDir` | identique (+ suppression, partage) |
| Synchronisation backend | device flow + `POST /api/v1/workouts` | identique (onglet Sync, onglet ouvert dans un Custom Tab) |
| Suivi en direct MQTT | oui (opt-in) | identique, reglages au clavier |
| Musique | fichiers copies par USB | identique (`Music/` du telephone) |
| Ecran allume | reglage de la montre | `FLAG_KEEP_SCREEN_ON` pendant la seance |

Ce qui est propre au telephone :

- **Navigation a cinq onglets** : Course, Historique, Musique, Sync, Reglages.
- **Notification de seance actionnable** : Pause/Reprendre et Stop depuis le
  bandeau, sans sortir le telephone de sa ceinture (le service est partage, la
  montre en profite aussi).
- **Bouton « Preparer »** : le chrono reste arme et part au premier pas.
- **Boutons « Annonce vocale » et « Fenetre d'allure »** : les deux commandes du
  moteur (`announce_now`, `reset_pace_window`) etaient jusqu ici sans bouton.
- **Reglages persistants** : `SharedPreferences` pour l'interface, secrets
  inchanges dans le socle (Keystore). Les reglages d'assistant et de voix sont
  desormais reellement transmis au moteur au depart de la seance
  (`SessionConfig`), ce qui manquait sur la montre.
- **Ceinture cardiaque Bluetooth LE** : recherche filtree sur le service Heart
  Rate, choix de l'appareil, reconnexion par Android. Le socle ne connait qu'une
  interface `HeartRateSource` : la seance demarre le capteur integre et la
  source declaree.
- **Musique du telephone** : `adb push ./run-170
  /sdcard/Android/data/com.mpacer.phone/files/Music/` puis « Importer (USB) »
  dans l'onglet Musique (le dossier exact est affiche a l'ecran).
- **Onglet Amis** : cercle ferme (code d'invitation a usage unique, ajout par
  code, retrait), interrupteur de partage, et carte **OpenStreetMap** des amis en
  direct (WebView alimentee par l'API, script `/static/map.js` du backend). Au
  depart de chaque seance, l'application **revendique son nom d'appareil**
  (`POST /api/v1/live/register`) : c'est ce qui relie le sujet MQTT a un compte
  et autorise le partage. Trois conditions pour etre vu : appareil appaire,
  broker MQTT renseigne (Reglages > Suivi en direct), seance en cours. Detail :
  [docs/13](../docs/13-amis-partage-position.md).

Permissions demandees au premier lancement : position, notifications, activite,
capteur cardiaque, Bluetooth (Android 12+). **Aucune n'est obligatoire** : la
seance est complete sans cardio, et l'archive reste locale sans backend.

## Flux d appairage (RFC 8628 simplifie)

Le backend expose `POST /api/v1/device/code` puis `POST /api/v1/device/token`, et la
page `/link` permet de saisir le code utilisateur. Les trois applications (montre,
course au telephone, compagnon) suivent le meme flux, porte par `:core`.

1. L application appelle `POST /api/v1/device/code` avec `{"label": "..."}`.
2. Le backend repond `device_code`, `user_code` (du type `BCDF-GHJK`), `verification_uri`,
   `verification_uri_complete`, `expires_in` et `interval`.
3. L utilisateur saisit le code :
   - sur la montre, [`ui/SyncScreen.kt`](app/src/main/java/com/mpacer/watch/ui/SyncScreen.kt)
     l affiche en grand et indique l URL ;
   - sur le telephone, [`ui/LoginScreen.kt`](companion/src/main/java/com/mpacer/companion/ui/LoginScreen.kt)
     ouvre `verification_uri_complete` dans un **Custom Tab**.
4. L application sonde `POST /api/v1/device/token` avec `device_code`. Tant que le code
   n est pas approuve, le backend repond HTTP 400 `{"error":"authorization_pending"}` :
   c est le fonctionnement normal, l application attend `interval` secondes avant de
   reessayer.
5. Apres approbation, le backend renvoie `access_token`. Il est stocke dans
   **EncryptedSharedPreferences** (cle maitresse du Keystore Android), jamais en clair.
6. Les appels suivants portent `Authorization: Bearer <jeton>`.

Appairage manuel par `curl` (verification) :

```bash
curl -s -X POST http://<backend>/api/v1/device/code -H "Content-Type: application/json" -d '{"label":"Montre"}'
# saisir le user_code sur http://<backend>/link, puis :
curl -s -X POST http://<backend>/api/v1/device/token -H "Content-Type: application/json" -d '{"device_code":"<device_code>"}'
```

## Synchronisation et Data Layer

- La montre poste ses seances locales sur `POST /api/v1/workouts` avec le `WorkoutSummary`
  produit par `mpacer-core` (voir `SyncClient.kt` et `WorkoutArchive.kt`). Un identifiant
  de seance deja envoye est memorise localement, ce qui rend l envoi idempotent.
- Le telephone liste `GET /api/v1/workouts`, lit `GET /api/v1/workouts/{id}`, importe un
  fichier `.pac`/JSON local, puis peut renvoyer une seance vers la montre.
- Envoi vers la montre (`WearSync.kt`) :
  - `MessageClient` si la charge utile tient sous 90 Ko ;
  - `DataClient` + `Asset` au-dela (trace GPS complete).
  La montre declare la capacite `mpacer_sync` (`app/src/main/res/values/wear.xml`) et
  recoit les deux formes dans `WearSyncListener.kt`.

## Musique (lecture locale et tempo)

Le coeur Rust decide *quoi* ecouter et *a quel tempo* ; la montre ne joue que des
fichiers presents sur son disque et remonte l'etat. Contrat complet :
[docs/07](../docs/07-musique-bpm-et-playlists.md) (v2).

- **Aucun audio sur le serveur, aucun telechargement sur la montre.** Les MP3
  restent sur le disque de l'ordinateur et sont copies sur la montre par USB
  (`adb push`) avec l'outil PC `mpacer-music`, qui ecrit un `manifest.json` par
  playlist dans `context.getExternalFilesDir("Music")`
  (`/sdcard/Android/data/com.mpacer.watch/files/Music/<playlist_id>/`).
- **Import** : bouton « Importer (USB) » de `MusicScreen.kt`, qui relit ce dossier
  (`MusicLibrary.kt`), construit l'index `filesDir/music-index.json` et affiche le
  nombre de titres, l'espace utilise et l'espace libre. Une playlist importee peut
  etre supprimee depuis la montre.
- **Deezer est la source de metadonnees** (page `/music` du backend) : M-pacer ne
  pilote aucune application de lecture et ne lit aucun flux protege par DRM.
- **Pendant la seance**, `TrackingService` applique la directive du moteur
  (`Play`, `Keep`, `Boost`, `Relax`, `SkipTo`, `Pause`, `Resume`) sur les fichiers
  locaux et remonte la piste en cours (`music_now_playing`). Le BPM cible est
  affiche sous le panneau d'assistant, avec une fleche quand le moteur accelere ou
  calme la musique.
- **Compagnon** : l'onglet Musique est informatif (playlists du backend + rappel de
  la procedure USB), sans televersement.

## Suivi en direct (MQTT)

Pendant une seance, la montre peut publier sa position sur un broker MQTT : un
point toutes les dix secondes, environ 130 octets, ce qui permet de suivre la
trace en temps reel depuis la page `/live` du backend. Contrat complet, budget
de ressources et limites : [docs/10](../docs/10-suivi-temps-reel.md).

- **Desactive par defaut** : sans adresse de broker, `LiveTracker.start` ne cree
  ni fil ni connexion — le suivi ne coute rien tant qu'il n'est pas configure.
- **Reglages sur la montre** : Reglages ▸ **Broker MQTT** (`ui/LiveSettingsScreen.kt`)
  saisit au clavier l'adresse (`mqtt://hote:1883`), l'utilisateur, le mot de passe,
  le prefixe de sujet, le nom de la montre et les cadences ; « Coller l'adresse »
  reprend le presse-papiers et « Tester la connexion » verifie le brouillon avant
  enregistrement (`live/LiveProbe.kt` : CONNACK puis point de test sur
  `<prefixe>/test/<montre>`, hors du filtre `<prefixe>/live/+` du backend, donc
  invisible sur `/live`). Le mot de passe part dans
  `EncryptedSharedPreferences` ; les reglages s'appliquent a la seance suivante.
  L'ecran Reglages garde l'activation, la cadence et l'etat de la liaison.

  Le reglage par `adb` reste utile en test (il ecrase l'adresse enregistree) :

  ```powershell
  # Identifiants dans l'URL si le broker en demande (analyses par la montre) :
  adb shell am start -n com.mpacer.watch/.MainActivity `
    --es mqtt_url "mqtt://joseph:motdepasse@192.168.0.115:1883"
  ```

  Les valeurs locales (broker, identifiants) vivent dans un fichier `.env`
  **non versionne** — gabarit sans valeur : [`.env.example`](../.env.example).

- **Cadence** : 10 s en course, 60 s en pause (auto-pause comprise, l'etat vient
  du moteur) ; un point dont la precision GPS depasse 50 m n'est pas publie.
- **Budget** : ~55 ko/h de trafic, un fil en priorite basse, aucune minuterie,
  aucun wake lock, file de 16 paquets au maximum (les plus anciens sont jetes si
  le reseau tombe). L'ecran Reglages affiche l'etat de la liaison, les points
  publies et les points jetes.
- **Tests** (JVM, sans montre) :

  ```powershell
  cd android
  ./gradlew :core:testDebugUnitTest
  ```

### Montre de developpement : ecran toujours allume

Sur une montre posee sur son chargeur, Wear OS eteint l ecran et revient au cadran au
bout de quelques secondes, ce qui gene les tests (l application perd le premier plan et
les captures d ecran reviennent vides). Deux commandes suffisent, et le script les
applique :

```powershell
pwsh ./local-ci.ps1 -KeepAwake       # ecran jamais eteint + veille active sur chargeur
pwsh ./local-ci.ps1 -RestoreSleep    # retour a la veille normale (15 s)
```

Ce que fait `-KeepAwake` (sur la montre selectionnee, `-AdbSerial` si plusieurs) :

| Reglage | Valeur | Effet |
|---|---|---|
| `system screen_off_timeout` | `2147483647` | l ecran ne s eteint plus par inactivite |
| `global stay_on_while_plugged_in` | `7` (lu `15` sur Galaxy Watch) | reste eveille sur tous les types de chargeur |
| `svc power stayon true` | — | equivalent de l option developpeur « rester eveille » |

Verifie sur une Galaxy Watch 6 (SM-R915F, Android 16) : apres **90 secondes sans aucune
interaction**, `mWakefulness=Awake`, `Display State=ON` et l application reste au premier
plan.

> Le verrouillage de l ecran reste actif : si tu veux aussi l eviter pendant le dev,
> desactive-le depuis la montre (Parametres > Securite > Verrouillage de l ecran > Aucun)
> ou avec `adb shell locksettings set-disabled true` (remettre `false` ensuite).
> Attention : ces reglages vident la batterie, ils sont destines a une montre en charge.

### Montre de developpement : allegements effectues

La Galaxy Watch 6 de test a ete allegee (suppression pour l'utilisateur 0 : les paquets
restent dans la partition systeme et se reinstalle en une commande). Mesures : ~101 Mo de
swap liberes, memoire disponible passee de ~216 a ~280 Mo.

**Surcouches Samsung** (aucun usage sur une montre de developpement) :

| Paquet | Quoi |
|---|---|
| `com.samsung.android.samsungpay.gear` | Samsung Pay |
| `com.samsung.android.oneconnect` | SmartThings |
| `com.samsung.android.app.routines` | Routines |
| `com.samsung.android.bixby.agent`, `com.samsung.android.bixby.wakeup` | Bixby |
| `com.samsung.android.app.reminder` | Rappels |

**Applications sans usage sur une montre** :

| Paquet | Quoi |
|---|---|
| `com.microsoft.office.outlook` | Outlook |
| `com.caisseepargne.android.mobilebanking` | Banque |
| `ch.publisheria.bring` | Bring |
| `com.cardiogram.v1` | Cardiogram |
| `com.google.android.apps.fitness` | Google Fit |
| `com.samsung.android.service.health`, `com.samsung.android.shealthmonitor` | Samsung Health |
| `com.spotify.music` | Spotify |
| `com.cronometer.android.gold` | Cronometer |
| `com.whatsapp` | WhatsApp |
| `com.acmeaom.android.myradar` | myRadar |

**Conserves volontairement** : Strava, Gboard (clavier), Google Wallet, Agenda, Keep,
gestionnaire de mots de passe, Stocard, les cadrans de developpement (`com.example.*`).

Restaurer un paquet :

```powershell
adb shell cmd package install-existing com.whatsapp
```

> Le premier poste de consommation de la montre n'est pas l'application mais le **cadran**
> (27 a 33 % du CPU contre 3 % pour M-pacer). Pendant les tests, laisser M-pacer au premier
> plan evite ce rendu permanent ; le script `-KeepAwake` fait le reste.

### Piege des bibliotheques natives locales

`:app` n a aucune source native : les `.so` viennent tous de `:core` (coeur Rust et shim
JNI). Un dossier `app/src/main/jniLibs/` a longtemps traine, ignore par git et datant
d avant l extraction du socle. Il etait **fusionne en priorite** et masquait donc les
bibliotheques fraiches de `:core` : un symbole ajoute cote Rust manquait au chargement de
l APK, et l application mourait sur `UnsatisfiedLinkError` sans que rien ne le signale a la
compilation. Ce dossier est supprime, et `app/build.gradle.kts` refuse desormais de
compiler tant que des `.so` y traînent. En cas de doute sur une bibliotheque embarquee :

```powershell
# extraire le .so de l APK et verifier un symbole
& "$env:LOCALAPPDATA\Android\Sdk\ndk\27.2.12479018\toolchains\llvm\prebuilt\windows-x86_64\bin\llvm-nm.exe" `
  --dynamic --defined-only libmpacer_ffi.so | Select-String mpacer_report
```

### Contrainte de paquet et de signature

Le Data Layer Wear OS **route les messages par nom de paquet** et n accepte que des
applications **signees par la meme cle**. Les identifiants demandes sont conserves
(`com.mpacer.watch` / `com.mpacer.companion`, prefixe commun `com.mpacer`), mais pour un
envoi direct fiable il faut en pratique :

1. signer les deux modules avec la **meme cle** : renseigner `android/keystore.properties`
   (non versionne) ; le meme fichier est lu par les deux `build.gradle.kts` ;
2. si le routage par paquet ne fonctionne pas avec deux identifiants distincts, aligner
   les deux `applicationId` (par exemple `com.mpacer` + suffixe, ou un identifiant unique
   partage) : c est le seul point qui peut demander un ajustement produit.

Format de `android/keystore.properties` (a ajouter dans `.gitignore`, deja ignore ici) :

```properties
storeFile=mpacer-release.jks
storePassword=<mot de passe>
keyAlias=mpacer
keyPassword=<mot de passe>
```

## Installation / mise a jour de l application montre depuis le telephone

`WearSync.openWatchStore()` (bouton **Installer la montre**) ouvre la fiche Play Store de
`com.mpacer.watch` en visant d abord l application compagnon Wear OS
(`com.google.android.apps.wear.companion`, puis `com.google.android.wearable.app`), ce qui
installe ou met a jour l application sur la montre. Un repli documente permet de partager
un APK compile (`WearSync.shareApk`) pour un transfert manuel ou via `adb`.

Le manifeste telephone declare les `<queries>` necessaires (Android 11+) pour que ces
intents soient resolus.

## Regles du depot

- **Aucun calcul de course cote Kotlin.** Les allures, tours et estimations viennent de
  `mpacer-core` (montre) ou du backend, qui les tient du meme coeur.
- **Aucun secret en dur.** Le jeton vit dans `EncryptedSharedPreferences` ; l URL du
  backend est un reglage (valeur par defaut : `http://10.0.2.2:8080`, surchargeable).
  Sur la montre : `adb shell am start -n com.mpacer.watch/.MainActivity --es api_url http://<hote>:8080`.
- **Un seul endroit par version** : `gradle/libs.versions.toml`.

## Points a verifier

1. ~~**Compilation complete**~~ : **VERIFIE le 6 octobre 2026** avec
   `pwsh ./local-ci.ps1 -Target all` sur Windows (JDK = JBR d'Android Studio, SDK 35,
   NDK 27.2.12479018, cargo-ndk 4.1.2, Gradle 8.11.1) :
   `app-debug.apk` 36,4 Mo et `companion-debug.apk` 11,5 Mo, aucune erreur Kotlin.
   Les trois ABI embarquent bien `libmpacer_ffi.so` (coeur Rust) et `libmpacer_jni.so`.
   **Refait le 6 octobre 2026** apres extraction du socle `:core` et ajout du module
   `:phone` : les quatre modules compilent (`.gradlew.bat assembleDebug`),
   `app-debug.apk` 39 Mo, `phone-debug.apk` 19,5 Mo, `companion-debug.apk` 11,5 Mo,
   les services de seance et de lecture sont bien fusionnes dans les manifestes des
   trois applications, et les 27 tests JVM du socle passent
   (`.gradlew.bat :core:testDebugUnitTest`).
2. **Wrapper** : le `gradle-wrapper.jar` (Gradle 8.11.1) et les scripts `gradlew`/
   `gradlew.bat` proviennent du depot Gradle (tag `v8.11.1`) ; leur somme de controle
   n a pas ete comparee a `gradle-wrapper.jar.sha256`. Sous Linux/macOS, `chmod +x gradlew`.
3. ~~**Publication des versions**~~ : **VERIFIE** — AGP 8.7.3, Kotlin 2.1.0, Compose BOM
   2024.10.01, Wear Compose 1.4.0, OkHttp 4.12.0, kotlinx.serialization 1.7.3,
   `security-crypto` 1.1.0, `play-services-wearable` 18.2.0, `browser` 1.8.0 resolues
   par Gradle sans conflit.
4. ~~**CMake et JNI**~~ : **VERIFIE** — le shim `mpacer_jni.c` importe
   `libmpacer_ffi.so` depuis `src/main/jniLibs/<abi>/` et les deux bibliotheques sont
   empaquetees cote a cote. Attention : `RUST_LIB_DIR` est relatif au dossier `cpp/`
   (`../jniLibs`, et non `../../jniLibs`).
5. ~~**Cargo-ndk**~~ : **VERIFIE** — `local-ci.ps1` compile le coeur avec `cargo ndk`
   puis appelle Gradle avec `-Pmpacer.buildRust=false` : pas de double compilation.
6. **EncryptedSharedPreferences** : tester sur un appareil reel (Keystore) ; l API est
   marquee obsolete par AndroidX mais reste fonctionnelle et correspond a la demande.
7. **Data Layer** : verifier l appairage montre/telephone, la capacite `mpacer_sync`, la
   signature identique et le routage. Le point 2 de la section "Contrainte de paquet et
   de signature" peut necessiter d aligner les `applicationId`.
8. **Limite des 100 Ko** : `MessageClient` plafonne a ~100 Ko ; le basculement vers
   `DataClient`+`Asset` (seuil 90 Ko dans `WearSync.kt`) doit etre teste avec une trace
   GPS longue.
9. **Android 15 / edge-to-edge** : le module telephone utilise `safeDrawingPadding()` ;
   verifier l affichage avec `targetSdk = 35`.
10. **Permissions** : la montre demande `ACCESS_FINE_LOCATION` et `POST_NOTIFICATIONS` au
    premier lancement ; verifier le service de premier plan et l ecran de synchronisation
    sur une vraie montre ronde (le code est concu pour un ecran rond).
11. **Import `.pac`** : verifier la relecture d un fichier exporte par la montre
    (`format: "mpacer.pac"`, `version: 1`) et l envoi vers `POST /api/v1/workouts`.
12. **Suivi en direct** : verifier sur le terrain que la trace apparait sur `/live`
    et que le cout reste dans le bruit de mesure — comparer une seance d'une heure
    avec et sans `mqtt_url` (`adb shell dumpsys batterystats`, voir docs/10 § 5.3).
    Les identifiants du broker sont ranges dans `EncryptedSharedPreferences`, comme
    le jeton d'appairage.
13. **Menu MQTT sur la montre** : le clavier Wear, le collage depuis le
    presse-papiers et le bouton « Tester la connexion » se verifient sur une vraie
    montre ronde (`ui/LiveSettingsScreen.kt`, `live/LiveProbe.kt`) ; l'APK release
    est signe par `android/keystore.properties` (non versionne).
14. **Seance au telephone** : rien n'a encore ete couru avec l'application
    `:phone`. A verifier en priorite : le GPS continue d'arriver ecran eteint
    (service de premier plan de type `location`), la precision en ville, la
    consommation sur une heure, et le comportement d'Android 12+ pour le demarrage
    du service depuis la notification.
15. **Ceinture cardiaque Bluetooth LE** : la recherche (`hr/BleHeartRate.kt`)
    filtre sur le service Heart Rate `0x180D` ; la connexion GATT, les
    notifications `0x2A37` et la reconnexion sont a valider avec une ceinture
    reelle (Polar H10, Garmin HRM-Dual, Decathlon Dual). Permission Bluetooth
    refusee : la seance doit rester complete, sans cardio.
16. **Correction apportee aux deux applications** : l'assistant et la voix
    n'etaient transmis au moteur par aucun ecran ; ils passent desormais par
    `SessionConfig` (applique au depart de la seance). Verifier sur la montre
    qu'un mode « temps vise » change bien le panneau a la seance suivante.
17. **Reglages persistants du telephone** : `PhoneSettings.kt` ecrit dans les
    `SharedPreferences` a chaque modification (et `LiveSettings` dans le
    Keystore). Verifier la latence percue sur un appareil modeste.
18. **Amis et partage de position** : la revendication d'appareil part du service
    de seance (`LiveTracker.start` -> `FriendsClient.registerDevice`) ; verifier
    qu'un telephone et une montre publient bien sous deux noms distincts, qu'un
    nom deja pris est refuse (409) et que la carte OpenStreetMap s'affiche dans la
    WebView (tuiles accessibles depuis le telephone). Les tests d'integration du
    backend (`friends_share_a_live_position_between_two_accounts`) exigent un
    PostgreSQL : `MPACER_TEST_DATABASE_URL=... cargo test -p mpacer-api`.
19. **Musique du telephone** : `MediaStore` n'est pas utilise ; les fichiers
    doivent etre pousses par `adb` dans
    `Android/data/com.mpacer.phone/files/Music/`. Une lecture depuis la
    bibliotheque du telephone (sans BPM, donc sans choix de tempo) reste une
    evolution possible.
