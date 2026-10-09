---
layout: default
title: La montre
description: Ce que fait l'application montre M-pacer : allure lissee, quatre modes d'assistant, shadow runner, voix, tours, archive locale et synchronisation Wear OS.
permalink: /montre/
---

<span class="eyebrow">Application Wear OS</span>
# La montre : guider l'allure, sans regarder l'écran

<p class="lead">
C'est la pièce qui sert pendant la course. Elle lit le GPS, laisse le cœur Rust
calculer, et n'affiche qu'une chose : l'allure. Autour d'elle, un assistant qui
compare en permanence le réalisé au plan.
</p>

<div class="mockups">
  <figure class="mockup">
    <svg class="watch" viewBox="0 0 200 200" role="img" aria-label="Écran principal de la montre">
      <circle cx="100" cy="100" r="93" fill="#0b0d10" stroke="rgba(255,255,255,.14)" stroke-width="2"/>
      <circle cx="100" cy="40" r="5" fill="#2fbf71"/>
      <text x="95" y="92" text-anchor="end" font-family="system-ui, sans-serif" font-size="44" font-weight="700" fill="#f3f5f8">5:12</text>
      <text x="99" y="92" font-family="system-ui, sans-serif" font-size="12" fill="#6b7480">/km</text>
      <text x="100" y="116" text-anchor="middle" font-family="system-ui, sans-serif" font-size="13" fill="#98a2b3">8,42 km &#183; 42:10</text>
      <rect x="63" y="129" width="74" height="22" rx="11" fill="rgba(47,191,113,.16)"/>
      <text x="100" y="144" text-anchor="middle" font-family="system-ui, sans-serif" font-size="12" font-weight="600" fill="#2fbf71">sur le plan</text>
      <circle cx="78" cy="168" r="22" fill="#fc4c02"/>
      <rect x="71" y="158" width="4" height="20" rx="1.4" fill="#ffffff"/>
      <rect x="81" y="158" width="4" height="20" rx="1.4" fill="#ffffff"/>
      <circle cx="124" cy="170" r="19" fill="#242933"/>
      <rect x="114" y="160" width="20" height="20" rx="5" fill="#ff5a5f"/>
    </svg>
    <figcaption>Écran principal, pensé pour un écran rond</figcaption>
  </figure>
</div>

## L'écran principal

Une seule information domine : **l'allure courante**, en 46 sp, lissée sur deux minutes.
Tout le reste est secondaire et tient dans le cercle.

Pendant la course, un **glissement de gauche à droite** fait défiler quatre vues : allure,
tour, cardio, objectif. Un cadran de 40 mm ne montre pas tout à la fois, et la bonne réponse
n'est pas de rapetisser les chiffres, c'est de séparer ce qu'on ne regarde pas au même
moment. L'allure reste la première vue, la seule qu'on lit en courant. Les commandes
d'arrêt et de pause, elles, ne défilent pas : arrêter une séance ne doit jamais obliger à
chercher la bonne page.

<div class="table-wrap">

| Élément | Ce qu'il montre | Détail |
|---|---|---|
| Allure | La valeur lue en courant | Moyenne glissante de 2 minutes ; tant qu'aucune position exploitable n'est reçue, <code>--:--</code> s'affiche en petit et en gris, pour ne pas se lire comme une allure |
| Distance et temps | <code>8,42 km &#183; 42:10</code> | Distance cumulée et temps en mouvement, pauses exclues ; la fréquence cardiaque s'ajoute à la suite, dans la couleur de sa zone |
| Voyant GPS | Un point de 11 px au-dessus de l'allure | Vert fixe quand le signal est bon, pulsation orange pendant l'acquisition, jaune si la précision est insuffisante, rouge si le GPS est coupé |
| Pastille d'assistant | Une ligne sous les métriques | « sur le plan » ou l'écart au shadow runner (<code>+120 m</code> / <code>-80 m</code>), en vert quand on est dans le plan, en orange en retard ; ou le temps de finish estimé. Rien en mode allure seule |
| Commandes | Des ronds à icône, jamais de texte | Lecture / pause / arrêt, et sur l'écran de repos musique, synchronisation et réglages. Une icône ne se tronque pas quand la taille de police du système augmente, un mot si |
| État <code>Armed</code> | Départ au premier mouvement | L'écran de repos propose alors l'arrêt, pas un second départ |

