# 11 - Playlists multi-sources (Spotify **ou** Deezer), liste des MP3 et push dans l'application

> **Statut** : ajout au contrat v2/v3 de [docs/07](07-musique-bpm-et-playlists.md)
> (7 octobre 2026). Le principe « le serveur ne stocke aucun audio » est
> **conserve** : ce document ne fait que l'etendre a une deuxieme source de
> metadonnees et a un envoi de fichiers dans **l'application locale**.

---

## 1. Ce que la fonctionnalite apporte

1. **Recuperer la liste des playlists depuis Spotify ou Deezer** : chaque source
   se connecte par OAuth, expose une **recherche** et la liste des **playlists du
   compte** (`Mes playlists`), puis importe la fiche choisie (titres, artistes,
   durees). Deezer n'expose aucun tempo : le BPM reste inconnu a l'import et se
   complete par la balise du MP3, le tap-tempo ou la saisie.
2. **Lister les fichiers a mettre en place en MP3** : pour la playlist
   selectionnee, la page `/music` affiche un nom de fichier attendu par piste
   (schema de la montre : `01 - Artiste - Titre.mp3`) et le propose en
   telechargement `.txt`. C'est la liste de courses : ce qu'il faut telecharger,
   convertir ou retrouver avant de transferer.
3. **Pousser des MP3 dans l'application M-pacer** : l'application locale
   `mpacer-music` accepte des fichiers pousses depuis le navigateur (glisser-deposer),
   les range dans une **bibliotheque geree**, les apparie au manifeste puis les
   **synchronise sur la montre** par USB (adb push) avec un **statut par fichier**.

## 2. Decision d'architecture (et alternative ecartee)

| Point | Decision retenue | Alternative ecartee |
|---|---|---|
| Origine des fichiers | **disque du PC** (dossier choisi) **ou** bibliotheque de l'application locale alimentee par push navigateur | televersement vers le serveur (`POST /api/v1/music/tracks`) |
| Transport vers la montre | **USB / adb push** par `mpacer-music` | telechargement Wi-Fi par la montre |
| Stockage serveur | **aucun octet audio** : fiches, liste MP3, manifeste | volume applicatif + `MPACER_MEDIA_DIR` |
| Statut de synchro | **par fichier**, dans l'application locale (`library.json`) | `music_download_plans.acked_at_ms` cote serveur |

Raisons : la montre lit deja `getExternalFilesDir("Music")` par USB, la copie est
hors ligne et gratuite, et le service reste sans volume audio (backup, RGPD,
cout cluster). Un vrai « push serveur puis synchro Wi-Fi » demanderait un
telechargement cote montre, une reprise sur coupure et un stockage applicatif :
c'est un chantier distinct, decrit en section 9.

## 3. Actions a mettre en place

### 3.1 Cote comptes developpeur (une fois)

**Deezer** (source ajoutee) :

1. se connecter sur <https://developers.deezer.com/myapps> ;
2. **Create a new Application** : nom (`M-pacer`), description, accepter les
   conditions ;
3. noter l'**Application ID** et la **Secret Key** ;
4. dans l'onglet **Application Domains**, declarer le domaine du service
   (`mpacer.p.zacharie.org` ou `localhost` en developpement) — Deezer refuse
   toute `redirect_uri` hors domaine declare ;
5. l'URL de redirection utilisee par M-pacer est
   `{public_url}/auth/deezer/callback` (surchargeable par
   `MPACER_DEEZER_REDIRECT_URI`) ;
