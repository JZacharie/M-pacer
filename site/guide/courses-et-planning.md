---
layout: default
title: Courses et planning
description: "Preparer une course dans M-pacer : fiche, dossard, hebergement, suivi, recherche dans le calendrier Finishers et planning des echeances."
permalink: /guide/courses-et-planning/
---

<span class="eyebrow">Guide utilisateur</span>
# Courses et planning

<p class="lead">
Le site ne sert pas qu'a relire le passe : il porte aussi ce que l'on prepare. Une course a
sa fiche (dossard, horaires, hotel, nutrition), ses elements a cocher, sa place dans le
planning — et le calendrier Finishers permet de la creer pre-remplie.
</p>

## La page Courses

<p>
<code>/courses</code> rassemble quatre informations d'un coup d'oeil : le nombre de courses a
venir, le <strong>prochain depart</strong> avec un compte a rebours vivant (J-48, puis les
heures et les minutes), le nombre d'elements encore a preparer, et les courses deja courues.
</p>

## Creer une course

<ol class="steps">
  <li><strong>Partir d'une recherche</strong> (recommande) : ouvrez
  <code>/courses/recherche</code>, filtrez le calendrier
  <a href="https://www.finishers.com/ou-courir/europe/france" target="_blank" rel="noopener noreferrer">Finishers</a>
  par mot-cle, region, discipline, mois, annee et bornes de distance.</li>
  <li>Cliquez sur <em>Creer la course</em> : la fiche s'ouvre <strong>pre-remplie</strong>
  (nom, date, distance, discipline, ville, coordonnees et lien d'inscription). Il ne reste
  qu'a completer le reste.</li>
  <li><strong>Ou partir d'une fiche vide</strong> : <code>/courses/nouvelle</code>. Utile
  pour une sortie organisee qui n'est pas au calendrier, ou un projet personnel.</li>
</ol>

## Remplir la fiche

<div class="table-wrap">

| Bloc | Ce qu'on y met |
|---|---|
| Course | Nom, date, distance, discipline, ville, lien d'inscription, trace GPX |
| Depart | Horaire et lieu de depart, numero de dossard, lien du live |
| Hebergement | Hotel (nom, adresse, telephone), heure d'arrivee et de depart, autres solutions pour dormir |
| Dossard | Rendez-vous de prise de dossard (lieu, horaire) |
| Nutrition | Ravitaillement, plan de nutrition, informations utiles |
| Informations importantes | Certificat medical, PPS, consignes, barrieres horaires |
| Suivi | Elements a cocher : « dossard retire », « hotel reserve », etc. |

</div>

<p>
Les elements de suivi se creent et se cochent directement depuis la fiche. Le compteur
d'elements restants remonte sur la page Courses : c'est la liste de courses a faire avant le
jour J.
</p>

## Le planning

<p>
<code>/courses/planning</code> met toutes les echeances a venir bout a bout :
le depart, la prise de dossard, l'arrivee et le depart de l'hotel, et les elements de suivi
dates. Le tout est trie chronologiquement, ce qui donne la semaine type d'avant-course sans
ouvrir chaque fiche.
</p>

## Carte, trace et confidentialite

<div class="note">
<p><strong>La carte d'une course est un lien, pas une carte interactive.</strong> Elle ouvre
OpenStreetMap sur les coordonnees si elles sont renseignees, sinon sur le nom du lieu. Aucun
script tiers n'est charge et aucune donnee ne part vers un service de cartographie. Les liens
saisis dans une fiche sont limites a <code>http://</code> et <code>https://</code> : un lien
stocke ne peut pas devenir du code executable dans la page.</p>
</div>

<p>
Quand la fiche porte une trace GPX (<code>/courses/&lt;id&gt;/trace.gpx</code>), elle sert
aussi au suivi en direct : la page <code>/live</code> peut situer le coureur <em>sur le
parcours planifie</em>, pas seulement sur une carte.
</p>

## Importer une ancienne course

<p>
<code>/courses/importer</code> accepte un export Strava ou Garmin en <strong>GPX ou
TCX</strong> : la trace devient une seance, analysable comme les autres. Pratique pour
remettre un historique dans M-pacer sans le refaire a la main.
</p>

<div class="grid">
  <a class="card" href="{{ '/guide/apres-la-course/' | relative_url }}">
    <h3>Relire une seance</h3>
    <p>L'analyse complete d'une trace, importee ou courue avec M-pacer.</p>
  </a>
  <a class="card" href="{{ '/guide/amis-et-partage/' | relative_url }}">
    <h3>Suivi en direct</h3>
    <p>Faire suivre sa course a ses proches, sans compte tiers.</p>
  </a>
</div>