</div>

<div class="table-wrap">

Les quatre vues de course :

| Vue | Ce qu'elle répond |
|---|---|
| **Allure** | Où j'en suis : allure lissée, distance, temps, et la pastille d'assistant |
| **Tour** | Où j'en suis dans mon kilomètre : numéro de tour, allure du tour en cours, allure du tour précédent, distance parcourue dans le tour |
| **Cardio** | Où en est mon cœur : pouls en grand dans la couleur de sa zone, et la zone en pastille |
| **Objectif** | Où je vais : temps de finish estimé, distance restante, écart au shadow runner — ou, sans plan réglé, ce qu'il faut faire pour en avoir un |

</div>

<div class="note">
<p><strong>Aucune dépendance d'icônes.</strong> Les tracés sont écrits en vectoriel
dans <code>ui/WatchIcons.kt</code> (grille de 24 x 24, seize symboles) plutôt que tirés de
<code>material-icons-extended</code> : la montre n'embarque que ce qu'elle affiche,
et le poids visuel reste homogène. Les quatre briques communes aux écrans
(<code>RoundButton</code>, <code>StatusPill</code>, <code>ScreenTitle</code>,
<code>SecondaryScreen</code>) vivent dans <code>ui/WatchDesign.kt</code>.</p>
</div>

<p class="tiny">
La notification du service de premier plan reprend la même ligne (allure, distance, temps)
et n'est republiée que si le texte change, au plus une fois toutes les 5 secondes : un aller-retour
vers <code>system_server</code> par seconde coûterait de la batterie pour rien.
</p>

## Les quatre modes d'assistant

Le mode se choisit dans les réglages, sous forme de puces (le motif Wear OS plutôt que des formulaires).

<div class="grid two">
  <div class="card">
    <h3>Allure seule</h3>
    <p>Le panneau d'assistance est masqué. Utile en footing libre : l'allure, la distance, le temps.</p>
  </div>
  <div class="card">
    <h3>Temps de finish estimé</h3>
    <p>À partir de l'allure courante et de la distance restante, la montre affiche l'heure d'arrivée
    projetée. Aucun plan n'est imposé, on regarde où l'on va.</p>
  </div>
  <div class="card">
    <h3>Shadow runner</h3>
    <p>Un second coureur virtuel suit exactement le plan (distance et temps cible, negative split
    éventuel). L'écart affiché est un <strong>écart en distance</strong>, plus parlant que l'écart en
    temps pendant l'effort : en avance, en retard, ou « sur le plan ».</p>
  </div>
  <div class="card">
    <h3>Course à distance</h3>
    <p>Le protocole de course multijoueur est implémenté dans le cœur (salon, positions, classement,
    fin de course). L'interface montre correspondante reste à brancher.</p>
  </div>
</div>

<div class="note">
<p>Le shadow runner est calculé à partir d'un plan exact : la courbe de distance en fonction du
temps est inversible, et le plan retombe sur l'arrivée à l'instant cible. Vérifié par test
(erreur inférieure à 1e-6) et sur l'exemple du manuel de Pace Control (5:52 → 5:30).</p>
</div>

## Pendant la séance

<ol class="steps">
  <li><strong>Départ suspendu (<code>Armed</code>) possible.</strong> La séance est armée, le chronomètre
  ne démarre qu'au premier mouvement : plus de kilomètre fantôme en sortant de la zone de départ.</li>
  <li><strong>GPS à 1 Hz.</strong> <code>FusedLocationProviderClient</code>, haute précision, dans un service
  de premier plan de type <code>location|health</code> : la montre enregistre sans le téléphone.</li>
  <li><strong>Filtrage.</strong> Chaque position passe le contrôle de qualité du cœur : précision,
  vitesse plausible, rejet des sauts GPS aberrants, confirmation sur plusieurs échantillons avant de
  passer au vert.</li>
  <li><strong>Tours automatiques</strong> au kilomètre (ou au mile selon les unités), avec l'allure du
  tour courant et celle du tour précédent ; une pause ne rallonge pas un tour.</li>
  <li><strong>Auto-pause et auto-reprise.</strong> Le cœur détecte l'arrêt à partir de la vitesse
  lissée (seuils et délais configurables) et reprend seul. En pause, la montre relâche la contrainte :
  une position toutes les 3 secondes en précision équilibrée au lieu d'une par seconde.</li>
  <li><strong>Annonces vocales.</strong> Elles sont rédigées par le cœur (FR ou EN) et seulement
  prononcées par la montre : périodiques (1, 2 ou 5 minutes), à chaque tour, au démarrage, à la
  pause, à la reprise, ou sur demande. La formulation peut être courte et informelle.</li>