6. permissions demandees par le code : `basic_access,email` (lecture du profil
   et des playlists, aucun droit d'ecriture).

**Spotify** (source existante, rappel) :

1. <https://developer.spotify.com/dashboard> -> **Create app** ;
2. *Redirect URI* : `{public_url}/auth/spotify/callback` ;
3. cocher **Web API**, scopes utilises : `playlist-read-private`,
   `playlist-read-collaborative` ;
4. noter *Client ID* et *Client secret*. Depuis le 27/11/2024,
   `GET /v1/audio-features` est refuse aux nouvelles applications : le BPM
   Spotify est un **bonus**, jamais une dependance.

### 3.2 Cote configuration du service

| Variable | Role | Defaut |
|---|---|---|
| `MPACER_DEEZER_APP_ID` | identifiant de l'application Deezer | *(vide : source eteinte)* |
| `MPACER_DEEZER_APP_SECRET` | secret applicatif Deezer | *(vide)* |
| `MPACER_DEEZER_REDIRECT_URI` | URI enregistree chez Deezer | `{public_url}/auth/deezer/callback` |
| `MPACER_SPOTIFY_CLIENT_ID` / `_SECRET` | OAuth Spotify | *(vide)* |
| `MPACER_SPOTIFY_REDIRECT_URI` | URI enregistree chez Spotify | `{public_url}/auth/spotify/callback` |

Sur jo3, ajouter les deux cles Deezer au secret Vault puis forcer la
synchronisation (voir [deploy/GITOPS-ET-SECRETS.md](../deploy/GITOPS-ET-SECRETS.md)) :

```bash
vault kv put apps/mpacer \
  MPACER_SESSION_SECRET="$(openssl rand -base64 48)" \
  MPACER_GOOGLE_CLIENT_ID="<ID>.apps.googleusercontent.com" \
  MPACER_GOOGLE_CLIENT_SECRET="GOCSPX-<secret>" \
  MPACER_SPOTIFY_CLIENT_ID="<id Spotify>" \
  MPACER_SPOTIFY_CLIENT_SECRET="<secret Spotify>" \
  MPACER_DEEZER_APP_ID="<id Deezer>" \
  MPACER_DEEZER_APP_SECRET="<secret Deezer>"

kubectl -n mpacer annotate externalsecret mpacer-secrets force-sync="$(date +%s)" --overwrite
```

Ne pas oublier de renseigner les cles correspondantes
(`externalSecrets.keys.deezerAppId`, `deezerAppSecret`) dans les valeurs du
chart : un `remoteRef` vers une propriete absente fait echouer l'ExternalSecret.
Sans identifiants, la source est simplement annoncee « non configuree » sur
`/music`.

### 3.3 Cote code (fait dans cette livraison)

| Chantier | Contenu |
|---|---|
| Client Deezer | `crates/mpacer-api/src/deezer.rs` : OAuth 2.0, `list_user_playlists`, `search_playlists`, `get_playlist`, `current_user`, `playlist_id_from_ref`, lecture du jeton (JSON **ou** chaine de requete), detection des erreurs applicatives renvoyees en HTTP 200 |
| Configuration | `MPACER_DEEZER_*` + `deezer_configured()` / `deezer_redirect_uri()` / `deezer_redirect_path()` |
| Base | migration `0006-sources-musique.sql` : colonne `music_playlists.deezer_id` + table `deezer_accounts` |
| Page `/music` | bloc 1 a **deux panneaux** (Spotify / Deezer) : connexion, recherche, `Mes playlists`, import ; bloc 4 **Fichiers a preparer (MP3)** ; blocs 5 et 6 renumerotes |
| Import | `POST /music/import` accepte `source` (`spotify`|`deezer`) + `ref` ; `spotify_ref` reste accepte (compatibilite) |
| Export MP3 | `GET /music/playlists/{id}/files` : piece jointe `.txt`, un nom de fichier attendu par piste |
| Application locale | `crates/mpacer-music` : bibliotheque geree (`library.json`), endpoints `POST/GET/DELETE /api/library/...`, push navigateur, statut par fichier, synchro USB |

### 3.4 Cote utilisateur (mode operatoire)

1. `/music` : connecter **Spotify** ou **Deezer** (bloc 1) ;
2. chercher une playlist ou cliquer **Mes playlists**, puis **Importer** ;
3. bloc 4 : telecharger la liste `.txt` -> rassembler les MP3 correspondants ;
4. dans `mpacer-music` : analyser le manifeste, puis soit designer le dossier des
   MP3, soit **pousser les fichiers** dans la section 6 de la page locale ;
5. **Transferer sur la montre** : chaque fichier passe de `a synchroniser` a
   `synchronise` (ou `erreur` avec le message) ;
6. sur la montre : **Musique > Importer (USB)**.

## 4. Contrat d'interface

### 4.1 API appareil (inchangee)

`GET /api/v1/music/playlists`, `GET /api/v1/music/playlists/{id}` et
`GET /api/v1/music/playlists/{id}/manifest` restent tels quels : la source d'une
playlist est une chaine (`spotify`, `deezer`, `manual`) deja publiee.

### 4.2 Interface web

| Methode | Chemin | Role |
|---|---|---|
| GET | `/music?source=spotify|deezer&q=...` | page complete (6 blocs) |
| GET | `/music/search?source=...&q=...` | recherche dans la source |
| GET | `/music/search?source=...&vue=mes` | **playlists du compte lie** |
| GET | `/auth/deezer` + `/auth/deezer/callback` | OAuth 2.0 Deezer |
| POST | `/music/deezer/disconnect` | deconnecte le compte Deezer |
| POST | `/music/import` | `source`, `ref`, `target_bpm` |
| GET | `/music/playlists/{id}/files` | **liste des MP3 a preparer** (`.txt`) |
| GET | `/music/playlists/{id}/manifest` | manifeste de transfert (inchange) |

Les routes Spotify existantes sont inchangees ; `POST /music/import` accepte en
plus `source` et `ref`, et continue d'accepter `spotify_ref`.

### 4.3 Page `/music` (six blocs)

```text
+------------------------------------------------------------------------------+
| 1. Source des playlists (Spotify ou Deezer)                                  |
|   [ Spotify ] Connecte  recherche [ running ] [Chercher] [Mes playlists]     |
|   [ Deezer  ] Connecte  recherche [ running ] [Chercher] [Mes playlists]     |
| 2. Playlists preparees                                                       |
| 3. Titres (playlist selectionnee)   BPM : [tapper] [saisir]                  |
| 4. Fichiers a preparer (MP3)   [ Telecharger la liste (.txt) ]               |
|    | 1 | Wake me up | Avicii | 01 - Avicii - Wake me up.mp3 |                |
| 5. Transfert vers la montre (USB)   [ Telecharger le manifeste ]             |
| 6. Assez de musique pour la course ?                                         |
+------------------------------------------------------------------------------+
```

### 4.4 Application locale `mpacer-music`

Nouveaux endpoints (page locale `127.0.0.1:8077`) :

| Methode | Chemin | Role |
|---|---|---|
| POST | `/api/library/{playlist_id}?name=fichier.mp3` | pousse un MP3 (corps brut) dans la bibliotheque |
| GET | `/api/library/{playlist_id}` | `{"files":[{"file_name","size_bytes","status","synced_at_ms","error"}]}` |
| DELETE | `/api/library/{playlist_id}/{name}` | retire un fichier de la bibliotheque |

Statuts : `a_synchroniser` (fichier present, pas encore copie), `synchronise`
(copie reussie, avec horodatage) et `erreur` (derniere copie en echec, message
conserve). Une **annulation** n'est pas un echec : les fichiers restent en
`a_synchroniser`. La bibliotheque vit sous
`%LOCALAPPDATA%\mpacer-music\library` (Windows) ou
`$XDG_DATA_HOME/mpacer-music/library` ; l'index `library.json` de chaque
playlist porte la taille, la date d'import, le statut et l'horodatage de synchro.
`--library DIR` sur `serve`, `inspect` et `transfer` remplace ce chemin.
`inspect` et `transfer` scannent **le dossier choisi et la bibliotheque** : un
fichier pousse est apparie exactement comme un fichier du disque.

## 5. Ce que la montre fait (inchange)

`Musique > Importer (USB)` relit `getExternalFilesDir("Music")` : chaque
sous-dossier contenant un `manifest.json` devient une playlist locale. Le statut
de synchro est celui de l'application locale (copie reelle sur la montre) ; la
montre n'a aucun appel reseau a faire.

## 6. Plan de verification

| Chantier | Preuve attendue |
|---|---|
| Deezer | `cargo test -p mpacer-api` : lecture du jeton (JSON et chaine de requete), erreurs applicatives, references de playlist, pagination, URL d'autorisation |
| Configuration | test `deezer_is_optional_and_configurable` : sans identifiants, sans gabarit, avec identifiants |
| Page `/music` | test unitaire : `prepared_files` (schema `01 - Artiste - Titre.mp3`), export texte, normalisation de source ; test d'integration : les six blocs, `01 - Avicii - Wake me up.mp3`, `Telecharger la liste (.txt)` |
| Application locale | `cargo test -p mpacer-music` : import, liste, suppression, statuts, nom de fichier refuse, index corrompu, endpoints `/api/library` |
| Qualite | `cargo clippy --workspace --all-targets -- -D warnings` et `cargo fmt --all --check` |
| Bout en bout | sans montre : `mpacer-music transfer --target-dir` sur un dossier de test + bibliotheque ; avec montre : `adb push` reel (non exerce ici) |

## 7. Hors perimetre

* lecture de l'audio Spotify ou Deezer (DRM) : jamais ;
* conversion automatique d'un format vers MP3 : la liste indique **le nom attendu**,
  la conversion reste a la charge de l'utilisateur (ffmpeg, etc.) ;
* telechargement automatique des MP3 depuis un service tiers ;
* synchro Wi-Fi montre <-> serveur : voir section 9.

## 8. Suites possibles

* bouton « Copier la liste » dans le bloc 4 (aujourd'hui l'export `.txt` suffit) ;
* detection automatique d'un dossier de MP3 par la montre pour un import sans
  commande ;
* conversion MP3 assistee dans `mpacer-music` si un encodeur est present.

## 9. Alternative « push serveur + synchro Wi-Fi » (non retenue ici)

Si l'on veut pousser les MP3 depuis le telephone ou une page web distante et
laisser la montre les recuperer en Wi-Fi, il faut : un stockage applicatif
(`MPACER_MEDIA_DIR` + volume), `POST /api/v1/music/tracks` (multipart), un
endpoint de telechargement par jeton d'appareil, un plan de telechargement avec
accuse (`music_download_plans.acked_at_ms`), et un client de telechargement cote
montre avec reprise sur coupure et quota d'espace. C'est un chantier complet,
independant de cette livraison : la decision v2/v3 (aucun audio sur le serveur)
reste donc en vigueur.

## 10. Etat d'implementation (7 octobre 2026)

| Chantier | Livre | Preuve |
|---|---|---|
| Deezer (backend) | `crates/mpacer-api/src/deezer.rs` (OAuth, recherche, playlists du compte, fiche, jeton JSON ou chaine de requete), configuration `MPACER_DEEZER_*`, migration `0006-sources-musique.sql`, table `deezer_accounts`, routes `/auth/deezer` + `/music/deezer/disconnect` | `cargo test -p mpacer-api` : 71 tests unitaires, 29 verts + 1 ignore en integration |
| Liste des MP3 | bloc 4 « Fichiers a preparer (MP3) » et export `GET /music/playlists/{id}/files` (`.txt`) | test unitaire `prepared_files_follow_the_watch_scheme` ; test d'integration de la page a six blocs (nom `01 - Avicii - Wake me up.mp3`) |
| Application locale | `crates/mpacer-music/src/library.rs`, push `POST /api/library/{id}?name=...`, liste et suppression, statut par fichier, `inspect_with_library`, `--library` | `cargo test -p mpacer-music` : 25 tests unitaires + 12 tests d'integration, dont le passage `a_synchroniser` -> `synchronise` apres une copie reelle |
| Deploiement | cles Deezer dans `charts/mpacer` (values, ConfigMap, Secret, ExternalSecret) et `deploy/GITOPS-ET-SECRETS.md` | `helm lint` + `helm template` : `MPACER_DEEZER_APP_ID`/`_SECRET` dans le Secret et l'ExternalSecret, `MPACER_DEEZER_REDIRECT_URI` dans le ConfigMap |
| Qualite | — | `cargo test --workspace` vert, `cargo clippy --workspace --all-targets -- -D warnings` propre, `cargo fmt --all --check` propre |
