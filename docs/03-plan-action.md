# 03 - Plan d'action pour le developpement

Ce document est le plan de travail : phases, taches, criteres d'acceptation, risques.
Il part de l'etat reel du depot : **le coeur Rust est fait et teste**, le shell montre
reste a ecrire.

---

## 0. Etat des lieux (ce qui est deja livre et verifie)

| Element | Etat | Preuve |
|---|---|---|
| Coeur de calcul (`mpacer-core`) | **Fait** | 71 tests unitaires verts |
| Pont Android (`mpacer-ffi`) | **Fait** | 6 tests verts, dont un aller-retour via la C ABI |
| Simulateur (`mpacer-sim`) | **Fait** | Seance de 3 km rejouee, GPX produit, negative split visible |
| Qualite | **Fait** | `cargo clippy --workspace --all-targets` sans avertissement, `cargo fmt` applique |
| Shell Wear OS (`android/`) | **Squelette non compile** | Ecrit ici, mais aucun JDK/SDK disponible dans cet environnement |

Fonctionnalites couvertes par le coeur, telles que testees :
allure lissee 2 min + detection de changement d'allure, allure du tour courant et du
tour precedent, feu de statut GPS (rouge/orange/jaune/vert), filtre anti-saut GPS,
machine a etats de seance (demarrage suspendu, pause, auto-pause, reprise, arret),
tours km/mile, 4 modes d'assistant, shadow runner + negative split, temps de finish
estime, meilleures distances (1/5/10 km, 1/5 mi), annonces vocales FR/EN,
historique + export/import `.pac` JSON, export GPX, course a distance (protocole et
classement, sans reseau), unites metrique/imperial + respect des unites de course.

## 1. Prerequis d'environnement (phase 0)

| Outil | Version cible | Pourquoi |
|---|---|---|
| JDK | 17+ | AGP 8.x |
| Android Studio | recente, avec SDK 35+ | Module Wear OS |
| Wear OS SDK / emulateur | Wear OS 4+ | Tests sans montre |
| Android NDK | r27+ | Compilation Rust -> `.so` |
| `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android` | - | Cibles Android |
| `cargo install cargo-ndk` | 4.x | Packaging des `.so` dans `jniLibs` |
| `cargo install cargo-apk` | **non** | Abandonne depuis 2023 |

**T0.1 - Projet montre vide + Rust branche** (0,5 j)
1. Creer un projet "Empty Wear App" (Compose for Wear OS) dans `android/`.
2. Ajouter le plugin Gradle `org.mozilla.rust-android-gradle` (ou la tache
   `cargo ndk` en pre-build) pointant sur `crates/mpacer-ffi`.
3. Compiler : `cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o android/app/src/main/jniLibs build --release -p mpacer-ffi`.
4. Dans l'app, appeler `mpacer_version()` et l'afficher a l'ecran.

*Acceptation* : l'APK debug s'installe sur l'emulateur montre et affiche la version
du coeur Rust. Tant que ce point n'est pas passe, rien d'autre ne doit etre commence :
c'est le risque technique le plus eleve du projet.

## 2. Phase 1 - MVP seance (le coeur utilisable)

Objectif : **courir 10 km avec la montre, sans telephone**, et obtenir les memes
chiffres qu'une montre de reference.

| Tache | Detail | Est. | Acceptation |
|---|---|---|---|
| T1.1 | `TrackingService` : service de premier plan type `location\|health`, notification permanente, wake lock ; boucle GPS 1 Hz -> `mpacer_command({"cmd":"gps",...})` | 2 j | La seance continue ecran eteint et app en arriere-plan |
| T1.2 | Ecran principal rond : allure courante (tres grand), distance, temps, feu GPS 4 couleurs | 2 j | Lisible d'un coup d'oeil, sans texte superflu |
| T1.3 | Boutons Start / Pause / Stop + appui long = demarrage suspendu + reprise au mouvement | 1 j | Tous les etats atteignables ; pas de double demarrage possible |
| T1.4 | Auto-pause (option) avec seuils reglables | 0,5 j | Pause sur place > 10 s, reprise apres 3 s de course |
| T1.5 | Tours km/mile : affichage du tour courant et du precedent | 1 j | Le tour precedent est exact au centieme |
| T1.6 | Mode ambiant / always-on : allure + distance, sans secondes | 1 j | Le systeme ne tue pas l'app, la batterie tient |
| T1.7 | Correction de la derive : comparer le total avec une montre de reference et ajuster les seuils | 1 j | Ecart de distance < 3 % sur 10 km, ecart d'allure < 5 s/km |

