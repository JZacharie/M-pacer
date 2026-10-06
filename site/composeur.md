---
layout: default
title: Composeur de tableaux de bord
description: Démonstration interactive du composeur de tableaux de bord M-pacer - neuf widgets, trois gabarits, ordre libre.
permalink: /composeur/
---

<span class="eyebrow">Démonstration interactive</span>
# Composez votre tableau de bord

<p class="lead">
Le site web n'affiche pas des pages figées : chaque écran est une liste ordonnée
de widgets. Cette page reproduit le constructeur de l'application — cochez,
montez, descendez, partez d'un gabarit — sans rien envoyer nulle part.
</p>

<div class="composeur" data-composeur>
  <div class="composeur-carte">
    <h3>1. Le catalogue</h3>
    <ul class="composeur-catalogue">
      {%- for widget in site.data.widgets.widgets %}
      <li>
        <label>
          <input type="checkbox" data-widget value="{{ widget.key }}">
          <strong>{{ widget.label }}</strong>
          <span>{{ widget.description }}</span>
        </label>
      </li>
      {%- endfor %}
    </ul>
  </div>

  <div class="composeur-carte">
    <div class="aprecu-entete">
      <h3>2. Votre tableau</h3>
      <span class="aprecu-compteur" data-composeur-compteur>0 widget</span>
    </div>
    <div class="gabarits">
      {%- for gabarit in site.data.widgets.templates %}
      <button type="button" data-gabarit data-widgets="{{ gabarit.widgets | join: ',' }}" title="{{ gabarit.description }}">{{ gabarit.name }}</button>
      {%- endfor %}
      <button type="button" data-composeur-vider>Vider</button>
    </div>
    <p class="aprecu-vide" data-composeur-vide hidden>Choisissez un widget dans le catalogue, ou partez d'un gabarit.</p>
    <ol class="aprecu-liste" data-composeur-liste></ol>
    <p class="composeur-note">
      L'application enregistre exactement cette composition, avec le même vocabulaire
      de clés. Elle est décrite dans
      <a href="https://github.com/JZacharie/M-pacer/blob/main/docs/08-tableaux-de-bord.md">docs/08</a>
      et se consulte dans la page <a href="{{ '/site-web/' | relative_url }}">Le site web</a>.
    </p>
  </div>
</div>

<p class="tiny">
Le catalogue affiché ici vient de <code>site/_data/widgets.yml</code>, qui reprend les clés
persistées par l'application : le site et le produit ne peuvent pas diverger.
</p>
