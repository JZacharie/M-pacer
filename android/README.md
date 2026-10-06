# Android : montre Wear OS (`:app`) + application telephone (`:companion`)

Ce dossier contient **deux modules Gradle** qui partagent le meme coeur Rust et le
meme backend auto-heberge :

| Module | Identifiant | Role |
|---|---|---|
| `:app` | `com.mpacer.watch` | Application Wear OS : course (GPS, voix, tours), historique local, synchronisation des seances. |
| `:companion` | `com.mpacer.companion` | Application telephone (Android 8+) : connexion au backend, liste et detail des seances, import de fichier `.pac`/JSON, envoi vers la montre. |

> **Rien n a ete compile ni teste.** La machine de redaction ne dispose ni de JDK ni de
> SDK Android. Le code a ete relu et **toutes les versions de dependances ont ete
> verifiees** sur Google Maven et Maven Central, mais la compilation, la fusion des
> ressources et l execution restent a valider sur une machine equipee (voir
> [Points a verifier](#points-a-verifier)).

## Arborescence

```
android/
  gradlew, gradlew.bat, gradle/wrapper/     wrapper Gradle 8.11.1 (jar inclus)
  gradle/libs.versions.toml                  catalogue de versions unique
  settings.gradle.kts                        include(":app") + include(":companion")
  build.gradle.kts                           plugins declares en apply false
  gradle.properties                          reglages communs
  .gitignore                                 secrets et sorties locales
  app/                                       module montre
    build.gradle.kts                         Wear OS, cargo-ndk, ABI, signature
    proguard-rules.pro
    src/main/AndroidManifest.xml             permissions reseau + listener Data Layer
    src/main/cpp/CMakeLists.txt, mpacer_jni.c  shim JNI vers la C ABI Rust
    src/main/java/com/mpacer/watch/
      MpacerCore.kt        pont JNI + data classes d etat (existant)
      TrackingService.kt   service de premier plan GPS (existant)
      VoiceCoach.kt        TTS + focus audio (existant)
      WorkoutArchive.kt    historique local .pac (existant)
      MpacerFormat.kt      formatage (existant)
      SyncClient.kt        NOUVEAU : device flow, envoi des seances, jeton chiffre
      WearSyncListener.kt  NOUVEAU : reception des seances envoyees par le telephone
      MainActivity.kt      navigation course / reglages / synchronisation
      ui/MainScreen.kt, ui/SettingsScreen.kt  ecrans ronds (existants)
      ui/SyncScreen.kt     NOUVEAU : code d appairage, etat, seances en attente
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
      ui/                  Theme, Format, Login, WorkoutList, WorkoutDetail, Send
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
pwsh ./local-ci.ps1 -Target all            # montre + telephone
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
# equivalente manuelle de ce que fait la tache :
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o android/app/src/main/jniLibs build --release -p mpacer-ffi
```

Options utiles :

```powershell
./gradlew :app:assembleDebug -Pmpacer.buildRust=false      # reutilise les .so deja presents
./gradlew :app:assembleDebug -Pmpacer.cargoProfile=debug   # profil cargo debug
./gradlew :app:installDebug                                # installe sur la montre/emulateur
```

APK : `app/build/outputs/apk/debug/app-debug.apk`.

### 2. Telephone (`:companion`)

```powershell
./gradlew :companion:assembleDebug
./gradlew :companion:installDebug
```

APK : `companion/build/outputs/apk/debug/companion-debug.apk`.

### 3. Les deux modules

```powershell
./gradlew assembleDebug        # :app + :companion
```

Sans SDK Android ni cargo-ndk, **aucune de ces commandes n a ete executee ici**.

## Flux d appairage (RFC 8628 simplifie)

Le backend expose `POST /api/v1/device/code` puis `POST /api/v1/device/token`, et la
page `/link` permet de saisir le code utilisateur. Les deux modules suivent le meme flux.

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
