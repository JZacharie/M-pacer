---
layout: default
title: Pendant la course
description: Les écrans de la montre et du téléphone pendant une séance M-pacer, les gestes, les commandes, les couleurs du voyant GPS et ce que disent les pastilles.
permalink: /guide/pendant-la-course/
---

<span class="eyebrow">Guide utilisateur</span>
# Pendant la course

<p class="lead">
En course, on ne lit pas : on jette un œil. Tout est donc organise autour d'une seule
valeur — l'allure — et le reste est rangé derrière un glissement de doigt. Voici ce que
montre chaque écran et ce que veut dire chaque couleur.
</p>

## L'écran principal de la montre

<div class="table-wrap">

| Ce que vous voyez | Ce que ça veut dire |
|---|---|
| <strong>Allure</strong> en grand | Moyenne glissante sur 2 minutes, pas l'allure instantanée : elle bouge moins, et se lit d'un coup d'œil. Tant qu'aucune position exploitable n'est reçue, <code>--:--</code> s'affiche en petit et en gris |
| <code>8,42 km &#183; 42:10</code> | Distance cumulée et temps en mouvement, pauses exclues. La fréquence cardiaque s'ajoute à la suite, dans la couleur de sa zone |
| Voyant GPS en haut | <span class="tag ok">vert</span> signal bon — <span class="tag todo">orange</span> acquisition en cours (pulsation) — <span class="tag todo">jaune</span> précision insuffisante — <span class="tag ko">rouge</span> GPS coupe |
| Pastille d'assistant | « sur le plan », un écart au shadow runner (<code>+120 m</code> en avance, <code>-80 m</code> en retard), ou le temps de finish estimé selon le mode choisi |
| Boutons ronds | <em>Lecture</em>, <em>Pause</em>, <em>Arrêter</em>. Des icônes, jamais de texte : elles ne se tronquent pas quand la taille de police du système augmente |

</div>

<div class="note">
<p><strong>Avance ou retard ?</strong> L'écart du shadow runner est un <strong>écart en
distance</strong>, pas en temps. Pendant l'effort, « 120 m d'avance » se comprend
immédiatement, alors qu'un écart en secondes demande de calculer. Le signe
<code>+</code> veut dire en avance.</p>
</div>

## Les quatre vues de course

Un glissement de gauche à droite fait défiler les vues. L'allure reste toujours la
première, la seule qu'on lit réellement en courant.

<div class="table-wrap">

| Vue | La question a laquelle elle répond |
|---|---|
| <strong>Allure</strong> | Où j'en suis : allure lissée, distance, temps, et la pastille d'assistant |
| <strong>Tour</strong> | Où j'en suis dans mon kilomètre : numéro de tour, allure du tour en cours, allure du tour précédent, distance parcourue dans le tour |
| <strong>Cardio</strong> | Où en est mon cœur : pouls en grand dans la couleur de sa zone, et la zone en pastille |
| <strong>Objectif</strong> | Où je vais : temps de finish estimé, distance restante, écart au shadow runner — ou, sans plan règle, ce qu'il faut faire pour en avoir un |

</div>

## Les commandes

<ol class="steps">
  <li><strong>Démarrer</strong> lance une séance normale.</li>
  <li><strong>Départ au premier pas</strong> arme la séance : le chronomètre et la trace ne
  commencent qu'au premier mouvement. L'écran de repos propose alors <em>Arrêter</em> — pas
  un second départ.</li>
  <li><strong>Pause</strong> et <strong>Reprendre</strong> sont aussi disponibles depuis la
  notification de séance, sur le téléphone : pas besoin de le sortir de sa ceinture.</li>
  <li><strong>Arrêter</strong> termine la séance, l'archive et déclenche l'envoi si
  l'appareil est appaire.</li>
</ol>

<p class="tiny">
L'auto-pause et l'auto-reprise sont actives par le moteur à partir de la vitesse lissée :
une pause ne rallonge pas un tour, et le temps en mouvement reste juste.
</p>

## Sur le téléphone

<p>
L'application <strong>Course</strong> reprend la même logique avec plus de place. Au-dessus
des commandes, elle affiche :
</p>

<div class="table-wrap">

| Bloc | Contenu |
|---|---|
| Voyant GPS | Vert, orange ou rouge, avec la précision annoncée en mètres (<code>GPS +/- 6 m</code>) |
| Allure instantanée | En très grand, avec l'allure du tour précédent juste en dessous |
| Tuiles | Distance, temps écoulé, fréquence cardiaque (avec zone), cadence et longueur de foulée |
| Assistant | « Sur le plan », « Finish estimé 1:44:12 », « Reste 4,2 km » |
| Musique | Titre en cours, BPM consigne par le moteur et directive de tempo |
| Commandes | <em>Démarrer la course</em>, <em>Pause</em>, <em>Reprendre</em>, <em>Arrêter</em>, plus <em>Annonce vocale</em> et <em>Fenêtre d'allure</em> à la demande |

</div>

<div class="info">
<p><strong>Le téléphone est un vrai coureur, pas un accessoire.</strong> GPS 1 Hz, ceinture
cardiaque Bluetooth LE, voix, musique, suivi en direct et onglet Amis : tout fonctionne sans
montre. C'est le même cœur Rust qui calcule, donc les chiffres sont identiques.</p>
</div>

## La musique pendant l'effort

<p>
Le moteur connait le tempo cible de la séance et la playlist ; il en déduit une
<strong>directive</strong> affichée sous le titre : <code>^</code> pour accélérer,
<code>v</code> pour se calmer, <code>&gt;&gt;</code> pour changer de piste. La voix peut
aussi annoncer les changements de tempo. Préparation des playlists :
<a href="{{ '/guide/musique/' | relative_url }}">Musique et tempo</a>.
</p>

## Regarder ou ne pas regarder

- **Le voyant GPS avant le départ, puis plus rien.** Une allure fausse au premier kilomètre
  vient presque toujours d'un départ trop tot, pas d'un problème de calcul.
- **La pastille d'assistant suffit** pour tenir un plan : inutile d'ouvrir la vue Objectif a
  chaque kilomètre.
- **La voix est faite pour ça.** Réglez la fréquence des annonces (1, 2 ou 5 minutes, ou a
  chaque tour) et laissez la montre parler :
  <a href="{{ '/guide/assistant-et-voix/' | relative_url }}">Assistant et voix</a>.

<div class="grid">
  <a class="card" href="{{ '/guide/assistant-et-voix/' | relative_url }}">
    <h3>Assistant et voix</h3>
    <p>Les quatre modes, la fréquence des annonces et ce qui arrive à la musique.</p>
  </a>
  <a class="card" href="{{ '/guide/apres-la-course/' | relative_url }}">
    <h3>Après l'arrivée</h3>
    <p>Terminer proprement, synchroniser, et lire l'analyse.</p>
  </a>
</div>
