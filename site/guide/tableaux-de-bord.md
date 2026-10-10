---
layout: default
title: Tableaux de bord
description: Composer ses écrans M-pacer à partir des neuf widgets, partir d'un gabarit Pace Control, Analyse ou Historique, et les ordonner sans JavaScript.
permalink: /guide/tableaux-de-bord/
---

<span class="eyebrow">Guide utilisateur</span>
# Tableaux de bord

<p class="lead">
Le site ouvre sur une page d'accueil utile, mais chacun ne regarde pas les mêmes chiffres.
Les tableaux de bord assemblent les widgets que <em>vous</em> voulez voir en premier, dans
l'ordre que vous choisissez.
</p>

## Ouvrir le constructeur

<p>
Depuis la page <code>/dashboards</code>, le constructeur se présente comme un formulaire :
un nom, un gabarit de départ facultatif, puis une liste de widgets à cocher et à ordonner.
Aucune ligne de JavaScript n'est nécessaire : tout passe par le serveur, et le résultat est
enregistre tel quel.
</p>

## Les neuf widgets

<div class="table-wrap">

| Widget | Contenu |
|---|---|
| Allure | L'allure du dernier tour et l'allure courante |
| Résumé de séance | Distance, durée, allure moyenne, cardio |
| Carte GPS | La trace sur OpenStreetMap |
| Tours | Les temps de passage, tour par tour |
| Meilleures distances | Les meilleurs 1, 5 et 10 km de la période |
| Cardio | Zones, moyenne et maximum |
| Historique | Les dernières séances |
| Statistiques | Volume hebdomadaire, nombre de séances |
| Courses | Les prochaines échéances |

</div>

## Trois gabarits pour démarrer vite

<div class="grid">
  <div class="card">
    <h3>Pace Control</h3>
    <p>Allure, tours, meilleures distances : l'écran de course, celui qu'on regarde
    pendant la séance.</p>
  </div>
  <div class="card">
    <h3>Analyse de séance</h3>
    <p>Résumé, carte, tours, cardio, meilleures distances : l'écran d'après-course.</p>
  </div>
  <div class="card">
    <h3>Historique</h3>
    <p>Volume, séances récentes, prochaines courses : l'écran de suivi de la charge.</p>
  </div>
</div>

<p>
Choisir un gabarit remplit la liste en un clic ; il reste ensuite à retirer, ajouter ou
déplacer des widgets. Les trois gabarits recréent les écrans de l'application de référence
Pace Control, ce qui permet de retrouver ses habitudes sans les copier.
</p>

## Ordonner et supprimer

<ol class="steps">
  <li>Ouvrez le tableau de bord : chaque widget est un bloc de page, dans l'ordre
  d'enregistrement.</li>
  <li>Modifiez le tableau de bord pour changer le nom, les widgets ou leur ordre.</li>
  <li>La suppression se fait depuis la page du tableau de bord, après confirmation.</li>
</ol>

<div class="info">
<p><strong>Essayez sans compte.</strong> La page <a href="{{ '/composeur/' | relative_url }}">Composeur</a>
reproduit le constructeur dans le navigateur : cochez les widgets, ordonnez-les, partez d'un
gabarit, pour voir ce que donne un écran avant de l'enregistrer.</p>
</div>

<p class="tiny">
Catalogue, modèle de données et routes sont décrits dans
<a href="https://github.com/JZacharie/M-pacer/blob/main/docs/08-tableaux-de-bord.md">docs/08 — Tableaux
de bord</a>.
</p>
