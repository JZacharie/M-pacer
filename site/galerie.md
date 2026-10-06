---
layout: default
title: Galerie
description: Toutes les images de la documentation M-pacer - schéma d'architecture, écrans du site web et propositions de logo - agrandissables en plein écran.
permalink: /galerie/
---

<span class="eyebrow">Illustrations</span>
# Galerie

<p class="lead">
Les images de <code>docs/images</code>, réunies au même endroit : le schéma
d'architecture, les écrans du site web et les propositions de logo. Cliquez une
vignette pour l'ouvrir en plein écran — flèches <kbd>←</kbd> <kbd>→</kbd> pour
passer d'une image à l'autre, <kbd>Échap</kbd> pour fermer.
</p>

<div class="filtres" data-filters>
  <button type="button" class="actif" data-filter="*">Tout</button>
  {%- assign groupes = site.data.images | map: "group" | uniq -%}
  {%- for groupe in groupes %}
  <button type="button" data-filter="{{ groupe }}">{{ groupe }}</button>
  {%- endfor %}
</div>

<p class="galerie-compteur" data-galerie-compteur>{{ site.data.images.size }} images</p>

<div class="galerie" data-galerie>
  {%- for image in site.data.images %}
  <figure class="vignette" data-group="{{ image.group }}">
    <button type="button" class="vignette-bouton" data-zoom aria-label="Agrandir : {{ image.title }}">
      <img src="{{ image.file | prepend: '/assets/images/' | relative_url }}" alt="{{ image.alt | default: image.title }}" loading="lazy">
    </button>
    <figcaption>
      <strong>{{ image.title }}</strong>
      <span>{{ image.caption }}</span>
    </figcaption>
  </figure>
  {%- endfor %}
</div>

<p class="tiny">
Les fichiers d'origine sont dans <code>docs/images/</code> (source de vérité, avec
le README et les documents techniques) ; le site publie dans
<code>site/assets/images/</code> une version allégée pour le web.
</p>
