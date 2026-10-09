# 13 - Amis et partage de la position en direct

Un petit cercle d'amis, ajoutes par une **demande que l'autre valide**, qui se
voient courir sur une carte **OpenStreetMap** — et rien d'autre : pas de position
qui sorte du cercle, et une identite qui est celle du **compte M-pacer**
(adresse ou nom), jamais celle d'une montre ou d'un appareil.

- Code : `crates/mpacer-api/src/friends.rs` (domaine), `src/routes/api.rs`
  (API), `src/routes/web.rs` (page `/amis` et avatars), `static/map.js`
  (carte), `android/core/src/main/java/com/mpacer/core/social/FriendsClient.kt`
  (application), `.../social/AvatarLoader.kt` (photos)
- Suivi temps reel sous-jacent : [10 - Suivi en direct](10-suivi-temps-reel.md)

---

## 1. Le probleme

Le suivi en direct (docs/10) publie la position de la montre ou du telephone sur
un broker MQTT, et la page `/live` l'affiche. Mais elle l'affiche **a tout le
monde** : c'est un tableau de bord personnel, pas un partage.

Pour courir a plusieurs, il manque quatre choses :

1. **savoir a qui appartient un appareil** — le sujet MQTT ne porte qu'un nom
   (`mpacer/live/montre-a1b2`) ;
2. **designer une personne par son compte** — on ajoute un ami par son adresse
   ou son nom dans la base M-pacer, pas par un identifiant de montre ;
3. **un cercle ferme** — qui a le droit de voir qui, et une demande que la
   personne visee accepte avant que l'amitie existe ;
4. **une carte** — une position seule ne dit rien ; il faut un fond de carte,
   une trace et un age.

## 2. Modele

```text
   Alice cherche "bob@example.org" dans les comptes M-pacer
      |  GET /api/v1/friends/search?q=bob
      v
   POST /api/v1/friends/requests {"email":"bob@example.org"}   -> friend_requests
      |                                                             (en attente)
      v
   Bob voit la demande (GET /api/v1/friends/requests) puis la valide
      |  POST /api/v1/friends/requests/{id}/accept   (ou /decline)
      v
   friendships (les deux sens : Alice->Bob et Bob->Alice)
      |
      |  au depart d'une seance
      |  POST /api/v1/live/register  {"device":"montre-alice"}
      v
   live_devices (device unique -> compte)     <-- c'est la cle du partage
      |
      |  PUBLISH mpacer/live/montre-alice  (broker MQTT, docs/10)
      v
   LiveStore (memoire, 24 h)  --->  GET /api/v1/friends/live  --->  Bob
```

**Quatre regles, volontairement simples :**

1. **Une amitie se demande et se valide.** Un compte se designe par son adresse
   ou son nom (tables `users`), on envoie une demande ; elle ne devient une
   amitie que lorsque la personne visee l'accepte. Aucun identifiant de montre
   n'entre dans le cercle. Le code d'invitation a usage unique reste disponible
   comme raccourci, pour les personnes qui se parlent deja.
2. **Le partage se coupe** d'un interrupteur par compte (`users.share_live`,
   actif par defaut des qu'un ami est ajoute). Coupe, aucune position n'est lue,
   meme par un ami.
3. **Seule une seance en cours est partagee.** Une trace terminee, un appareil
   muet depuis cinq minutes ou un appareil jamais revendique ne sortent jamais
   du serveur. Les seances archivees restent privees : elles ne sont visibles que
   par leur proprietaire, comme avant.
4. **La photo de profil reste chez le service.** La route `/avatar/{id}`
   telecharge la photo Google et la sert, ou dessine les initiales du compte :
   le navigateur et le telephone ne contactent jamais Google.

## 3. Base de donnees (migrations 0007 et 0010)

