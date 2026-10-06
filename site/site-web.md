---
layout: default
title: Le site web
description: Ce que fait le site M-pacer : tableau de bord des seances, analyse complete, statistiques, courses et planning, appairage des montres, API et export GPX.
permalink: /site-web/
---

<span class="eyebrow">Interface web auto-hébergée</span>
# Le site web : relire, comprendre, préparer

<p class="lead">
Le site est la seconde moitié du produit. Il ne sert pas seulement à relire le passé :
il analyse chaque séance en détail, suit le volume d'entraînement, et gère les courses
à venir — dossard, horaires, hébergement, éléments à préparer.
</p>

<div class="mockups">
  <figure class="mockup">
    <svg class="phone" viewBox="0 0 180 320" role="img" aria-label="Page des séances sur téléphone">
      <rect x="4" y="4" width="172" height="312" rx="22" fill="#0b0d10" stroke="rgba(255,255,255,.14)" stroke-width="2"/>
      <text x="20" y="42" font-family="system-ui, sans-serif" font-size="15" font-weight="700" fill="#f3f5f8">Vos séances</text>
      <g font-family="system-ui, sans-serif">
        <rect x="14" y="58" width="152" height="46" rx="12" fill="#16191f" stroke="rgba(255,255,255,.08)"/>
        <text x="26" y="78" font-size="11" fill="#f3f5f8">mardi 6 octobre</text>
        <text x="26" y="94" font-size="10" fill="#98a2b3">12,04 km &#183; 5:02 /km</text>
        <text x="152" y="88" text-anchor="end" font-size="12" font-weight="700" fill="#fc4c02">1h00</text>
        <rect x="14" y="112" width="152" height="46" rx="12" fill="#16191f" stroke="rgba(255,255,255,.08)"/>
        <text x="26" y="132" font-size="11" fill="#f3f5f8">dimanche 4 octobre</text>
        <text x="26" y="148" font-size="10" fill="#98a2b3">21,10 km &#183; 5:28 /km</text>
        <text x="152" y="142" text-anchor="end" font-size="12" font-weight="700" fill="#fc4c02">1h55</text>
        <rect x="14" y="166" width="152" height="46" rx="12" fill="#16191f" stroke="rgba(255,255,255,.08)"/>
        <text x="26" y="186" font-size="11" fill="#f3f5f8">jeudi 1er octobre</text>
        <text x="26" y="202" font-size="10" fill="#98a2b3">8,00 km &#183; 4:58 /km</text>
        <text x="152" y="196" text-anchor="end" font-size="12" font-weight="700" fill="#fc4c02">39:44</text>
        <rect x="14" y="228" width="152" height="52" rx="12" fill="#16191f" stroke="rgba(252,76,2,.38)"/>
        <text x="26" y="248" font-size="10" fill="#98a2b3">PROCHAIN DÉPART</text>
        <text x="26" y="266" font-size="11" fill="#f3f5f8">Marathon de La Rochelle &#183; J-48</text>
        <g fill="#6b7480" font-size="9">
          <circle cx="26" cy="302" r="4"/><circle cx="56" cy="302" r="4" fill="#fc4c02"/>
          <circle cx="86" cy="302" r="4"/><circle cx="116" cy="302" r="4"/><circle cx="146" cy="302" r="4"/>
        </g>
      </g>
    </svg>
    <figcaption>Barre d'onglets basse sur téléphone</figcaption>
  </figure>
</div>

## Les pages

Le site est rendu côté serveur (bibliothèque <code>maud</code>, du HTML typé en Rust) :
une seule image à déployer, aucun jeton exposé au JavaScript, aucune chaîne de build front-end.

<div class="table-wrap">

| Page | Adresse | Ce qu'on y fait |
|---|---|---|
| Accueil | <code>/</code> | Présentation publique tant qu'on n'est pas connecté |
| Séances | <code>/</code> (connecté) | Statistiques des 30 derniers jours et historique paginé |
| Détail d'une séance | <code>/workouts/&#123;id&#125;</code> | L'analyse complète (voir ci-dessous) |
| Statistiques | <code>/stats</code> | Volume hebdomadaire, graphique et tableau |
| Courses | <code>/courses</code> | Cartes des courses à venir, courses déjà courues |
| Nouvelle course | <code>/courses/nouvelle</code> | Création d'une fiche |
| Fiche de course | <code>/courses/&#123;id&#125;</code> | Toutes les informations pratiques et le suivi |
| Planning | <code>/courses/planning</code> | Toutes les échéances à venir, mises bout à bout |
| Appairer | <code>/link</code> | Saisie du code affiché par la montre |
| Jetons | <code>/settings</code> | Appareils appairés, dernier envoi, révocation |
| Connexion | <code>/login</code> | Google, ou connexion de développement |

</div>

## L'analyse d'une séance

C'est la page la plus dense du site : tout est calculé à partir du résumé envoyé par la montre,
en une seule passe sur la trace.

<h3>Résumé</h3>

<p>
Distance, temps en mouvement, temps écoulé, allure moyenne, fréquence cardiaque moyenne et
maximale, dénivelé positif, et — s'il y en a — le nombre de pauses et leur durée cumulée.
</p>