**Definition of Done de la phase** : trois sorties de 30 a 60 min validees sur route
(degagee et arborée), aucune perte de trace, batterie < 25 %/h, aucun crash.

## 3. Phase 2 - Assistant et voix (la valeur du produit)

| Tache | Detail | Est. | Acceptation |
|---|---|---|---|
| T2.1 | Ecran de reglages : mode d'assistant, distance, temps cible, negative split | 2 j | Reglages persistants entre deux lancements |
| T2.2 | Panneau d'assistant : temps de finish estime (mode *predict*) | 1 j | Coherent avec le calcul du coeur |
| T2.3 | Shadow runner : ecart temps + distance, code couleur avance/plan/retard | 1,5 j | Verifie sur un 10 km objectif 50 min |
| T2.4 | `VoiceCoach` Android : `TextToSpeech`, file d'attente, langue, voix hors ligne | 1,5 j | Aucune annonce perdue ni repetee |
| T2.5 | Focus audio : duck / pause / ignorer (trois strategies reprises de Pace Control) | 1 j | La musique reprend correctement son volume dans les trois cas |
| T2.6 | `MediaSession` : simple / double / triple clic -> pause, annonce, reset allure | 2 j | Fonctionne ecran eteint, dans les 2 s |
| T2.7 | Test terrain du negative split sur une course reelle ou une simulation | 0,5 j | Ecart final au plan < 1 % du temps cible |

**Definition of Done** : une seance de fractionne (6 x 1000 m) et un 10 km allure cible
sont realises avec uniquement la montre et les ecouteurs, sans jamais regarder l'ecran.

## 4. Phase 3 - Historique, integration, finition

