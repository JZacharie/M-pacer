# 13 - Amis et partage de la position en direct

Un petit cercle d'amis, ajoutes par un code court, qui se voient courir sur une
carte **OpenStreetMap** — et rien d'autre : pas d'annuaire, pas de reseau social,
pas de position qui sorte du cercle.

- Code : `crates/mpacer-api/src/friends.rs` (domaine), `src/routes/api.rs`
  (API), `src/routes/web.rs` (page `/amis`), `static/map.js` (carte),
  `android/core/src/main/java/com/mpacer/core/social/FriendsClient.kt`
- Suivi temps reel sous-jacent : [10 - Suivi en direct](10-suivi-temps-reel.md)

---

## 1. Le probleme

Le suivi en direct (docs/10) publie la position de la montre ou du telephone sur
un broker MQTT, et la page `/live` l'affiche. Mais elle l'affiche **a tout le
monde** : c'est un tableau de bord personnel, pas un partage.

Pour courir a plusieurs, il manque trois choses :

1. **savoir a qui appartient un appareil** — le sujet MQTT ne porte qu'un nom
   (`mpacer/live/montre-a1b2`) ;
2. **un cercle ferme** — qui a le droit de voir qui ;
3. **une carte** — une position seule ne dit rien ; il faut un fond de carte,
   une trace et un age.

## 2. Modele

```text
        code d'invitation (BCDF-GHJK, usage unique, 24 h)
  Alice ---------------------------------------------> Bob
     |  POST /api/v1/friends/invite        POST /api/v1/friends/accept
     v                                                   v
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

**Trois regles, volontairement simples :**

1. **Une amitie est mutuelle** et se noue en saisissant un code d'invitation a
   usage unique, transmis de la main a la main. Aucun annuaire, aucun import de
   contacts, aucune recherche par email : on n'ajoute que qui on connait.
2. **Le partage se coupe** d'un interrupteur par compte (`users.share_live`,
   actif par defaut des qu'un ami est ajoute). Coupe, aucune position n'est lue,
   meme par un ami.
3. **Seule une seance en cours est partagee.** Une trace terminee, un appareil
   muet depuis cinq minutes ou un appareil jamais revendique ne sortent jamais
   du serveur. Les seances archivees restent privees : elles ne sont visibles que
   par leur proprietaire, comme avant.

## 3. Base de donnees (migration 0007)

| Table | Role |
|---|---|
| `users.share_live` | interrupteur de partage du compte (bool, defaut vrai) |
| `friendships` | amities, stockees **dans les deux sens** : lecture en un SELECT, retrait en un DELETE |
| `friend_invites` | codes d'invitation : usage unique, expiration 24 h |
| `live_devices` | revendication d'un nom d'appareil par un compte (nom **unique** dans tout le service) |

La revendication est unique : si un autre compte publiait sous le meme nom
d'appareil, sa position apparaitrait dans le cercle de quelqu'un d'autre. La
route repond alors une erreur 409 et invite a changer de nom dans les reglages.

## 4. API

Authentification : jeton d'appareil (en-tete Authorization: Bearer), le meme que
la synchronisation des seances.

| Route | Effet |
|---|---|
| `POST /api/v1/live/register` | revendique un appareil pour le compte |
| `GET /api/v1/friends` | cercle : amis, partage, positions courantes (sans trace) |
| `GET /api/v1/friends/live?trace=1` | idem, avec la trace de la seance (carte) |
| `POST /api/v1/friends/invite` | cree (ou renvoie) un code ; `{"nouvelle":false}` |
| `POST /api/v1/friends/accept` | ajoute un ami a partir de son code |
| `PUT /api/v1/friends/share` | active ou coupe le partage : `{"share_live":true}` |
| `DELETE /api/v1/friends/{id}` | retire un ami (les deux sens) |

Reponse du cercle (extrait) :

```json
{
  "now_ms": 1760000000000,
  "public_url": "https://mpacer.example.org",
  "share_live": true,
  "total": 2, "live": 1,
  "me":  { "device": "pixel-8", "lat": 48.85, "lon": 2.35, "age_s": 4 },
  "friends": [
    { "id": "u2", "name": "Joseph", "email": "joseph@example.org",
      "sharing": true,
      "live": { "device": "montre-a1b2", "state": "run",
                "lat": 48.85, "lon": 2.35, "age_s": 3,
                "distance_m": 1200.0, "pace_s_per_km": 300.0,
                "heart_rate_bpm": 148, "lap": 2,
                "trace": [[48.85, 2.35]] } }
  ]
}
```

Sans `trace=1`, la trace est videe : la reponse pese quelques centaines
d'octets par ami, ce qui permet un rafraichissement toutes les dix secondes
depuis un telephone.

## 5. Page web /amis

- **Mon partage** : etat et interrupteur.
- **Inviter un ami** : code en grand, bouton *Copier*, lien `/amis?code=...` a
  envoyer tel quel, bouton *Nouveau code*.
- **Ajouter un ami** : champ de saisie (le lien d'invitation le pre-remplit).
- **Carte** : fond OpenStreetMap, marqueurs des amis et de soi-meme, trace de la
  seance en cours, rafraichissement toutes les dix secondes via `/amis.json`.
- **Mon cercle** : fiche par ami (etat, distance, allure, cardio, batterie,
  tour, age de la position), lien vers OpenStreetMap, bouton *Retirer*.

## 6. La carte : OpenStreetMap sans dependance

`static/map.js` est une carte « slippy map » ecrite a la main (aucune
bibliotheque, aucun CDN) :

- tuiles `https://tile.openstreetmap.org/{z}/{x}/{y}.png` avec l'attribution
  exigee par la politique d'usage (OpenStreetMap contributeurs, cliquable) ;
- projection Web Mercator, deplacement a la souris ou au doigt, zoom (boutons,
  molette, pincement) ;
- marqueurs colores par etat (en course, en pause, pret) et polylignes de trace ;
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

L'onglet **Amis** du telephone (module `:phone`) propose l'interrupteur de
partage, le code d'invitation (copier, partager par n'importe quelle messagerie,
regenerer), l'ajout par code, la liste des amis avec leurs chiffres et la carte
OpenStreetMap en WebView.

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
  (`LiveStore`, docs/10). Seuls le cercle, les codes et les noms d'appareils
  sont persistes.
- **Un ami ne voit rien hors seance** : le partage s'arrete avec la seance.
- **Budget reseau** : environ 1 ko par ami toutes les dix secondes sans trace.
- **Nom d'appareil unique** dans tout le service : sur une installation
  partagee, choisir un nom explicite dans les reglages (le telephone et la
  montre peuvent avoir chacun le leur, jusqu'a huit).
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
