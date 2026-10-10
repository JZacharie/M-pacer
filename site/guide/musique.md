---
layout: default
title: Musique et tempo
description: Preparer une playlist dans M-pacer, la recuperer via Deezer ou Deemix, l'envoyer sur la montre par USB ou Wi-Fi, et suivre les directives de tempo pendant la course.
permalink: /guide/musique/
---

<span class="eyebrow">Guide utilisateur</span>
# Musique et tempo

<p class="lead">
La bande-son d'une seance se prepare sur le site, se range sur l'appareil, et se pilote
depuis l'ecran Musique. Le coeur ne se contente pas de jouer : il connait le tempo cible du
plan et vous dit s'il faut accelérer ou se calmer.
</p>

## Ce que fait la page /music

<p>
La page <code>/music</code> est organisee en six blocs, du plus amont au plus concret :
</p>

<div class="table-wrap">

| Bloc | Contenu |
|---|---|
| 1. Source des playlists | Connexion Deezer, recherche, <em>Mes playlists</em> |
| 2. Playlists preparees | Les playlists importees, avec leur cible de tempo |
| 3. Titres | Les morceaux de la playlist selectionnee, avec le BPM (a taper ou a saisir) |
| 4. Fichiers a preparer | La liste des MP3 attendus, l'envoi vers Deemix, la file et sa progression |
| 5. Transfert vers la montre | Le manifeste a copier, la passerelle USB |
| 6. Assez de musique pour la course ? | La couverture de la duree prevue par la playlist |

</div>

## Preparer une playlist

<ol class="steps">
  <li><strong>Connecter une source.</strong> Deezer peut etre relie par OAuth, ou utilise
  par cookie <code>arl</code> si le service est configure ainsi ; le catalogue public
  fonctionne aussi sans compte, pour chercher des titres a ajouter.</li>
  <li><strong>Chercher une playlist</strong> ou ouvrir <em>Mes playlists</em>, puis
  <em>Importer</em> : la playlist arrive dans le bloc 2 avec ses titres.</li>
  <li><strong>Regler le tempo cible.</strong> Le BPM se fixe en tapant le rythme ou en
  saisissant la valeur ; le bloc 6 verifie que la playlist couvre la duree de la course.</li>
  <li><strong>Telecharger dans Deemix.</strong> Le bouton remet la playlist dans la file de
  l'instance Deemix ; <em>File Deemix</em> montre la progression, et la liste
  <code>.txt</code> donne les noms de fichiers attendus.</li>
</ol>

## Envoyer les MP3 sur l'appareil

<div class="grid two">
  <div class="card">
    <h3>Par USB — sans serveur</h3>
    <p>L'agent local <code>mpacer-music</code> copie les MP3 directement sur la montre ou le
    telephone (<code>adb push</code>) : rien ne transite par le serveur. Lancez-le, pointez
    le dossier des MP3, analysez le manifeste, puis poussez.</p>
  </div>
  <div class="card">
    <h3>En Wi-Fi — par le service</h3>
    <p>La page televerse les MP3 sur le serveur (volume <code>MPACER_MEDIA_DIR</code>), et
    l'appareil les recupere avec reprise et les acquitte : le serveur les supprime des qu'ils
    sont arrives.</p>
  </div>
</div>

<p>
Pour l'agent local :
</p>

~~~
cargo run -p mpacer-music              # interface locale http://127.0.0.1:8077
cargo run -p mpacer-music -- --folder "D:\MP3" --allow-origin https://mpacer.exemple.org
~~~

<p class="tiny">
<code>localhost:8080</code> et <code>mpacer.p.zacharie.org</code> sont deja autorises comme
origines ; ajoutez la votre avec <code>--allow-origin</code> si votre service a une autre
adresse.
</p>

## Sur la montre et sur le telephone

<ol class="steps">
  <li>Ouvrez l'onglet <strong>Musique</strong>. La bibliotheque indique le dossier local et
  propose <em>Importer (USB)</em> ou <em>Telecharger (serveur)</em> selon le chemin choisi.</li>
  <li>Les commandes de lecture sont au centre : piste precedente, lecture ou pause, piste
  suivante, volume.</li>
  <li>Les playlists apparaissent plus bas, avec <em>Jouer</em> et <em>Supprimer</em>.</li>
</ol>

<div class="info">
<p><strong>Le tempo pendant la course.</strong> Le moteur compare votre allure au BPM cible
et affiche une directive : <code>^</code> accelerer, <code>v</code> se calmer,
<code>&gt;&gt;</code> changer de piste. Les changements de tempo peuvent aussi etre annonces
a la voix, et la maniere dont la musique reagit a la voix se regle dans
<a href="{{ '/guide/assistant-et-voix/' | relative_url }}">Assistant et voix</a>.</p>
</div>

## Ce qu'il faut savoir

<div class="table-wrap">

| Point | Detail |
|---|---|
| Aucun audio stocke par defaut | Les fichiers du disque partent sur l'appareil par USB ; seules les fiches de playlist vivent en base |
| Depot Wi-Fi desactive par defaut | Il s'active explicitement avec <code>MPACER_MEDIA_DIR</code> (volume et quota) |
| Copie temporaire | Quand le depot Wi-Fi est actif, le serveur garde une copie le temps du transfert et la supprime apres acquittement |
| Deemix separe | M-pacer met en file et suit la progression ; le telechargement lui-meme appartient a votre instance Deemix |

</div>

<div class="grid">
  <a class="card" href="{{ '/guide/depannage/' | relative_url }}">
    <h3>Aucune musique sur la montre ?</h3>
    <p>Le chemin USB, le depot Wi-Fi et la file Deemix : les points a verifier.</p>
  </a>
  <a class="card" href="{{ '/guide/assistant-et-voix/' | relative_url }}">
    <h3>Assistant et voix</h3>
    <p>Le comportement de la musique quand la voix parle.</p>
  </a>
</div>
