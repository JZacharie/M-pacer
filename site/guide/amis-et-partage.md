---
layout: default
title: Amis et suivi en direct
description: "Courir à plusieurs avec M-pacer : demandes d'amis, invitation par code, cercle, partage de position sur carte OpenStreetMap et suivi en direct par MQTT."
permalink: /guide/amis-et-partage/
---

<span class="eyebrow">Guide utilisateur</span>
# Amis et suivi en direct

<p class="lead">
Deux choses différentes, un même socle : retrouver ses proches dans M-pacer et voir ou ils
en sont, et publier sa propre position pendant une séance pour qu'on puisse vous suivre.
Tout est privé, et le suivi est désactivé tant que vous ne le configurez pas.
</p>

## Se retrouver entre comptes

<div class="table-wrap">

| Étape | Où | Ce qui se passe |
|---|---|---|
| Demander | Page <code>/amis</code> ou onglet <strong>Amis</strong> du téléphone | Recherche par adresse ou nom, puis <em>Demander en ami</em> |
| Valider | Chez l'ami | <em>Accepter</em> ou <em>Refuser</em>. Rien n'est partagé avant son accord |
| Annuler | Chez vous | <em>Annuler</em> retire votre demande en attente |
| Être prévenu | Partout | Une pastille sur l'onglet <em>Amis</em> et le nombre entre parenthèses dans le titre du navigateur ; une notification Android à l'arrivée d'une demande |

</div>

<div class="info">
<p><strong>Le raccourci d'invitation.</strong> Un code en grand, un bouton <em>Copier</em>
et un lien <code>/amis?code=...</code> permettent d'inviter quelqu'un sans le chercher :
le destinataire saisit le code reçu dans <em>Ajouter un ami</em>.</p>
</div>

## Mon cercle

<p>
Chaque ami apparaît avec sa <strong>photo de profil</strong> (ou ses initiales si le service
n'en a pas), son état, sa distance, son allure, sa fréquence cardiaque, la batterie de son
appareil, son tour courant et l'age de sa position. Un lien ouvre sa position sur
OpenStreetMap. La carte de la page <code>/amis</code> et celle de l'onglet Amis affichent les
marqueurs des amis, le votre et la trace de la séance en cours, rafraîchis toutes les dix
secondes.
</p>

## Publier sa position pendant une course

Le suivi en direct passe par un <strong>broker MQTT</strong>. Il est <strong>désactivé par
défaut</strong> : sans adresse de broker, l'appareil n'ouvre aucune connexion et ne crée
aucun fil.

<div class="table-wrap">

| Réglage | Défaut | Effet |
|---|---|---|
| Adresse du broker | vide | Vide = suivi désactivé. Les identifiants peuvent y figurer : <code>mqtt://coureur:secret@192.168.1.20:1883</code> |
| Préfixe de sujet | <code>mpacer</code> | Sujet publié : <code>&lt;prefixe&gt;/live/&lt;appareil&gt;</code> |
| Nom de l'appareil | identifiant Android court | Dernier segment du sujet ; c'est le nom que vos amis voient |
| Cadence en course | 10 s | De 5 s à 5 min |
| Cadence en pause | 60 s | La position ne bouge plus, on ralentit |
| Précision minimale | 50 m | Un point moins précis n'est pas publié |
| Tester la connexion | — | Vérifie le <code>CONNACK</code> et publie un point de test, hors du filtre du suivi |

</div>

<ol class="steps">
  <li>Renseignez l'adresse du broker et un nom d'appareil reconnaissable.</li>
  <li>Appuyez sur <em>Tester la connexion</em> : le message distingue une adresse illisible,
  un broker injoignable et un refus d'authentification.</li>
  <li>Vérifiez que la publication est activée, puis partez courir. Les réglages prennent
  effet <strong>à la prochaine séance</strong> : une séance en cours garde la configuration
  avec laquelle elle a démarré.</li>
  <li>Ouvrez la page <code>/live</code> pour voir la trace se dessiner en temps réel.</li>
</ol>

## Les trois conditions pour être vu

<div class="table-wrap">

| Condition | Où le vérifier |
|---|---|
| L'appareil est appaire au service | Réglages &rsaquo; Synchronisation &rsaquo; <em>Appareil appaire</em> |
| Le broker MQTT est renseigné et le suivi activé | Réglages &rsaquo; Suivi en direct |
| Une séance est en cours | Écran de course, état <em>En course</em> |

</div>

<p>
Si les deux premières manquent, l'écran Amis le rappelle. Une montre est vue exactement
comme un téléphone : c'est l'appareil qui est revendiqué au départ de la séance.
</p>

## Vie privée et coût

<ul>
  <li><strong>Volatil.</strong> Rien n'est écrit en base : la séance reste la source de
  vérité et arrive à la fin, comme avant. Le dernier point peut être conservé par le broker
  pour qu'un proche qui se connecte en route voie immédiatement où vous êtes.</li>
  <li><strong>Privé.</strong> La page demande une connexion ; le mot de passe du broker est
  range dans le Keystore Android, jamais en clair, et <code>mqtts://</code> est accepté.</li>
  <li><strong>Économe.</strong> Environ 55 ko par heure : une position toutes les dix
  secondes, un fil en priorité basse, aucune minuterie, aucun verrou d'eveil.</li>
  <li><strong>Sans carte tierce.</strong> La carte est un fond OpenStreetMap, sans
  dépendance JavaScript externe pour le rendu de la trace.</li>
</ul>

<p class="tiny">
Contrat MQTT, budget chiffre et limites :
<a href="https://github.com/JZacharie/M-pacer/blob/main/docs/10-suivi-temps-reel.md">docs/10 — Suivi
en temps réel</a>. Amis et partage :
<a href="https://github.com/JZacharie/M-pacer/blob/main/docs/13-amis-partage-position.md">docs/13 —
Amis et partage de position</a>.
</p>

<div class="grid">
  <a class="card" href="{{ '/guide/courses-et-planning/' | relative_url }}">
    <h3>Suivre un parcours planifie</h3>
    <p>Associer une trace GPX à une course, et la retrouver sur la page de suivi.</p>
  </a>
  <a class="card" href="{{ '/guide/depannage/' | relative_url }}">
    <h3>Personne ne me voit courir</h3>
    <p>Les trois conditions, et ce qu'il faut vérifier ensuite.</p>
  </a>
</div>
