# 01 - Analyse des fonctionnalites de Pace Control

> Source analysee : site officiel <https://pacecontrol.pbksoft.com/en/> (accueil,
> manuel utilisateur, FAQ, support) et sa page Google Play, releves le 5 octobre 2026.
> Pace Control est une application Android de **controle d'allure** developpee par
> PBkSoft (Piotr Bieniek), disponible sur le Play Store (`com.pbksoft.pacecontrol`).
> Ce document est une synthese fonctionnelle : il ne reproduit aucun code ni aucune
> ressource de l'application d'origine.

---

## 1. Positionnement du produit

Pace Control ne se presente pas comme un traqueur d'activite ni comme un reseau
social. La FAQ est explicite : l'application fait **peu de choses, mais bien** -
aider le coureur a tenir une allure et a respecter une strategie de course. Pas de
compte utilisateur, pas de synchronisation cloud, pas de tableau de bord web. Le
GPS est utilise pour calculer une allure, pas pour alimenter une communaute.

Trois problemes utilisateurs structurent toute l'application :

| Probleme | Reponse de Pace Control |
|---|---|
| « Je pars trop vite et je m'effondre a la fin » | Assistant *atteindre le temps prevu* avec **shadow runner** |
| « Ma montre m'affiche une allure instable et inutilisable » | Allure **moyennee sur 2 minutes** + detection de changement d'allure |
| « Je ne veux pas regarder mon telephone en courant » | **Retour vocal** complet + commandes par boutons du casque |

## 2. Inventaire des fonctionnalites

Legende priorite : **P0** = indispensable au coeur de metier (doit exister dans le
MVP), **P1** = important (v1), **P2** = confort / differenciation.

### 2.1 Mesure et affichage de l'allure

| # | Fonctionnalite | Comportement observe | Prio |
|---|---|---|---|
| F1 | Allure courante lissee | Moyenne glissante sur **2 minutes**, mise a jour toutes les quelques secondes. Choix assume : la precision long terme primerait sur la reactivite. | P0 |
| F2 | Allure du km/mile courant | Allure du tronçon **partiel** en cours (ex. a 10,05 km : moyenne des 50 derniers metres). | P0 |
| F3 | Allure du km/mile precedent | Allure du dernier tour **complet** (ex. entre 9 et 10 km). | P0 |
| F4 | Distance totale | Distance cumulee depuis le depart. | P0 |
| F5 | Temps total | Temps ecoule, pauses exclues. | P0 |
| F6 | Detection de changement d'allure | Option : compare l'allure des dernieres secondes aux precedentes ; en cas d'ecart significatif, **redemarre le calcul** de l'allure courante. Indispensable au fractionne ; peut produire des faux positifs si le signal se degrade. | P0 |
| F7 | Remise a zero manuelle de l'allure | Triple clic sur le bouton multifonction du casque (ou bouton "piste precedente") : relance la fenetre de moyennage quand on change volontairement d'allure. | P1 |
| F8 | Unites | Metrique (km, min/km) ou imperial (mi, min/mi), **plus** une option "respecter les unites de la course" : l'allure garde l'unite preferee mais la distance reste dans l'unite de l'epreuve (ex. 5 km) pour situer facilement l'arrivee. | P0 |

### 2.2 Etat du signal et controle de la seance

