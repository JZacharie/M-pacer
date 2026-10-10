---
layout: default
title: Dépannage
description: "Les problèmes courants de M-pacer et leur solution : GPS, allure, synchronisation, appairage, voix, cardio, musique, suivi en direct et limites connues."
permalink: /guide/depannage/
---

<span class="eyebrow">Guide utilisateur</span>
# Dépannage

<p class="lead">
Presque tous les problèmes se rangent dans trois familles : une autorisation manquante, une
adresse de service fausse, ou un réglage qui ne prend effet qu'à la prochaine séance. Voici
le tableau des symptômes, puis les limites connues du produit.
</p>

## Les problèmes courants

<div class="table-wrap">

| Symptôme | Cause probable | Quoi faire |
|---|---|---|
| Le voyant GPS reste orange ou rouge | Ciel masque, intérieur, permission de localisation refusee | Sortez a decouvert ; vérifiez la permission dans les réglages de l'application |
| La séance s'arrête seule au bout de quelques secondes (Android 12+) | Android peut ignorer une demande de localisation qui ne couvre pas la localisation approximative | Autorisez la localisation en mode précis, puis relancez |
| L'allure affiche <code>--:--</code> | Aucune position exploitable encore reçue | Attendez le voyant vert : la valeur grisée n'est pas une allure |
| La fréquence cardiaque reste vide sur la montre | Le capteur optique de la montre n'est pas encore branché dans cette version | Utilisez le téléphone avec une ceinture Bluetooth LE, ou attendez le branchement du capteur |
| Pas d'annonce vocale | Voix désactivée ou fréquence sur <em>Jamais</em> | Réglages &rsaquo; Retour vocal : activez la voix et choisissez 1, 2, 5 minutes ou chaque tour |
| Un réglage semble sans effet pendant une séance | Certains réglages sont lus au démarrage de la séance, pas en direct | Arrêtez et redémarrez la séance, ou terminez-la et vérifiez sur la suivante |
| La séance n'apparaît pas sur le site | Appareil non appaire ou pas de réseau | Écran Sync &rsaquo; <em>Envoyer les séances</em> ; la séance est toujours archivée localement |
| Le code d'appairage expire avant approbation | Le code est a usage unique et limite dans le temps | Relancez <em>S'appairer</em> et saisissez le nouveau code |
| « Appareil non appaire » après un redémarrage | Jeton révoque depuis la page Jetons | Reappairez : c'est volontaire et les séances ne sont pas perdues |
| L'adresse du backend ne répond pas | Adresse fausse, service arrêté, réseau local inaccessible | Testez l'adresse dans le navigateur du téléphone avant de la saisir |
| Aucune musique sur l'appareil | Mauvais chemin de transfert, ou dépôt Wi-Fi non configuré | Vérifiez la file Deemix, le dossier de l'agent USB, et <code>MPACER_MEDIA_DIR</code> côte service |
| Personne ne me voit courir | Appareil non appaire, broker MQTT vide, ou aucune séance en cours | Reprenez les trois conditions de <a href="{{ '/guide/amis-et-partage/' | relative_url }}">Amis et suivi en direct</a> |
| Les amis n'apparaissent pas sur la carte | Aucun ami accepté, ou dernier point trop ancien | Acceptez la demande dans les deux sens ; l'age de la position est affiché sur chaque fiche |
| L'installation de l'APK est refusee | Android bloque les sources inconnues | Autorisez l'installation depuis cette source, ou passez par <code>adb install -r</code> |

</div>

## Vérifier ce qui tourne

<div class="table-wrap">

| Où | Ce qu'on y lit |
|---|---|
| Réglages de l'application, bloc <strong>Version</strong> | La version de l'APK et son jour de compilation |
| Pied de page du site | La version du projet et le jour de construction du site |
| Page Jetons (<code>/settings</code>) | Les appareils appaires et leur dernier envoi |
| Écran Sync | L'état de l'appairage et le nombre de séances en attente |

</div>

<p>
Comparer ces deux numéros de version est le premier reflexe quand un comportement differe
entre deux appareils : le site et les applications se mettent à jour séparément.
</p>

## Limites connues

<p>
Le produit est un projet personnel, documenté honnêtement. Trois limites méritent d'être
connues avant de chercher une panne :
</p>

<ul>
  <li><strong>Le capteur cardiaque de la montre n'est pas branché.</strong> Le cœur Rust sait
  tout traiter (zones, dérive, résumé) ; c'est la lecture du capteur qui manque. Au
  téléphone, la ceinture Bluetooth LE fonctionne.</li>
  <li><strong>Les boutons du casque audio</strong> (double clic pour une annonce, triple clic
  pour recaler l'allure) ne sont pas encore captés par une session multimédia.</li>
  <li><strong>La validation terrain reste à faire</strong> : l'écart visé avec une montre de
  référence est inférieur a 3 %, et la consommation à surveiller. Les chiffres du cœur sont
  testés, le comportement en conditions réelles se construit.</li>
</ul>

<p class="tiny">
Liste technique complète et points ouverts :
<a href="https://github.com/JZacharie/M-pacer#8-limites-et-points-ouverts">README, section 8</a>.
</p>

## Signaler un problème

<p>
Le dépôt public est sur
<a href="https://github.com/JZacharie/M-pacer">github.com/JZacharie/M-pacer</a>. Ouvrez une
<em>issue</em> avec le modèle propose : une capture d'écran, la version de l'application et du
site, et ce que vous faisiez. Pour un problème de fond (architecture, nouvelle
fonctionnalité), le modèle <em>feature_architecture</em> sert à cadrer la discussion avant de
coder.
</p>

<div class="note">
<p><strong>Deux règles pour se dépanner soi-même.</strong> La montre est la source de
vérité : une séance « perdue » est presque toujours encore sur l'appareil. Et tout le calcul
vit dans un seul cœur : si deux écrans divergent, cherchez la synchronisation, pas le
calcul.</p>
</div>
