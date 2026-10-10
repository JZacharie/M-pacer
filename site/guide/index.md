---
layout: default
title: Guide utilisateur
description: Le mode d'emploi de M-pacer, de l'installation a l'analyse d'une seance, pour la montre Wear OS, l'application telephone et le site web auto-heberge.
permalink: /guide/
---

<span class="eyebrow">Guide utilisateur</span>
# Le guide utilisateur

<p class="lead">
Ce guide explique <strong>comment s'en servir</strong>. Il suit l'ordre dans lequel on
rencontre le produit : installer, appairer, regler un plan, courir, puis relire la seance
et preparer la suivante. Chaque page tient en une tache, avec les gestes exacts et ce qui
doit se passer a l'ecran.
</p>

<div class="note">
<p>Vous cherchez plutot <em>ce que fait</em> le produit et <em>pourquoi</em> il est concu
ainsi ? Les pages <a href="{{ '/montre/' | relative_url }}">La montre</a> et
<a href="{{ '/site-web/' | relative_url }}">Le site web</a> racontent le produit ;
la <a href="{{ '/demarrage/' | relative_url }}">page Demarrage</a> s'adresse a ceux qui
compilent et deploient. Ce guide-ci s'adresse au coureur.</p>
</div>

## Par ou commencer

<div class="grid">
  <a class="card" href="{{ '/guide/installer/' | relative_url }}">
    <h3>1. Installer</h3>
    <p>APK de la montre et du telephone, backend, premiere connexion.
    Rien a compiler : les versions publiees sont pretes a installer.</p>
  </a>
  <a class="card" href="{{ '/guide/premiere-seance/' | relative_url }}">
    <h3>2. Courir une premiere fois</h3>
    <p>De la montre posee sur la table jusqu'a la seance affichee sur le site,
    etape par etape, avec la liste des verifications d'avant-depart.</p>
  </a>
  <a class="card" href="{{ '/guide/apres-la-course/' | relative_url }}">
    <h3>3. Relire sa seance</h3>
    <p>Le resume, les courbes, le plan contre le realise, les zones cardiaques,
    les pauses et l'export GPX.</p>
  </a>
</div>

## Les pages du guide

<div class="table-wrap">