<h3>Graphique multi-courbes</h3>

<p>
Un graphique SVG tracé par le serveur (aucune librairie de graphiques) superpose l'allure,
la fréquence cardiaque et l'altitude en fonction de la distance.
</p>

<h3>Plan de course contre réalisé</h3>

<p>
Quand la séance a été courue avec un plan (shadow runner), la page affiche la cible, le réalisé,
l'écart à l'arrivée, l'allure cible et l'allure réalisée, le pourcentage de negative split, puis
un <strong>écart cumulé kilomètre par kilomètre</strong> : barres vers le haut quand on est en
avance, vers le bas quand on est en retard.
</p>

<h3>Fréquence cardiaque</h3>

<p>
Cinq zones calculées en pourcentage de la fréquence cardiaque maximale (ou en réserve de FC si
une FC de repos est renseignée), avec le temps passé dans chacune, plus la
<strong>dérive cardiaque</strong> (découplage aérobie) entre la première et la seconde moitié :
le signe et l'amplitude disent si l'effort a été tenu à coût constant.
</p>

<h3>Temps de passage</h3>

<p>
Un tableau par tour : distance, temps, allure, écart d'allure au tour précédent, et — selon les
données disponibles — fréquence cardiaque moyenne, dénivelé et écart au plan.
</p>

<h3>Chronologie, pauses et accélération</h3>

<p>
Les pauses (automatiques ou manuelles) sont listées avec leur instant, leur distance et leur durée,
y compris quand la reprise a été automatique. L'analyse d'accélération donne l'allure de croisière
de référence et, pour chaque phase de départ ou de reprise après pause, le temps mis pour
l'atteindre, puis la répartition du temps entre accélération, allure stable et ralentissement.
</p>

<h3>Meilleures distances</h3>

<p>
Les meilleurs 1, 5 et 10 km (et 1 et 5 miles) contenus dans la séance, quel que soit l'endroit
où ils se trouvent dans la trace.
</p>

<h3>Export et suppression</h3>

