---
layout: default
title: Installer
description: Installer M-pacer sur une montre Wear OS, sur un telephone Android ou comme application d'appoint, et savoir quelle adresse de backend saisir.
permalink: /guide/installer/
---

<span class="eyebrow">Guide utilisateur</span>
# Installer M-pacer

<p class="lead">
Rien a compiler : chaque version publiee fournit des APK signes pour la montre, le telephone
et l'application d'appoint, avec leurs empreintes SHA-256. Il faut ensuite une adresse de
service M-pacer et un compte de navigateur.
</p>

## 1. Ce qu'il vous faut

<div class="table-wrap">

| Element | Minimum | A quoi ca sert |
|---|---|---|
| Montre Wear OS | Wear OS 3 / Android 11 (API 30) | Courir avec la montre : GPS, ecran rond, voix, archive locale |
| Telephone Android | Android 8 (API 26) | Courir avec le telephone seul, ou accompagner la montre |
| GPS | — | Toutes les applications en ont besoin ; le voyant de la montre confirme la qualite |
| Ceinture cardiaque Bluetooth LE | — | Optionnelle. Indispensable au telephone pour la frequence cardiaque |
| Service M-pacer | une URL | Synchronisation, site web, amis, musique |

</div>

<div class="note">
<p><strong>Un moyen de transfert.</strong> Le plus simple est le cable USB et
<code>adb</code> (paquet <em>Android SDK Platform Tools</em>). Sur une montre recente, le
debogage Wi-Fi (<em>Wireless debugging</em>) evite meme le cable. L'application d'appoint,
elle, peut installer ou pointer vers l'application montre depuis le telephone.</p>
</div>

## 2. Prendre les bons fichiers

Les versions publiees sont sur la page des publications du depot :
<a href="https://github.com/JZacharie/M-pacer/releases">github.com/JZacharie/M-pacer/releases</a>.
Chaque publication contient les APK des trois applications, les archives des outils Rust
(Linux et Windows) et le fichier d'empreintes <code>SHA256SUMS.txt</code>. Verifiez les
empreintes avant d'installer quoi que ce soit.

<div class="table-wrap">

| Application | Paquet Android | Fichier publie | S'installe sur |
|---|---|---|---|
| M-pacer | <code>com.mpacer.watch</code> | <code>M-pacer-montre-&lt;version&gt;-release.apk</code> | La montre Wear OS |
| M-pacer Course | <code>com.mpacer.phone</code> | <code>M-pacer-telephone-&lt;version&gt;-release.apk</code> | Le telephone |
| M-pacer Compagnon | <code>com.mpacer.companion</code> | <code>M-pacer-compagnon-&lt;version&gt;-release.apk</code> | Le telephone (optionnel) |

</div>

<p class="tiny">
Les archives d'outils contiennent le serveur <code>mpacer-api</code>, le simulateur
<code>mpacer-sim</code> et <code>mpacer-music</code>, qui copie les MP3 sur la montre par USB
(<a href="{{ '/guide/musique/' | relative_url }}">Musique et tempo</a>). L'application Garmin,
elle, ne figure pas dans la publication : elle se compile sur le poste avec le SDK Connect IQ.
</p>

## 3. Installer sur la montre

<ol class="steps">
  <li><strong>Activer le debogage.</strong> Sur la montre : Parametres &rsaquo; A propos
  &rsaquo; Informations sur la version, puis tapoter sept fois <em>Numero de build</em>.
  Le menu <em>Options developpeur</em> apparait.</li>
  <li><strong>Brancher</strong> la montre en USB, ou activer <em>Debogage sans fil</em>
  dans les options developpeur et appairer le poste avec
  <code>adb pair &lt;ip&gt;:&lt;port&gt;</code>.</li>
  <li><strong>Verifier la connexion</strong> puis installer :</li>
</ol>

~~~
adb devices
adb install -r M-pacer-montre-0.2.0-release.apk
~~~