</ol>

<h3>Ce que la montre ne fait pas</h3>

<p>
Elle ne calcule rien de métier. Aucun algorithme de course n'est écrit en Kotlin : la montre
collecte (<code>on_gps</code>, <code>on_heart_rate</code>, <code>tick</code>) et affiche le résultat
du moteur. C'est ce qui permet de tester tous les calculs sur un PC, sans montre, sans GPS et sans Android.
</p>

## À la fin de la séance

1. La séance est **archivée localement** dans le stockage privé de l'application
   (<code>filesDir/workouts/</code>), au format d'échange du cœur : distance, temps en mouvement,
   temps écoulé, allure moyenne, tours, meilleures distances, trace GPS, unités, pauses et
   fréquence cardiaque quand elle est disponible.
2. Si la montre est appairée, l'envoi part **automatiquement** dans le même mouvement, sans ouvrir
   l'écran de synchronisation. Sinon la séance reste sur la montre : courir ne dépend jamais du réseau.
3. Un envoi déjà accepté est mémorisé localement : renvoyer la même séance **remplace** la version
   distante au lieu de créer un doublon.

## Suivre la course en direct

<p>
Pendant la séance, la montre peut publier sa position sur un <strong>broker MQTT</strong> : un point
toutes les dix secondes, environ 130 octets. Le service s'y abonne et la page <code>/live</code>
affiche la trace en temps réel — une polyligne SVG dessinée par le service, rafraîchie toute les
dix secondes, sans fond de carte ni script tiers.
</p>

<ul>
  <li><strong>Désactivé par défaut.</strong> Sans adresse de broker, la montre n'ouvre aucune
  connexion et ne crée aucun fil : le suivi ne coûte rien tant qu'on ne le configure pas.</li>
  <li><strong>Cadence maîtrisée.</strong> Une position toutes les 10 s en course, une toutes les 60 s
  en pause ; un point dont la précision GPS dépasse 50 m n'est pas publié. Environ 55 ko par heure,
  un fil en priorité basse, aucune minuterie, aucun wake lock.</li>
  <li><strong>Volatil.</strong> Rien n'est écrit en base : la séance reste la source de vérité et
  arrive à la fin, comme avant. Un proche qui se connecte en route voit immédiatement la dernière
  position (message retenu).</li>
  <li><strong>Privé.</strong> La page demande une connexion ; le mot de passe du broker, s'il y en a
  un, est rangé dans le Keystore Android, et <code>mqtts://</code> est accepté.</li>
</ul>

<p class="muted">
Détail du contrat MQTT, budget de ressources chiffré et limites :
<a href="https://github.com/JZacharie/M-pacer/blob/main/docs/10-suivi-temps-reel.md">docs/10 — Suivi en direct</a>.
</p>

## Appairer la montre

<ol class="steps">
  <li>Sur la montre, écran <strong>Sync</strong> → « S'appairer ». La montre demande un code au backend.</li>
  <li>Elle affiche un code court du type <code>BCDF-GHJK</code> en 28 sp, avec l'adresse à ouvrir.</li>
  <li>Sur le site web, on ouvre <a href="{{ '/site-web/' | relative_url }}">la page d'appairage</a> (/link), on saisit le code (le champ se formate
  tout seul) et on approuve.</li>
  <li>La montre sonde le jeton jusqu'à l'approbation, puis l'enregistre dans
  <strong>EncryptedSharedPreferences</strong> (clé AES-256-GCM gérée par le Keystore Android).</li>
  <li>Le jeton est révocable à tout moment depuis la page <strong>Jetons</strong> du site.</li>
</ol>

<div class="note">
<p><strong>Aucun secret Google sur la montre.</strong> Elle ne voit jamais le client OAuth : elle
manipule seulement un jeton opaque, propre à l'appareil, que l'on peut révoquer si la montre est perdue.</p>
</div>