<p>
Téléchargement du GPX 1.1 (avec l'extension de fréquence cardiaque Garmin) pour l'envoyer vers
Strava, Garmin ou OpenRunner, et suppression définitive de la séance.
</p>

## Les tableaux de bord

Chaque utilisateur compose ses propres écrans à partir d'un catalogue de neuf
widgets : allure, résumé de séance, carte GPS, tours, meilleures distances,
cardio, historique, statistiques et courses. Les widgets s'ajoutent, se
retirent, se réordonnent et se renomment, puis sont enregistrés.

Trois gabarits recréent en un clic les écrans de l'application de référence :
**Pace Control** (allure, tours, meilleures distances), **Analyse de séance**
(résumé, carte, tours, cardio, meilleures distances) et **Historique** (volume,
séances récentes, prochaines courses). Tout le constructeur fonctionne sans
JavaScript : des formulaires, des redirections, rien d'autre.

![Tableau de bord « Pace Control »]({{ '/assets/images/dashboards/pace-control.png' | relative_url }})

## Statistiques

<p>
Un histogramme SVG du volume hebdomadaire et un tableau semaine par semaine : nombre de séances,
distance, durée. De quoi vérifier que la charge monte sans à-coups.
</p>

## Les courses à venir

<p class="lead">
L'interface ne sert pas qu'à relire le passé : elle gère aussi ce que l'on prépare.
</p>

<div class="grid">
  <div class="card">
    <h3>Cartes des courses</h3>
    <p>Nombre de courses à venir, prochain départ avec un compte à rebours vivant (J-48, puis
    heures et minutes), éléments encore à préparer, courses déjà courues.</p>
  </div>
  <div class="card">
    <h3>Fiche de course</h3>
    <p>Numéro de dossard, horaire et lieu de départ, lien du live, hôtel (nom, adresse, téléphone,
    arrivée, départ), rendez-vous de prise de dossard, autres solutions pour dormir, nutrition et
    ravitaillement, informations importantes, autres informations.</p>
  </div>
  <div class="card">
    <h3>Planning</h3>
    <p>Toutes les échéances à venir rassemblées : départ, prise de dossard, arrivée et départ de
    l'hôtel, éléments de suivi datés. Trié chronologiquement.</p>
  </div>
  <div class="card">
    <h3>Suivi</h3>
    <p>Une liste d'éléments à cocher (« dossard retiré », « hôtel réservé »…), créés et cochés
    directement depuis la fiche.</p>
  </div>
</div>

<div class="note">
<p><strong>La carte d'une course est un lien, pas une carte interactive.</strong> Elle ouvre
OpenStreetMap sur les coordonnées si elles sont renseignées, sinon sur le nom du lieu. Aucun script
tiers n'est chargé et aucune donnée ne part vers un service de cartographie.</p>
</div>

## Sécurité et appairage

<ol class="steps">
  <li><strong>Navigateur : Google OAuth 2.0, code + PKCE.</strong> L'identité est vérifiée en
  validant l'<code>id_token</code> contre les clés publiques de Google (JWKS). Le jeton de session
  vit dans un cookie <code>HttpOnly</code> signé ; aucun mot de passe n'est stocké par M-pacer.</li>
  <li><strong>Montre : appairage par code.</strong> La montre demande un code, l'utilisateur le
  saisit sur <code>/link</code> et l'approuve. Le backend délivre alors un jeton opaque, propre à
  l'appareil, révocable depuis la page Jetons.</li>
  <li><strong>API : jeton d'appareil.</strong> Les échanges montre ↔ serveur utilisent
  <code>Authorization: Bearer</code>. Les jetons sont stockés hachés côté serveur.</li>
  <li><strong>Liens vérifiés.</strong> Dans une fiche de course, seuls <code>http://</code> et
  <code>https://</code> sont acceptés : un lien stocké ne peut pas devenir du code exécutable
  dans la page.</li>
</ol>

<div class="info">
<p>En développement, <code>MPACER_DEV_AUTH=1</code> affiche un bouton « Connexion développeur »
qui court-circuite Google : pratique pour travailler en local, à laisser désactivé en production.</p>
</div>

## L'expérience d'usage

- **Mobile d'abord.** Sur téléphone, la navigation devient une barre d'onglets basse (six entrées :
  Séances, Courses, Planning, Statistiques, Appairer, Jetons) ; sur grand écran, les mêmes entrées
  passent dans l'en-tête collant.
- **Thème sombre et clair.** Les couleurs suivent la préférence du système, avec une palette
  unique et un seul accent orange.
- **Écran et encoche.** Les zones sûres iOS sont prises en compte, la page se déclare en
  <code>color-scheme: dark light</code> et fournit une icône d'application.
- **Presque pas de JavaScript.** Le fichier servi fait moins de 100 lignes : formatage du code
  d'appairage, confirmation avant suppression, compte à rebours, apparitions discrètes au
  défilement. Le reste fonctionne sans script.
- **Accessibilité raisonnable.** Contrastes élevés, focus visible au clavier, respect de
  <code>prefers-reduced-motion</code>.

## L'API, pour les intégrateurs

Toutes les routes sont sous <code>/api/v1</code>, authentifiées par le jeton d'appareil, et
les erreurs respectent <code>{"error": "code", "message": "..."}</code>.

<div class="table-wrap">

| Méthode | Route | Rôle |
|---|---|---|
| <code>POST</code> | <code>/api/v1/device/code</code> | Demander un code d'appairage |
| <code>POST</code> | <code>/api/v1/device/token</code> | Échanger le code approuvé contre un jeton |
| <code>GET</code> | <code>/api/v1/me</code> | Identité et unités de l'utilisateur |
| <code>POST</code> | <code>/api/v1/workouts</code> | Envoyer une séance (idempotent : renvoyer remplace) |
| <code>GET</code> | <code>/api/v1/workouts</code> | Lister les séances (pagination, filtres) |
| <code>GET</code> / <code>DELETE</code> | <code>/api/v1/workouts/&#123;id&#125;</code> | Lire ou supprimer une séance |
| <code>GET</code> | <code>/api/v1/workouts/&#123;id&#125;/gpx</code> | Export GPX d'une séance |
| <code>GET</code> / <code>POST</code> | <code>/api/v1/races</code> | Lister ou créer une course |
| <code>GET</code> / <code>PUT</code> / <code>DELETE</code> | <code>/api/v1/races/&#123;id&#125;</code> | Lire, modifier ou supprimer une course |
| <code>GET</code> | <code>/api/v1/stats</code> | Statistiques agrégées |
| <code>GET</code> | <code>/api/v1/export</code> | Export complet au format <code>.pac</code> |
| <code>GET</code> | <code>/api/v1/version</code> | Version du service |
| <code>GET</code> | <code>/healthz</code>, <code>/readyz</code> | Sondes de vivacité et de disponibilité (base incluse) |

</div>

## Les données

<div class="table-wrap">

| Table | Contenu |
|---|---|
| <code>users</code> | Compte (identité Google), unités préférées |
| <code>api_tokens</code> | Jetons d'appareil, hachés, avec date de révocation |
| <code>device_codes</code> | Codes d'appairage en cours, avec expiration |
| <code>workouts</code> | Séances : distance, durée, allure, date, et le résumé complet en JSON |
| <code>races</code>, <code>race_tasks</code> | Fiches de course et éléments de suivi |
| <code>oauth_states</code> | États OAuth et vérificateurs PKCE, à usage unique |

</div>

<p class="tiny">
Les migrations SQL sont idempotentes et rejouées au démarrage du service : une mise à jour
d'image ne demande aucune intervention manuelle sur la base.
</p>

<div class="note">
<p><strong>En préparation : la musique.</strong> Une documentation technique complète décrit
l'ajout de playlists (fichiers personnels ou Spotify), la calibration tempo / allure et le
directeur d'orchestre qui adapte la playlist au plan de course. Le modèle de données et l'API
correspondants sont en cours d'ajout ; cette page sera complétée quand la fonctionnalité sera
livrée.</p>
</div>
