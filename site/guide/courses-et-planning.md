---
layout: default
title: Courses et planning
description: "Préparer une course dans M-pacer : fiche, dossard, hébergement, suivi, recherche dans le calendrier Finishers et planning des échéances."
permalink: /guide/courses-et-planning/
---

<span class="eyebrow">Guide utilisateur</span>
# Courses et planning

<p class="lead">
Le site ne sert pas qu'à relire le passé : il porte aussi ce que l'on prépare. Une course a
sa fiche (dossard, horaires, hôtel, nutrition), ses éléments à cocher, sa place dans le
planning — et le calendrier Finishers permet de la créer pre-remplie.
</p>

## La page Courses

<p>
<code>/courses</code> rassemble quatre informations d'un coup d'œil : le nombre de courses a
venir, le <strong>prochain départ</strong> avec un compte a rebours vivant (J-48, puis les
heures et les minutes), le nombre d'éléments encore à préparer, et les courses déjà courues.
</p>

## Créer une course

<ol class="steps">
  <li><strong>Partir d'une recherche</strong> (recommande) : ouvrez
  <code>/courses/recherche</code>, filtrez le calendrier
  <a href="https://www.finishers.com/ou-courir/europe/france" target="_blank" rel="noopener noreferrer">Finishers</a>
  par mot-cle, region, discipline, mois, annee et bornes de distance.</li>
  <li>Cliquez sur <em>Créer la course</em> : la fiche s'ouvre <strong>pre-remplie</strong>
  (nom, date, distance, discipline, ville, coordonnées et lien d'inscription). Il ne reste
  qu'à compléter le reste.</li>
  <li><strong>Où partir d'une fiche vide</strong> : <code>/courses/nouvelle</code>. Utile
  pour une sortie organisée qui n'est pas au calendrier, ou un projet personnel.</li>
</ol>

## Remplir la fiche

<div class="table-wrap">

| Bloc | Ce qu'on y met |
|---|---|
| Course | Nom, date, distance, discipline, ville, lien d'inscription, trace GPX |
| Départ | Horaire et lieu de départ, numéro de dossard, lien du live |
| Hébergement | Hôtel (nom, adresse, téléphone), heure d'arrivée et de départ, autres solutions pour dormir |
| Dossard | Rendez-vous de prise de dossard (lieu, horaire) |
| Nutrition | Ravitaillement, plan de nutrition, informations utiles |
| Informations importantes | Certificat médical, PPS, consignes, barrières horaires |
| Suivi | Éléments à cocher : « dossard retire », « hôtel réserve », etc. |

</div>

<p>
Les éléments de suivi se creent et se cochent directement depuis la fiche. Le compteur
d'éléments restants remonte sur la page Courses : c'est la liste de courses à faire avant le
jour J.
</p>

## Le planning

<p>
<code>/courses/planning</code> met toutes les échéances à venir bout a bout :
le départ, la prise de dossard, l'arrivée et le départ de l'hôtel, et les éléments de suivi
dates. Le tout est trie chronologiquement, ce qui donne la semaine type d'avant-course sans
ouvrir chaque fiche.
</p>

## Carte, trace et confidentialité

<div class="note">
<p><strong>La carte d'une course est un lien, pas une carte interactive.</strong> Elle ouvre
OpenStreetMap sur les coordonnées si elles sont renseignées, sinon sur le nom du lieu. Aucun
script tiers n'est charge et aucune donnée ne part vers un service de cartographie. Les liens
saisis dans une fiche sont limites a <code>http://</code> et <code>https://</code> : un lien
stocke ne peut pas devenir du code exécutable dans la page.</p>
</div>

<p>
Quand la fiche porte une trace GPX (<code>/courses/&lt;id&gt;/trace.gpx</code>), elle sert
aussi au suivi en direct : la page <code>/live</code> peut situer le coureur <em>sur le
parcours planifie</em>, pas seulement sur une carte.
</p>

## Importer une ancienne course

<p>
<code>/courses/importer</code> accepte un export Strava ou Garmin en <strong>GPX ou
TCX</strong> : la trace devient une séance, analysable comme les autres. Pratique pour
remettre un historique dans M-pacer sans le refaire à la main.
</p>

<div class="grid">
  <a class="card" href="{{ '/guide/apres-la-course/' | relative_url }}">
    <h3>Relire une séance</h3>
    <p>L'analyse complète d'une trace, importée ou courue avec M-pacer.</p>
  </a>
  <a class="card" href="{{ '/guide/amis-et-partage/' | relative_url }}">
    <h3>Suivi en direct</h3>
    <p>Faire suivre sa course à ses proches, sans compte tiers.</p>
  </a>
</div>
