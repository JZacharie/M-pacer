---
layout: default
title: Lire l'analyse d'une séance
description: "Comprendre l'écran d'analyse M-pacer : résumé, courbes, plan contre réalise, zones cardiaques, dérive, temps de passage, pauses, accélération et export GPX."
permalink: /guide/apres-la-course/
---

<span class="eyebrow">Guide utilisateur</span>
# Lire l'analyse d'une séance

<p class="lead">
Tout est calculé à partir du résumé envoyé par l'appareil, en une seule passe sur la trace.
La page ne se contente pas de stocker : elle compare, découpe et explique. Voici comment la
lire dans l'ordre, du plus général au plus fin.
</p>

## Ouvrir une séance

<ol class="steps">
  <li>Connectez-vous : l'accueil montre les statistiques des 30 derniers jours et
  l'historique paginé.</li>
  <li>Cliquez sur une séance : la page <code>/workouts/&lt;id&gt;</code> s'ouvre.</li>
  <li>Le bouton de commentaire permet d'écrire un mot sur la séance, pour la retrouver plus
  tard.</li>
</ol>

## Le résumé

<div class="table-wrap">

| Indicateur | Ce qu'il raconte |
|---|---|
| Distance | Le total de la trace, après filtrage des points aberrants |
| Temps en mouvement / temps écoulé | La différence, ce sont les pauses : c'est le premier indice d'une sortie hachée |
| Allure moyenne | Distance divisée par le temps en mouvement |
| Fréquence cardiaque moyenne et maximale | Le coût réel de la séance |
| Dénivelé positif | Le relief cumule monte |
| Pauses | Nombre et durée cumulée |

</div>

## Les courbes

<p>
Un graphique SVG trace par le serveur (aucune librairie de graphiques, aucun script tiers)
superpose <strong>allure</strong>, <strong>fréquence cardiaque</strong> et
<strong>altitude</strong> en fonction de la distance. C'est la vue qui montre un départ trop
vite, une côte, ou une dérive sur la fin.
</p>

## Plan contre réalise

<p>
Quand la séance a été courue avec un plan, l'écran affiche la cible, le réalisé, l'écart a
l'arrivée, l'allure cible, l'allure réalisée et le pourcentage de negative split. Puis vient
la vue la plus utile : l'<strong>écart cumule kilomètre par kilomètre</strong>, en barres
vers le haut quand vous êtes en avance, vers le bas quand vous êtes en retard.
</p>

<div class="note">
<p><strong>Un seul kilomètre ne veut rien dire</strong> ; la forme de la courbe, si. Un
début en haut suivi d'une descente régulière, c'est le scénario classique du départ trop
vite — celui que le shadow runner sert précisément à éviter.</p>
</div>

## Fréquence cardiaque

<div class="table-wrap">

| Élément | Calcul |
|---|---|
| Cinq zones | En pourcentage de la fréquence cardiaque maximale, ou en réserve de FC si une FC de repos est renseignée |
| Temps par zone | Combien de temps vous avez réellement passé dans chaque intensité |
| Dérive cardiaque (découplage aérobique) | L'écart entre la première et la seconde moitié : le signe et l'amplitude disent si l'effort a été tenu a coût constant |

</div>

<p class="tiny">
Une dérive faible à allure constante est le signe d'une bonne endurance. Une dérive forte au
même allure veut souvent dire chaleur, déshydratation ou fatigue accumulée.
</p>

## Temps de passage

<p>
Un tableau par tour : distance, temps, allure, écart d'allure au tour précédent, et — selon
les données disponibles — fréquence cardiaque moyenne, dénivelé et écart au plan. Quand
l'altitude est exploitable, l'<strong>allure ajustée à la pente</strong> (GAP) et la
<em>distance équivalente à plat</em> permettent de comparer un parcours vallonné à une sortie
plate.
</p>

## Chronologie, pauses et accélération

<p>
Les pauses — automatiques ou manuelles — sont listées avec leur instant, leur distance et
leur durée, y compris quand la reprise a été automatique. L'analyse d'accélération donne
l'allure de croisière de référence et, pour chaque départ ou reprise, le temps mis pour
l'atteindre, puis la répartition du temps entre accélération, allure stable et
ralentissement.
</p>

## Meilleures distances

<p>
Les meilleurs 1, 5 et 10 km (et 1 et 5 miles) contenus dans la séance, <strong>ou qu'ils se
trouvent dans la trace</strong>. Un 10 km rapide peut très bien être cache au milieu d'une
sortie plus longue : c'est cette vue qui le retrouve.
</p>

## Sur le téléphone

<p>
L'onglet <strong>Historique</strong> de l'application Course reprend l'essentiel : liste des
séances, puis un détail avec <em>A retenir</em>, carte OpenStreetMap, fréquence cardiaque,
profil altimétrique, temps de passage, meilleures distances, et les actions
<em>Partager (.pac)</em> et <em>Supprimer</em>.
</p>

## Exporter, partager, supprimer

<div class="table-wrap">

| Action | Où | Résultat |
|---|---|---|
| Export GPX | Page de la séance | Fichier GPX 1.1 avec l'extension de fréquence cardiaque Garmin, à envoyer vers Strava, Garmin ou OpenRunner |
| Export KML | Page de la séance | Trace pour la cartographie |
| Export complet | <code>/api/v1/export</code> | Toutes vos données au format <code>.pac</code> |
| Commentaire | Page de la séance | Une note personnelle attachée à la séance |
| Suppression | Page de la séance | Définitive, après confirmation |

</div>

<div class="warn">
<p><strong>La suppression est définitive</strong> et ne se propage pas vers les appareils :
l'archive locale de la montre garde sa copie. Si vous voulez conserver la trace ailleurs,
exportez le GPX avant.</p>
</div>

<div class="grid">
  <a class="card" href="{{ '/guide/tableaux-de-bord/' | relative_url }}">
    <h3>Tableaux de bord</h3>
    <p>Composer ses propres écrans à partir des neuf widgets.</p>
  </a>
  <a class="card" href="{{ '/site-web/' | relative_url }}">
    <h3>Le site web, en détail</h3>
    <p>Les routes, les formules et l'API.</p>
  </a>
</div>