| Table | Role |
|---|---|
| `users.share_live` | interrupteur de partage du compte (bool, defaut vrai) |
| `friendships` | amities, stockees **dans les deux sens** : lecture en un SELECT, retrait en un DELETE |
| `friend_requests` | demandes en attente : de qui, vers qui, mot, date ; supprimees a l'acceptation ou au refus |
| `friend_invites` | codes d'invitation : usage unique, expiration 24 h (raccourci) |
| `live_devices` | revendication d'un nom d'appareil par un compte (nom **unique** dans tout le service) |

Une demande identique n'est jamais dupliquee (`UNIQUE (from_user_id, to_user_id)`).
Si Bob avait deja demande Alice, la demande en retour d'Alice vaut acceptation
immediate : deux demandes qui se croisent ne restent pas en suspens.

La revendication d'appareil reste unique : si un autre compte publiait sous le
meme nom de montre, sa position apparaitrait dans le cercle de quelqu'un d'autre.
La route repond alors une erreur 409 et invite a changer de nom dans les reglages.

## 4. API

Authentification : jeton d'appareil (en-tete Authorization: Bearer), le meme que
la synchronisation des seances (ou session navigateur pour la page web).

| Route | Effet |
|---|---|
| `POST /api/v1/live/register` | revendique un appareil pour le compte |
| `GET /api/v1/friends` | cercle : amis, partage, positions courantes, demandes en attente |
| `GET /api/v1/friends/live?trace=1` | idem, avec la trace de la seance (carte) |
| `PUT /api/v1/live/route` | publie (ou efface) le **parcours planifie** de l'appareil : `{"device":"...","points":[[lat,lon],...]}` ; une liste vide efface |
| `GET /api/v1/friends/search?q=` | comptes M-pacer par adresse ou nom (2 caracteres minimum) |
| `POST /api/v1/friends/requests` | envoie une demande : `{"email":"ami@example.org","message":"..."}` |
| `GET /api/v1/friends/requests` | demandes recues et envoyees |
| `POST /api/v1/friends/requests/{id}/accept` | accepte : l'amitie est creee dans les deux sens |
| `POST /api/v1/friends/requests/{id}/decline` | refuse : la demande disparait, sans amitie |
| `POST /api/v1/friends/invite` | cree (ou renvoie) un code ; `{"nouvelle":false}` |
| `POST /api/v1/friends/accept` | ajoute un ami a partir d'un code (raccourci) |
| `PUT /api/v1/friends/share` | active ou coupe le partage : `{"share_live":true}` |
| `DELETE /api/v1/friends/{id}` | retire un ami (les deux sens) |
| `GET /avatar/{id}` | photo de profil d'un compte lie (ami ou demande en cours) |

Reponse du cercle (extrait) :

```json
{
  "now_ms": 1760000000000,
  "public_url": "https://mpacer.example.org",
  "share_live": true,
  "pending_requests": 1,
  "total": 2, "live": 1,
  "me":  { "device": "pixel-8", "lat": 48.85, "lon": 2.35, "age_s": 4 },
  "friends": [
    { "id": "u2", "name": "Joseph", "email": "joseph@example.org",
      "picture_url": "https://lh3.googleusercontent.com/...",
      "sharing": true,
      "live": { "device": "montre-a1b2", "state": "run",
                "lat": 48.85, "lon": 2.35, "age_s": 3,
                "distance_m": 1200.0, "pace_s_per_km": 300.0,
                "heart_rate_bpm": 148, "lap": 2,
                "trace": [[48.85, 2.35]] } }
  ]
}
```

Sans `trace=1`, la trace **et le parcours** sont vides : la reponse pese
quelques centaines d'octets par ami, ce qui permet un rafraichissement toutes
les dix secondes depuis un telephone. L'avancement (`progress_pct`) reste
toujours transmis : c'est lui qui dit « 42 % du parcours ».

## 4 bis. Le parcours planifie

Un coureur qui suit un trace prepare (une course, une boucle) le **publie au
depart de la seance** : les suiveurs voient alors, sur la meme carte, le
parcours prevu **et** la position courante, avec le pourcentage deja couvert.

