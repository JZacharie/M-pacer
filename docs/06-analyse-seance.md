# 06 - Analyse d'une seance : plan de course, cardio, pauses

Ce document decrit les ecrans d'analyse ajoutes a l'interface web, ce qui a ete
repris des applications concurrentes, et les formules utilisees.

## 1. Ce que font les applications concurrentes

| Application | Ce qu'elle affiche sur l'ecran d'analyse |
|---|---|
| **Strava** | Resume persistant, carte, **splits** (survol = surbrillance sur le profil), graphe allure + altitude, **Best Efforts**, onglet **Heart Rate** (temps par zone FC), zones d'allure calculees sur l'allure ajustee a la pente |
| **Garmin Connect** | Onglets Stats / Splits / Time in Zones, **Time in Zone** sur la montre, **Training Effect** (aerobic + anaerobic, 0,0-5,0), **PacePro** (allure cible par split, avance/retard) |
| **Polar Flow** | Duree, distance, FC moy/max, **temps par zone FC**, allure moy/max, D+/D-, laps colores selon la zone, courbes configurables |
| **Coros** | Graphes allure / FC / D+ superposables, laps auto (1 km / 1 mi), seances structurees |
| **Runalyze / intervals.icu** | **Decouplage aerobie** (Pw:Hr), GAP, planifie contre realise avec calcul de conformite, TRIMP, VO2max estimee |

