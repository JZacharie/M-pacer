---
layout: default
title: Demarrage
description: Essayer le coeur Rust, lancer le backend en local, synchroniser une seance, compiler les applications Android et deployer sur Kubernetes.
permalink: /demarrage/
---

<span class="eyebrow">Mise en route</span>
# Démarrage

<p class="lead">
On peut essayer le produit par étapes, de la plus légère à la plus complète : le cœur seul,
puis le backend en local, puis les applications Android, puis le déploiement.
</p>

<div class="note">
<p>Tout ce qui suit est repris du <a href="https://github.com/JZacharie/M-pacer#readme">README</a>
et des guides du dépôt. Les commandes sont données pour PowerShell sous Windows, mais restent
valables dans un shell POSIX.</p>
</div>

## 1. Le cœur, sans montre et sans serveur

~~~
cargo test --workspace

cargo run -p mpacer-sim -- --mode plan --distance 10000 --time 3000 --split 0.03 --gpx trace.gpx
~~~

<p>
Le simulateur rejoue une course synthétique dans le vrai moteur, affiche les annonces vocales,
écrit un GPX et montre l'écart au plan kilomètre par kilomètre. C'est le moyen le plus rapide de
comprendre ce que fait le produit.
</p>

## 2. Le backend en local

~~~
# un PostgreSQL local, puis :
export MPACER_DATABASE_URL="postgresql://mpacer:mpacer@127.0.0.1:5432/mpacer?sslmode=disable"
export MPACER_DEV_AUTH=1 MPACER_PUBLIC_URL=http://localhost:8080
cargo run -p mpacer-api
~~~

<p>
Le site est alors servi sur <code>http://localhost:8080</code>, avec le bouton
« Connexion développeur » pour entrer sans Google.
</p>

## 3. Synchroniser une séance

~~~
cargo run -p mpacer-sim -- --mode plan --distance 5000 --time 1500 --api-url http://localhost:8080
~~~

<p>
Le simulateur demande un code d'appairage du type <code>BCDF-GHJK</code> : on le saisit sur
<code>http://localhost:8080/link</code>, on approuve, et la séance part. Elle apparaît aussitôt
dans le tableau de bord.
</p>

## 4. Tester le backend contre PostgreSQL

~~~
MPACER_TEST_DATABASE_URL="postgresql://mpacer:motdepasse@127.0.0.1:5432/mpacer?sslmode=disable" cargo test -p mpacer-api
~~~

## 5. Les applications Android

<p>
Prérequis : JDK 17, SDK Android 35, NDK r27, Rust avec les trois cibles Android et
<code>cargo-ndk</code>. Le script à la racine du dépôt diagnostique l'environnement et enchaîne
tout le reste :
</p>

~~~
pwsh ./local-ci.ps1 -Check      # diagnostic seul
pwsh ./local-ci.ps1             # application montre, debug
pwsh ./local-ci.ps1 -Target all # montre + application telephone
pwsh ./local-ci.ps1 -Install    # installe sur la montre branchee
~~~

<p class="tiny">
Détail des prérequis, des commandes Gradle et du dépannage :
<a href="https://github.com/JZacharie/M-pacer/blob/main/android/README.md">android/README.md</a>.
</p>

## 6. Déployer

~~~
# 1. Image (ghcr.io, ou publication manuelle)
pwsh deploy/build-image.ps1 -Push

# 2. Déploiement
helm upgrade --install mpacer charts/mpacer -n mpacer --create-namespace \
  -f charts/mpacer/values-jo3.yaml \
  --set auth.googleClientId=... --set auth.googleClientSecret=...

# 3. Vérifications
kubectl -n mpacer get cluster,pods,ingress,certificate
curl -s https://mpacer.p.zacharie.org/readyz
~~~

<div class="warn">
<p>Le client OAuth Google se crée dans la console Google Cloud (type « Application Web »), avec
l'URI de redirection <code>https://&lt;votre-domaine&gt;/auth/google/callback</code>. Sans lui,
seul le mode développement permet de se connecter.</p>
</div>

## 7. Publier cette documentation

<p>
Le site est construit par le workflow <code>Documentation (GitHub Pages)</code> à chaque
modification du dossier <code>site/</code>. Il faut l'activer une fois dans le dépôt :
<strong>Settings → Pages → Build and deployment → Source = GitHub Actions</strong>.
</p>

<p>
En local, une simple lecture des fichiers Markdown de <code>site/</code> suffit pour relire le
contenu ; Jekyll ne sert qu'à appliquer la mise en page commune.
</p>

## Pour aller plus loin

<div class="grid">
  <a class="card" href="{{ '/montre/' | relative_url }}">
    <h3>L'application montre</h3>
    <p>Écrans, modes d'assistant, voix, synchronisation.</p>
  </a>
  <a class="card" href="{{ '/site-web/' | relative_url }}">
    <h3>Le site web</h3>
    <p>Pages, analyse de séance, courses, API.</p>
  </a>
  <a class="card" href="https://github.com/JZacharie/M-pacer/tree/main/docs">
    <h3>Documentation technique</h3>
    <p>Analyse fonctionnelle, architecture Wear OS, backend, courses, analyse de séance.</p>
  </a>
</div>
