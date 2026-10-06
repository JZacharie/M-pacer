---
layout: default
title: La montre Garmin
description: M-pacer sur une montre Garmin : application Connect IQ en Monkey C, meme coeur d'allure que la version Wear OS, enregistrement FIT, alertes par vibration et synchronisation vers le meme backend.
permalink: /montre-garmin/
---

<span class="eyebrow">Application Connect IQ</span>
# La montre Garmin : le même pacer, dans un fichier FIT

<p class="lead">
Votre Garmin peut courir avec M-pacer. L'allure lissée sur deux minutes, les
tours, l'assistant et le shadow runner sont calculés par le <strong>même
cœur</strong> que la montre Wear OS — porté en Monkey C — et la séance part vers
le même backend auto-hébergé.
</p>

<div class="mockups">
  <figure class="mockup">
    <svg class="watch" viewBox="0 0 200 200" role="img" aria-label="Écran course de l'application Garmin">
      <circle cx="100" cy="100" r="93" fill="#0b0d10" stroke="rgba(255,255,255,.14)" stroke-width="2"/>
      <circle cx="100" cy="24" r="6" fill="#2fbf71"/>
      <text x="100" y="86" text-anchor="middle" font-family="system-ui, sans-serif" font-size="42" font-weight="700" fill="#f3f5f8">5:12</text>
      <text x="100" y="108" text-anchor="middle" font-family="system-ui, sans-serif" font-size="11" fill="#98a2b3">Allure / min/km</text>
      <text x="100" y="132" text-anchor="middle" font-family="system-ui, sans-serif" font-size="18" font-weight="600" fill="#f3f5f8">8,42 km &#183; 42:10</text>
      <text x="100" y="152" text-anchor="middle" font-family="system-ui, sans-serif" font-size="12" fill="#98a2b3">Tour 480 m &#183; 5:07</text>
      <text x="100" y="172" text-anchor="middle" font-family="system-ui, sans-serif" font-size="13" font-weight="600" fill="#2fbf71">sur le plan</text>
    </svg>
    <figcaption>Écran course : allure, distance, temps, tour, écart au plan</figcaption>
  </figure>
</div>

## Ce qui change, ce qui ne change pas

<div class="table-wrap">

| | Wear OS | Garmin |
|---|---|---|
| Cœur métier | Rust compilé (JNI) | portage Monkey C, même formules et mêmes seuils |
| Retour au coureur | voix (TTS Android) | **vibrations et textes** : aucune synthèse vocale n'existe en Connect IQ |
| Enregistrement | service de premier plan, archive `.pac` | **fichier FIT** écrit par la montre, visible dans Garmin Connect |
| Synchronisation | HTTPS + jeton d'appareil | identique (appairage par code, `POST /api/v1/workouts`) |
| Musique | Media3, fichiers copiés en USB | non portée : Connect IQ ne lit pas de fichier audio |

</div>

## La même allure, le même plan

L'allure affichée est **moyennée sur deux minutes**, avec la détection de
changement d'allure pour le fractionné. Les tours de kilomètre (ou de mile) sont
interpolés au franchissement, en temps de course — une pause ne rallonge donc
jamais un tour. L'assistant reprend les trois modes embarqués : allure seule,
temps de finish estimé, temps visé avec **shadow runner** et negative split.

<div class="note">
<p><strong>Un seul cœur, deux montres.</strong> Les formules du plan de course
(allure de départ plus lente, accélération progressive, plan exact à l'arrivée),
les zones de fréquence cardiaque et le filtre anti-saut GPS sont ceux du cœur
Rust, portés module par module. Ce qui s'écrit sur une montre Garmin se relit
dans l'interface web comme n'importe quelle séance.</p>
</div>

## Les vibrations remplacent la voix

Garmin n'expose pas de synthèse vocale aux applications. M-pacer conserve donc
les mêmes déclencheurs — départ, pause, reprise, arrêt, tour franchi, annonce
périodique, écart au plan — et les traduit en **profils de vibration** (courte,
double, longue) accompagnés d'un texte affiché huit secondes.

## Les contraintes, prises au sérieux

<div class="table-wrap">

| Contrainte Connect IQ | Ce que fait M-pacer |
|---|---|
| 8 Ko par clé de stockage, 128 Ko au total | une séance en attente par clé, sans trace GPS ; historique borné |
| Mémoire applicative limitée | trace en tableaux de nombres, plafonnée, cardio sous-échantillonnée |
| Requêtes BLE de petite taille | trace GPS désactivée par défaut vers le backend ; elle reste dans le FIT |
| HTTPS via le téléphone appairé | même backend, même jeton révocable |

</div>

## Construire et installer

```powershell
# Pré-vol : XML, réglages, chaînes, compilation
pwsh ./garmin/build.ps1 -Check

# Exécutable pour votre modèle, puis copie dans GARMIN/APPS
pwsh ./garmin/build.ps1 -Device fr965 -Install
```

Les profils d'appareils s'installent avec le **SDK Manager** de Garmin (onglet
Devices) ; `garmin/build.ps1` sait aussi télécharger l'archive du SDK et
générer la clé de signature. Détail complet :
[garmin/README.md](https://github.com/JZacharie/M-pacer/blob/main/garmin/README.md)
et [docs/09](https://github.com/JZacharie/M-pacer/blob/main/docs/09-montre-garmin.md).

<div class="warn">
<p>L'application <strong>compile</strong> (SDK Connect IQ 9.2.0) et la CI
rejoue ce contrôle à chaque envoi de code. La simulation avec un profil
d'appareil et la validation terrain restent à faire.</p>
</div>
