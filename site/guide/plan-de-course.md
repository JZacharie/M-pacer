---
layout: default
title: Preparer un plan d'allure
description: Regler la distance, le temps vise et le negative split dans M-pacer, comprendre le shadow runner et lire l'ecart au plan pendant la course.
permalink: /guide/plan-de-course/
---

<span class="eyebrow">Guide utilisateur</span>
# Preparer un plan d'allure

<p class="lead">
Un plan, c'est deux nombres — une distance et un temps — et une intention : partir
regulierement, quitte a finir plus vite. M-pacer en fait un <em>coureur virtuel</em> qui
courra exactement votre plan, et vous dit a chaque instant si vous etes devant ou derriere.
</p>

## Ou regler le plan

<div class="table-wrap">

| Support | Chemin |
|---|---|
| Montre | Reglages &rsaquo; <strong>Assistant de course</strong> |
| Telephone | Reglages &rsaquo; <strong>Assistant</strong> |

</div>

Trois champs suffisent :

<div class="table-wrap">

| Champ | Format | Exemple |
|---|---|---|
| Distance de course | kilometres (ou miles selon les unites) | <code>10</code> |
| Temps vise | <code>h:mm:ss</code> ou <code>mm:ss</code> | <code>50:00</code> |
| Part negative | pourcentage | <code>3</code> % |

</div>

<p>
La <strong>part negative</strong> decrit l'ecart entre la premiere et la seconde moitie :
0 % signifie une allure parfaitement constante ; 3 % signifie que la seconde moitie est
courue 3 % plus vite que la premiere. C'est le reglage classique d'un marathon ou d'un
semi ou l'on veut se garder.
</p>

## Quelques objectifs courants

<div class="table-wrap">

| Course | Temps vise | Allure moyenne du plan |
|---|---|---|
| 5 km | 25:00 | 5:00 /km |
| 10 km | 50:00 | 5:00 /km |
| 10 km | 45:00 | 4:30 /km |
| Semi-marathon | 1:45:00 | 4:59 /km |
| Marathon | 3:30:00 | 4:59 /km |
| Marathon | 4:00:00 | 5:41 /km |

</div>

<p class="tiny">
Ce tableau ne sert qu'a verifier que le temps saisi tient debout : la montre n'affiche
jamais l'allure moyenne du plan, elle affiche votre allure et l'ecart au coureur virtuel.
</p>

## Comment le moteur s'en sert

<ol class="steps">
  <li><strong>Un plan exact, pas un tableau de passages.</strong> Le moteur construit la
  courbe de distance en fonction du temps, et sait l'inverser : a tout instant il connait la
  distance que le coureur virtuel <em>devrait</em> avoir parcourue. Le plan retombe sur
  l'arrivee a l'instant vise, verifie par test a mieux que <code>1e-6</code> pres.</li>
  <li><strong>Allure lissee sur deux minutes.</strong> Votre allure affichee est une moyenne
  glissante : elle ignore les soubresauts du GPS et les quelques secondes d'hesitation, ce
  qui evite de courir apres le bruit.</li>
  <li><strong>Un ecart en distance.</strong> Le shadow runner est devant ou derriere vous sur
  le meme parcours ; l'ecart est exprime en metres, dans le sens de la course.</li>
</ol>

<div class="note">
<p><strong>Pas de plan ? Aucun probleme.</strong> Choisissez le mode <em>Allure</em> : le
panneau d'assistance disparait, et la montre se contente de l'allure, de la distance et du
temps. Un footing libre n'a pas besoin de cible.</p>
</div>

## Les quatre modes d'assistant

<div class="grid two">
  <div class="card">
    <h3>Allure</h3>
    <p>Le panneau est masque. Pour les footings libres et les sorties ou l'on ne veut rien
    d'autre que la vitesse.</p>
  </div>
  <div class="card">
    <h3>Finish estime</h3>
    <p>A partir de l'allure courante et de la distance restante, la montre affiche l'heure
    d'arrivee projetee. Aucun plan n'est impose : on regarde ou l'on va.</p>
  </div>
  <div class="card">
    <h3>Temps vise <span class="tag">shadow runner</span></h3>
    <p>Le coureur virtuel suit exactement le plan (distance, temps cible, negative split).
    L'ecart affiche est un ecart en distance, plus parlant que l'ecart en temps.</p>
  </div>
  <div class="card">
    <h3>Course a distance</h3>
    <p>Le protocole multijoueur existe dans le coeur (salon, positions, classement, fin de
    course). L'interface correspondante reste a brancher : le mode s'affiche, il n'est pas
    encore jouable.</p>
  </div>
</div>

## Lire l'ecart pendant la course

<div class="table-wrap">

| Ce que vous lisez | Interpretation |
|---|---|
| <span class="tag ok">sur le plan</span> | L'ecart est dans la tolerance ; ne changez rien |
| <code>+120 m</code> | Vous avez 120 m d'avance sur le plan. Trop d'avance se paie plus tard |
| <code>-80 m</code>, en orange | Vous etes en retard ; le chiffre dit de combien, en metres |
| Aucune pastille | Vous etes en mode <em>Allure</em>, sans assistant |

</div>

<p class="tiny">
Voir aussi <a href="{{ '/guide/assistant-et-voix/' | relative_url }}">Assistant et voix</a>
pour la frequence des annonces vocales, et
<a href="{{ '/guide/apres-la-course/' | relative_url }}">Lire l'analyse</a> pour le
<em>plan contre realise</em> kilometre par kilometre.
</p>