| # | Fonctionnalite | Comportement observe | Prio |
|---|---|---|---|
| F9 | Feu de statut GPS | Rouge = GPS coupe (l'app ne peut pas fonctionner) ; orange = acquisition en cours ; jaune = signal faible, valeurs incertaines ; vert = OK. Texte explicatif en plus de la couleur. | P0 |
| F10 | Start / Pause / Stop | Bouton vert au demarrage, bleu en course, puis reprise (vert) ou fin (rouge). Apres l'arret : ecran de resume de seance. | P0 |
| F11 | Demarrage suspendu | **Appui long** sur Start : la seance est prete mais le chrono attend ; pratique pour ranger le telephone puis partir. | P1 |
| F12 | Reprise au mouvement | Depuis l'etat suspendu, le premier mouvement lance le chrono. | P1 |
| F13 | Auto-pause | Detection d'immobilisation (feux rouges) : pause automatique, reprise automatique **uniquement** apres une pause automatique (jamais apres une pause manuelle). | P0 |
| F14 | Ecran maintenu allume | Option "keep screen on" ; theme sombre assume pour menager les dalles AMOLED. | P1 |
| F15 | Carte du parcours | Bouton affichant la trace sur une carte. | P2 |
| F16 | Journal de support | Fichier de log des mesures GPS et des valeurs calculees, active a la demande du support. | P2 |

### 2.3 Assistant de course - le coeur du produit

L'assistant existe en **quatre modes** exclusifs :

| # | Mode | Entrees | Sortie | Prio |
|---|---|---|---|---|
| F17 | *Track pace* | aucune | allure et temps de tour, aucune hypothese sur la distance ou la duree | P0 |
| F18 | *Predict finish time* | distance de course | temps de finish estime en supposant l'allure courante maintenue | P0 |
| F19 | *Achieve planned time* (shadow runner) | distance + temps prevu + ratio de negative split | coureur virtuel suivant exactement le plan ; ecart **en temps** et **en distance**, indicateur devant/derriere/sur le plan | P0 |
| F20 | *Remote race* | nom de course + distance + pseudo, cote adversaire identique | course en temps reel avec un ami distant : classement, decompte synchronise, tableau de resultats ; necessite Internet et **consomme nettement plus de batterie** | P2 |

Details du negative split (F19), tels que documentes par l'editeur : on demarre
**plus lentement que la moyenne** puis on accelere progressivement, la seconde
moitie etant plus rapide que la moyenne. Valeurs conseillees : **2 % a 4 %**.
Exemple officiel : marathon en 4 h avec un ratio de 3 % -> depart a **5:52 min/km**,
arrivee a **5:30 min/km**, moyenne 5:41 min/km.

### 2.4 Retour vocal

| # | Fonctionnalite | Comportement observe | Prio |
|---|---|---|---|
| F21 | Langue du retour vocal | Selection de la langue ; utilise le moteur TTS systeme (Google TTS recommande, voix hors ligne conseillee). | P0 |
| F22 | Frequence des annonces | Choix de la periodicite des messages d'allure. | P0 |
| F23 | Info etendue en fin de tour | Message enrichi apres chaque km/mile : distance, temps depuis le depart, temps du dernier tour. | P1 |
| F24 | Formes courtes | Formulations plus informelles ("klicks" au lieu de "kilometres") et unites omises. | P2 |
| F25 | Coexistence avec la musique | Trois strategies : **baisser le volume (duck)**, **mettre en pause**, **ignorer et parler**. L'editeur documente les limites de chaque approche. | P1 |
| F26 | Annonce a la demande | Double clic sur le bouton multifonction (ou "piste suivante") : message immediat distance/temps/allure + assistant. | P1 |

### 2.5 Commandes par boutons du casque audio

| # | Fonctionnalite | Comportement observe | Prio |
|---|---|---|---|
| F27 | Pause / reprise | Simple clic sur le bouton multifonction, ou bouton lecture/pause. | P1 |
| F28 | Allure instantanee a la demande | Double clic / piste suivante. | P1 |
| F29 | Reset de l'allure courante | Triple clic / piste precedente. | P1 |
| F30 | Retour sonore de confirmation | Un son court confirme la prise en compte de la commande. | P2 |

Contraintes documentees : les boutons ne fonctionnent pas si la seance n'est pas
demarree et que l'ecran n'est pas affiche ; un appui trop long peut etre interprete
par Android comme un appel a la recherche vocale ; certaines surcouches telefoniques
introduisent des latences importantes.

### 2.6 Historique, export, partage

| # | Fonctionnalite | Comportement observe | Prio |
|---|---|---|---|
| F31 | Historique des seances | Liste des seances, puis fiche detaillee. | P1 |
| F32 | Resume | Date et heure, distance, temps ecoule, allure et vitesse moyennes, trace sur carte. | P1 |
| F33 | Tours | Temps et allure de chaque km/mile. | P1 |
| F34 | Meilleures distances | Meilleurs 1/5/10 km et 1/5/10 mi **realises a l'interieur** de la seance. | P1 |
| F35 | Export / import `.pac` | Fichier proprietaire permettant de transferer ou restaurer les seances. | P2 |
| F36 | Export GPX | Partage de la trace vers une autre application. | P1 |
| F37 | Integration Strava | Export vers Strava (l'editeur renvoie explicitement vers les services externes pour l'analyse et le social). | P2 |

### 2.7 Contraintes produit et techniques observees

- Android 6+ ; **GPS materiel obligatoire** ; fonctionne **hors ligne** (sauf remote race).
- Permissions : localisation precise, notifications (affichage de la seance en cours).
- Aucune donnee envoyee a un serveur, sauf pour la fonction remote race.
- Application disponible en plusieurs langues, traduction communautaire.
- Modele economique : gratuit avec publicite (emplacement publicitaire explicitement
  identifie dans l'interface).

---

## 3. Ce qui est transposable sur une montre Wear OS

### 3.1 A garder tel quel (coeur de metier)

1. **L'algorithme d'allure lissee + detection de changement d'allure** : c'est la
   valeur du produit, et c'est du calcul pur, donc parfaitement portable.
2. **Le shadow runner et le negative split** : modele mathematique simple, testable,
   et sans equivalent convaincant sur montres aujourd'hui.
3. **Le retour vocal** : sur une montre, l'ecran est petit et souvent couvert par
   une manche ; la voix passe de "confort" a "necessite".
4. **Les 4 modes d'assistant** et la coherence des reglages associes.
5. **Le feu de statut GPS** : sur montre, la question "puis-je partir ?" doit etre
   repondue d'un coup d'oeil, sans texte.

### 3.2 A adapter a la montre

| Sujet | Telephone (Pace Control) | Montre Wear OS |
|---|---|---|
| Ecran | Portrait 1080x2000+, texte + pubs | **Rond** ~450x450, 1 a 3 lignes lisibles, pas de publicite |
| Affichage permanent | Ecran allume pendant la seance | **Ambient / always-on** : rafraichissement minimal, secondes masquees |
| Capteurs | GPS du telephone | GPS de la montre **+ cardio optique** (FC, zones), podometre |
| Entrees | Ecran tactile + boutons du casque | Couronne rotative, boutons physiques, gestes ; le casque Bluetooth reste utile |
| Session d'entrainement | Service de premier plan | **Health Services / ExerciseClient** : la seance est declaree au systeme (elle apparait dans les tableaux de bord sante, l'ecran ne s'eteint pas, les donnees sont agregees) |
| Lancement | Icone de l'app | **Tuile (Tile)** et **complication** de montre pour demarrer en un geste |
| Batterie | Contrainte faible | Contrainte **centrale** : GPS + ecran + TTS sont les premiers postes de consommation |
| Reseau | Remote race possible | Remote race deconnectee du telephone = gros risque batterie : a repousser |
| Historique | Base locale + export | Stockage local + **Health Connect / export GPX**, pas de compte |
| Musique | Duck / pause via AudioManager | Meme API, mais **focus audio** et durcissement Wear OS 7 a respecter |

### 3.3 A ecarter (au moins au depart)

- **Publicite** : incompatible avec l'ergonomie d'une montre et avec une seance en cours.
- **Carte du parcours** : inutile sur 450 px ; la trace part vers GPX ou Strava.
- **Remote race** : depend d'un serveur et d'une connexion permanente ; c'est la
  fonctionnalite la plus couteuse a developper et la plus risquee (batterie,
  exploitation d'un backend). A traiter en phase 4, apres le reste.
- **Modele proprietaire `.pac`** : on preferera un JSON documente et versionne,
  plus sain, plus interoperable, sans perdre l'export/import.

---

## 4. Modele mathematique retenu (verifiable)

Tout le coeur de metier se resume a quatre formules, implementees et testees dans
`mpacer-core` :

**Allure courante** (F1) : moyenne glissante sur une fenetre `W = 120 s` depuis un
point de depart de fenetre `s` (remis a zero lors d'une detection de changement) :

```text
allure(t) = 1000 / ( (d(t) - d(s)) / (t - s) )        [s/km]
```

**Negative split** (F19) - allure visee a la fraction `f = d/D` du parcours, ratio `r` :

```text
a(f) = A * (1 + r - 2*r*f)          A = allure moyenne = temps cible / (D/1000)
T(d) = A * (d/1000) * (1 + r - r*d/D)        temps du shadow runner a la distance d
T(D) = A * D/1000                            (le plan est exact a l'arrivee)
```

Verification sur l'exemple officiel (marathon 4 h, r = 3 %) : a(0) = 5:52 min/km,
a(D) = 5:30 min/km, A = 5:41 min/km - ce que l'editeur annonce.

**Position du shadow runner** (F19) : inversion de `T(d)` (equation du second degre),
qui donne la distance planifiee a l'instant `t`, donc l'ecart en distance ; l'ecart
en temps vaut `T(d_reel) - t`.

**Meilleures distances** (F34) : plus petite duree `t(j) - t(i)` telle que
`d(j) - d(i) >= cible`, temps interpoles lineairement sur la trace.

---

## 5. Enseignements pour la conception

1. **Le produit tient dans le coeur de calcul, pas dans l'ecran.** C'est une bonne
   nouvelle pour une reecriture en Rust : 100 % des fonctionnalites P0 sont du
   calcul pur, testable sans montre, sans GPS et sans Android. C'est ce qui a ete
   livre dans `mpacer-core`.
2. **Les reglages sont le vrai produit.** Quatre modes d'assistant, une dizaine
   d'options vocales et d'unites : chaque combinaison doit rester comprehensible.
   D'ou une configuration unique et serialisable, et un simulateur en ligne de
   commande pour rejouer un scenario sans sortir courir.
3. **Les cas limites sont documentes par l'editeur lui-meme** (faux positifs de la
   detection d'allure, latence des boutons du casque, ducking defaillant de certains
   lecteurs) : ce sont des exigences de robustesse, pas des details.
4. **Ne pas se disperser.** Le succes de Pace Control vient de son refus du social
   et de l'analyse. Le portage montre doit garder cette discipline : allure,
   assistant, voix, historique. Le reste (remote race, moteur d'analyse) est un
   programme, pas un prerequis.