| Tache | Detail | Est. | Acceptation |
|---|---|---|---|
| T3.1 | Persistance des seances (JSON `.pac` versionne, ou `rusqlite` si volume) | 1,5 j | Survit a un redemarrage et a une reinstallation (via export) |
| T3.2 | Ecran historique : liste, detail, tours, meilleures distances | 2 j | Fiches conformes au resume produit par le coeur |
| T3.3 | Export GPX (partage systeme) et import/export `.pac` | 1 j | Le GPX s'ouvre dans Strava/Garmin/Google Earth |
| T3.4 | Tuile et complication : demarrer une seance en un geste | 1,5 j | Visible sur le cadran, lance la seance |
| T3.5 | Unites et "respecter les unites de course" dans l'interface | 0,5 j | Bascule immediate sans casser la seance |
| T3.6 | Journal de support (option) : trace GPS et valeurs calculees | 0,5 j | Fichier exportable en cas de bug |
| T3.7 | Internationalisation FR/EN (l'anglais est deja rendu par le coeur) | 1 j | Aucune chaine en dur |

## 5. Phase 4 - Differenciation (optionnelle, a decider apres la phase 2)

| Sujet | Interet | Cout | Decision |
|---|---|---|---|
| Remote race (backend + WebSocket) | Fort pour la motivation, mais c'est un **service a exploiter** | 2-3 semaines + hebergement | Repousse : le protocole et le classement existent deja dans `remote_race.rs` |
| Cardio : zones, alertes FC, allure guidee par la FC | Naturel sur montre et absent de Pace Control | 1-2 semaines | Candidat n°1 pour se differencier |
| Allure ajustee a la pente (GAP) | Tres utile en trail | 1 semaine | Candidat n°2 (necessite barometre ou altitude GPS) |
| Seances structurees (echauffement / intervalles / retour au calme) | Remplace le fractionne "a la main" | 1 semaine | Candidat n°3, gros gain pour l'entrainement |
| Health Connect / export automatique | Evite de tout reimplementer cote analyse | 3-5 j | A faire des la phase 3 si possible |

## 6. Registre des risques

| Risque | Impact | Probabilite | Mitigation |
|---|---|---|---|
| Precision GPS de la montre insuffisante pour l'allure | Eleve (coeur du produit) | Moyenne | Fenetre de 2 min deja protectrice ; porte de plausibilite ; comparaison terrain systematique ; Kalman constant-velocity si besoin |
| Batterie : GPS + ecran + TTS | Eleve | Elevee | Mode ambiant obligatoire, GPS 1 Hz, aucune animation, TTS uniquement planifie par le moteur, pas de reseau en seance |
| Service de premier plan tue par le systeme | Eleve | Moyenne | Type `location\|health` correct, wake lock, notification permanente, test sur montre physique (l'emulateur ne reproduit pas la gestion memoire) |
| Boutons Bluetooth non captes ecran eteint | Moyen | Moyenne | `MediaSession` cote Kotlin (et non les evenements de l'Activity) |
| TTS hors ligne absent / voix absente | Moyen | Moyenne | Detection de la disponibilite, repli sur les sons + affichage, message d'aide a l'utilisateur |
| Politique Play Store sur les permissions de sante | Bloquant a la publication | Moyenne | Declaration "application de sante", minimisation des permissions, justification ecrite des acces |
| Fragmentation Wear OS (versions, ecrans) | Moyen | Elevee | Tester sur au moins un ecran rond et un ecran plus ancien ; ne pas dependre d'API recentes |
| Derive du perimetre (le devenir d'une app de suivi generaliste) | Eleve (produit) | Moyenne | Discipline revendiquee par Pace Control : allure, assistant, voix, historique. Tout le reste est en phase 4 |

## 7. Strategie de test

| Niveau | Moyen | Etat |
|---|---|---|
| Unitaire Rust (algorithmes P0) | `cargo test -p mpacer-core` | **Fait** : 71 tests |
| Integration du pont | `cargo test -p mpacer-ffi` (JSON -> C ABI -> JSON) | **Fait** : 6 tests |
| Rejeu de traces | `mpacer-sim` avec `--noise`, `--split`, `--gpx` | **Fait** (a enrichir avec des traces reelles) |
| Test de non-regression d'allure | Ajouter des traces GPX reelles comme fixtures : le resultat doit rester dans une tolerance | A faire (phase 1) |
| Integration Android | Tests instrumentes sur l'emulateur montre (service, JNI) | A faire (phase 0-1) |
| Terrain | 3 sorties de reference, comparaison avec une montre/telephone connu, mesure de batterie | A faire (phase 1) |
| Revue de configuration | Matrice des combinaisons (4 modes x 2 unites x options vocales) parcourue manuellement avant chaque publication | A faire (phase 3) |

Conventions de depot : `cargo fmt --all`, `cargo clippy --workspace --all-targets`
sans avertissement, une CI qui execute ces deux commandes plus `cargo test --workspace`.

## 8. Les cinq prochaines actions concretes

1. **Installer la chaine Android** (JDK 17, SDK 35+, NDK r27+, `cargo-ndk`, cibles rustup).
2. **T0.1** : projet Wear OS vide + `cargo ndk` + affichage de `mpacer_version()` sur la montre.
3. **T1.1/T1.2** : service de premier plan GPS + ecran principal branche sur `EngineOutput`.
4. **T1.7** : premiere sortie de 10 km avec comparaison a une montre de reference ; ajuster les seuils dans `GpsThresholds`/`PaceConfig`.
5. **T2.1/T2.3** : reglages + shadow runner, puis validation sur un 10 km a objectif de temps.

## 9. Estimation d'ensemble

| Phase | Contenu | Estimation (dev solo, a temps partiel) |
|---|---|---|
| 0 | Chaine d'outils + hello world Rust sur montre | 2-3 jours |
| 1 | MVP seance utilisable en course | 1,5-2 semaines |
| 2 | Assistant + voix + boutons | 1,5-2 semaines |
| 3 | Historique, exports, tuiles, finition | 1,5 semaine |
| 4 | Differenciation (cardio, pente, seances structurees) | 2-4 semaines, par lots |

Un MVP credibile en course ("je sors avec la montre et je controle mon allure") est
donc atteignable en **environ un mois de travail a temps partiel**, la partie la plus
risquee (algorithmes) etant deja faite et testee.
