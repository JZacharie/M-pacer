---
layout: default
title: Depannage
description: Les problemes courants de M-pacer et leur solution : GPS, allure, synchronisation, appairage, voix, cardio, musique, suivi en direct et limites connues.
permalink: /guide/depannage/
---

<span class="eyebrow">Guide utilisateur</span>
# Depannage

<p class="lead">
Presque tous les problemes se rangent dans trois familles : une autorisation manquante, une
adresse de service fausse, ou un reglage qui ne prend effet qu'a la prochaine seance. Voici
le tableau des symptomes, puis les limites connues du produit.
</p>

## Les problemes courants

<div class="table-wrap">

| Symptome | Cause probable | Quoi faire |
|---|---|---|
| Le voyant GPS reste orange ou rouge | Ciel masque, interieur, permission de localisation refusee | Sortez a decouvert ; verifiez la permission dans les reglages de l'application |
| La seance s'arrete seule au bout de quelques secondes (Android 12+) | Android peut ignorer une demande de localisation qui ne couvre pas la localisation approximative | Autorisez la localisation en mode precis, puis relancez |
| L'allure affiche <code>--:--</code> | Aucune position exploitable encore recue | Attendez le voyant vert : la valeur grisee n'est pas une allure |
| La frequence cardiaque reste vide sur la montre | Le capteur optique de la montre n'est pas encore branche dans cette version | Utilisez le telephone avec une ceinture Bluetooth LE, ou attendez le branchement du capteur |
| Pas d'annonce vocale | Voix desactivee ou frequence sur <em>Jamais</em> | Reglages &rsaquo; Retour vocal : activez la voix et choisissez 1, 2, 5 minutes ou chaque tour |
| Un reglage semble sans effet pendant une seance | Certains reglages sont lus au demarrage de la seance, pas en direct | Arretez et redemarrez la seance, ou terminez-la et verifiez sur la suivante |
| La seance n'apparait pas sur le site | Appareil non appaire ou pas de reseau | Ecran Sync &rsaquo; <em>Envoyer les seances</em> ; la seance est toujours archivee localement |
| Le code d'appairage expire avant approbation | Le code est a usage unique et limite dans le temps | Relancez <em>S'appairer</em> et saisissez le nouveau code |
| « Appareil non appaire » apres un redemarrage | Jeton revoque depuis la page Jetons | Reappairez : c'est volontaire et les seances ne sont pas perdues |
| L'adresse du backend ne repond pas | Adresse fausse, service arrete, reseau local inaccessible | Testez l'adresse dans le navigateur du telephone avant de la saisir |
| Aucune musique sur l'appareil | Mauvais chemin de transfert, ou depot Wi-Fi non configure | Verifiez la file Deemix, le dossier de l'agent USB, et <code>MPACER_MEDIA_DIR</code> cote service |
| Personne ne me voit courir | Appareil non appaire, broker MQTT vide, ou aucune seance en cours | Reprenez les trois conditions de <a href="{{ '/guide/amis-et-partage/' | relative_url }}">Amis et suivi en direct</a> |
| Les amis n'apparaissent pas sur la carte | Aucun ami accepte, ou dernier point trop ancien | Acceptez la demande dans les deux sens ; l'age de la position est affiche sur chaque fiche |
| L'installation de l'APK est refusee | Android bloque les sources inconnues | Autorisez l'installation depuis cette source, ou passez par <code>adb install -r</code> |

</div>

## Verifier ce qui tourne

<div class="table-wrap">

| Ou | Ce qu'on y lit |
|---|---|
| Reglages de l'application, bloc <strong>Version</strong> | La version de l'APK et son jour de compilation |
| Pied de page du site | La version du projet et le jour de construction du site |
| Page Jetons (<code>/settings</code>) | Les appareils appaires et leur dernier envoi |
| Ecran Sync | L'etat de l'appairage et le nombre de seances en attente |

</div>

<p>
Comparer ces deux numeros de version est le premier reflexe quand un comportement differe
entre deux appareils : le site et les applications se mettent a jour separement.
</p>

## Limites connues

<p>
Le produit est un projet personnel, documente honnetement. Trois limites meritent d'etre
connues avant de chercher une panne :
</p>

<ul>
  <li><strong>Le capteur cardiaque de la montre n'est pas branche.</strong> Le coeur Rust sait
  tout traiter (zones, derive, resume) ; c'est la lecture du capteur qui manque. Au
  telephone, la ceinture Bluetooth LE fonctionne.</li>
  <li><strong>Les boutons du casque audio</strong> (double clic pour une annonce, triple clic
  pour recaler l'allure) ne sont pas encore captes par une session multimedia.</li>
  <li><strong>La validation terrain reste a faire</strong> : l'ecart vise avec une montre de
  reference est inferieur a 3 %, et la consommation a surveiller. Les chiffres du coeur sont
  testes, le comportement en conditions reelles se construit.</li>
</ul>

<p class="tiny">
Liste technique complete et points ouverts :
<a href="https://github.com/JZacharie/M-pacer#8-limites-et-points-ouverts">README, section 8</a>.
</p>

## Signaler un probleme

<p>
Le depot public est sur
<a href="https://github.com/JZacharie/M-pacer">github.com/JZacharie/M-pacer</a>. Ouvrez une
<em>issue</em> avec le modele propose : une capture d'ecran, la version de l'application et du
site, et ce que vous faisiez. Pour un probleme de fond (architecture, nouvelle
fonctionnalite), le modele <em>feature_architecture</em> sert a cadrer la discussion avant de
coder.
</p>

<div class="note">
<p><strong>Deux regles pour se depanner soi-meme.</strong> La montre est la source de
verite : une seance « perdue » est presque toujours encore sur l'appareil. Et tout le calcul
vit dans un seul coeur : si deux ecrans divergent, cherchez la synchronisation, pas le
calcul.</p>
</div>
