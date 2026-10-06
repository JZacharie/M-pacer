---
layout: default
title: Architecture
description: Architecture de M-pacer : coeur Rust partage, pont JNI vers Wear OS, synchronisation idempotente, backend Axum, PostgreSQL et deploiement Kubernetes.
permalink: /architecture/
---

<span class="eyebrow">Sous le capot</span>
# Architecture

<p class="lead">
Trois principes tiennent tout le reste : la montre est la source de vérité, le calcul vit
dans un cœur Rust testable partout, et aucun secret tiers ne descend sur le poignet.
</p>

## Vue d'ensemble

<div class="schema">
  <div class="box"><strong>Montre Wear OS</strong><span>Kotlin, Wear Compose<br>GPS 1 Hz, TTS, écran rond</span></div>
  <div class="box"><strong>Cœur Rust</strong><span>mpacer-core<br>allure, tours, assistant, voix</span></div>
  <div class="box"><strong>Backend Rust</strong><span>Axum + PostgreSQL<br>API, site web, OAuth</span></div>
  <div class="box"><strong>Navigateur</strong><span>tableau de bord, analyse,<br>courses et planning</span></div>
</div>

<div class="table-wrap">

| Lien | Transport | Contenu |
|---|---|---|
| Montre → cœur | JNI / C ABI, JSON | Positions GPS, cardio, horloge, commandes ; le cœur renvoie un état complet à afficher |
| Montre → backend | HTTPS, jeton d'appareil | Résumé de séance (le même format que le fichier archivé) |
| Téléphone → montre | Data Layer Wear OS | Séance envoyée vers la montre, message ou asset selon la taille |
| Navigateur → backend | HTTPS, cookie de session | Pages rendues côté serveur |

</div>

## Le cœur Rust, une seule fois

<p>
Tous les algorithmes vivent dans <code>mpacer-core</code>, un crate pur : aucune entrée-sortie,
aucun accès réseau, aucun appel Android. Il est donc testable sur un PC, embarquable dans la montre
et réutilisé tel quel par le simulateur, le pont FFI, le client de synchronisation et le backend.
</p>

<div class="table-wrap">