```text
   Application                 Backend                     Suiveur
   fichier GPX choisi          PUT /api/v1/live/route      carte /amis ou /live
   (RunScreen, carte            -> LiveStore (memoire,       trace bleue = parcours
    « Parcours planifie »)         24 h, 2000 points max)     point = position
        |                                                     42 % (3,2 / 7,6 km)
        |  au depart de chaque seance                            hors trace 120 m
        v
   POST /api/v1/live/register  puis  PUT /api/v1/live/route
```

Regles tenues :

1. **Le nom d'appareil doit etre revendique** avant qu'un parcours puisse y etre
   accroche : personne ne peut planter un faux trace sous le nom d'un autre
   coureur (le service repond 400 si l'appareil n'appartient pas au compte).
2. **Rien n'est ecrit en base** : le parcours vit dans le meme magasin en memoire
   que les positions, et disparait apres 24 h sans seance (comme la trace).
3. **Le pourcentage vient du service**, pas de l'application : la position
   courante est projetee sur le parcours (plan local en metres), ce qui donne la
   distance parcourue, la distance restante et l'ecart au trace. Le calcul est
   teste dans `crates/mpacer-api/src/live.rs` (mi-parcours = 50 %, sortie de
   trace mesuree).
4. **Le parcours ne remplace pas la position** : sans seance en cours, il n'est
   pas publie ; le partage reste « seulement pendant la course ».