## L'application téléphone (compagnon)

Un second module Android accompagne la montre, pour les cas où le téléphone est plus confortable
que le petit écran.

<div class="table-wrap">

| Écran | Ce qu'il fait |
|---|---|
| Connexion | Saisie de l'URL du backend, puis appairage par code (le téléphone ouvre directement la page d'approbation) |
| Séances | Statistiques des 30 derniers jours, liste des séances, rafraîchissement |
| Détail | Distance, durée, allure moyenne, nombre de tours et de points GPS, temps de passage |
| Envoi | Import d'un fichier de séance et envoi vers la montre par le Data Layer Wear OS |
| Montre | Ouvre la fiche de l'application montre sur le Play Store, ou partage un APK compilé |

</div>

<p class="tiny">
L'envoi vers la montre passe par <code>MessageClient</code> quand la charge utile reste sous
90 Ko, et bascule sur <code>DataClient</code> + <code>Asset</code> au-delà (trace GPS complète).
La montre déclare la capacité <code>mpacer_sync</code> et accepte les deux formes.
</p>

## Ce qui reste à faire

<div class="table-wrap">

| Chantier | État |
|---|---|
| Capteur de fréquence cardiaque | <span class="tag todo">À faire</span> Le cœur sait déjà tout traiter (zones, dérive, résumé), la lecture du capteur sur la montre n'est pas branchée |
| Boutons du casque audio | <span class="tag todo">À faire</span> Les commandes existent dans le cœur (double clic = annonce, triple clic = recalage de l'allure), aucun <code>MediaSession</code> ne les déclenche |
| Mode ambiant | <span class="tag todo">À faire</span> Aucune gestion d'<code>AmbientMode</code> : l'écran suit le comportement système |
| Musique et BPM | <span class="tag todo">Spécifié</span> Contrat complet dans la documentation technique, implémentation en cours |
| Validation terrain | <span class="tag todo">À faire</span> Écart visé sous 3 % avec une montre de référence, batterie sous 25 %/h |

</div>

<div class="warn">
<p>Les deux modules Android <strong>compilent</strong> (APK montre et téléphone produits et
installés sur une Galaxy Watch 6 de test), mais l'application n'a pas encore d'usage terrain
documenté. Une permission d'exécution incomplète et le branchement des réglages au moteur
figurent parmi les points ouverts de la revue de code.</p>
</div>

## Sous le capot

<div class="table-wrap">

| Fichier | Rôle |
|---|---|
| <code>MpacerCore.kt</code> | Pont JNI : commandes JSON vers la C ABI Rust, lecture de l'état du moteur |
| <code>TrackingService.kt</code> | Service de premier plan, boucle GPS 1 Hz (3 s en pause), notification, archivage et envoi en fin de séance |
| <code>VoiceCoach.kt</code> | Synthèse vocale et focus audio (baisser la musique, la mettre en pause, ou parler par-dessus) |
| <code>WorkoutArchive.kt</code> | Historique local des séances, dans le stockage privé de l'application |
| <code>SyncClient.kt</code> | Appairage par code, envoi des séances, jeton chiffré, identifiants déjà envoyés |
| <code>WearSyncListener.kt</code> | Réception des séances envoyées par le téléphone via le Data Layer |
| <code>ui/MainScreen.kt</code> | Écran principal rond : allure, distance, temps, assistant, commandes |
| <code>ui/SettingsScreen.kt</code> | Mode d'assistant, unités, voix |
| <code>ui/SyncScreen.kt</code> | Appairage, état de connexion, séances en attente |
| <code>ui/WatchDesign.kt</code> | Theme Wear et briques communes : bouton rond, pastille, ligne de réglage, écran secondaire |
| <code>ui/WatchIcons.kt</code> | Les seize icônes vectorielles de la montre, écrites à la main |
| <code>core/ui/Palette.kt</code> | Palette commune (montre, téléphone, site) et voyant GPS |
| <code>cpp/mpacer_jni.c</code> | Shim JNI (une quarantaine de lignes) vers la C ABI Rust |

</div>

<p class="tiny">
Prérequis de compilation, commandes Gradle et dépannage : voir
<a href="https://github.com/JZacharie/M-pacer/blob/main/android/README.md">android/README.md</a>.
</p>