| Module | Ce qu'il calcule |
|---|---|
| <code>pace.rs</code> | L'allure lissée sur 2 minutes et la détection du changement d'allure |
| <code>gps.rs</code> | Qualité du signal (rouge, orange, jaune, vert), filtre anti-aberration |
| <code>geo.rs</code> | Distance Haversine, détection de saut GPS |
| <code>workout.rs</code> | Machine à états : départ suspendu, pause, auto-pause, reprise |
| <code>lap.rs</code> | Tours au kilomètre ou au mile, allure du tour courant et du précédent |
| <code>race_plan.rs</code> | Plan de course, negative split, shadow runner |
| <code>assistant.rs</code> | Les quatre modes et le panneau affiché |
| <code>voice.rs</code> | Planification et rédaction des annonces (FR / EN) |
| <code>analysis.rs</code> | Temps de passage, plan contre réalisé, pauses, accélération, dénivelé |
| <code>cardio.rs</code> | Zones de fréquence cardiaque, bilan, dérive cardiaque |
| <code>best_distances.rs</code> | Meilleurs 1 / 5 / 10 km et 1 / 5 miles dans une séance |
| <code>history.rs</code> | Format d'échange <code>.pac</code> version 2 |
| <code>gpx.rs</code> | Export GPX 1.1, avec l'extension cardio |
| <code>remote_race.rs</code> | Protocole et classement de course à distance |
| <code>music.rs</code> | Playlists, BPM et adaptation du tempo au plan de course (spécification [docs/07](https://github.com/JZacharie/M-pacer/blob/main/docs/07-musique-bpm-et-playlists.md), intégration en cours) |
| <code>units.rs</code> | Unités métrique / impérial, formatage |
| <code>engine.rs</code> | Orchestrateur : une seule structure de sortie à afficher |

</div>

<div class="note">
<p><strong>Un seul point d'entrée.</strong> Le shell Android ne parle jamais aux modules
directement : il pousse des échantillons dans <code>PacerEngine</code> et affiche la structure
<code>EngineOutput</code> qui en sort. Le pont JNI reste donc minuscule (une quarantaine de lignes
de C), et aucun panic ne traverse la frontière : les erreurs remontent en JSON.</p>
</div>

## Synchronisation idempotente

<ol class="steps">
  <li>La montre archive la séance localement et mémorise les identifiants déjà acceptés par le serveur.</li>
  <li>Elle envoie le résumé sur <code>POST /api/v1/workouts</code> avec son jeton d'appareil.</li>
  <li>Le serveur valide, puis <strong>remplace</strong> la version existante si l'identifiant est déjà connu.</li>
  <li>Un renvoi après une coupure réseau ne crée donc jamais de doublon, et une séance corrigée
  remplace l'ancienne.</li>
</ol>

## Le backend

<div class="table-wrap">

| Élément | Rôle |
|---|---|
| <code>axum</code> | Serveur HTTP, routes API et pages web |
| <code>maud</code> | HTML typé, rendu côté serveur |
| <code>sqlx</code> + PostgreSQL | Requêtes paramétrées, migrations idempotentes rejouées au démarrage |
| <code>tower-http</code> | Compression, journalisation, limites |
| OAuth Google | Code + PKCE, vérification de l'identité par JWKS |
| Ressources embarquées | Le CSS et le JavaScript sont compilés dans le binaire |

</div>

## Déploiement

<p>
Le service se déploie avec un chart Helm unique : application, base PostgreSQL gérée par
CloudNativePG, ingress Traefik, certificat TLS et secret de session conservé entre les mises à jour.
L'application tourne sans privilège, avec un système de fichiers en lecture seule et des sondes
<code>/healthz</code> et <code>/readyz</code>.
</p>

<p class="tiny">
Détail complet : <a href="https://github.com/JZacharie/M-pacer/blob/main/deploy/README.md">deploy/README.md</a>.
</p>

## Ce qui est assumé

- **La montre est la source de vérité** : la synchronisation est un confort, jamais un prérequis
  pour courir.
- **Pas de secret Google sur la montre** : appairage par code, jeton opaque révocable.
- **Interface web rendue côté serveur** plutôt qu'une application WebAssembly : une seule image,
  aucun jeton exposé au JavaScript. L'API reste consommable par une application séparée si le
  besoin apparaît.
- **Un seul secret de session partagé** : le service est sans état, donc mise à jour sans coupure
  et montée en réplicats possible.
- **TLS désactivé par défaut vers PostgreSQL** (réseau interne du cluster), configurable pour un
  serveur externe.
- **Le format <code>.pac</code> est versionné** : une version future est refusée explicitement,
  les champs ajoutés par une version plus récente sont ignorés sans erreur.

## Qualité

<div class="table-wrap">

| Vérification | Résultat |
|---|---|
| Tests Rust | 123 tests : 93 cœur, 6 FFI, 23 backend (dont 18 d'intégration PostgreSQL), 1 test de documentation |
| Lints | <code>cargo clippy --all-targets</code> sans avertissement |
| Format | <code>cargo fmt --check</code> |
| Chart Helm | <code>helm lint</code> et rendu validés, installation à blanc acceptée par l'API du cluster |
| Image | construite et publiée par la CI, conteneur démarré avec sondes vertes |
| Bout en bout | appairage, envoi par le simulateur, séance visible dans l'API, le tableau de bord et la page détail |

</div>

<p class="tiny">
Les tests d'intégration du backend ont besoin d'un PostgreSQL joignable
(<code>MPACER_TEST_DATABASE_URL</code>) ; sans cette variable, ils s'arrêtent en affichant un
message explicite plutôt que d'échouer.
</p>