Cote application telephone, le parcours se choisit dans **Course ▸ Parcours
planifie** (fichier `.gpx`, lu sur l'appareil, garde localement) ; l'avancement
s'affiche sur la meme carte, en course. La trace est sous-echantillonnee a 400
points pour la carte, 2000 points au maximum en memoire.

## 5. Page web /amis

- **Mon partage** : etat et interrupteur.
- **Chercher un compte** : adresse ou nom, resultats avec photo de profil et
  bouton *Demander en ami*.
- **Demandes recues** : photo, nom, adresse, mot, boutons *Accepter* et
  *Refuser*.
- **Demandes envoyees** : la liste des demandes en attente, avec leur photo et
  un bouton *Annuler* (l'expediteur retire lui-meme sa demande).
- **Notification** : une pastille sur l'onglet *Amis* (en-tete et barre
  d'onglets) et le nombre entre parentheses dans le titre du navigateur, sur
  toutes les pages ; le compteur vient de `users.pending_friend_requests`,
  rempli a la resolution de la session.
- **Inviter un ami** (raccourci) : code en grand, bouton *Copier*, lien
  `/amis?code=...`, bouton *Nouveau code*.
- **Carte** : fond OpenStreetMap, marqueurs des amis et de soi-meme, trace de la
  seance en cours, rafraichissement toutes les dix secondes via `/amis.json`.
- **Mon cercle** : fiche par ami avec **photo de profil** (servie par
  `/avatar/{id}`, initiales a defaut), etat, distance, allure, cardio, batterie,
  tour, age de la position, lien vers OpenStreetMap, bouton *Retirer*.

## 6. La carte : OpenStreetMap sans dependance

`static/map.js` est une carte « slippy map » ecrite a la main (aucune
bibliotheque, aucun CDN) :

- tuiles `https://tile.openstreetmap.org/{z}/{x}/{y}.png` avec l'attribution
  exigee par la politique d'usage (OpenStreetMap contributeurs, cliquable) ;
- projection Web Mercator, deplacement a la souris ou au doigt, zoom (boutons,
  molette, pincement) ;
- marqueurs colores par etat (en course, en pause, pret), polylignes de trace et
  **parcours planifie** dessine en bleu sous la trace enregistree (ce qui reste a
  faire reste visible) ; le marqueur porte le pourcentage de parcours couvert ;
- chargement paresseux des tuiles, nettoyage de celles sorties de l'ecran.

Le meme fichier sert le navigateur et **l'application Android** : la vue Carte de
l'onglet Amis charge `/static/map.js` dans une WebView et y pousse les positions
recues par l'API. Une seule implementation de carte, donc un seul endroit a
corriger.

## 7. Application Android

Le socle (`:core`) parle l'API avec le jeton d'appareil deja utilise pour la
synchronisation, et **revendique l'appareil au depart de chaque seance**
(`LiveTracker.start` puis `FriendsClient.registerDevice`). L'appel est fait au
mieux : sans jeton, sans reseau ou si le nom est pris, la seance continue — seul
le partage avec les amis est indisponible.

L'onglet **Amis** du telephone (module `:phone`) propose la recherche de compte
(adresse ou nom), l'envoi d'une demande, les **demandes recues** avec *Accepter* /
*Refuser*, les **demandes envoyees** avec *Annuler*, la liste des amis avec
**photo de profil** (`AvatarLoader` charge `/avatar/{id}` et affiche les
initiales si le service n'a pas de photo), leurs chiffres et la carte
OpenStreetMap en WebView. Le code d'invitation reste disponible comme raccourci.

Une **notification systeme** previent quand une demande arrive : le socle
compare les demandes a chaque chargement et `FriendsNotifier` poste une
notification (canal « Demandes d'amis ») pour les nouvelles seulement.

Trois conditions pour que vos amis vous voient courir, toutes verifiees a
l'ecran :

1. l'appareil est **appaire** au backend (onglet Reglages > Synchronisation) ;
2. le **broker MQTT** est renseigne (onglet Reglages > Suivi en direct) : sans
   lui, personne ne publie ;
3. une **seance est en cours** — l'ecran Amis le rappelle si les deux premiers
   points manquent. La montre, elle, ne change pas d'interface : c'est son
   appareil qui est revendique, donc un ami peut suivre une montre exactement comme
   un telephone.

## 8. Cout et limites

- **Aucune ecriture en base pour les positions** : le suivi reste en memoire
  (`LiveStore`, docs/10). Seuls le cercle, les demandes, les codes et les noms
  d'appareils sont persistes.
- **Un ami ne voit rien hors seance** : le partage s'arrete avec la seance.
- **Budget reseau** : environ 1 ko par ami toutes les dix secondes sans trace.
- **Nom d'appareil unique** dans tout le service : sur une installation
  partagee, choisir un nom explicite dans les reglages (le telephone et la
  montre peuvent avoir chacun le leur, jusqu'a huit).
- **La recherche n'est pas un annuaire** : deux caracteres minimum, dix resultats
  au plus, et rien d'autre que les comptes M-pacer (pas d'import de contacts).
- **Photo de profil** : servie uniquement aux comptes lies par une amitie ou une
  demande en cours ; le monogramme prend le relais sans photo.
- **Pas encore verifie sur une installation reelle** : les tests d'integration
  (`crates/mpacer-api/tests/api.rs`) exigent un PostgreSQL
  (`MPACER_TEST_DATABASE_URL`), et la carte demande un acces reseau aux tuiles
  OpenStreetMap. Les tests unitaires du domaine (codes, visibilite, expiration)
  tournent sans base.
- **HTTP en clair** : le backend auto-heberge vit souvent sur le reseau local.
  Les applications montrent et telephone autorisent donc le trafic en clair
  (`usesCleartextTraffic`), sinon la synchronisation et la carte ne
  fonctionneraient que derriere HTTPS. Aucun secret ne transite en clair : le
  jeton d'appairage et le mot de passe du broker restent chiffres sur l'appareil.
- **Pas de notification d'arrivee** ni d'historique partage : c'est volontaire,
  la fonctionnalite se limite a la position du moment.
- **Le parcours n'est publie que par l'appareil qui le porte** : aujourd'hui le
  telephone (choix d'un `.gpx` dans Course ▸ Parcours planifie). Une seance
  courue avec la montre seule partage donc la position et la trace, mais pas le
  parcours — le transfert du parcours vers la montre reste a faire.
- **Un parcours par appareil** : le dernier publie remplace le precedent ;
  changer de trace en cours de seance met la carte a jour au message suivant.
