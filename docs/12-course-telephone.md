# 12 - Courir avec le telephone

Application Android de course autonome : le telephone remplace la montre. Meme
coeur Rust, meme seance, memes reglages ; seul le materiel change (capteurs,
ecran, chemin des fichiers musicaux).

- Code : [`android/phone/`](../android/phone/) (module Gradle `:phone`)
- Socle partage avec la montre : [`android/core/`](../android/core/) (module `:core`)
- Construction et installation : [`android/README.md`](../android/README.md)

---

## 1. Pourquoi une application distincte

La montre est le meilleur capteur de course, mais elle n'est pas toujours la :
batterie vide, poignet blesse, envie de tester une seance sans sortir la montre,
telephone deja en main pour la musique. L'application telephone repond a ce seul
besoin : **courir avec le telephone, avec exactement les memes fonctions**.

Le module `:companion` reste ce qu'il etait : l'application d'appoint qui liste
les seances du backend, importe un fichier `.pac` et pousse une seance vers la
montre par le Data Layer. Il ne fait pas de seance.

| Aspect | Montre (`:app`) | Telephone (`:phone`) |
|---|---|---|
| Calcul de la seance | `mpacer-core` (Rust, JNI) | `mpacer-core` (Rust, JNI) — **le meme** |
| Position | FusedLocationProvider 1 Hz | FusedLocationProvider 1 Hz |
| Frequence cardiaque | capteur integre (`TYPE_HEART_RATE`) | ceinture Bluetooth LE (service `0x180D`) |
| Voix | TextToSpeech + focus audio | identique (voix systeme du telephone) |
| Annonces et tours | ecran rond | grand ecran, tuiles |
| Archive locale | `filesDir/workouts/*.json` | identique |
| Synchronisation backend | device flow + `POST /api/v1/workouts` | identique |
| Suivi en direct MQTT | oui (opt-in) | identique (reglages au clavier) |
| Musique | fichiers copies par USB dans `Music/` | identique (+ lecture depuis le telephone) |
| Ecran allume pendant la seance | option de la montre | option `FLAG_KEEP_SCREEN_ON` |

## 2. Architecture

```text
android/
  core/    :core  (bibliotheque Android)
           MpacerCore (JNI) · TrackingService · VoiceCoach · WorkoutArchive
           SyncClient · live/ (MQTT) · music/ (Media3) · SessionConfig
             ^                                   ^
             |                                   |
  app/     :app  (Wear OS)              phone/  :phone  (telephone)
           ecrans ronds, Data Layer      ecrans Material 3, ceinture BLE
```

Le socle ne connait **aucune classe d'ecran** : le service de premier plan ouvre
l'activite que le systeme associe au paquet, et c'est l'application qui pose son
identite au demarrage (`SyncClient.pairingLabel`, `LiveSettings.deviceFallback`,
`HeartRateSources.external`). C'est ce qui permet aux deux applications de
partager la meme seance sans la dupliquer.

**Regle du depot respectee** : aucun calcul de course cote Kotlin. Allure, tours,
estimation de finish, shadow runner, zones cardiaques, plan de course et textes
des annonces viennent tous de `mpacer-core`.

## 3. Ecran de course

1. **Voyant GPS** (vert / orange / rouge) et precision annoncee en metres.
2. **Allure courante** en tres grand, avec l'allure du tour precedent.
3. **Tuiles** : distance, duree, cardio (avec zone), tour courant, allure du tour.
4. **Assistant** : temps de finish estime, ou ecart au shadow runner (« +120 m
   d'avance », « sur le plan ») et distance restante.
5. **Musique** : BPM consigne par le moteur, fleche de directive (`^` accelerer,
   `v` calmer, `>>` changer de piste) et titre en cours.
6. **Commandes** : *Demarrer la course*, *Preparer* (le chrono attend le premier
   pas), *Pause*, *Reprendre*, *Arreter*, plus *Annonce vocale* et *Fenetre
   d'allure* (les deux commandes qui n'etaient branchees sur aucun bouton).

La notification de seance porte **Pause/Reprendre** et **Arreter** : plus besoin
de sortir le telephone de sa ceinture. Le meme service sert la montre et le
telephone.

## 4. Permissions

