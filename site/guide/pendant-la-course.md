---
layout: default
title: Pendant la course
description: Les ecrans de la montre et du telephone pendant une seance M-pacer, les gestes, les commandes, les couleurs du voyant GPS et ce que disent les pastilles.
permalink: /guide/pendant-la-course/
---

<span class="eyebrow">Guide utilisateur</span>
# Pendant la course

<p class="lead">
En course, on ne lit pas : on jette un oeil. Tout est donc organise autour d'une seule
valeur — l'allure — et le reste est range derriere un glissement de doigt. Voici ce que
montre chaque ecran et ce que veut dire chaque couleur.
</p>

## L'ecran principal de la montre

<div class="table-wrap">

| Ce que vous voyez | Ce que ca veut dire |
|---|---|
| <strong>Allure</strong> en grand | Moyenne glissante sur 2 minutes, pas l'allure instantanee : elle bouge moins, et se lit d'un coup d'oeil. Tant qu'aucune position exploitable n'est recue, <code>--:--</code> s'affiche en petit et en gris |
| <code>8,42 km &#183; 42:10</code> | Distance cumulee et temps en mouvement, pauses exclues. La frequence cardiaque s'ajoute a la suite, dans la couleur de sa zone |
| Voyant GPS en haut | <span class="tag ok">vert</span> signal bon — <span class="tag todo">orange</span> acquisition en cours (pulsation) — <span class="tag todo">jaune</span> precision insuffisante — <span class="tag ko">rouge</span> GPS coupe |
| Pastille d'assistant | « sur le plan », un ecart au shadow runner (<code>+120 m</code> en avance, <code>-80 m</code> en retard), ou le temps de finish estime selon le mode choisi |
| Boutons ronds | <em>Lecture</em>, <em>Pause</em>, <em>Arreter</em>. Des icones, jamais de texte : elles ne se tronquent pas quand la taille de police du systeme augmente |

</div>

<div class="note">
<p><strong>Avance ou retard ?</strong> L'ecart du shadow runner est un <strong>ecart en
distance</strong>, pas en temps. Pendant l'effort, « 120 m d'avance » se comprend
immediatement, alors qu'un ecart en secondes demande de calculer. Le signe
<code>+</code> veut dire en avance.</p>
</div>

## Les quatre vues de course

Un glissement de gauche a droite fait defiler les vues. L'allure reste toujours la
premiere, la seule qu'on lit reellement en courant.

<div class="table-wrap">

| Vue | La question a laquelle elle repond |
|---|---|
| <strong>Allure</strong> | Ou j'en suis : allure lissee, distance, temps, et la pastille d'assistant |
| <strong>Tour</strong> | Ou j'en suis dans mon kilometre : numero de tour, allure du tour en cours, allure du tour precedent, distance parcourue dans le tour |
| <strong>Cardio</strong> | Ou en est mon coeur : pouls en grand dans la couleur de sa zone, et la zone en pastille |
| <strong>Objectif</strong> | Ou je vais : temps de finish estime, distance restante, ecart au shadow runner — ou, sans plan regle, ce qu'il faut faire pour en avoir un |

</div>

## Les commandes

<ol class="steps">
  <li><strong>Demarrer</strong> lance une seance normale.</li>
  <li><strong>Depart au premier pas</strong> arme la seance : le chronometre et la trace ne
  commencent qu'au premier mouvement. L'ecran de repos propose alors <em>Arreter</em> — pas
  un second depart.</li>
  <li><strong>Pause</strong> et <strong>Reprendre</strong> sont aussi disponibles depuis la
  notification de seance, sur le telephone : pas besoin de le sortir de sa ceinture.</li>
  <li><strong>Arreter</strong> termine la seance, l'archive et declenche l'envoi si
  l'appareil est appaire.</li>
</ol>

<p class="tiny">
L'auto-pause et l'auto-reprise sont actives par le moteur a partir de la vitesse lissee :
une pause ne rallonge pas un tour, et le temps en mouvement reste juste.
</p>

## Sur le telephone

<p>
L'application <strong>Course</strong> reprend la meme logique avec plus de place. Au-dessus
des commandes, elle affiche :
</p>

<div class="table-wrap">

| Bloc | Contenu |
|---|---|
| Voyant GPS | Vert, orange ou rouge, avec la precision annoncee en metres (<code>GPS +/- 6 m</code>) |
| Allure instantanee | En tres grand, avec l'allure du tour precedent juste en dessous |
| Tuiles | Distance, temps ecoule, frequence cardiaque (avec zone), cadence et longueur de foulee |
| Assistant | « Sur le plan », « Finish estime 1:44:12 », « Reste 4,2 km » |
| Musique | Titre en cours, BPM consigne par le moteur et directive de tempo |
| Commandes | <em>Demarrer la course</em>, <em>Pause</em>, <em>Reprendre</em>, <em>Arreter</em>, plus <em>Annonce vocale</em> et <em>Fenetre d'allure</em> a la demande |

</div>

<div class="info">
<p><strong>Le telephone est un vrai coureur, pas un accessoire.</strong> GPS 1 Hz, ceinture
cardiaque Bluetooth LE, voix, musique, suivi en direct et onglet Amis : tout fonctionne sans
montre. C'est le meme coeur Rust qui calcule, donc les chiffres sont identiques.</p>
</div>

## La musique pendant l'effort

<p>
Le moteur connait le tempo cible de la seance et la playlist ; il en deduit une
<strong>directive</strong> affichee sous le titre : <code>^</code> pour accelerer,
<code>v</code> pour se calmer, <code>&gt;&gt;</code> pour changer de piste. La voix peut
aussi annoncer les changements de tempo. Preparation des playlists :
<a href="{{ '/guide/musique/' | relative_url }}">Musique et tempo</a>.
</p>

## Regarder ou ne pas regarder

- **Le voyant GPS avant le depart, puis plus rien.** Une allure fausse au premier kilometre
  vient presque toujours d'un depart trop tot, pas d'un probleme de calcul.
- **La pastille d'assistant suffit** pour tenir un plan : inutile d'ouvrir la vue Objectif a
  chaque kilomètre.
- **La voix est faite pour ca.** Reglez la frequence des annonces (1, 2 ou 5 minutes, ou a
  chaque tour) et laissez la montre parler :
  <a href="{{ '/guide/assistant-et-voix/' | relative_url }}">Assistant et voix</a>.

<div class="grid">
  <a class="card" href="{{ '/guide/assistant-et-voix/' | relative_url }}">
    <h3>Assistant et voix</h3>
    <p>Les quatre modes, la frequence des annonces et ce qui arrive a la musique.</p>
  </a>
  <a class="card" href="{{ '/guide/apres-la-course/' | relative_url }}">
    <h3>Apres l'arrivee</h3>
    <p>Terminer proprement, synchroniser, et lire l'analyse.</p>
  </a>
</div>