| Page | La question a laquelle elle repond |
|---|---|
| [Installer M-pacer]({{ '/guide/installer/' | relative_url }}) | Quels fichiers prendre, sur quels appareils, et comment entrer son adresse de backend ? |
| [Votre premiere seance]({{ '/guide/premiere-seance/' | relative_url }}) | Que faut-il faire dans l'ordre pour courir et retrouver sa seance sur le site ? |
| [Pendant la course]({{ '/guide/pendant-la-course/' | relative_url }}) | Que montrent les ecrans, quels gestes, que signifient les couleurs et les commandes ? |
| [Preparer un plan d'allure]({{ '/guide/plan-de-course/' | relative_url }}) | Comment regler une distance, un temps vise et un negative split, et lire l'ecart au plan ? |
| [Assistant et voix]({{ '/guide/assistant-et-voix/' | relative_url }}) | Quels sont les quatre modes d'assistant et que dit la voix, a quel moment ? |
| [Appairer et synchroniser]({{ '/guide/appairer-et-synchroniser/' | relative_url }}) | Comment lier la montre au site, envoyer une seance, et que se passe-t-il sans reseau ? |
| [Lire l'analyse d'une seance]({{ '/guide/apres-la-course/' | relative_url }}) | Que veulent dire derive cardiaque, ecart au plan, temps de passage, acceleration ? |
| [Tableaux de bord]({{ '/guide/tableaux-de-bord/' | relative_url }}) | Comment composer ses propres ecrans a partir des neuf widgets ? |
| [Courses et planning]({{ '/guide/courses-et-planning/' | relative_url }}) | Comment preparer une course, chercher dans le calendrier Finishers et suivre ses echeances ? |
| [Musique et tempo]({{ '/guide/musique/' | relative_url }}) | Comment preparer une playlist, l'envoyer sur la montre et suivre le tempo de la seance ? |
| [Amis et suivi en direct]({{ '/guide/amis-et-partage/' | relative_url }}) | Comment courir a plusieurs, partager sa position et suivre une seance en direct ? |
| [Depannage]({{ '/guide/depannage/' | relative_url }}) | Quel est le probleme, quelle est la cause probable, et que faire ? |

</div>

## Le parcours complet, en une image

<ol class="steps">
  <li><strong>Installer</strong> l'application sur la montre (ou sur le telephone, ou les deux)
  et ouvrir un compte sur le site. Le site et les applications sont <em>auto-heberges</em> :
  vous saisissez l'adresse de votre service, pas celle d'un tiers.</li>
  <li><strong>Appairer</strong> l'appareil avec un code court affiche par la montre et saisi
  sur la page <a href="{{ '/guide/appairer-et-synchroniser/' | relative_url }}">/link</a>.
  Un jeton opaque, revocable, suffit ensuite a tous les echanges.</li>
  <li><strong>Regler un plan</strong> : distance de course, temps vise, part negative. Le plan
  sert au <em>shadow runner</em>, le coureur virtuel qui court exactement l'allure cible.</li>
  <li><strong>Courir</strong>. La montre lit le GPS a 1 Hz, lisse l'allure sur deux minutes,
  decoupe les tours, parle, et archive la seance en local. Le reseau n'est jamais necessaire.</li>
  <li><strong>Synchroniser</strong>, automatiquement en fin de seance si l'appareil est appaire.
  Reenvoyer la meme seance remplace la version distante : aucun doublon.</li>
  <li><strong>Relire</strong> sur le site : analyse complete, plan contre realise, zones
  cardiaques, pauses, meilleures distances, export GPX — puis la note de course et la
  preparation de la prochaine echeance.</li>
</ol>

## Ce que le guide suppose

- **Une montre Wear OS 3 ou plus** (Android 11 / API 30) pour l'application montre, ou
  **un telephone Android 8 ou plus** (API 26) pour courir avec le telephone seul.
- **Un GPS** et, si vous voulez la frequence cardiaque au telephone, **une ceinture
  Bluetooth LE** : le capteur optique de la montre n'est pas encore branche (voir les
  <a href="{{ '/guide/depannage/' | relative_url }}">limites connues</a>).
- **Un service M-pacer joignable** — le votre, ou une instance que l'on vous a ouverte —
  pour la synchronisation, le site et les amis.

<div class="info">
<p><strong>Version de reference de ce guide : </strong>{{ site.data.version.version }}.
Le pied de page affiche la version du site et son jour de construction ; les applications
affichent la meme version dans leurs reglages, ce qui permet de savoir d'un coup d'oeil ce
qui tourne sur chaque appareil.</p>
</div>

## En cas de doute

Les deux regles qui expliquent presque tous les comportements surprenants :

1. **La montre est la source de verite.** Une seance peut toujours rester sur la montre
   et partir plus tard ; rien ne se perd parce que le reseau manque.
2. **Tout le calcul vit dans un coeur Rust unique.** Montre, telephone et site sortent
   exactement les memes chiffres : si deux ecrans diffèrent, c'est un probleme de
   synchronisation, pas de calcul.

<p class="tiny">
Guide ecrit pour la version {{ site.data.version.version }} du projet. Les pages produit
detaillees : <a href="{{ '/montre/' | relative_url }}">La montre</a>,
<a href="{{ '/montre-garmin/' | relative_url }}">Montre Garmin</a>,
<a href="{{ '/site-web/' | relative_url }}">Le site web</a>,
<a href="{{ '/architecture/' | relative_url }}">Architecture</a>.
La documentation technique est dans le depot, sous <code>docs/</code>.
</p>
