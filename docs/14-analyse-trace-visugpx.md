# 14 - Analyse de trace : ce que fait VisuGPX, ce que M-pacer en reprend

VisuGPX (<https://www.visugpx.com/>) est un service francais d'affichage et
d'analyse de traces GPS : on y decharge un `.gpx`, un `.fit` ou un `.tcx`, et la
page publique affiche la trace sur une carte, un profil altimetrique interactif
et une fiche de statistiques. La page analysee est celle d'un marathon
(<https://www.visugpx.com/jyq1OEbKRt>, trace Polar M400 de 42,44 km, 15 282 points),
relevee le 6 octobre 2026.

Ce document inventorie ce que fait cette page, ce que M-pacer faisait deja, ce
qui a ete repris ici, et ce qui est volontairement laisse de cote.

- Code : `crates/mpacer-core/src/analysis.rs` (denivele, denivele horaire,
  vitesse maximale, pauses par tour), `crates/mpacer-core/src/gpx.rs` (KML),
  `crates/mpacer-api/src/routes/web.rs` (fiche de seance), 
  `crates/mpacer-api/static/map.js` (carte)
- Ecran d'analyse existant : [06 - Analyse d'une seance](06-analyse-seance.md)
- Carte sans dependance : [13 - Amis et partage de la position](13-amis-partage-position.md)

---

## 1. Inventaire de la page VisuGPX

| # | Fonctionnalite | Comportement observe | Verdict M-pacer |
|---|---|---|---|
| V1 | Depot d'un fichier | `.gpx`, `.fit` ou `.tcx`, par formulaire, glisser-deposer ou URL ; type d'activite choisi | Deja la : la montre enregistre, l'import de courses lit GPX et TCX |
| V2 | Carte interactive | trace sur fond OpenStreetMap, reperes de lieux (POI) | **Repris** : carte de la trace sur la fiche de seance |
| V3 | Profil altimetrique interactif | courbe distance/altitude, curseur, deplacable sur la carte | **Repris** : profil colore par pente, infobulle au survol |
| V4 | Reglages du denivele | seuil (defaut 10 m) et lissage (defaut 5 points) modifiables | **Repris** : recalcul depuis la fiche de seance |
| V5 | Resume | distance, D+/D-, duree, heure de depart et d'arrivee | Deja la, complete par l'altitude min/max/moyenne |
| V6 | Vitesse moyenne sans les pauses | allure de deplacement, pauses exclues | Deja la (temps en mouvement) |
| V7 | Tableau par kilometre | km, temps, vitesse, **pause** | Colonne **pause** ajoutee |
| V8 | Vitesse maximale | valeur lissee et **kilometre** ou elle a ete atteinte | **Repris** |
| V9 | Denivele horaire + / - | montees (et descentes) de plus de 3 % sur au moins 200 m, en m/h | **Repris** |
| V10 | Altitudes | maximum, minimum, moyenne | **Repris** |
| V11 | Export | GPX **et KML** | **Repris** : KML ajoute |
| V12 | Trace en 3D | page dediee | Non repris : bibliotheque graphique tierce |
| V13 | Impression | page dediee | Non repris : le navigateur imprime la fiche |
| V14 | Suivi sur le terrain | position en direct pendant la sortie | Deja la : `/live` et le cercle d'amis (docs/10 et 13) |
| V15 | Partage, commentaires | lien court, commentaires publics, compteurs de consultation | Non repris : M-pacer est mono-utilisateur, le commentaire de seance personnel existe deja |
| V16 | Indice IBP | effort global calcule par un service tiers | Non repris : dependance externe |
| V17 | Meteo au depart, itineraires voisins | liens vers meteo et suggestions locales | Non repris : dependance externe |
| V18 | Edition de la trace | renvoi vers EditGPX | Non repris : hors perimetre |

Sources : page publique VisuGPX citee ci-dessus (en-tete, blocs `profil2_host`
et `stats2_host`, menus d'actions) ; uPlot et Leaflet sont les bibliotheques
utilisees par le site, M-pacer n'en a aucune.

---

## 2. Ce qui a ete repris

### 2.1 Carte de la trace (V2)

La fiche `/workouts/{id}` affiche desormais la trace sur le fond
OpenStreetMap deja utilise par la page `/amis` (`static/map.js`). Le serveur
n'envoie que des coordonnees : la trace est echantillonnee a 600 points
maximum, et un repere est pose tous les kilometres (tous les 5 km au-dela de
25 km, pour que les etiquettes ne se recouvrent pas), avec le depart et
l'arrivee. Aucun script tiers, aucune donnee envoyee ailleurs.

### 2.2 Profil altimetrique colore (V3, V4)

Le profil est dessine en SVG, portion par portion, chaque portion portant la
couleur de sa pente :

| Pente | Couleur |
|---|---|
| descente de plus de 6 % | bleu fonce |
| descente de 2 a 6 % | bleu clair |
| plat (moins de 2 %) | gris |
| montee de 2 a 6 % | orange |
| montee de plus de 6 % | rouge |

Le survol d'une portion affiche la distance, la pente et l'altitude
(`<title>` SVG : aucune bibliotheque, aucun script). Deux champs permettent de
recalculer le denivele depuis la page, comme les reglages de VisuGPX :

```text
GET /workouts/{id}?seuil=10&lissage=5
```

`seuil` est borne a 0-100 m et `lissage` a 1-51 points.

### 2.3 Denivele robuste (V4, V5, V10)

Le denivele brut — somme de toutes les variations d'altitude — surestime le D+
des que l'altimetre bruit : sur la trace de test (6 km a 2 %, bruit de +/-2 m),
il annonce plus de 2 000 m de D+ pour 120 m reels. Deux garde-fous, ceux de
VisuGPX, sont appliques dans `mpacer_core::analysis::elevation_summary` :

1. **lissage** : moyenne glissante centree sur `lissage` points ;
2. **seuil** : hysteresis. On part de l'altitude de reference ; tant que l'ecart
   reste sous le seuil, rien n'est compte ; des que le seuil est franchi, on
   ajoute l'ecart et la reference suit. Une oscillation de +/-3 m autour d'une
   altitude ne compte donc rien, une montee de 25 m compte 25 m.

Le resume gagne les cartes **D+ / D-**, **altitude min / max** et
**altitude moyenne** ; le D+ affiche est celui des reglages choisis, et non plus
la somme brute.

### 2.4 Denivele horaire (V9)

`mpacer_core::analysis::climb_rates` reprend la definition de VisuGPX : une
portion compte si sa pente depasse **3 %** sur au moins **200 m**. La recherche
avance de fenetre de 200 m en fenetre de 200 m tant que le sens de la pente est
conserve, puis le denivele de la portion est divise par sa duree :

```text
denivele horaire (m/h) = denivele de la portion (m) / duree de la portion (h)
```

Le resultat apparait en deux cartes du resume (D+/h et D-/h) et en une phrase
sous le profil : les faux plats et le bruit ne comptent pas.

### 2.5 Vitesse maximale (V8)

La vitesse est lissee sur 5 s avant la recherche (`speed_extremes`), puis
replacee sur l'axe des distances : un point GPS isole ne devient jamais un
record. Le resume affiche la valeur et la distance (« 12,4 km/h au 26,70 km »),
dans l'unite du coureur.

### 2.6 Pause par tour (V7)

`analysis::splits` ajoute `pause_s` a chaque tronçon : le temps de pause
enregistre qui tombe dans l'intervalle de temps de course du tronçon. Une pause
a cheval sur deux kilometres est repartie entre les deux, et la somme des
tronçons reste egale au temps de pause total. La colonne n'apparait que si la
seance comporte une pause.

### 2.7 Export KML (V11)

`mpacer_core::gpx::export_kml` produit un KML 2.2 (LineString, altitude
absolue quand elle est connue), telechargeable depuis la fiche de seance
(`/workouts/{id}/kml`) comme depuis l'API
(`GET /api/v1/workouts/{id}/kml`). C'est le format que lisent Google Earth et
les applications de cartes hors ligne.

---

## 3. Ce qui n'est pas repris

| Fonctionnalite | Pourquoi |
|---|---|
| Indice IBP, meteo, itineraires voisins | services tiers : le service se veut auto-heberge et sans appel externe |
| Trace en 3D | demande une bibliotheque graphique (WebGL) que le site n'embarque pas |
| Page d'impression dediee | le navigateur imprime la fiche telle quelle |
| Commentaires publics, compteurs de consultation, partage social | M-pacer n'est pas un reseau : le commentaire de course reste prive, sur la seance |
| Depot `.fit`/`.tcx` sur la fiche de seance | la montre est la source de verite ; l'import GPX/TCX existe deja pour les courses |
| Edition de trace | hors perimetre : M-pacer controle l'allure, il n'edite pas d'itineraire |

---

## 4. Verifications

| Ce qui est verifie | Ou |
|---|---|
| Seuil et lissage du denivele (bruit, pic isole, absence d'altitude) | `analysis::tests` (cœur) |
| Denivele horaire (montee soutenue, plat ignore) | `analysis::tests` |
| Vitesse maximale lissee | `analysis::tests` |
| Pause par tour, pause partagee entre deux tours | `analysis::tests` |
| KML (coordonnees, altitude, echappement XML) | `gpx::tests` |
| Charge utile de la carte (echantillonnage, reperes kilometriques) | `routes::web::workout_web_tests` |
| Bornes des reglages seuil/lissage, classes de pente | `routes::web::workout_web_tests` |

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```
