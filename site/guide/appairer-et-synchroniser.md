---
layout: default
title: Appairer et synchroniser
description: Appairer une montre ou un téléphone a M-pacer par code court, envoyer les séances, revoquer un appareil et comprendre le fonctionnement hors réseau.
permalink: /guide/appairer-et-synchroniser/
---

<span class="eyebrow">Guide utilisateur</span>
# Appairer et synchroniser

<p class="lead">
L'appairage est le seul moment où l'on recopie quelque chose à la main, et il dure dix
secondes. Ensuite, tout est automatique — et rien ne depend du réseau : une séance reste sur
l'appareil jusqu'à ce qu'elle soit partie.
</p>

## Se connecter au site

<ol class="steps">
  <li>Ouvrez l'adresse de votre service M-pacer dans un navigateur.</li>
  <li><em>Continuer avec Google</em> : OAuth 2.0 avec code et PKCE. M-pacer ne stocke aucun
  mot de passe, et le jeton de session vit dans un cookie <code>HttpOnly</code> signe.</li>
  <li>En développement local seulement, un bouton <em>Connexion développeur</em> peut
  remplacer Google (<code>MPACER_DEV_AUTH=1</code>) : à laisser désactivé en production.</li>
</ol>

## Appairer un appareil par code

<div class="grid two">
  <div class="card">
    <h3>Sur la montre</h3>
    <p>Écran <strong>Sync</strong> &rsaquo; <em>S'appairer</em>. La montre demande un code au
    service et affiche un code court du type <code>BCDF-GHJK</code> avec l'adresse a
    ouvrir.</p>
  </div>
  <div class="card">
    <h3>Sur le téléphone</h3>
    <p>Réglages &rsaquo; <em>Synchronisation et backend</em> : saisissez l'adresse, puis
    appairez de la même façon — le téléphone peut ouvrir directement la page d'approbation.</p>
  </div>
</div>

<ol class="steps">
  <li>Dans le navigateur, ouvrez <code>/link</code>. Le champ formate le code au fur et a
  mesure de la frappe.</li>
  <li><strong>Approuvez.</strong> Le service delivre alors un <strong>jeton opaque propre a
  l'appareil</strong>.</li>
  <li>L'appareil sonde le service jusqu'à l'approbation, puis range son jeton dans un
  stockage chiffre (<code>EncryptedSharedPreferences</code>, cle AES-256-GCM geree par le
  Keystore Android).</li>
  <li>L'écran Sync affiche <em>Appareil appaire</em>, et le nombre de séances en attente.</li>
</ol>

<div class="note">
<p><strong>Aucun secret Google sur la montre.</strong> Elle ne voit jamais le client OAuth :
elle manipule un jeton opaque, révocable, qui ne donne accès qu'à votre propre compte.</p>
</div>

## Envoyer les séances

<div class="table-wrap">

| Situation | Ce qui se passe |
|---|---|
| Appareil appaire, réseau disponible | La séance part <strong>automatiquement</strong> à la fin de la séance |
| Appareil appaire, pas de réseau | La séance reste archivée localement ; l'écran Sync la compte dans les envois en attente |
| Appareil non appaire | Tout reste sur l'appareil ; appairez quand vous voulez, rien n'est perdu |
| Même séance envoyée deux fois | Le service <strong>remplace</strong> la version distante : aucun doublon |

</div>

<p>
Pour forcer un envoi : écran <strong>Sync</strong> &rsaquo; <em>Envoyer les séances</em>.
L'application d'appoint sait aussi envoyer une séance du téléphone vers la montre par le
Data Layer Wear OS, et importer une trace existante (GPX ou TCX) pour la retrouver sur le
site.
</p>

## Vérifier et revoquer un appareil

<p>
La page <strong>Jetons</strong> (<code>/settings</code>) liste les appareils appaires, leur
dernier envoi, et permet de <strong>revoquer</strong> un jeton. C'est le geste à faire si une
montre est perdue ou vendue : le jeton révoqué ne permet plus rien, et un nouvel appairage
en delivre un autre.
</p>

<div class="table-wrap">

| Route | Ce qu'on y fait |
|---|---|
| <code>/link</code> | Saisir et approuver un code d'appairage |
| <code>/settings</code> | Liste des jetons, dernier envoi, révocation |
| <code>/courses/importer</code> | Importer une ancienne course (export Strava ou Garmin) en GPX ou TCX |
| <code>/api/v1/export</code> | Export complet des données au format <code>.pac</code> |

</div>

## Comprendre le mode hors ligne

<p>
Le modèle est simple : <strong>l'appareil est la source de vérité</strong>. La séance est
écrite dans le stockage privé de l'application dès qu'elle est terminée, avec sa trace, ses
tours et sa fréquence cardiaque. La synchronisation est un confort, jamais un prérequis pour
courir.
</p>

<ul>
  <li>Un envoi rejoue ne crée pas de doublon : la cle est l'identifiant de la séance.</li>
  <li>Les séances en attente sont visibles sur l'écran Sync, avec leur nombre.</li>
  <li>Le site reste consultable pendant ce temps : il montre ce qui est déjà arrive.</li>
</ul>

## Si l'appairage ne va pas au bout

<div class="table-wrap">

| Symptôme | Cause probable | Quoi faire |
|---|---|---|
| Le code a expiré avant l'approbation | Le code d'appairage est a usage unique et limite dans le temps | Relancez <em>S'appairer</em> et saisissez le nouveau code |
| L'appareil reste sur « En attente d'approbation » | La page <code>/link</code> n'a pas été validée, ou un autre compte est connecté | Vérifiez le compte connecté, approuvez, laissez la montre sonder |
| « Appareil non appaire » après un redémarrage | Le jeton a été révoqué depuis la page Jetons | Reappairez : c'est volontaire et sans conséquence sur les séances |
| L'adresse du backend ne répond pas | Adresse fausse, service arrêté, ou réseau local inaccessible | Testez l'adresse dans le navigateur du téléphone avant de la saisir |

</div>

<p class="tiny">
Voir <a href="{{ '/guide/depannage/' | relative_url }}">Dépannage</a> pour les autres cas,
et <a href="{{ '/guide/amis-et-partage/' | relative_url }}">Amis et suivi en direct</a> pour
les trois conditions nécessaires au partage de position.
</p>
