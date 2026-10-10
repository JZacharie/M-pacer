---
layout: default
title: Lire l'analyse d'une seance
description: "Comprendre l'ecran d'analyse M-pacer : resume, courbes, plan contre realise, zones cardiaques, derive, temps de passage, pauses, acceleration et export GPX."
permalink: /guide/apres-la-course/
---

<span class="eyebrow">Guide utilisateur</span>
# Lire l'analyse d'une seance

<p class="lead">
Tout est calcule a partir du resume envoye par l'appareil, en une seule passe sur la trace.
La page ne se contente pas de stocker : elle compare, decoupe et explique. Voici comment la
lire dans l'ordre, du plus general au plus fin.
</p>

## Ouvrir une seance

<ol class="steps">
  <li>Connectez-vous : l'accueil montre les statistiques des 30 derniers jours et
  l'historique pagine.</li>
  <li>Cliquez sur une seance : la page <code>/workouts/&lt;id&gt;</code> s'ouvre.</li>
  <li>Le bouton de commentaire permet d'ecrire un mot sur la seance, pour la retrouver plus
  tard.</li>
</ol>

## Le resume

<div class="table-wrap">

| Indicateur | Ce qu'il raconte |
|---|---|
| Distance | Le total de la trace, apres filtrage des points aberrants |
| Temps en mouvement / temps ecoule | La difference, ce sont les pauses : c'est le premier indice d'une sortie hachee |
| Allure moyenne | Distance divisee par le temps en mouvement |
| Frequence cardiaque moyenne et maximale | Le cout reel de la seance |
| Denivele positif | Le relief cumule monte |
| Pauses | Nombre et duree cumulee |

</div>

## Les courbes

<p>
Un graphique SVG trace par le serveur (aucune librairie de graphiques, aucun script tiers)
superpose <strong>allure</strong>, <strong>frequence cardiaque</strong> et
<strong>altitude</strong> en fonction de la distance. C'est la vue qui montre un depart trop
vite, une cote, ou une derive sur la fin.
</p>

## Plan contre realise

<p>
Quand la seance a ete courue avec un plan, l'ecran affiche la cible, le realise, l'ecart a
l'arrivee, l'allure cible, l'allure realisee et le pourcentage de negative split. Puis vient
la vue la plus utile : l'<strong>ecart cumule kilometre par kilometre</strong>, en barres
vers le haut quand vous etes en avance, vers le bas quand vous etes en retard.
</p>

<div class="note">
<p><strong>Un seul kilometre ne veut rien dire</strong> ; la forme de la courbe, si. Un
debut en haut suivi d'une descente reguliere, c'est le scenario classique du depart trop
vite — celui que le shadow runner sert precisement a eviter.</p>
</div>

## Frequence cardiaque

<div class="table-wrap">

| Element | Calcul |
|---|---|
| Cinq zones | En pourcentage de la frequence cardiaque maximale, ou en reserve de FC si une FC de repos est renseignee |
| Temps par zone | Combien de temps vous avez reellement passe dans chaque intensite |
| Derive cardiaque (decouplage aerobique) | L'ecart entre la premiere et la seconde moitie : le signe et l'amplitude disent si l'effort a ete tenu a cout constant |

</div>

<p class="tiny">
Une derive faible a allure constante est le signe d'une bonne endurance. Une derive forte au
meme allure veut souvent dire chaleur, deshydratation ou fatigue accumulee.
</p>

## Temps de passage

<p>
Un tableau par tour : distance, temps, allure, ecart d'allure au tour precedent, et — selon
les donnees disponibles — frequence cardiaque moyenne, denivele et ecart au plan. Quand
l'altitude est exploitable, l'<strong>allure ajustee a la pente</strong> (GAP) et la
<em>distance equivalente a plat</em> permettent de comparer un parcours vallonne a une sortie
plate.
</p>

## Chronologie, pauses et acceleration

<p>
Les pauses — automatiques ou manuelles — sont listees avec leur instant, leur distance et
leur duree, y compris quand la reprise a ete automatique. L'analyse d'acceleration donne
l'allure de croisiere de reference et, pour chaque depart ou reprise, le temps mis pour
l'atteindre, puis la repartition du temps entre acceleration, allure stable et
ralentissement.
</p>

## Meilleures distances

<p>
Les meilleurs 1, 5 et 10 km (et 1 et 5 miles) contenus dans la seance, <strong>ou qu'ils se
trouvent dans la trace</strong>. Un 10 km rapide peut tres bien etre cache au milieu d'une
sortie plus longue : c'est cette vue qui le retrouve.
</p>

## Sur le telephone

<p>
L'onglet <strong>Historique</strong> de l'application Course reprend l'essentiel : liste des
seances, puis un detail avec <em>A retenir</em>, carte OpenStreetMap, frequence cardiaque,
profil altimetrique, temps de passage, meilleures distances, et les actions
<em>Partager (.pac)</em> et <em>Supprimer</em>.
</p>

## Exporter, partager, supprimer

<div class="table-wrap">

| Action | Ou | Resultat |
|---|---|---|
| Export GPX | Page de la seance | Fichier GPX 1.1 avec l'extension de frequence cardiaque Garmin, a envoyer vers Strava, Garmin ou OpenRunner |
| Export KML | Page de la seance | Trace pour la cartographie |
| Export complet | <code>/api/v1/export</code> | Toutes vos donnees au format <code>.pac</code> |
| Commentaire | Page de la seance | Une note personnelle attachee a la seance |
| Suppression | Page de la seance | Definitive, apres confirmation |

</div>

<div class="warn">
<p><strong>La suppression est definitive</strong> et ne se propage pas vers les appareils :
l'archive locale de la montre garde sa copie. Si vous voulez conserver la trace ailleurs,
exportez le GPX avant.</p>
</div>

<div class="grid">
  <a class="card" href="{{ '/guide/tableaux-de-bord/' | relative_url }}">
    <h3>Tableaux de bord</h3>
    <p>Composer ses propres ecrans a partir des neuf widgets.</p>
  </a>
  <a class="card" href="{{ '/site-web/' | relative_url }}">
    <h3>Le site web, en detail</h3>
    <p>Les routes, les formules et l'API.</p>
  </a>
</div>
