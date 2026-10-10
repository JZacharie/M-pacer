---
layout: default
title: Votre première séance
description: Courir une première fois avec M-pacer, de l'installation à la séance affichée sur le site, avec la liste des vérifications d'avant-départ.
permalink: /guide/premiere-seance/
---

<span class="eyebrow">Guide utilisateur</span>
# Votre première séance

<p class="lead">
Objectif : une séance réelle, du premier appui sur <em>Démarrer</em> jusqu'à l'analyse
ouverte dans le navigateur. Comptez un quart d'heure pour les préparatifs, une fois.
</p>

## Avant de partir

<div class="table-wrap">

| A faire | Sur quoi | Pourquoi |
|---|---|---|
| Charger la montre | Montre | Le GPS à 1 Hz est le poste le plus gourmand de la séance |
| Vérifier la version | Réglages de l'application | On sait quel binaire tourne, et le site affiche la même version |
| Autoriser la localisation | Montre ou téléphone | Sans elle, aucune position n'arrive au moteur |
| Renseigner l'adresse du backend | Sync (montre) ou Réglages (téléphone) | Nécessaire seulement pour synchroniser et pour les amis |
| Régler le plan | Réglages &rsaquo; Assistant de course | Distance, temps visé, part négative. Voir [Préparer un plan]({{ '/guide/plan-de-course/' | relative_url }}) |
| Vérifier le voyant GPS | Écran de course | Vert fixe = signal bon ; attendez le vert avant de partir |

</div>

## Étape 1 — Appairer l'appareil

<ol class="steps">
  <li>Sur la montre : <strong>Sync</strong> &rsaquo; <strong>S'appairer</strong>. Elle
  demande un code au backend et affiche un code court du type
  <code>BCDF-GHJK</code> avec l'adresse à ouvrir.</li>
  <li>Dans le navigateur : ouvrez <code>/link</code>, saisissez le code (le champ se
  formate tout seul) puis approuvez.</li>
  <li>La montre sonde jusqu'à l'approbation, puis enregistre son jeton. L'écran affiche
  <em>Appareil appaire</em> ; le nombre de séances en attente apparaît juste en dessous.</li>
</ol>

<p class="tiny">
Détail, révocation et cas sans réseau : <a href="{{ '/guide/appairer-et-synchroniser/' | relative_url }}">Appairer
et synchroniser</a>. Vous pouvez aussi courir tout de suite sans appairer : la séance sera
archivée sur la montre et partira plus tard.
</p>

## Étape 2 — Régler le plan

<p>
Le plan n'est pas obligatoire, mais c'est ce qui donne son sens au <em>shadow runner</em>.
Dans <strong>Réglages &rsaquo; Assistant de course</strong>, renseignez la distance de la
course et le temps visé (par exemple <code>10</code> km et <code>50:00</code>), puis la part
négative si vous voulez partir un peu plus lentement que la moyenne.
</p>

<div class="note">
<p><strong>Un footing libre ne demande aucun plan.</strong> Choisissez le mode
<em>Allure</em> dans le même écran : le panneau d'assistance disparaît et il reste
l'allure, la distance et le temps.</p>
</div>

## Étape 3 — Démarrer la séance

<ol class="steps">
  <li>Ouvrez l'écran de course. Le voyant GPS pulse en orange : il acquiert. Attendez qu'il
  passe au <strong>vert fixe</strong>.</li>
  <li>Appuyez sur <strong>Démarrer</strong> — ou choisissez <strong>Départ au premier
  pas</strong> : la séance est <em>armée</em> et le chronomètre attend votre premier
  mouvement. Fini le kilomètre fantôme pendant l'échauffement.</li>
  <li>Pendant la course, faites défiler les vues d'un glissement de gauche à droite :
  <strong>Allure</strong>, <strong>Tour</strong>, <strong>Cardio</strong>,
  <strong>Objectif</strong>. Les commandes, elles, ne défilent pas : <em>Pause</em> et
  <em>Arrêter</em> restent à la même place.</li>
  <li>Un appui long n'est jamais nécessaire : les pauses se déclenchent au doigt, et
  l'auto-pause prend le relais quand le moteur détecte un arrêt.</li>
</ol>

<p class="tiny">
Tout le détail des écrans et des gestes : <a href="{{ '/guide/pendant-la-course/' | relative_url }}">Pendant
la course</a>.
</p>

## Étape 4 — Terminer et synchroniser

<ol class="steps">
  <li>Appuyez sur <strong>Arrêter</strong>. La séance est <strong>archivée localement</strong>
  dans le stockage privé de l'application, avec sa trace GPS et ses tours.</li>
  <li>Si l'appareil est appaire, l'envoi part <strong>automatiquement</strong> dans la foulée.
  Sinon, ouvrez l'écran <em>Sync</em> et appuyez sur <em>Envoyer les séances</em>.</li>
  <li>Le site affiche la séance dans la liste dès que l'envoi est accepté. Un renvoi de la
  même séance <strong>remplace</strong> la version distante : jamais de doublon.</li>
</ol>

## Étape 5 — Relire la séance

<p>
Ouvrez le site, connectez-vous, cliquez sur la séance : le résumé, les courbes, le plan
contre le réalisé, les zones cardiaques, les temps de passage et les meilleures distances
sont calculés à la volée. Vous pouvez exporter le GPX pour l'envoyer vers Strava, Garmin ou
OpenRunner.
</p>

<div class="grid">
  <a class="card" href="{{ '/guide/apres-la-course/' | relative_url }}">
    <h3>Lire l'analyse</h3>
    <p>Ce que veulent dire chaque chiffre et chaque courbe.</p>
  </a>
  <a class="card" href="{{ '/guide/tableaux-de-bord/' | relative_url }}">
    <h3>Composer un tableau de bord</h3>
    <p>Choisir et ordonner les widgets que vous voulez voir revenir.</p>
  </a>
</div>

## Si quelque chose ne se passe pas comme prévu

<div class="table-wrap">

| Symptôme | Cause probable | Quoi faire |
|---|---|---|
| Le voyant GPS reste orange ou rouge | Ciel masque, intérieur, permission refusee | Sortez a decouvert, vérifiez la permission de localisation |
| L'allure affiche <code>--:--</code> | Aucune position exploitable encore reçue | Attendez le passage au vert ; la valeur reste petite et grisée pour ne pas être lue comme une allure |
| La séance n'est pas sur le site | Appareil non appaire, pas de réseau | Écran Sync &rsaquo; <em>Envoyer les séances</em> ; la séance est toujours sur la montre |
| Pas d'annonce vocale | Voix désactivée, ou fréquence sur <em>Jamais</em> | Réglages &rsaquo; Retour vocal |

</div>

<p class="tiny">
La liste complète : <a href="{{ '/guide/depannage/' | relative_url }}">Dépannage</a>.
</p>
