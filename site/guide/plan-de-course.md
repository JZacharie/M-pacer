---
layout: default
title: Préparer un plan d'allure
description: Régler la distance, le temps visé et le negative split dans M-pacer, comprendre le shadow runner et lire l'écart au plan pendant la course.
permalink: /guide/plan-de-course/
---

<span class="eyebrow">Guide utilisateur</span>
# Préparer un plan d'allure

<p class="lead">
Un plan, c'est deux nombres — une distance et un temps — et une intention : partir
régulièrement, quitte à finir plus vite. M-pacer en fait un <em>coureur virtuel</em> qui
courra exactement votre plan, et vous dit à chaque instant si vous êtes devant ou derrière.
</p>

## Où régler le plan

<div class="table-wrap">

| Support | Chemin |
|---|---|
| Montre | Réglages &rsaquo; <strong>Assistant de course</strong> |
| Téléphone | Réglages &rsaquo; <strong>Assistant</strong> |

</div>

Trois champs suffisent :

<div class="table-wrap">

| Champ | Format | Exemple |
|---|---|---|
| Distance de course | kilomètres (ou miles selon les unités) | <code>10</code> |
| Temps visé | <code>h:mm:ss</code> ou <code>mm:ss</code> | <code>50:00</code> |
| Part négative | pourcentage | <code>3</code> % |

</div>

<p>
La <strong>part négative</strong> décrit l'écart entre la première et la seconde moitié :
0 % signifie une allure parfaitement constante ; 3 % signifie que la seconde moitié est
courue 3 % plus vite que la première. C'est le réglage classique d'un marathon ou d'un
semi où l'on veut se garder.
</p>

## Quelques objectifs courants

<div class="table-wrap">

| Course | Temps visé | Allure moyenne du plan |
|---|---|---|
| 5 km | 25:00 | 5:00 /km |
| 10 km | 50:00 | 5:00 /km |
| 10 km | 45:00 | 4:30 /km |
| Semi-marathon | 1:45:00 | 4:59 /km |
| Marathon | 3:30:00 | 4:59 /km |
| Marathon | 4:00:00 | 5:41 /km |

</div>

<p class="tiny">
Ce tableau ne sert qu'à vérifier que le temps saisi tient debout : la montre n'affiche
jamais l'allure moyenne du plan, elle affiche votre allure et l'écart au coureur virtuel.
</p>

## Comment le moteur s'en sert

<ol class="steps">
  <li><strong>Un plan exact, pas un tableau de passages.</strong> Le moteur construit la
  courbe de distance en fonction du temps, et sait l'inverser : a tout instant il connait la
  distance que le coureur virtuel <em>devrait</em> avoir parcourue. Le plan retombe sur
  l'arrivée à l'instant visé, vérifie par test à mieux que <code>1e-6</code> près.</li>
  <li><strong>Allure lissée sur deux minutes.</strong> Votre allure affichée est une moyenne
  glissante : elle ignore les soubresauts du GPS et les quelques secondes d'hésitation, ce
  qui evite de courir après le bruit.</li>
  <li><strong>Un écart en distance.</strong> Le shadow runner est devant ou derrière vous sur
  le même parcours ; l'écart est exprimé en mètres, dans le sens de la course.</li>
</ol>

<div class="note">
<p><strong>Pas de plan ? Aucun problème.</strong> Choisissez le mode <em>Allure</em> : le
panneau d'assistance disparaît, et la montre se contente de l'allure, de la distance et du
temps. Un footing libre n'a pas besoin de cible.</p>
</div>

## Les quatre modes d'assistant

<div class="grid two">
  <div class="card">
    <h3>Allure</h3>
    <p>Le panneau est masque. Pour les footings libres et les sorties où l'on ne veut rien
    d'autre que la vitesse.</p>
  </div>
  <div class="card">
    <h3>Finish estimé</h3>
    <p>À partir de l'allure courante et de la distance restante, la montre affiche l'heure
    d'arrivée projetée. Aucun plan n'est imposé : on regarde où l'on va.</p>
  </div>
  <div class="card">
    <h3>Temps visé <span class="tag">shadow runner</span></h3>
    <p>Le coureur virtuel suit exactement le plan (distance, temps cible, negative split).
    L'écart affiche est un écart en distance, plus parlant que l'écart en temps.</p>
  </div>
  <div class="card">
    <h3>Course à distance</h3>
    <p>Le protocole multijoueur existe dans le cœur (salon, positions, classement, fin de
    course). L'interface correspondante reste à brancher : le mode s'affiche, il n'est pas
    encore jouable.</p>
  </div>
</div>

## Lire l'écart pendant la course

<div class="table-wrap">

| Ce que vous lisez | Interprétation |
|---|---|
| <span class="tag ok">sur le plan</span> | L'écart est dans la tolérance ; ne changez rien |
| <code>+120 m</code> | Vous avez 120 m d'avance sur le plan. Trop d'avance se paie plus tard |
| <code>-80 m</code>, en orange | Vous êtes en retard ; le chiffre dit de combien, en mètres |
| Aucune pastille | Vous êtes en mode <em>Allure</em>, sans assistant |

</div>

<p class="tiny">
Voir aussi <a href="{{ '/guide/assistant-et-voix/' | relative_url }}">Assistant et voix</a>
pour la fréquence des annonces vocales, et
<a href="{{ '/guide/apres-la-course/' | relative_url }}">Lire l'analyse</a> pour le
<em>plan contre réalise</em> kilomètre par kilomètre.
</p>