| Permission | Pourquoi | Si refusee |
|---|---|---|
| `ACCESS_FINE_LOCATION` | position de la seance | la course ne peut pas demarrer |
| `FOREGROUND_SERVICE_LOCATION` | GPS ecran eteint | idem |
| `POST_NOTIFICATIONS` | notification de seance | la seance tourne, sans bandeau |
| `ACTIVITY_RECOGNITION` | detection d'activite | sans effet : l'auto-pause lit la vitesse |
| `BODY_SENSORS` | capteur integre eventuel | cardio absent de la seance |
| `BLUETOOTH_SCAN`, `BLUETOOTH_CONNECT` | ceinture cardiaque | cardio absent de la seance |
| `INTERNET` | synchronisation et MQTT | archive locale uniquement |

Aucune permission n'est obligatoire pour courir : la seance est complete sans
cardio, et l'archive reste sur le telephone si le backend n'est pas joignable.

## 5. Reglages

Les reglages du telephone sont **persistants** (`SharedPreferences`) : un
telephone se ferme et se rouvre sans arret, contrairement a la montre. Les
secrets, eux, restent dans le socle, chiffres par le Keystore
(`EncryptedSharedPreferences`) : mot de passe du broker MQTT et jeton
d'appairage.

- **Assistant** : allure seule, finish estime, temps vise (shadow runner) ou
  course a distance ; distance de course, temps vise, part negative.
- **Affichage** : km ou miles ; ecran allume pendant la seance.
- **Voix** : activation, frequence (1, 2, 5 min, chaque tour, manuel), langue
  (FR/EN), detail du tour, formes courtes, et ce que fait la musique quand la
  voix parle (baisser, mettre en pause, parler par-dessus).
- **Musique** : activation, annonces de tempo, BPM de reference.
- **Cardiaque** : frequence maximale de l'utilisateur (reference des zones du
  compte rendu), recherche des ceintures Bluetooth LE a portee (filtre sur le
  service Heart Rate), choix, retrait.
- **Suivi en direct** : active/desactive, adresse du broker, prefixe des sujets,
  nom de l'appareil, identifiants, cadence en course et en pause, conservation du
  dernier point, bouton **Tester la connexion** (CONNACK + point de test hors du
  sujet `<prefixe>/live/+`).
- **Backend** (onglet Sync) : adresse, appairage, envoi des seances.

## 6. Analyse d'une seance

L'onglet **Historique** ouvre chaque seance sur une fiche d'analyse : la meme
matiere que la page web `/workouts/{id}`, mais calculee **sur le telephone**,
sans reseau.

1. **A retenir** - deux a quatre observations redigees par le coeur : regularite
   de l'allure, negative split, derive cardiaque, temps passe en Z4-Z5, cout du
   relief, poids des pauses. Les seuils sont des constantes nommees et testees
   dans `mpacer-core`, pas des phrases dispersees dans l'interface.
2. **Carte** - la trace et un repere de distance sur un fond OpenStreetMap. Le
   script et la feuille de style sont ceux du site (`static/map.js`,
   `app.css`), copies dans les assets a la compilation : une seule
   implementation a corriger, et la carte s'affiche hors ligne (seules les tuiles
   demandent le reseau). Un lien ouvre la trace sur openstreetmap.org.
3. **Frequence cardiaque** - moyenne, maximale, minimale, courbe du pouls en
   fonction de la distance, temps par zone, derive cardiaque.
4. **Energie et terrain** - denivele, altitude maximale, **allure ajustee a la
   pente (GAP)**, distance equivalente sur le plat, profil altimetrique.
5. **Temps de passage** - un tour par kilometre, barre proportionnelle a la
   vitesse, pouls moyen du tour, ecart-type des allures, partage entre les deux
   moities, vitesse maximale.
6. **Meilleures distances**.

Le calcul traverse la C ABI du coeur : `mpacer_report` recoit la seance archivee
et les zones cardiaques, et renvoie un rapport JSON (`mpacer_core::report`).
Aucun algorithme de course n'est ecrit en Kotlin : la regle du depot vaut aussi
pour le telephone. Une seance illisible n'empeche pas la fiche de s'ouvrir, elle
affiche seulement ses totaux.

## 7. Reprise automatique des seances de la montre

Courir avec la montre puis ouvrir le telephone : la seance est deja la. Le
chainon manquant etait la **reprise** — la montre envoyait bien ses seances au
backend des la fin de la course, mais rien ne les ramenait sur le telephone.

- **Declencheur** : le retour de l'application au premier plan (lancement, retour
  d'arriere-plan) et l'ouverture de l'historique. Aucune minuterie, aucun service
  d'arriere-plan : un appareil non appaire ne fait aucune requete.
- **Ce qui est repris** : `GET /api/v1/workouts` donne la liste du compte,
  `GET /api/v1/workouts/{id}` le resume complet de chaque seance absente du
  telephone (trace GPS et frequence cardiaque comprises). Le fichier range
  localement est celui qu'a produit mpacer-core, sans transformation.
