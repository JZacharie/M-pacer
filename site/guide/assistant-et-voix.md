---
layout: default
title: Assistant et voix
description: Les quatre modes d'assistant de M-pacer, les réglages du retour vocal, la langue, la fréquence des annonces et ce qui arrive à la musique quand la voix parle.
permalink: /guide/assistant-et-voix/
---

<span class="eyebrow">Guide utilisateur</span>
# Assistant et voix

<p class="lead">
La montre peut se contenter d'afficher des chiffres, ou vous accompagner : estimer votre
heure d'arrivée, courir à votre place le rôle de meneur d'allure, et surtout <em>parler</em>.
Tout se règle une fois, avant de partir.
</p>

## Ce que dit la voix

Les annonces sont <strong>rédigées par le cœur Rust</strong> (en francais ou en anglais) et
seulement <em>prononcées</em> par l'appareil. Exemple, en course :

~~~
Allure 5:03 par kilomètre. Distance 0,98 km. Temps 5:00. Vous êtes sur le plan.
~~~

<div class="table-wrap">

| Moment | Annonce |
|---|---|
| Au démarrage | La séance commence, le plan en quelques mots |
| Périodiquement | Allure, distance, temps, et l'écart au plan — au rythme choisi |
| À chaque tour | L'allure du tour terminé, avec l'allure du tour précédent si l'option est activé |
| À la pause et à la reprise | L'état de la séance, pour ne pas s'inquiéter en s'arrêtant |
| Sur demande | Le bouton <em>Annonce vocale</em> (téléphone) ou les commandes du casque audio, quand elles seront branchées |

</div>

## Régler la voix

<div class="table-wrap">

| Réglage | Valeurs | Remarque |
|---|---|---|
| Voix activée | oui / non | Le retour vocal complet s'éteint d'un coup |
| Fréquence des annonces | 1, 2 ou 5 minutes, chaque tour, manuel | <em>Jamais</em> laisse la montre silencieuse |
| Langue | Francais / English | Les phrases sont écrites par le cœur, la prononciation par le système |
| Détail du tour | oui / non | Ajoute l'allure du tour précédent |
| Formes courtes | oui / non | Phrases plus brèves, pour les longues sorties |
| Quand la voix parle | Baisser la musique / Mettre en pause / Parler par-dessus | Le comportement du lecteur audio pendant l'annonce |

</div>

<div class="info">
<p><strong>La voix n'interrompt pas la séance.</strong> Le réglage « Quand la voix parle »
décide seulement de la manière : baisser le volume (le plus discret), mettre la musique en
pause, ou parler par-dessus sans toucher au volume. Voir
<a href="{{ '/guide/musique/' | relative_url }}">Musique et tempo</a>.</p>
</div>

## Choisir un mode d'assistant

<p>
Les quatre modes et ce qu'ils affichent sont détaillés dans
<a href="{{ '/guide/plan-de-course/' | relative_url }}">Préparer un plan d'allure</a>. En
résumé :
</p>

<div class="table-wrap">

| Mode | Ce que la montre affiche en plus |
|---|---|
| <strong>Allure</strong> | Rien : allure, distance, temps |
| <strong>Finish estimé</strong> | L'heure d'arrivée projetée, calculée à chaque instant |
| <strong>Temps visé</strong> (shadow runner) | L'écart en mètres au coureur virtuel, ou « sur le plan » |
| <strong>Course à distance</strong> | Protocole multijoueur présent dans le cœur, interface à brancher |

</div>

## Le tempo musical

<p>
Quand une playlist est préparée avec un BPM cible, le moteur compare votre allure à ce
tempo et affiche une directive : <code>^</code> accélérer, <code>v</code> ralentir,
<code>&gt;&gt;</code> changer de piste. Les changements de tempo peuvent aussi être
annonces à la voix, et le BPM de référence se règle dans les réglages Musique (par défaut le
BPM correspondant a 5:00 /km).
</p>

## Ce qui n'est pas encore branché

<p>
Par honnêteté, deux morceaux manquent encore côté montre, et sont documentés comme points
ouverts du projet :
</p>

<ul>
  <li><strong>Les boutons du casque audio</strong> — double clic pour une annonce, triple
  clic pour recaler l'allure : le cœur sait les traiter, aucun <code>MediaSession</code> ne
  les capte pour l'instant.</li>
  <li><strong>Certains réglages de la montre</strong> (mode d'assistant, voix) étaient
  signalés comme non reliés au moteur lors de la revue de code du dépôt. Si un changement
  ne semble pas pris en compte, vérifiez-le d'une séance à l'autre, et reportez le cas sur
  le dépôt.</li>
</ul>

<p class="tiny">
Points ouverts du dépôt :
<a href="https://github.com/JZacharie/M-pacer#8-limites-et-points-ouverts">README, section 8</a>.
</p>

<div class="grid">
  <a class="card" href="{{ '/guide/musique/' | relative_url }}">
    <h3>Musique et tempo</h3>
    <p>Préparer une playlist, l'envoyer sur la montre, suivre les directives de tempo.</p>
  </a>
  <a class="card" href="{{ '/guide/depannage/' | relative_url }}">
    <h3>Dépannage</h3>
    <p>Pas de voix, pas de cardio, séance absente : les causes probables.</p>
  </a>
</div>