<ol class="steps" start="4">
  <li><strong>Autoriser</strong> la localisation au premier lancement (« Pendant
  l'utilisation ») : sans elle, Android ne fournit aucune position et le voyant GPS reste
  rouge.</li>
  <li><strong>Ouvrir l'application</strong> : l'ecran de repos propose <em>Demarrer</em>,
  <em>Reglages</em> et <em>Synchronisation</em>.</li>
</ol>

<div class="warn">
<p>Sur Android 12 et plus, Android peut ignorer une demande qui ne couvre pas la
localisation approximative. Si la seance s'arrete seule au bout de quelques secondes,
verifiez dans les permissions de l'application que la localisation est bien accordee
(en mode precis), puis relancez.</p>
</div>

## 4. Installer sur le telephone

~~~
adb install -r M-pacer-telephone-0.2.0-release.apk
~~~

<p>
L'application de course s'ouvre sur cinq onglets : <strong>Course</strong>,
<strong>Amis</strong>, <strong>Historique</strong>, <strong>Musique</strong> et
<strong>Reglages</strong>. Au premier lancement, accordez la localisation, puis les
permissions Bluetooth (Android 12+) pour pouvoir chercher une ceinture cardiaque.
</p>

<p>
L'<strong>application d'appoint</strong> sert quand la montre est plus pratique que le
telephone pour saisir quelque chose : elle affiche les seances du compte et permet
d'<em>envoyer</em> une seance vers la montre par le Data Layer Wear OS.
</p>

## 5. Renseigner l'adresse du backend

Une application M-pacer n'est liee a aucun service : c'est vous qui indiquez ou joindre le
votre. L'adresse ressemble a <code>https://mpacer.exemple.org</code> ou, en local, a
<code>http://192.168.1.20:8080</code>.

<ol class="steps">
  <li><strong>Sur le telephone</strong> : Reglages &rsaquo; <em>Synchronisation et
  backend</em> &rsaquo; <em>Adresse du backend</em>, puis <em>Enregistrer</em> et
  <em>Rafraichir l'etat</em>.</li>
  <li><strong>Sur la montre</strong> : ecran <em>Sync</em>, meme champ, puis
  <em>S'appairer</em> (voir la page <a href="{{ '/guide/appairer-et-synchroniser/' | relative_url }}">Appairer
  et synchroniser</a>).</li>
  <li><strong>Dans le navigateur</strong> : ouvrez la meme adresse et connectez-vous. Le
  bouton <em>Continuer avec Google</em> utilise OAuth 2.0 ; en developpement seulement, un
  bouton <em>Connexion developpeur</em> peut remplacer Google.</li>
</ol>

<div class="info">
<p><strong>Vous hebergez le service vous-meme ?</strong> Tout est decrit dans le
<a href="https://github.com/JZacharie/M-pacer/blob/main/deploy/README.md">guide de deploiement</a>
(image conteneur, chart Helm, PostgreSQL CloudNativePG, ingress et TLS) et, pour un essai
local, dans la section 5 du <a href="https://github.com/JZacharie/M-pacer#5-d%C3%A9marrage-rapide">README</a>.</p>
</div>

## 6. Mettre a jour, verifier, desinstaller

<p>
Une mise a jour s'installe par-dessus la precedente : <code>adb install -r</code>, sans
perdre les seances archivees ni le jeton d'appairage. Pour verifier ce qui tourne, ouvrez les
reglages de l'application : le bloc <strong>Version</strong> affiche la version de l'APK et
son jour de compilation, exactement comme le pied de page du site.
</p>

~~~
adb shell pm list packages | Select-String mpacer
adb uninstall com.mpacer.watch
~~~

<div class="note">
<p><strong>Desinstaller n'effface pas ce qui est deja synchronise</strong> : les seances du
site restent dans votre base. L'archive locale de la montre, elle, part avec
l'application — envoyez les seances en attente avant de la retirer.</p>
</div>

<div class="grid">
  <a class="card" href="{{ '/guide/premiere-seance/' | relative_url }}">
    <h3>Etape suivante : votre premiere seance</h3>
    <p>Appairer, regler, courir, synchroniser, relire.</p>
  </a>
  <a class="card" href="{{ '/demarrage/' | relative_url }}">
    <h3>Compiler depuis les sources</h3>
    <p>Scripts <code>local-ci.ps1</code>, emulateur Wear OS, backend local.</p>
  </a>
</div>