- **Idempotent** : une seance est identifiee par son horodatage de depart. Une
  seance deja presente n'est pas reprise deux fois, et une seance reprise est
  marquee « deja envoyee » — sinon les deux appareils se renverraient la meme
  seance sans fin.
- **Tolerant** : un echec isole n'interrompt pas le lot (trois echecs consecutifs
  arretent la reprise : le reseau est tombe). Le compte rendu affiche separe ce
  qui a ete repris de ce qui a echoue, et la tentative suivante reprend le reste.
- **Visible** : l'onglet Historique porte une carte « Seances de la montre » qui
  dit ou en est la reprise et propose **Chercher maintenant**.

La montre, elle, garde son propre envoi vers le backend en fin de seance. Le
lien direct montre <-> telephone par le Wear Data Layer n'est pas utilise ici :
il exige que les deux applications partagent le meme identifiant de paquet, ce
que le depot signale comme un choix produit (voir la section « Contrainte de
paquet et de signature » de `android/README.md`).

## 8. Cardio : ceinture Bluetooth LE

Les telephones n'ont pas de capteur cardio. L'application lit donc une ceinture
standard (Polar H10, Garmin HRM, Decathlon Dual...) : profil Heart Rate
(`0x180D`, mesure `0x2A37`), notifications GATT, reconnexion automatique par
Android.

Le socle ne connait que l'interface `HeartRateSource` : la seance demarre le
capteur integre **et** la source declaree par l'application. Chaque mesure part
telle quelle dans le moteur, qui la rattache a la seance (zones, derive
cardiaque, export GPX) — aucun calcul cote Kotlin, comme sur la montre.

## 9. Musique

Meme contrat que la montre (voir [07](07-musique-bpm-et-playlists.md) v2) : le
moteur decide *quoi* jouer et a *quel tempo*, l'application joue uniquement des
fichiers presents sur son disque, avec leur `manifest.json`.

```bash
# Depuis le PC, telephone branche en USB (debogage active) :
adb push ./run-170 /sdcard/Android/data/com.mpacer.phone/files/Music/
# Puis dans l'application : onglet Musique > « Importer (USB) »
```

Le dossier exact est affiche en haut de l'onglet Musique. Aucun flux, aucun DRM,
aucun appel reseau pendant la course. Lire la musique du telephone via
`MediaStore` (sans `manifest.json`, donc sans BPM) reste une evolution
possible : le moteur ne choisit alors aucune piste pour un changement de tempo.

## 10. Construction et installation

```powershell
# Depuis la racine du depot : coeur Rust + APK du telephone
pwsh ./local-ci.ps1 -Target phone

# Ou directement :
cd android
./gradlew.bat :phone:assembleDebug '-Pmpacer.buildRust=false'
adb install -r phone/build/outputs/apk/debug/phone-debug.apk
```

L'URL du backend peut etre inscrite dans l'APK
(`-Pmpacer.apiUrl=https://mpacer.p.zacharie.org`) ou saisie dans l'application.

## 11. Limites connues et verification a faire

1. **Aucun essai sur appareil reel** : le code compile et les tests unitaires du
   socle passent (27 tests JVM), mais la seance n'a pas encore ete courue avec un
   telephone. A verifier en priorite : GPS ecran eteint (service de premier plan
   de type `location`), precision en ville, duree de batterie sur 1 h.
2. **Ceinture Bluetooth LE** : la recherche filtre sur le service Heart Rate et
   la reconnexion est confiee a Android ; a valider avec une ceinture reelle
   (Polar H10, Garmin HRM-Dual).
3. **Android 14+** : le type de service de premier plan est `location`
   uniquement. Le type `health` de Wear OS n'est pas necessaire (aucun
   `ExerciseService`) et imposerait `BODY_SENSORS` au telephone.
4. **Musique** : le chemin `Android/data/.../files/Music` n'est accessible que
   par `adb` (Android 11+ masque ce dossier en MTP) ; c'est le meme choix que la
   montre, documente dans [07](07-musique-bpm-et-playlists.md).
5. **Partage `.pac`** : l'historique partage le JSON par `ACTION_SEND` ; un
   export de fichier (`FileProvider`) serait plus confortable pour de gros
   volumes.
6. **Fenetre d'allure et annonce vocale** : les deux commandes sont desormais
   branchees sur des boutons cote telephone ; sur la montre elles restent
   accessibles par les boutons du casque (non branches).
