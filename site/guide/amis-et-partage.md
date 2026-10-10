---
layout: default
title: Amis et suivi en direct
description: "Courir a plusieurs avec M-pacer : demandes d'amis, invitation par code, cercle, partage de position sur carte OpenStreetMap et suivi en direct par MQTT."
permalink: /guide/amis-et-partage/
---

<span class="eyebrow">Guide utilisateur</span>
# Amis et suivi en direct

<p class="lead">
Deux choses differentes, un meme socle : retrouver ses proches dans M-pacer et voir ou ils
en sont, et publier sa propre position pendant une seance pour qu'on puisse vous suivre.
Tout est prive, et le suivi est desactive tant que vous ne le configurez pas.
</p>

## Se retrouver entre comptes

<div class="table-wrap">

| Etape | Ou | Ce qui se passe |
|---|---|---|
| Demander | Page <code>/amis</code> ou onglet <strong>Amis</strong> du telephone | Recherche par adresse ou nom, puis <em>Demander en ami</em> |
| Valider | Chez l'ami | <em>Accepter</em> ou <em>Refuser</em>. Rien n'est partage avant son accord |
| Annuler | Chez vous | <em>Annuler</em> retire votre demande en attente |
| Etre prevenu | Partout | Une pastille sur l'onglet <em>Amis</em> et le nombre entre parentheses dans le titre du navigateur ; une notification Android a l'arrivee d'une demande |

</div>

<div class="info">
<p><strong>Le raccourci d'invitation.</strong> Un code en grand, un bouton <em>Copier</em>
et un lien <code>/amis?code=...</code> permettent d'inviter quelqu'un sans le chercher :
le destinataire saisit le code recu dans <em>Ajouter un ami</em>.</p>
</div>

## Mon cercle

<p>
Chaque ami apparait avec sa <strong>photo de profil</strong> (ou ses initiales si le service
n'en a pas), son etat, sa distance, son allure, sa frequence cardiaque, la batterie de son
appareil, son tour courant et l'age de sa position. Un lien ouvre sa position sur
OpenStreetMap. La carte de la page <code>/amis</code> et celle de l'onglet Amis affichent les
marqueurs des amis, le votre et la trace de la seance en cours, rafraichis toutes les dix
secondes.
</p>

## Publier sa position pendant une course

Le suivi en direct passe par un <strong>broker MQTT</strong>. Il est <strong>desactive par
defaut</strong> : sans adresse de broker, l'appareil n'ouvre aucune connexion et ne cree
aucun fil.

<div class="table-wrap">

| Reglage | Defaut | Effet |
|---|---|---|
| Adresse du broker | vide | Vide = suivi desactive. Les identifiants peuvent y figurer : <code>mqtt://coureur:secret@192.168.1.20:1883</code> |
| Prefixe de sujet | <code>mpacer</code> | Sujet publie : <code>&lt;prefixe&gt;/live/&lt;appareil&gt;</code> |
| Nom de l'appareil | identifiant Android court | Dernier segment du sujet ; c'est le nom que vos amis voient |
| Cadence en course | 10 s | De 5 s a 5 min |
| Cadence en pause | 60 s | La position ne bouge plus, on ralentit |
| Precision minimale | 50 m | Un point moins precis n'est pas publie |
| Tester la connexion | — | Verifie le <code>CONNACK</code> et publie un point de test, hors du filtre du suivi |

</div>

<ol class="steps">
  <li>Renseignez l'adresse du broker et un nom d'appareil reconnaissable.</li>
  <li>Appuyez sur <em>Tester la connexion</em> : le message distingue une adresse illisible,
  un broker injoignable et un refus d'authentification.</li>
  <li>Verifiez que la publication est activee, puis partez courir. Les reglages prennent
  effet <strong>a la prochaine seance</strong> : une seance en cours garde la configuration
  avec laquelle elle a demarre.</li>
  <li>Ouvrez la page <code>/live</code> pour voir la trace se dessiner en temps reel.</li>
</ol>

## Les trois conditions pour etre vu

<div class="table-wrap">

| Condition | Ou le verifier |
|---|---|
| L'appareil est appaire au service | Reglages &rsaquo; Synchronisation &rsaquo; <em>Appareil appaire</em> |
| Le broker MQTT est renseigne et le suivi active | Reglages &rsaquo; Suivi en direct |
| Une seance est en cours | Ecran de course, etat <em>En course</em> |

</div>

<p>
Si les deux premieres manquent, l'ecran Amis le rappelle. Une montre est vue exactement
comme un telephone : c'est l'appareil qui est revendique au depart de la seance.
</p>

## Vie privee et cout

<ul>
  <li><strong>Volatil.</strong> Rien n'est ecrit en base : la seance reste la source de
  verite et arrive a la fin, comme avant. Le dernier point peut etre conserve par le broker
  pour qu'un proche qui se connecte en route voie immediatement ou vous etes.</li>
  <li><strong>Prive.</strong> La page demande une connexion ; le mot de passe du broker est
  range dans le Keystore Android, jamais en clair, et <code>mqtts://</code> est accepte.</li>
  <li><strong>Econome.</strong> Environ 55 ko par heure : une position toutes les dix
  secondes, un fil en priorite basse, aucune minuterie, aucun verrou d'eveil.</li>
  <li><strong>Sans carte tierce.</strong> La carte est un fond OpenStreetMap, sans
  dependance JavaScript externe pour le rendu de la trace.</li>
</ul>

<p class="tiny">
Contrat MQTT, budget chiffre et limites :
<a href="https://github.com/JZacharie/M-pacer/blob/main/docs/10-suivi-temps-reel.md">docs/10 — Suivi
en temps reel</a>. Amis et partage :
<a href="https://github.com/JZacharie/M-pacer/blob/main/docs/13-amis-partage-position.md">docs/13 —
Amis et partage de position</a>.
</p>

<div class="grid">
  <a class="card" href="{{ '/guide/courses-et-planning/' | relative_url }}">
    <h3>Suivre un parcours planifie</h3>
    <p>Associer une trace GPX a une course, et la retrouver sur la page de suivi.</p>
  </a>
  <a class="card" href="{{ '/guide/depannage/' | relative_url }}">
    <h3>Personne ne me voit courir</h3>
    <p>Les trois conditions, et ce qu'il faut verifier ensuite.</p>
  </a>
</div>
