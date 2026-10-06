# Montre Garmin — application Connect IQ (Monkey C)

Cette partie du dépôt porte **M-pacer sur les montres Garmin** : un projet
[Connect IQ](https://developer.garmin.com/connect-iq/) écrit en Monkey C, à côté
de l'application Wear OS en Kotlin. Les deux montres partagent le **même cœur
métier** (allure lissée 2 minutes, tours, assistant, shadow runner, meilleures
distances, pauses, cardio) et le **même backend** : une séance envoyée par une
Garmin arrive exactement dans le même format qu'une séance Wear OS.

| | Montre Wear OS (`android/app`) | Montre Garmin (`garmin/`) |
|---|---|---|
| Langage | Kotlin + cœur Rust (JNI) | Monkey C (Connect IQ) |
| Cœur métier | `mpacer-core` compilé en `.so` | portage fidèle, module par module |
| Enregistrement | service de premier plan, archive `.pac` | `ActivityRecording` (fichier FIT Garmin) |
| Retour au coureur | voix (TTS Android) | **vibrations + textes** (Garmin n'expose pas de TTS) |
| Musique | Media3 + fichiers copiés en USB | non porté (pas de lecture audio locale en Connect IQ) |
| Synchronisation | OkHttp + jeton d'appareil | `Communications.makeWebRequest` + même jeton |
| Réglages | écran Réglages Compose | `settings.xml` (Garmin Connect Mobile) |

## 1. Ce que fait l'application

- **Allure lissée sur 2 minutes**, distance, temps de course, voyant GPS
  (rouge / orange / jaune / vert) — les mêmes seuils et le même algorithme que la
  montre Wear OS, avec la détection de changement d'allure en option.
- **Tours** kilomètre ou mile, automatiques, plus un tour manuel au bouton LAP ;
  le tour est ajouté à l'écran **et** au fichier FIT.
- **Assistant** : allure seule, temps de finish estimé, ou temps visé avec
  **shadow runner** (negative split) et écart temps / distance.
- **Cardio** : fréquence cardiaque et zone (1 à 5) sur la ceinture ou le capteur
  du poignet.
- **Alertes** : départ, pause, reprise, arrêt, tour franchi, annonce périodique
  (1, 2, 5 minutes ou à chaque tour) — vibration et texte à l'écran.
- **Enregistrement FIT** : la séance apparaît dans Garmin Connect comme
  n'importe quelle activité, avec la trace GPS complète.
- **Synchronisation** : appairage par code (device flow RFC 8628) puis envoi des
  séances sur `POST /api/v1/workouts` du backend auto-hébergé, avec la même
  structure JSON que le fichier `.pac`.

## 2. Prérequis

- **Un SDK Connect IQ** — le plus simple :
  [SDK Manager](https://developer.garmin.com/connect-iq/sdk/) (compte Garmin,
  licence à accepter), onglets **SDK** *et* **Devices**.
  `garmin/build.ps1` sait aussi télécharger l'archive du SDK tout seul
  (`developer.garmin.com/downloads/connect-iq/sdks/sdks.json`).
- **Un JRE/JDK 11 ou plus** (monkeyc est un programme Java). Le JBR livré avec
  Android Studio convient — le script le détecte.
- **PowerShell 7** (`pwsh`).
- Une montre Garmin compatible Connect IQ 4.0+ (fēnix 7 et suivantes,
  Forerunner 165/255/265/955/965/570/970, Venu 2/3/4, vívoactive 5/6,
  epix 2, Instinct 3, Enduro 3…). La liste des appareils ciblés est dans
  `manifest.xml`.

> **Les profils d'appareils ne sont pas dans l'archive du SDK** : c'est le SDK
> Manager qui les installe (`%APPDATA%\Garmin\ConnectIQ\Devices`), et ils
> demandent un compte Garmin. Sans profil, la compilation reste possible et
> valide la syntaxe, les types et les ressources, mais elle ne produit pas
> d'exécutable destiné à une montre précise.

## 3. Construire

```powershell
# Pré-vol : XML, cohérence des réglages, chaînes, compilation (sans appareil)
pwsh ./garmin/build.ps1 -Check

# Exécutable pour une montre précise (nécessite son profil d'appareil)
pwsh ./garmin/build.ps1 -Device fr965

# Version release, ou paquet signé pour la Connect IQ Store
pwsh ./garmin/build.ps1 -Device fr965 -Release
pwsh ./garmin/build.ps1 -Package

# Tests unitaires Monkey C (compilation, puis exécution dans le simulateur)
pwsh ./garmin/build.ps1 -Test -Device fr965

# Simulateur Connect IQ
pwsh ./garmin/build.ps1 -Device fr965 -Run

# Copie dans GARMIN/APPS de la montre branchée en USB
pwsh ./garmin/build.ps1 -Device fr965 -Install
pwsh ./garmin/build.ps1 -Device fr965 -Install -WatchPath E:\
```

Résultat : `garmin/build/mpacer-<appareil>.prg` (ou `mpacer.iq` avec
`-Package`, `mpacer-tests.prg` avec `-Test`). La clé de signature `garmin/developer_key.der` est générée au
premier build (RSA 4096, PKCS#8) et **n'est pas versionnée** : conservez-la, elle
signe les mises à jour de l'application.

### Ajouter une montre au manifeste

```powershell
# Aligne <iq:products> sur les appareils installés sur ce poste
pwsh ./garmin/tools/sync-products.ps1

# Ajoute ou retire un identifiant à la main
pwsh ./garmin/tools/sync-products.ps1 -Add fenix847mm
pwsh ./garmin/tools/sync-products.ps1 -Remove fr255s -DryRun
```

Les identifiants sont ceux des dossiers du SDK (`Devices/<id>`) et de la
[Device Reference](https://developer.garmin.com/connect-iq/device-reference/) :
`fenix7`, `fenix847mm`, `fr965`, `venu3`, `vivoactive5`, `enduro3`…

### Installer sur la montre sans câble

Publiez le `.iq` (`-Package`) sur la Connect IQ Store, ou copiez le `.prg`
dans `GARMIN/APPS` de la montre (mode stockage de masse / MTP), puis débranchez :
la montre installe l'application à la prochaine ouverture du menu.

## 4. Utilisation sur la montre

| Bouton | Écran course | Autres écrans |
|---|---|---|
| **START** | démarrer, mettre en pause, reprendre | — |
| **LAP** | tour manuel | — |
| **haut / bas** | changer d'écran (course → sync → réglages) | idem |
| **MENU** | annonce immédiate (allure, distance, temps, écart au plan) | appairer / envoyer les séances |
| **retour** | arrêter la séance (appui quand la séance tourne) | revenir à l'écran course |

À la fin de la séance, M-pacer enregistre le FIT **et** envoie la séance si la
montre est appairée. Sinon, elle reste dans l'archive locale : courir ne dépend
jamais du réseau.

## 5. Appairer la montre

1. Écran **Sync** (bouton bas) → **MENU** sur « appairer ».
2. La montre affiche un code court du type `BCDF-GHJK` et l'adresse du site.
3. Sur le site, ouvrez la page **/link**, saisissez le code et approuvez.
4. La montre récupère un jeton d'appareil et l'enregistre ; l'écran Sync affiche
   alors le nombre de séances en attente.
5. Le jeton est révocable à tout moment depuis la page **Jetons** du site.

L'adresse du backend se règle dans **Garmin Connect Mobile → Appareils →
M-pacer → Réglages** (`backendUrl`). Le jeton n'est pas chiffré : Connect IQ
n'offre ni Keystore ni stockage chiffré, contrairement à la montre Wear OS.

## 6. Réglages

Ils se modifient depuis Garmin Connect Mobile (ou l'écran de réglages de la
montre quand le modèle le permet). `resources/settings/settings.xml` déclare
les champs, `resources/settings/properties.xml` les valeurs par défaut, et le
code relit tout au démarrage.

| Réglage | Effet |
|---|---|
| `assistantMode` | allure seule / temps de finish estimé / temps visé |
| `units` | kilomètres ou miles |
| `detectPaceChange` | redémarrer la fenêtre d'allure quand le rythme change (fractionné) |
| `autoPause` | pause automatique à l'arrêt, reprise au mouvement |
| `alertsEnabled`, `alertFrequency`, `extendedLapInfo` | vibrations et annonces |
| `plannedDistanceKm`, `plannedTimeMin`, `negativeSplit`, `negativeSplitRatio` | plan de course et shadow runner |
| `maxHeartRate` | référence des zones de fréquence cardiaque |
| `backendUrl` | adresse du backend auto-hébergé |
| `syncTrace`, `traceIntervalS` | trace GPS envoyée au backend (débit et volume) |

## 7. Architecture

```
garmin/
  manifest.xml                  application, produits, permissions
  monkey.jungle                 project.manifest + chemins source/ressources
  resources/
    strings/strings.xml         tous les textes (UTF-8, accents)
    drawables/                  icône du lanceur
    settings/settings.xml       réglages visibles dans Garmin Connect
    settings/properties.xml     valeurs par défaut
  tests.jungle                  jungle de test (-t), ajoute test/
  test/
    MpacerCoreTests.mc          tests unitaires des formules (valeurs du cœur Rust)
  source/
    MpacerUnits.mc              unités, allure, formatage        <- units.rs
    MpacerGeo.mc                distance Haversine               <- geo.rs
    MpacerGpsMonitor.mc         qualité GPS, filtre anti-saut     <- gps.rs
    MpacerPaceEngine.mc         allure lissée 2 min, détection   <- pace.rs
    MpacerLapTracker.mc         tours km/mi, tour manuel         <- lap.rs
    MpacerWorkout.mc            machine à états, auto-pause      <- workout.rs
    MpacerRacePlan.mc           plan, negative split, shadow     <- race_plan.rs
    MpacerAssistant.mc          les modes de l'assistant         <- assistant.rs
    MpacerTrack.mc              trace en mémoire, meilleures distances <- best_distances.rs
    MpacerCardio.mc             zones de fréquence cardiaque     <- cardio.rs
    MpacerCoach.mc              quand et quoi annoncer           <- voice.rs
    MpacerEngine.mc             orchestrateur                    <- engine.rs
    MpacerText.mc               accès aux ressources de chaînes
    MpacerSettings.mc           lecture des réglages
    MpacerArchive.mc            archive locale (Storage)
    MpacerSync.mc               appairage et envoi des séances
    MpacerApp.mc                capteurs, FIT, vibrations, écrans  <- shell Kotlin
    MpacerView.mc               écran rond
    MpacerDelegate.mc           boutons
  tools/
    make_icon.py                génère l'icône du lanceur
    sync-products.ps1           aligne <iq:products> sur le SDK installé
  build.ps1                     SDK, clé, compilation, simulateur, copie USB
  preflight.ps1                 contrôle de cohérence + compilation
```

Règle du dépôt, conservée : **aucun calcul de course dans le shell**. Le shell
Garmin collecte (`Position`, `Sensor`) et affiche ; les formules vivent dans les
modules `Mpacer*`, portage direct du cœur Rust.

## 8. Ce qui n'est pas porté (et pourquoi)

| Fonction Wear OS | Sur Garmin | Raison |
|---|---|---|
| Voix (TTS) | vibrations + texte | Connect IQ n'expose aucune synthèse vocale |
| Musique locale, BPM, Boost/Relax | absent | pas de lecture de fichier audio pour une watch-app |
| Course à distance (adversaire en ligne) | absent | suppose un serveur et un classement temps réel |
| Tableaux de bord web | inchangés | ils restent dans l'interface web du backend |
| Trace GPS complète vers le backend | trace sous-échantillonnée | le BLE plafonne la taille des requêtes ; la trace complète vit dans le FIT Garmin |
| Pause de l'enregistrement FIT | non propagée | `ActivityRecording` n'expose pas de commande de pause pour l'application |

### Tests unitaires

`garmin/test/MpacerCoreTests.mc` reprend les **valeurs attendues des tests du
cœur Rust** : formatage de durée et d'allure, Haversine, feu GPS, allure lissée
sur 2 minutes, tours au kilomètre, plan exact à l'arrivée, projection de finish,
meilleur kilomètre, zones de fréquence cardiaque. Ils se compilent avec
`-Test` et s'exécutent dans le simulateur (`monkeydo -t`) dès qu'un profil
d'appareil est installé.

## 9. Vérification

- **Compilation vérifiée** avec le SDK Connect IQ **9.2.0** (`monkeyc -l 2`) :
  `BUILD SUCCESSFUL` pour l'application **et** pour les tests unitaires,
  ressources (chaînes, icône, réglages) comprises.
- `garmin/preflight.ps1` rejoue ces contrôles ; la CI les exécute à chaque
  `push`.
- **Reste à faire** : lancement dans le simulateur avec un profil d'appareil,
  validation terrain (écart d'allure < 3 % avec une montre de référence,
  batterie < 25 %/h) et publication sur la Connect IQ Store.

## 10. Dépannage

| Symptôme | Piste |
|---|---|
| `Aucun SDK Connect IQ trouve` | installez le SDK Manager, ou laissez le script télécharger l'archive (`-SdkVersion 9.2.0`) |
| `Appareil inconnu : <id>` | le profil n'est pas installé : SDK Manager → onglet **Devices** |
| « Invalid device id » (avertissement) | appareil listé dans `manifest.xml` mais absent du poste : `tools/sync-products.ps1` |
| `La compilation a echoue` | relancez avec `-Warnings` pour voir le détail, ou ouvrez le projet dans VS Code + extension Connect IQ |
| Le `.prg` n'apparaît pas sur la montre | vérifiez `GARMIN/APPS`, la casse du nom, et que l'appareil ciblé est bien votre modèle |
| Rien ne s'envoie | vérifiez `backendUrl` (HTTPS conseillé), l'appairage, et que le téléphone est appairé à la montre (les requêtes passent par Garmin Connect Mobile) |
| Les réglages ne changent rien | Garmin Connect Mobile → Appareils → M-pacer → Réglages, puis rouvrez l'application |