# Exemple de pacer : 1 h à 4:00 min/km autour du parc de Parilly

But : disposer d'un cas concret, mesurable et reproductible pour tester le
**shadow runner** de la montre (mode *Shadow runner* = `AchievePlannedTime`,
distance cible + temps cible) sur un parcours réel, autour du **parc de Parilly**
(Saint-Priest / Bron / Vénissieux).

> **Nom du parc.** Il n'existe pas de « parc de Pailly » à Saint-Priest : le parc
> visé est le **parc de Parilly**, dont la limite nord-est borde Saint-Priest
> (boulevard de Parilly, avenue de la République). C'est le parc du
> « 5 & 10 km de Parilly ».

## 1. Le parcours

Le 5 & 10 km de Parilly (Comité départemental d'athlétisme du Rhône) se court sur
**une boucle de 5 km** : une fois pour le 5 km, deux fois pour le 10 km. Le tracé
publié sur la page *Parcours* du site de la course est un circuit fermé de
**4 999 m** en 187 points (45,7123–45,7222 N ; 4,8928–4,9079 E), décrit comme
« roulant et accessible ».

| Fichier | Contenu |
|---|---|
| `parilly-boucle-5km.gpx` | La boucle seule (4 999 m), horodatée à 4:00 min/km (20:00). |
| `pacer-1h-4min-km-15km.gpx` | 3 tours (14 997 m), horodatés à 4:00 min/km (1:00:00). |

Les deux fichiers sont au format GPX 1.1 de M-pacer (`<trkpt>` + `<time>`), donc
directement lisibles par Strava, Garmin ou Google Earth, et utilisables comme
trace de référence pour comparer une séance réelle.

## 2. L'exemple de pacer

**1 h à 4:00 min/km = 15,0 km = 3 tours de la boucle de Parilly.**

| Réglage | Valeur |
|---|---|
| Mode assistant | **Shadow runner** (`AchievePlannedTime`) |
| Distance cible | **15 000 m** |
| Temps cible | **3 600 s** (1:00:00) |
| Negative split | 0 % (allure régulière) ou 3 % |
| Unités | km |
| Voix | activée, annonce toutes les 5 min |

### 2.1 Allure régulière

Chaque kilomètre en 4:00, passage aux repères de la boucle :

| Repère | Distance | Temps |
|---|---|---|
| Fin du tour 1 | 5,00 km | 20:00 |
| Fin du tour 2 | 10,00 km | 40:00 |
| Arrivée | 15,00 km | 1:00:00 |

### 2.2 Negative split 3 % (option du cœur Rust)

Allure `a(f) = A × (1 + r − 2·r·f)` avec A = 4:00/km et r = 0,03 : on part à
4:07 et on finit à 3:53, pour le même temps final.

| km | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| temps | 4:07 | 4:06 | 4:05 | 4:04 | 4:03 | 4:02 | 4:01 | 4:00 | 3:59 | 3:58 | 3:57 | 3:56 | 3:55 | 3:54 | 3:53 |

Passages : 20:24 (5 km), 40:24 (10 km), 1:00:00 (15 km).

## 3. Ce que la montre doit afficher (simulation de référence)

Faute de pouvoir entrer ces valeurs sur la montre aujourd'hui (voir §4), le
comportement attendu est vérifiable avec le simulateur :

```bash
cargo run -q -p mpacer-sim -- --mode plan --distance 15000 --time 3600 --split 0    --minutes 70 --every 600 --seed 42
cargo run -q -p mpacer-sim -- --mode plan --distance 15000 --time 3600 --split 0.03 --minutes 70 --every 600 --seed 42
```

Sorties complètes conservées dans `simulation-allure-reguliere.txt` et
`simulation-negative-split-3.txt`. Extrait (allure régulière) :

```text
[VERT ] t= 20:00 dist=4.99 km allure= 4:00 tour= 4:00 | ecart plan -6 m
    🔊 Allure 4:00 par kilometers. Distance 4.99 km. Temps 20:00. Vous etes en retard de 0:02, soit 6 m.
[VERT ] t= 30:00 dist=7.50 km allure= 4:00 tour= 4:00 | ecart plan -2 m
    🔊 Allure 4:00 par kilometers. Distance 7.50 km. Temps 30:00. Vous etes sur le plan.
[VERT ] t=1:00:00 dist=14.99 km allure= 4:00 tour= 4:00 | ecart plan -8 m
    🔊 Allure 4:00 par kilometers. Distance 14.99 km. Temps 1:00:00. Vous etes en retard de 0:02, soit 8 m.
    🔊 Seance terminee. 15.00 km en 1:00:02.
```

À l'écran : pastille GPS **VERT**, allure 4:00, allure du tour 4:00, écart au
shadow runner dans la tolérance (« sur le plan » = moins de 5 m ou 2 s), et un
message vocal toutes les 5 minutes. À l'arrivée : 15,00 km, 1:00:0x, allure
moyenne 4:00, 15 tours de 1 km, meilleurs temps 5 km ≈ 19:58 et 10 km ≈ 39:59.

## 4. Ce qu'il manque pour le tester sur la montre aujourd'hui

L'écran **Réglages** de la montre
(`android/app/src/main/java/com/mpacer/watch/ui/SettingsScreen.kt`) ne propose que
le **mode**, les **unités** et la **voix**. Or `WatchSettings` porte bien
`raceDistanceM`, `plannedTimeS` et `negativeSplitRatio`
(`android/app/src/main/java/com/mpacer/watch/MainActivity.kt:90-92`) : aucun appel
ne les renseigne, donc `MpacerCore.setAssistant()` envoie `null` pour la distance
et le temps cible, et `RacePlan::is_valid()` refuse le plan. Le mode
« Shadow runner » reste donc inutilisable sur la montre en l'état (le téléphone ne
pousse, lui, que les séances déjà enregistrées via `/mpacer/workout`).

