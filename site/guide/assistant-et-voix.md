---
layout: default
title: Assistant et voix
description: Les quatre modes d'assistant de M-pacer, les reglages du retour vocal, la langue, la frequence des annonces et ce qui arrive a la musique quand la voix parle.
permalink: /guide/assistant-et-voix/
---

<span class="eyebrow">Guide utilisateur</span>
# Assistant et voix

<p class="lead">
La montre peut se contenter d'afficher des chiffres, ou vous accompagner : estimer votre
heure d'arrivee, courir a votre place le role de meneur d'allure, et surtout <em>parler</em>.
Tout se regle une fois, avant de partir.
</p>

## Ce que dit la voix

Les annonces sont <strong>redigees par le coeur Rust</strong> (en francais ou en anglais) et
seulement <em>prononcees</em> par l'appareil. Exemple, en course :

~~~
Allure 5:03 par kilometre. Distance 0,98 km. Temps 5:00. Vous etes sur le plan.
~~~

<div class="table-wrap">

| Moment | Annonce |
|---|---|
| Au demarrage | La seance commence, le plan en quelques mots |
| Periodiquement | Allure, distance, temps, et l'ecart au plan — au rythme choisi |
| A chaque tour | L'allure du tour termine, avec l'allure du tour precedent si l'option est active |
| A la pause et a la reprise | L'etat de la seance, pour ne pas s'inquieter en s'arretant |
| Sur demande | Le bouton <em>Annonce vocale</em> (telephone) ou les commandes du casque audio, quand elles seront branchees |

</div>

## Regler la voix

<div class="table-wrap">

| Reglage | Valeurs | Remarque |
|---|---|---|
| Voix activee | oui / non | Le retour vocal complet s'eteint d'un coup |
| Frequence des annonces | 1, 2 ou 5 minutes, chaque tour, manuel | <em>Jamais</em> laisse la montre silencieuse |
| Langue | Francais / English | Les phrases sont ecrites par le coeur, la prononciation par le systeme |
| Detail du tour | oui / non | Ajoute l'allure du tour precedent |
| Formes courtes | oui / non | Phrases plus breves, pour les longues sorties |
| Quand la voix parle | Baisser la musique / Mettre en pause / Parler par-dessus | Le comportement du lecteur audio pendant l'annonce |

</div>

<div class="info">
<p><strong>La voix n'interrompt pas la seance.</strong> Le reglage « Quand la voix parle »
decide seulement de la maniere : baisser le volume (le plus discret), mettre la musique en
pause, ou parler par-dessus sans toucher au volume. Voir
<a href="{{ '/guide/musique/' | relative_url }}">Musique et tempo</a>.</p>
</div>

## Choisir un mode d'assistant

<p>
Les quatre modes et ce qu'ils affichent sont detailles dans
<a href="{{ '/guide/plan-de-course/' | relative_url }}">Preparer un plan d'allure</a>. En
resume :
</p>

<div class="table-wrap">

| Mode | Ce que la montre affiche en plus |
|---|---|
| <strong>Allure</strong> | Rien : allure, distance, temps |
| <strong>Finish estime</strong> | L'heure d'arrivee projetee, calculee a chaque instant |
| <strong>Temps vise</strong> (shadow runner) | L'ecart en metres au coureur virtuel, ou « sur le plan » |
| <strong>Course a distance</strong> | Protocole multijoueur present dans le coeur, interface a brancher |

</div>

## Le tempo musical

<p>
Quand une playlist est preparee avec un BPM cible, le moteur compare votre allure a ce
tempo et affiche une directive : <code>^</code> accelerer, <code>v</code> ralentir,
<code>&gt;&gt;</code> changer de piste. Les changements de tempo peuvent aussi etre
annonces a la voix, et le BPM de reference se regle dans les reglages Musique (par defaut le
BPM correspondant a 5:00 /km).
</p>

## Ce qui n'est pas encore branche

<p>
Par honnetete, deux morceaux manquent encore cote montre, et sont documentes comme points
ouverts du projet :
</p>

<ul>
  <li><strong>Les boutons du casque audio</strong> — double clic pour une annonce, triple
  clic pour recaler l'allure : le coeur sait les traiter, aucun <code>MediaSession</code> ne
  les capte pour l'instant.</li>
  <li><strong>Certains reglages de la montre</strong> (mode d'assistant, voix) etaient
  signales comme non relies au moteur lors de la revue de code du depot. Si un changement
  ne semble pas pris en compte, verifiez-le d'une seance a l'autre, et reportez le cas sur
  le depot.</li>
</ul>

<p class="tiny">
Points ouverts du depot :
<a href="https://github.com/JZacharie/M-pacer#8-limites-et-points-ouverts">README, section 8</a>.
</p>

<div class="grid">
  <a class="card" href="{{ '/guide/musique/' | relative_url }}">
    <h3>Musique et tempo</h3>
    <p>Preparer une playlist, l'envoyer sur la montre, suivre les directives de tempo.</p>
  </a>
  <a class="card" href="{{ '/guide/depannage/' | relative_url }}">
    <h3>Depannage</h3>
    <p>Pas de voix, pas de cardio, seance absente : les causes probables.</p>
  </a>
</div>
