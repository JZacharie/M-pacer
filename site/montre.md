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
      <circle cx="100" cy="24" r="6" fill="#2fbf71"/>
      <text x="100" y="96" text-anchor="middle" font-family="system-ui, sans-serif" font-size="46" font-weight="700" fill="#f3f5f8">5:12</text>
      <text x="100" y="122" text-anchor="middle" font-family="system-ui, sans-serif" font-size="13" fill="#98a2b3">8,42 km &#183; 42:10</text>
      <text x="100" y="150" text-anchor="middle" font-family="system-ui, sans-serif" font-size="14" font-weight="600" fill="#2fbf71">sur le plan</text>
      <g font-family="system-ui, sans-serif" font-size="11" font-weight="600">
        <rect x="46" y="164" width="48" height="22" rx="11" fill="#fc4c02"/>
        <text x="70" y="179" text-anchor="middle" fill="#ffffff">Pause</text>
        <rect x="104" y="164" width="46" height="22" rx="11" fill="#242933"/>
        <text x="127" y="179" text-anchor="middle" fill="#f3f5f8">Stop</text>
      </g>
    </svg>
    <figcaption>Écran principal, pensé pour un écran rond</figcaption>
  </figure>
</div>

## L'écran principal

Une seule information domine : **l'allure courante**, en 54 sp, lissée sur deux minutes.
Tout le reste est secondaire et tient dans le cercle.

<div class="table-wrap">

| Élément | Ce qu'il montre | Détail |
|---|---|---|
| Allure | La valeur lue en courant | Moyenne glissante de 2 minutes ; <code>--:--</code> tant qu'aucune position exploitable n'est reçue |
| Distance et temps | <code>8,42 km &#183; 42:10</code> | Distance cumulée et temps en mouvement, pauses exclues |
| Voyant GPS | Un point de 11 px au-dessus de l'allure | Vert fixe quand le signal est bon, pulsation orange pendant l'acquisition, jaune si la précision est insuffisante, rouge si le GPS est coupé |
| Panneau assistant | Une ligne sous les métriques | « sur le plan », l'écart au shadow runner (<code>+120 m</code> / <code>-80 m</code>), ou le temps de finish estimé |
| Boutons | Démarrer / Sync / Réglages, ou Pause / Stop | L'état <code>Armed</code> (départ au premier mouvement) bascule sur Reprendre / Stop |

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
| <code>ui/Theme.kt</code> | Palette et voyant GPS |
| <code>cpp/mpacer_jni.c</code> | Shim JNI (une quarantaine de lignes) vers la C ABI Rust |

</div>

<p class="tiny">
Prérequis de compilation, commandes Gradle et dépannage : voir
<a href="https://github.com/JZacharie/M-pacer/blob/main/android/README.md">android/README.md</a>.
</p>
