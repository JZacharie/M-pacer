---
layout: default
title: Appairer et synchroniser
description: Appairer une montre ou un telephone a M-pacer par code court, envoyer les seances, revoquer un appareil et comprendre le fonctionnement hors reseau.
permalink: /guide/appairer-et-synchroniser/
---

<span class="eyebrow">Guide utilisateur</span>
# Appairer et synchroniser

<p class="lead">
L'appairage est le seul moment ou l'on recopie quelque chose a la main, et il dure dix
secondes. Ensuite, tout est automatique — et rien ne depend du reseau : une seance reste sur
l'appareil jusqu'a ce qu'elle soit partie.
</p>

## Se connecter au site

<ol class="steps">
  <li>Ouvrez l'adresse de votre service M-pacer dans un navigateur.</li>
  <li><em>Continuer avec Google</em> : OAuth 2.0 avec code et PKCE. M-pacer ne stocke aucun
  mot de passe, et le jeton de session vit dans un cookie <code>HttpOnly</code> signe.</li>
  <li>En developpement local seulement, un bouton <em>Connexion developpeur</em> peut
  remplacer Google (<code>MPACER_DEV_AUTH=1</code>) : a laisser desactive en production.</li>
</ol>

## Appairer un appareil par code

<div class="grid two">
  <div class="card">
    <h3>Sur la montre</h3>
    <p>Ecran <strong>Sync</strong> &rsaquo; <em>S'appairer</em>. La montre demande un code au
    service et affiche un code court du type <code>BCDF-GHJK</code> avec l'adresse a
    ouvrir.</p>
  </div>
  <div class="card">
    <h3>Sur le telephone</h3>
    <p>Reglages &rsaquo; <em>Synchronisation et backend</em> : saisissez l'adresse, puis
    appairez de la meme facon — le telephone peut ouvrir directement la page d'approbation.</p>
  </div>
</div>

<ol class="steps">
  <li>Dans le navigateur, ouvrez <code>/link</code>. Le champ formate le code au fur et a
  mesure de la frappe.</li>
  <li><strong>Approuvez.</strong> Le service delivre alors un <strong>jeton opaque propre a
  l'appareil</strong>.</li>
  <li>L'appareil sonde le service jusqu'a l'approbation, puis range son jeton dans un
  stockage chiffre (<code>EncryptedSharedPreferences</code>, cle AES-256-GCM geree par le
  Keystore Android).</li>
  <li>L'ecran Sync affiche <em>Appareil appaire</em>, et le nombre de seances en attente.</li>
</ol>

<div class="note">
<p><strong>Aucun secret Google sur la montre.</strong> Elle ne voit jamais le client OAuth :
elle manipule un jeton opaque, revocable, qui ne donne acces qu'a votre propre compte.</p>
</div>

## Envoyer les seances

<div class="table-wrap">

| Situation | Ce qui se passe |
|---|---|
| Appareil appaire, reseau disponible | La seance part <strong>automatiquement</strong> a la fin de la seance |
| Appareil appaire, pas de reseau | La seance reste archivee localement ; l'ecran Sync la compte dans les envois en attente |
| Appareil non appaire | Tout reste sur l'appareil ; appairez quand vous voulez, rien n'est perdu |
| Meme seance envoyee deux fois | Le service <strong>remplace</strong> la version distante : aucun doublon |

</div>

<p>
Pour forcer un envoi : ecran <strong>Sync</strong> &rsaquo; <em>Envoyer les seances</em>.
L'application d'appoint sait aussi envoyer une seance du telephone vers la montre par le
Data Layer Wear OS, et importer une trace existante (GPX ou TCX) pour la retrouver sur le
site.
</p>

## Verifier et revoquer un appareil

<p>
La page <strong>Jetons</strong> (<code>/settings</code>) liste les appareils appaires, leur
dernier envoi, et permet de <strong>revoquer</strong> un jeton. C'est le geste a faire si une
montre est perdue ou vendue : le jeton revoque ne permet plus rien, et un nouvel appairage
en delivre un autre.
</p>

<div class="table-wrap">

| Route | Ce qu'on y fait |
|---|---|
| <code>/link</code> | Saisir et approuver un code d'appairage |
| <code>/settings</code> | Liste des jetons, dernier envoi, revocation |
| <code>/courses/importer</code> | Importer une ancienne course (export Strava ou Garmin) en GPX ou TCX |
| <code>/api/v1/export</code> | Export complet des donnees au format <code>.pac</code> |

</div>

## Comprendre le mode hors ligne

<p>
Le modele est simple : <strong>l'appareil est la source de verite</strong>. La seance est
ecrite dans le stockage prive de l'application des qu'elle est terminee, avec sa trace, ses
tours et sa frequence cardiaque. La synchronisation est un confort, jamais un prerequis pour
courir.
</p>

<ul>
  <li>Un envoi rejoue ne cree pas de doublon : la cle est l'identifiant de la seance.</li>
  <li>Les seances en attente sont visibles sur l'ecran Sync, avec leur nombre.</li>
  <li>Le site reste consultable pendant ce temps : il montre ce qui est deja arrive.</li>
</ul>

## Si l'appairage ne va pas au bout

<div class="table-wrap">

| Symptome | Cause probable | Quoi faire |
|---|---|---|
| Le code a expire avant l'approbation | Le code d'appairage est a usage unique et limite dans le temps | Relancez <em>S'appairer</em> et saisissez le nouveau code |
| L'appareil reste sur « En attente d'approbation » | La page <code>/link</code> n'a pas ete validee, ou un autre compte est connecte | Verifiez le compte connecte, approuvez, laissez la montre sonder |
| « Appareil non appaire » apres un redemarrage | Le jeton a ete revoque depuis la page Jetons | Reappairez : c'est volontaire et sans consequence sur les seances |
| L'adresse du backend ne repond pas | Adresse fausse, service arrete, ou reseau local inaccessible | Testez l'adresse dans le navigateur du telephone avant de la saisir |

</div>

<p class="tiny">
Voir <a href="{{ '/guide/depannage/' | relative_url }}">Depannage</a> pour les autres cas,
et <a href="{{ '/guide/amis-et-partage/' | relative_url }}">Amis et suivi en direct</a> pour
les trois conditions necessaires au partage de position.
</p>
