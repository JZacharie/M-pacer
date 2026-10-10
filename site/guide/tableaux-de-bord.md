---
layout: default
title: Tableaux de bord
description: Composer ses ecrans M-pacer a partir des neuf widgets, partir d'un gabarit Pace Control, Analyse ou Historique, et les ordonner sans JavaScript.
permalink: /guide/tableaux-de-bord/
---

<span class="eyebrow">Guide utilisateur</span>
# Tableaux de bord

<p class="lead">
Le site ouvre sur une page d'accueil utile, mais chacun ne regarde pas les memes chiffres.
Les tableaux de bord assemblent les widgets que <em>vous</em> voulez voir en premier, dans
l'ordre que vous choisissez.
</p>

## Ouvrir le constructeur

<p>
Depuis la page <code>/dashboards</code>, le constructeur se presente comme un formulaire :
un nom, un gabarit de depart facultatif, puis une liste de widgets a cocher et a ordonner.
Aucune ligne de JavaScript n'est necessaire : tout passe par le serveur, et le resultat est
enregistre tel quel.
</p>

## Les neuf widgets

<div class="table-wrap">

| Widget | Contenu |
|---|---|
| Allure | L'allure du dernier tour et l'allure courante |
| Resume de seance | Distance, duree, allure moyenne, cardio |
| Carte GPS | La trace sur OpenStreetMap |
| Tours | Les temps de passage, tour par tour |
| Meilleures distances | Les meilleurs 1, 5 et 10 km de la periode |
| Cardio | Zones, moyenne et maximum |
| Historique | Les dernieres seances |
| Statistiques | Volume hebdomadaire, nombre de seances |
| Courses | Les prochaines echeances |

</div>

## Trois gabarits pour demarrer vite

<div class="grid">
  <div class="card">
    <h3>Pace Control</h3>
    <p>Allure, tours, meilleures distances : l'ecran de course, celui qu'on regarde
    pendant la seance.</p>
  </div>
  <div class="card">
    <h3>Analyse de seance</h3>
    <p>Resume, carte, tours, cardio, meilleures distances : l'ecran d'apres-course.</p>
  </div>
  <div class="card">
    <h3>Historique</h3>
    <p>Volume, seances recentes, prochaines courses : l'ecran de suivi de la charge.</p>
  </div>
</div>

<p>
Choisir un gabarit remplit la liste en un clic ; il reste ensuite a retirer, ajouter ou
deplacer des widgets. Les trois gabarits recréent les ecrans de l'application de reference
Pace Control, ce qui permet de retrouver ses habitudes sans les copier.
</p>

## Ordonner et supprimer

<ol class="steps">
  <li>Ouvrez le tableau de bord : chaque widget est un bloc de page, dans l'ordre
  d'enregistrement.</li>
  <li>Modifiez le tableau de bord pour changer le nom, les widgets ou leur ordre.</li>
  <li>La suppression se fait depuis la page du tableau de bord, apres confirmation.</li>
</ol>

<div class="info">
<p><strong>Essayez sans compte.</strong> La page <a href="{{ '/composeur/' | relative_url }}">Composeur</a>
reproduit le constructeur dans le navigateur : cochez les widgets, ordonnez-les, partez d'un
gabarit, pour voir ce que donne un ecran avant de l'enregistrer.</p>
</div>

<p class="tiny">
Catalogue, modele de donnees et routes sont decrits dans
<a href="https://github.com/JZacharie/M-pacer/blob/main/docs/08-tableaux-de-bord.md">docs/08 — Tableaux
de bord</a>.
</p>
