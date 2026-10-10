---
layout: default
title: Musique et tempo
description: Préparer une playlist dans M-pacer, la recuperer via Deezer ou Deemix, l'envoyer sur la montre par USB ou Wi-Fi, et suivre les directives de tempo pendant la course.
permalink: /guide/musique/
---

<span class="eyebrow">Guide utilisateur</span>
# Musique et tempo

<p class="lead">
La bande-son d'une séance se prépare sur le site, se range sur l'appareil, et se pilote
depuis l'écran Musique. Le cœur ne se contente pas de jouer : il connait le tempo cible du
plan et vous dit s'il faut accelérer où se calmer.
</p>

## Ce que fait la page /music

<p>
La page <code>/music</code> est organisée en six blocs, du plus amont au plus concret :
</p>

<div class="table-wrap">

| Bloc | Contenu |
|---|---|
| 1. Source des playlists | Connexion Deezer, recherche, <em>Mes playlists</em> |
| 2. Playlists préparées | Les playlists importées, avec leur cible de tempo |
| 3. Titres | Les morceaux de la playlist sélectionnée, avec le BPM (à taper ou à saisir) |
| 4. Fichiers à préparer | La liste des MP3 attendus, l'envoi vers Deemix, la file et sa progression |
| 5. Transfert vers la montre | Le manifeste à copier, la passerelle USB |
| 6. Assez de musique pour la course ? | La couverture de la durée prévue par la playlist |

</div>

## Préparer une playlist

<ol class="steps">
  <li><strong>Connecter une source.</strong> Deezer peut être relie par OAuth, ou utilise
  par cookie <code>arl</code> si le service est configuré ainsi ; le catalogue public
  fonctionne aussi sans compte, pour chercher des titres à ajouter.</li>
  <li><strong>Chercher une playlist</strong> ou ouvrir <em>Mes playlists</em>, puis
  <em>Importer</em> : la playlist arrive dans le bloc 2 avec ses titres.</li>
  <li><strong>Régler le tempo cible.</strong> Le BPM se fixe en tapant le rythme ou en
  saisissant la valeur ; le bloc 6 vérifie que la playlist couvre la durée de la course.</li>
  <li><strong>Télécharger dans Deemix.</strong> Le bouton remet la playlist dans la file de
  l'instance Deemix ; <em>File Deemix</em> montre la progression, et la liste
  <code>.txt</code> donne les noms de fichiers attendus.</li>
</ol>

## Envoyer les MP3 sur l'appareil

<div class="grid two">
  <div class="card">
    <h3>Par USB — sans serveur</h3>
    <p>L'agent local <code>mpacer-music</code> copie les MP3 directement sur la montre ou le
    téléphone (<code>adb push</code>) : rien ne transite par le serveur. Lancez-le, pointez
    le dossier des MP3, analysez le manifeste, puis poussez.</p>
  </div>
  <div class="card">
    <h3>En Wi-Fi — par le service</h3>
    <p>La page téléverse les MP3 sur le serveur (volume <code>MPACER_MEDIA_DIR</code>), et
    l'appareil les récupère avec reprise et les acquitte : le serveur les supprime dès qu'ils
    sont arrivés.</p>
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
<code>localhost:8080</code> et <code>mpacer.p.zacharie.org</code> sont déjà autorisés comme
origines ; ajoutez la votre avec <code>--allow-origin</code> si votre service à une autre
adresse.
</p>

## Sur la montre et sur le téléphone

<ol class="steps">
  <li>Ouvrez l'onglet <strong>Musique</strong>. La bibliothèque indique le dossier local et
  propose <em>Importer (USB)</em> ou <em>Télécharger (serveur)</em> selon le chemin choisi.</li>
  <li>Les commandes de lecture sont au centre : piste précédente, lecture ou pause, piste
  suivante, volume.</li>
  <li>Les playlists apparaissent plus bas, avec <em>Jouer</em> et <em>Supprimer</em>.</li>
</ol>

<div class="info">
<p><strong>Le tempo pendant la course.</strong> Le moteur compare votre allure au BPM cible
et affiche une directive : <code>^</code> accélérer, <code>v</code> se calmer,
<code>&gt;&gt;</code> changer de piste. Les changements de tempo peuvent aussi être annonces
à la voix, et la manière dont la musique réagit à la voix se règle dans
<a href="{{ '/guide/assistant-et-voix/' | relative_url }}">Assistant et voix</a>.</p>
</div>

## Ce qu'il faut savoir

<div class="table-wrap">

| Point | Détail |
|---|---|
| Aucun audio stocke par défaut | Les fichiers du disque partent sur l'appareil par USB ; seules les fiches de playlist vivent en base |
| Depot Wi-Fi désactivé par défaut | Il s'active explicitement avec <code>MPACER_MEDIA_DIR</code> (volume et quota) |
| Copie temporaire | Quand le dépôt Wi-Fi est actif, le serveur garde une copie le temps du transfert et la supprime après acquittement |
| Deemix sépare | M-pacer met en file et suit la progression ; le téléchargement lui-même appartient à votre instance Deemix |

</div>

<div class="grid">
  <a class="card" href="{{ '/guide/depannage/' | relative_url }}">
    <h3>Aucune musique sur la montre ?</h3>
    <p>Le chemin USB, le dépôt Wi-Fi et la file Deemix : les points à vérifier.</p>
  </a>
  <a class="card" href="{{ '/guide/assistant-et-voix/' | relative_url }}">
    <h3>Assistant et voix</h3>
    <p>Le comportement de la musique quand la voix parle.</p>
  </a>
</div>