Sources : [Strava - pages d'activite](https://support.strava.com/en-us/articles/15401883-run-activity-pages),
[Strava - temps en mouvement](https://support.strava.com/hc/en-us/articles/115001188684-Moving-Time-Speed-and-Pace-Calculations),
[Strava - zones d'entrainement](https://support.strava.com/en-us/articles/15401569-training-zones-on-strava),
[Garmin - Training Effect](https://www8.garmin.com/manuals/webhelp/GUID-2492EDA1-1182-400B-8F43-D8B3211E4F3C/EN-GB/GUID-7275629E-743A-4658-A284-C84F42A66AE5.html),
[Garmin - zones FC](https://www8.garmin.com/manuals/webhelp/GUID-025D75CF-3445-49E1-8D81-1AA74AB4E00F/EN-GB/GUID-30C91919-943C-44E9-8048-901AC0881AEA.html),
[Garmin - PacePro](https://www8.garmin.com/manuals/webhelp/GUID-708A8F4D-9A78-49CF-9528-DE109BBCC472/EN-US/GUID-27B26831-3708-46EA-BF15-18039D28EC3A.html),
[Polar - resume de seance](https://support.polar.com/e_manuals/pacer/polar-pacer-user-manual-english/training-summary.htm),
[Runalyze - decouplage](https://runalyze.com/glossary/aerobic-decoupling?_locale=en),
[intervals.icu - decouplage course](https://forum.intervals.icu/t/decoupling-for-running/24856/21).

A retenir : l'ordre dominant est **resume -> graphe -> splits -> zones -> records**,
et trois notions reviennent partout : le **temps en mouvement** distinct du temps
ecoule, les **zones de frequence cardiaque**, et la **comparaison au plan**.

## 2. Les ecrans de la fiche de seance

`/workouts/{id}` presente, dans cet ordre :

1. **En-tete de resume** - distance, temps en mouvement, temps ecoule, allure
   moyenne, FC moyenne et maximale, denivele positif, temps de pause.
2. **Graphique multi-courbes** - allure, frequence cardiaque et altitude sur un
   meme axe de distance, chacune dans sa bande et sur sa propre echelle. Un
   repere vertical marque chaque kilometre. Le resume affiche aussi l'**allure
   ajustee a la pente (GAP)**, et le tableau des temps de passage une colonne GAP
   et une colonne pente (voir 3.7).
3. **Plan de course** (si la seance a suivi un plan) - cible, realise, ecart au
   finish, allure cible et allure realisee, negative split, puis un graphe de
   l'**ecart cumule au plan** tronçon par tronçon (barres vers le haut : en
   avance ; vers le bas : en retard).
4. **Frequence cardiaque** - barre de repartition et tableau par zone (plage en
   bpm, temps, pourcentage), puis la **derive cardiaque**.
5. **Temps de passage** - un tour par kilometre (ou mile) plus le dernier
   tronçon partiel : distance, temps, allure, ecart avec le tour precedent,
   FC moyenne, denivele, et ecart au plan.
6. **Chronologie** - temps de course, temps ecoule, temps de pause, liste des
   pauses (instant, distance, duree, manuelle ou automatique), puis les phases
   d'**acceleration** et la repartition du temps entre accelerer, allure stable
   et ralentir.
7. **Meilleures distances** - 1 km, 1 mi, 5 km, 10 km, semi.
8. **Carte de la trace** - fond OpenStreetMap dessine par le script du site
   (`static/map.js`), trace echantillonnee et reperes de distance.
9. **Profil altimetrique** - distance contre altitude, chaque portion colorée
   selon sa pente, avec infobulle ; les reglages **seuil** et **lissage**
   recalculent le denivele (voir [14 - Analyse de trace : VisuGPX](14-analyse-trace-visugpx.md)).

## 3. Les formules

### 3.1 Temps en mouvement et temps ecoule

- **temps de course (mouvement)** : le chrono n'avance qu'en etat `Running`.
  Les pauses manuelles et automatiques sont exclues de la duree, de l'allure et
  des tours.
- **temps ecoule** : du depart au dernier instant de la seance, pauses comprises.
- **temps de pause** : la somme des pauses enregistrees. Une pause encore
  ouverte a l'arret de la seance compte aussi.

C'est la convention de Strava et de Garmin : le mouvement fait reference, mais
l'ecoule reste visible pour situer une seance de competition.

### 3.2 Zones de frequence cardiaque

Cinq zones. Deux methodes :

- **pourcentage de la FC maximale** (defaut, methode de Strava) :
  `borne = fraction x FC max` ;
- **reserve de frequence cardiaque** (Karvonen, proposee par Garmin) :
  `borne = FC repos + fraction x (FC max - FC repos)`.

Fractions : 50-60, 60-70, 70-80, 80-90, 90-100 %. Le temps est attribue a la
zone de la mesure qui ouvre l'intervalle. Un trou de plus de 15 s entre deux
mesures ne compte pas : aucune valeur n'est inventee.

### 3.3 Derive cardiaque (decouplage aerobie)

Rendement = vitesse / frequence cardiaque, calcule sur chaque moitie de la
seance (coupee a la moitie de la distance) :

```text
decouplage % = (rendement 1re moitie - rendement 2e moitie) / rendement 1re moitie x 100
```

Positif = le coeur a derive (meme allure, frequence plus haute). Les valeurs
usuelles vont de 0 a 10 % ; au-dela, la fatigue, la chaleur ou la deshydratation
se sont fait sentir. Formule identique a celle de Runalyze et d'intervals.icu.

### 3.4 Temps de passage

Les durees viennent des tours enregistres (temps de course, pauses exclues) ; la
frequence cardiaque et le denivele sont releves sur la trace, dans la plage de
distance de chaque tronçon. Le plan donne, pour chaque tronçon, l'allure prevue
`A x (1 + r - 2 x r x f)` et le temps cumule `A x (d/1000) x (1 + r - r x d/D)`,
avec `A` l'allure moyenne, `r` le ratio de negative split et `f = d/D` la
fraction du parcours.

### 3.5 Acceleration

La vitesse est lissee sur 30 s (moyenne glissante centree), puis :

- **phases** : le depart, puis chaque reprise apres un trou de trace de plus de
  5 s. Pour chacune, le temps mis a atteindre 80 % de la vitesse moyenne
  (l'allure de croisiere) et la distance parcourue pendant la phase ;
- **repartition du temps** : accelerer, allure stable, ralentir, selon la pente
  de vitesse (seuil : 0,05 m/s par seconde).

### 3.6 Denivele

Deux mesures cohabitent :

- le **denivele brut**, somme des variations d'altitude tronçon par tronçon
  (colonne D+ du tableau des temps de passage) ;
- le **denivele filtre**, celui du resume et du profil : l'altitude est lissee
  par une moyenne glissante, puis un seuil (hysteresis) ignore les variations
  plus petites que la valeur choisie. Meme reglage que VisuGPX, avec un seuil de
  10 m et un lissage sur 5 points par defaut.

Le **denivele horaire** (m/h) ne retient que les portions ou la pente depasse
3 % sur au moins 200 m. Aucun denivele n'est affiche si la montre n'a pas
enregistre d'altitude.


### 3.7 Allure ajustee a la pente (GAP)

L'allure equivalente sur terrain plat, a effort egal. Le cout energetique de la
course selon la pente suit le modele de **Minetti et al. (2002)**, celui
qu'utilisent Runalyze et la plupart des calculateurs :

```text
Cr(i) = 155,4 i^5 - 30,4 i^4 - 43,3 i^3 + 46,3 i^2 + 19,5 i + 3,6   (J/kg/m)
```

avec `i` la pente (denivele / distance), bornee a +/-30 % pour resister au bruit
GPS. Pour chaque tronçon elementaire, la distance equivalente sur le plat vaut
`d x Cr(i) / Cr(0)` : l'allure ajustee est le temps total divise par cette
distance equivalente.

- En **montee**, `Cr(i) > Cr(0)` : la distance equivalente augmente, donc le GAP
  est **plus rapide** que l'allure reelle ;
- en **descente**, `Cr(i) < Cr(0)` : le GAP est **plus lent** (le modele passe par
  un minimum vers -20 %, puis remonte a cause du freinage).

Le GAP n'est affiche que si la montre a enregistre l'altitude : sur un profil
inconnu, il serait identique a l'allure reelle et n'apprendrait rien. C'est la
convention de [Strava](https://support.strava.com/en-us/articles/15402117-what-is-grade-adjusted-pace-gap-on-strava)
et de [Runalyze](https://runalyze.com/glossary/grade-adjusted-pace?_locale=en).

## 4. Le plan de course

Le plan est celui de l'assistant (`race_plan::RacePlan`) : distance, temps cible
et negative split. Il est **archive avec la seance** : une seance courue avec un
plan reste comparable des annees plus tard, meme si le plan a change depuis.

## 5. Le commentaire de course

Sous le resume de chaque seance, un champ libre permet d'ecrire ce qu'aucune courbe
ne dit : sensations, meteo, parcours, chaussures, compagnie. Le texte est enregistre
dans sa **propre colonne** (`workouts.comment`), pas dans le resume JSON envoye par
la montre : une nouvelle synchronisation de la meme seance remplace le resume mais
ne touche jamais au commentaire.

| Ou | Quoi |
|---|---|
| Fiche de seance (`/workouts/{id}`) | champ de saisie pre-rempli et bouton « Enregistrer le commentaire » |
| Historique (`/`) | le commentaire apparait en italique sous la seance |
| API (`PUT /api/v1/workouts/{id}/comment`) | corps JSON `{"comment": "..."}`, texte vide pour effacer |

Le commentaire est borne a 2 000 caracteres et nettoye a l'enregistrement (espaces
superflus retires, texte vide converti en `NULL`) : l'affichage distingue toujours
« pas de commentaire » de « commentaire vide ».

## 6. Ce qui n'est pas repris

| Ecran | Pourquoi |
|---|---|
| Training Effect / TRIMP | demande l'historique complet du coureur et un modele d'EPOC |
| Cadence, puissance | non mesurees par la montre visee |
| ~~Carte interactive~~ | **reprise depuis** : la carte de la trace est dessinee par `static/map.js`, le script maison sans dependance (docs/13), alimente par des coordonnees echantillonnees |
| Segments, classements | suppose un service centralise, contraire au choix auto-heberge |
