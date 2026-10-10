---
layout: default
title: Guide utilisateur
description: Le mode d'emploi de M-pacer, de l'installation à l'analyse d'une séance, pour la montre Wear OS, l'application téléphone et le site web auto-hébergé.
permalink: /guide/
---

<span class="eyebrow">Guide utilisateur</span>
# Le guide utilisateur

<p class="lead">
Ce guide explique <strong>comment s'en servir</strong>. Il suit l'ordre dans lequel on
rencontre le produit : installer, appairer, régler un plan, courir, puis relire la séance
et préparer la suivante. Chaque page tient en une tâche, avec les gestes exacts et ce qui
doit se passer à l'écran.
</p>

<div class="note">
<p>Vous cherchez plutôt <em>ce que fait</em> le produit et <em>pourquoi</em> il est conçu
ainsi ? Les pages <a href="{{ '/montre/' | relative_url }}">La montre</a> et
<a href="{{ '/site-web/' | relative_url }}">Le site web</a> racontent le produit ;
la <a href="{{ '/demarrage/' | relative_url }}">page Démarrage</a> s'adresse à ceux qui
compilent et déploient. Ce guide-ci s'adresse au coureur.</p>
</div>

## Par où commencer

<div class="grid">
  <a class="card" href="{{ '/guide/installer/' | relative_url }}">
    <h3>1. Installer</h3>
    <p>APK de la montre et du téléphone, backend, première connexion.
    Rien à compiler : les versions publiées sont prêtes à installer.</p>
  </a>
  <a class="card" href="{{ '/guide/premiere-seance/' | relative_url }}">
    <h3>2. Courir une première fois</h3>
    <p>De la montre posée sur la table jusqu'à la séance affichée sur le site,
    étape par étape, avec la liste des vérifications d'avant-départ.</p>
  </a>
  <a class="card" href="{{ '/guide/apres-la-course/' | relative_url }}">
    <h3>3. Relire sa séance</h3>
    <p>Le résumé, les courbes, le plan contre le réalisé, les zones cardiaques,
    les pauses et l'export GPX.</p>
  </a>
</div>

## Les pages du guide

<div class="table-wrap">

| Page | La question a laquelle elle répond |
|---|---|
| [Installer M-pacer]({{ '/guide/installer/' | relative_url }}) | Quels fichiers prendre, sur quels appareils, et comment entrer son adresse de backend ? |
| [Votre première séance]({{ '/guide/premiere-seance/' | relative_url }}) | Que faut-il faire dans l'ordre pour courir et retrouver sa séance sur le site ? |
| [Pendant la course]({{ '/guide/pendant-la-course/' | relative_url }}) | Que montrent les écrans, quels gestes, que signifient les couleurs et les commandes ? |
| [Préparer un plan d'allure]({{ '/guide/plan-de-course/' | relative_url }}) | Comment régler une distance, un temps visé et un negative split, et lire l'écart au plan ? |
| [Assistant et voix]({{ '/guide/assistant-et-voix/' | relative_url }}) | Quels sont les quatre modes d'assistant et que dit la voix, à quel moment ? |
| [Appairer et synchroniser]({{ '/guide/appairer-et-synchroniser/' | relative_url }}) | Comment lier la montre au site, envoyer une séance, et que se passe-t-il sans réseau ? |
| [Lire l'analyse d'une séance]({{ '/guide/apres-la-course/' | relative_url }}) | Que veulent dire dérive cardiaque, écart au plan, temps de passage, accélération ? |
| [Tableaux de bord]({{ '/guide/tableaux-de-bord/' | relative_url }}) | Comment composer ses propres écrans à partir des neuf widgets ? |
| [Courses et planning]({{ '/guide/courses-et-planning/' | relative_url }}) | Comment préparer une course, chercher dans le calendrier Finishers et suivre ses échéances ? |
| [Musique et tempo]({{ '/guide/musique/' | relative_url }}) | Comment préparer une playlist, l'envoyer sur la montre et suivre le tempo de la séance ? |
| [Amis et suivi en direct]({{ '/guide/amis-et-partage/' | relative_url }}) | Comment courir à plusieurs, partager sa position et suivre une séance en direct ? |
| [Dépannage]({{ '/guide/depannage/' | relative_url }}) | Quel est le problème, quelle est la cause probable, et que faire ? |

</div>

## Le parcours complet, en une image

<ol class="steps">
  <li><strong>Installer</strong> l'application sur la montre (ou sur le téléphone, ou les deux)
  et ouvrir un compte sur le site. Le site et les applications sont <em>auto-hébergés</em> :
  vous saisissez l'adresse de votre service, pas celle d'un tiers.</li>
  <li><strong>Appairer</strong> l'appareil avec un code court affiche par la montre et saisi
  sur la page <a href="{{ '/guide/appairer-et-synchroniser/' | relative_url }}">/link</a>.
  Un jeton opaque, révocable, suffit ensuite à tous les échanges.</li>
  <li><strong>Régler un plan</strong> : distance de course, temps visé, part négative. Le plan
  sert au <em>shadow runner</em>, le coureur virtuel qui court exactement l'allure cible.</li>
  <li><strong>Courir</strong>. La montre lit le GPS à 1 Hz, lisse l'allure sur deux minutes,
  découpe les tours, parle, et archive la séance en local. Le réseau n'est jamais nécessaire.</li>
  <li><strong>Synchroniser</strong>, automatiquement en fin de séance si l'appareil est appaire.
  Renvoyer la même séance remplace la version distante : aucun doublon.</li>
  <li><strong>Relire</strong> sur le site : analyse complète, plan contre réalise, zones
  cardiaques, pauses, meilleures distances, export GPX — puis la note de course et la
  préparation de la prochaine échéance.</li>
</ol>

## Ce que le guide suppose

- **Une montre Wear OS 3 ou plus** (Android 11 / API 30) pour l'application montre, ou
  **un téléphone Android 8 ou plus** (API 26) pour courir avec le téléphone seul.
- **Un GPS** et, si vous voulez la fréquence cardiaque au téléphone, **une ceinture
  Bluetooth LE** : le capteur optique de la montre n'est pas encore branché (voir les
  <a href="{{ '/guide/depannage/' | relative_url }}">limites connues</a>).
- **Un service M-pacer joignable** — le votre, ou une instance que l'on vous a ouverte —
  pour la synchronisation, le site et les amis.

<div class="info">
<p><strong>Version de référence de ce guide : </strong>{{ site.data.version.version }}.
Le pied de page affiche la version du site et son jour de construction ; les applications
affichent la même version dans leurs réglages, ce qui permet de savoir d'un coup d'œil ce
qui tourne sur chaque appareil.</p>
</div>

## En cas de doute

Les deux règles qui expliquent presque tous les comportements surprenants :

1. **La montre est la source de vérité.** Une séance peut toujours rester sur la montre
   et partir plus tard ; rien ne se perd parce que le réseau manque.
2. **Tout le calcul vit dans un cœur Rust unique.** Montre, téléphone et site sortent
   exactement les mêmes chiffres : si deux écrans diffèrent, c'est un problème de
   synchronisation, pas de calcul.

<p class="tiny">
Guide écrit pour la version {{ site.data.version.version }} du projet. Les pages produit
détaillées : <a href="{{ '/montre/' | relative_url }}">La montre</a>,
<a href="{{ '/montre-garmin/' | relative_url }}">Montre Garmin</a>,
<a href="{{ '/site-web/' | relative_url }}">Le site web</a>,
<a href="{{ '/architecture/' | relative_url }}">Architecture</a>.
La documentation technique est dans le dépôt, sous <code>docs/</code>.
</p>
