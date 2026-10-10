---
layout: default
title: Votre premiere seance
description: Courir une premiere fois avec M-pacer, de l'installation a la seance affichee sur le site, avec la liste des verifications d'avant-depart.
permalink: /guide/premiere-seance/
---

<span class="eyebrow">Guide utilisateur</span>
# Votre premiere seance

<p class="lead">
Objectif : une seance reelle, du premier appui sur <em>Demarrer</em> jusqu'a l'analyse
ouverte dans le navigateur. Comptez un quart d'heure pour les preparatifs, une fois.
</p>

## Avant de partir

<div class="table-wrap">

| A faire | Sur quoi | Pourquoi |
|---|---|---|
| Charger la montre | Montre | Le GPS a 1 Hz est le poste le plus gourmand de la seance |
| Verifier la version | Reglages de l'application | On sait quel binaire tourne, et le site affiche la meme version |
| Autoriser la localisation | Montre ou telephone | Sans elle, aucune position n'arrive au moteur |
| Renseigner l'adresse du backend | Sync (montre) ou Reglages (telephone) | Necessaire seulement pour synchroniser et pour les amis |
| Regler le plan | Reglages &rsaquo; Assistant de course | Distance, temps vise, part negative. Voir [Preparer un plan]({{ '/guide/plan-de-course/' | relative_url }}) |
| Verifier le voyant GPS | Ecran de course | Vert fixe = signal bon ; attendez le vert avant de partir |

</div>

## Etape 1 — Appairer l'appareil

<ol class="steps">
  <li>Sur la montre : <strong>Sync</strong> &rsaquo; <strong>S'appairer</strong>. Elle
  demande un code au backend et affiche un code court du type
  <code>BCDF-GHJK</code> avec l'adresse a ouvrir.</li>
  <li>Dans le navigateur : ouvrez <code>/link</code>, saisissez le code (le champ se
  formate tout seul) puis approuvez.</li>
  <li>La montre sonde jusqu'a l'approbation, puis enregistre son jeton. L'ecran affiche
  <em>Appareil appaire</em> ; le nombre de seances en attente apparait juste en dessous.</li>
</ol>

<p class="tiny">
Detail, revocation et cas sans reseau : <a href="{{ '/guide/appairer-et-synchroniser/' | relative_url }}">Appairer
et synchroniser</a>. Vous pouvez aussi courir tout de suite sans appairer : la seance sera
archivee sur la montre et partira plus tard.
</p>

## Etape 2 — Regler le plan

<p>
Le plan n'est pas obligatoire, mais c'est ce qui donne son sens au <em>shadow runner</em>.
Dans <strong>Reglages &rsaquo; Assistant de course</strong>, renseignez la distance de la
course et le temps vise (par exemple <code>10</code> km et <code>50:00</code>), puis la part
negative si vous voulez partir un peu plus lentement que la moyenne.
</p>

<div class="note">
<p><strong>Un footing libre ne demande aucun plan.</strong> Choisissez le mode
<em>Allure</em> dans le meme ecran : le panneau d'assistance disparait et il reste
l'allure, la distance et le temps.</p>
</div>

## Etape 3 — Demarrer la seance

<ol class="steps">
  <li>Ouvrez l'ecran de course. Le voyant GPS pulse en orange : il acquiert. Attendez qu'il
  passe au <strong>vert fixe</strong>.</li>
  <li>Appuyez sur <strong>Demarrer</strong> — ou choisissez <strong>Depart au premier
  pas</strong> : la seance est <em>armee</em> et le chronometre attend votre premier
  mouvement. Fini le kilometre fantome pendant l'echauffement.</li>
  <li>Pendant la course, faites defiler les vues d'un glissement de gauche a droite :
  <strong>Allure</strong>, <strong>Tour</strong>, <strong>Cardio</strong>,
  <strong>Objectif</strong>. Les commandes, elles, ne defilent pas : <em>Pause</em> et
  <em>Arreter</em> restent a la meme place.</li>
  <li>Un appui long n'est jamais necessaire : les pauses se declenchent au doigt, et
  l'auto-pause prend le relais quand le moteur detecte un arret.</li>
</ol>

<p class="tiny">
Tout le detail des ecrans et des gestes : <a href="{{ '/guide/pendant-la-course/' | relative_url }}">Pendant
la course</a>.
</p>

## Etape 4 — Terminer et synchroniser

<ol class="steps">
  <li>Appuyez sur <strong>Arreter</strong>. La seance est <strong>archivee localement</strong>
  dans le stockage prive de l'application, avec sa trace GPS et ses tours.</li>
  <li>Si l'appareil est appaire, l'envoi part <strong>automatiquement</strong> dans la foulee.
  Sinon, ouvrez l'ecran <em>Sync</em> et appuyez sur <em>Envoyer les seances</em>.</li>
  <li>Le site affiche la seance dans la liste des que l'envoi est accepte. Un reenvoi de la
  meme seance <strong>remplace</strong> la version distante : jamais de doublon.</li>
</ol>

## Etape 5 — Relire la seance

<p>
Ouvrez le site, connectez-vous, cliquez sur la seance : le resume, les courbes, le plan
contre le realise, les zones cardiaques, les temps de passage et les meilleures distances
sont calcules a la volee. Vous pouvez exporter le GPX pour l'envoyer vers Strava, Garmin ou
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

## Si quelque chose ne se passe pas comme prevu

<div class="table-wrap">

| Symptome | Cause probable | Quoi faire |
|---|---|---|
| Le voyant GPS reste orange ou rouge | Ciel masque, interieur, permission refusee | Sortez a decouvert, verifiez la permission de localisation |
| L'allure affiche <code>--:--</code> | Aucune position exploitable encore recue | Attendez le passage au vert ; la valeur reste petite et grisee pour ne pas etre lue comme une allure |
| La seance n'est pas sur le site | Appareil non appaire, pas de reseau | Ecran Sync &rsaquo; <em>Envoyer les seances</em> ; la seance est toujours sur la montre |
| Pas d'annonce vocale | Voix desactivee, ou frequence sur <em>Jamais</em> | Reglages &rsaquo; Retour vocal |

</div>

<p class="tiny">
La liste complete : <a href="{{ '/guide/depannage/' | relative_url }}">Depannage</a>.
</p>