Le correctif minimal, dans `SettingsScreen.kt` : deux réglages par paliers
(distance 15 000 m, temps 3 600 s, negative split 0 ou 3 %) qui appellent
`setAssistant(ACHIEVE_PLANNED_TIME, 15_000.0, 3_600.0, ratio)` — de quoi rendre
cet exemple testable tel quel.

## 5. Références publiques (Strava et alentours)

- **La course et ses meneurs d'allure** — le 5 & 10 km de Parilly aligne cinq
  pacers : 40, 45, 50, 55 min et 1 h. Sur le 10 km, le pacer « 40 min » court
  exactement à **4:00 min/km** : c'est le pacer réel de référence pour cet
  exemple. (<https://5et10kmdeparilly.odoo.com/allure-meneurs-d-allure>)
- **Le tracé** — page *Parcours* de la course, qui renvoie vers le circuit
  calculitineraires.fr « 10km-Parilly25 » (187 points, 4 999 m) : <https://5et10kmdeparilly.odoo.com/parcours> et
  <https://www.calculitineraires.fr/index.php?id=1531021>.
- **Strava** — le segment « Zigzag Parilly » (Vénissieux) existe, mais Strava
  n'affiche ni segments ni activités sans session connectée : impossible de lire
  les séances d'un tiers (dont celles de Manon Gabarret) depuis l'extérieur.
  <https://www.strava.com/segments/18430206>
- **Manon Gabarret** — coureuse de trail du Rhône, suivie par Rate My Trail et
  ITRA sous le nom « Roche Gabarret Manon » : 25 km du Trail de la Pierre Plantée
  (Veyras) en 2:42:28, SaintéLyon 2024 (≈83 km), La 6000D 2025. Niveau « Expert B ».
  <https://fr.ratemytrail.com/runner/manon-roche-gabarret>
- **Boucles publiques du parc** — Komoot « Parilly loop from Saint-Priest » et
  AllTrails « Parc de Parilly » pour d'autres tracés de référence.

## 6. Sources

- 5 & 10 km de Parilly (CDA du Rhône) — parcours, meneurs d'allure, résultats :
  <https://5et10kmdeparilly.odoo.com/>
- Tracé « 10km-Parilly25 » (GAILLARD Olivier) : <https://www.calculitineraires.fr/index.php?id=1531021>
- Simulateur : `crates/mpacer-sim` (mode `plan`), cœur : `crates/mpacer-core/src/race_plan.rs`.
