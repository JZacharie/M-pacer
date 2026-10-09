# 11 - Playlists Deezer, liste des MP3 et push dans l'application

> **Statut** : ajout au contrat v2/v3 de [docs/07](07-musique-bpm-et-playlists.md)
> (7 octobre 2026). Le principe « le serveur ne stocke aucun audio » est
> **conserve** : ce document decrit la source de metadonnees **Deezer**, le
> telechargement des MP3 par l'instance **Deemix** de l'utilisateur et l'envoi de
> fichiers dans **l'application locale**.

---

## 1. Ce que la fonctionnalite apporte

1. **Recuperer la liste des playlists depuis Deezer** : la source se connecte
   par cookie `arl` (ou par OAuth), expose une **recherche** et la liste des
   **playlists du compte** (`Mes playlists`), puis importe la fiche choisie
   (titres, artistes, durees). Deezer n'expose aucun tempo : le BPM reste inconnu
   a l'import et se complete par la balise du MP3, le tap-tempo ou la saisie.
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

**Deezer par cookie `arl` (voie recommandee, aucune inscription) :**

1. se connecter sur <https://www.deezer.com> dans un navigateur ;
2. ouvrir les outils de developpement > **Application** > **Cookies** >
   `https://www.deezer.com` et copier la valeur du cookie **`arl`** ;
3. la sceller dans le secret du service (`MPACER_DEEZER_ARL`, voir
   [deploy/GITOPS-ET-SECRETS.md](../deploy/GITOPS-ET-SECRETS.md) section 2) ou la
   poser dans `.env` en local.

Ce cookie ouvre l'**API privee** du site (`gw-light`) : profil, playlists du
compte (**y compris privees**) et pistes avec leur identifiant Deezer — c'est cet
identifiant qui construit le lien de telechargement Deemix. Le mode OAuth
ci-dessous reste disponible mais n'est plus necessaire ; quand le cookie est
present, il est prioritaire. Deezer renouvelle rarement l'`arl` : si le service
repond « Le cookie Deezer (ARL) a ete refuse », il suffit de recopier la nouvelle
valeur.

**Deezer par OAuth (repli) :**

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

### 3.2 Cote configuration du service

| Variable | Role | Defaut |
|---|---|---|
| `MPACER_DEEZER_ARL` | **cookie `arl` du compte Deezer** : lecture des playlists sans application developpeur | *(vide : source eteinte)* |
| `MPACER_DEEMIX_URL` | instance Deemix des liens MP3 (routes `#/playlist/{id}`, `#/track/{id}`) | `https://deemix.p.zacharie.org` |
| `MPACER_DEEMIX_USER` / `MPACER_DEEMIX_PASSWORD` | authentification HTTP basique de l'instance Deemix : active l'**envoi dans la file** de telechargement et l'affichage de la progression | *(vide : liens seuls)* |
| `MPACER_DEEZER_APP_ID` | identifiant de l'application Deezer (repli OAuth) | *(vide : source eteinte)* |
| `MPACER_DEEZER_APP_SECRET` | secret applicatif Deezer (repli OAuth) | *(vide)* |
| `MPACER_DEEZER_REDIRECT_URI` | URI enregistree chez Deezer | `{public_url}/auth/deezer/callback` |

Sur jo3, ajouter les deux cles Deezer au secret Vault puis forcer la
synchronisation (voir [deploy/GITOPS-ET-SECRETS.md](../deploy/GITOPS-ET-SECRETS.md)) :

```bash
vault kv put apps/mpacer \
  MPACER_SESSION_SECRET="$(openssl rand -base64 48)" \
  MPACER_GOOGLE_CLIENT_ID="<ID>.apps.googleusercontent.com" \
  MPACER_GOOGLE_CLIENT_SECRET="GOCSPX-<secret>" \
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
| Client Deezer par cookie `arl` | meme fichier : `arl_session`, `list_user_playlists_arl`, `search_playlists_arl`, `get_playlist_arl` sur l'API privee `gw-light` (cookie de session `sid` repris apres le premier appel, pagination, identifiant Deezer de chaque piste) |
| Telechargement Deemix | bloc 4 : lien `#/playlist/{id}` pour la playlist et `#/track/{id}` par piste (`MPACER_DEEMIX_URL`), export `GET /music/playlists/{id}/deemix` |
| Configuration | `MPACER_DEEZER_*` + `deezer_configured()` / `deezer_redirect_uri()` / `deezer_redirect_path()` |
| Base | migration `0006-sources-musique.sql` : colonne `music_playlists.deezer_id` + table `deezer_accounts` ; migration `0009-source-unique-deezer.sql` : retrait des restes de Spotify (table `spotify_accounts`, colonnes `spotify_id`/`spotify_uri`, valeurs `spotify` ramenees a `manual`) |
| Page `/music` | bloc 1 : panneau **Deezer** (connexion ou cookie `arl`, recherche, `Mes playlists`, import) ; bloc 4 **Fichiers a preparer (MP3)** ; blocs 5 et 6 renumerotes |
| Import | `POST /music/import` accepte `ref` (lien, URI ou identifiant Deezer) + `target_bpm` |
| Export MP3 | `GET /music/playlists/{id}/files` : piece jointe `.txt`, un nom de fichier attendu par piste |
| Application locale | `crates/mpacer-music` : bibliotheque geree (`library.json`), endpoints `POST/GET/DELETE /api/library/...`, push navigateur, statut par fichier, synchro USB |

### 3.4 Cote utilisateur (mode operatoire)

1. `/music` : rien a connecter quand `MPACER_DEEZER_ARL` est renseigne ; sinon
   **Connecter Deezer** (OAuth, bloc 1) ;
2. chercher une playlist ou cliquer **Mes playlists**, puis **Importer** ;
3. bloc 4 : **Telecharger dans Deemix** remet la playlist dans la file de
   l'instance (ou **Envoyer** piste par piste), puis **File Deemix** montre la
   progression ; la liste `.txt` donne les noms de fichiers attendus ;
4. dans `mpacer-music` : pointer le dossier des MP3 telecharges par Deemix
   (`--folder`), analyser le manifeste, puis soit designer ce dossier, soit
   **pousser les fichiers** dans la section 6 de la page locale ;
5. **Transferer sur la montre** : chaque fichier passe de `a synchroniser` a
   `synchronise` (ou `erreur` avec le message) ;
6. sur la montre : **Musique > Importer (USB)**.

## 4. Contrat d'interface

### 4.1 API appareil (inchangee)

`GET /api/v1/music/playlists`, `GET /api/v1/music/playlists/{id}` et
`GET /api/v1/music/playlists/{id}/manifest` restent tels quels : la source d'une
playlist est une chaine (`deezer`, `manual`) deja publiee.

### 4.2 Interface web

| Methode | Chemin | Role |
|---|---|---|
| GET | `/music?q=...` | page complete (6 blocs) |
| GET | `/music/search?q=...` | recherche de playlists Deezer |
| GET | `/music/search?vue=mes` | **playlists du compte Deezer** |
| GET | `/auth/deezer` + `/auth/deezer/callback` | OAuth 2.0 Deezer |
| POST | `/music/deezer/disconnect` | deconnecte le compte Deezer |
| POST | `/music/import` | `ref`, `target_bpm` |
| GET | `/music/playlists/{id}/files` | **liste des MP3 a preparer** (`.txt`) |
| GET | `/music/playlists/{id}/deemix` | **liste de telechargement Deemix** (`.txt`) : un fichier attendu par ligne, suivi du lien Deemix de la piste |
| POST | `/music/playlists/{id}/deemix` | **envoie la playlist dans la file de Deemix** (instance de l'utilisateur) |
| POST | `/music/playlists/{id}/deemix/track` | envoie une piste (`track_id`) dans la file de Deemix |
| GET | `/music/playlists/{id}/manifest` | manifeste de transfert (inchange) |

### 4.3 Page `/music` (six blocs)

```text
+------------------------------------------------------------------------------+
| 1. Source des playlists (Deezer)                                             |
|   [ Deezer ] Connecte  recherche [ rock ] [Chercher] [Mes playlists]         |
| 2. Playlists preparees                                                       |
| 3. Titres (playlist selectionnee)   BPM : [tapper] [saisir]                  |
| 4. Fichiers a preparer (MP3)                                                 |
|    [ Telecharger dans Deemix ] [ Ouvrir dans Deemix ]                        |
|    [ Telecharger la liste (.txt) ] [ Liste Deemix (.txt) ] [ File Deemix ]   |
|    | 1 | Wake me up | Avicii | 01 - Avicii - Wake me up.mp3 | Ouvrir Envoyer ||
|    File Deemix : Automatic For The People | R.E.M. | 98 %                  |
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

### 4.5 Instance Deemix (mise en file des MP3)

M-pacer ne telecharge rien : il remet la reference Deezer a l'instance **Deemix**
de l'utilisateur, qui ecrit les MP3 dans son dossier `downloads` (volume NFS
`/apool/share/plex/Deemix` sur jo3). L'API utilisee est celle de l'interface
Deemix — il n'y en a pas d'autre — relevee puis verifiee contre une instance
reelle :

| Methode | Chemin | Role |
|---|---|---|
| POST | `/api/loginArl` | ouvre la session Deezer de l'instance (`{"arl": "..."}`) |
| POST | `/api/addToQueue` | ajoute une reference Deezer (`{"url", "bitrate"}`) |
| GET | `/api/getQueue` | file de telechargement (titre, taille, progression, erreurs) |

Deux contraintes a connaitre :

1. l'instance est protegee par une **authentification HTTP basique** (Traefik) :
   le service envoie `MPACER_DEEMIX_USER` / `MPACER_DEEMIX_PASSWORD` ;
2. la session Deezer est portee par un **cookie** (`connect.sid`) : sans le
   cookie rendu par `loginArl`, Deemix repond `NotLoggedIn` a `addToQueue`,
   meme quand le cookie `arl` est deja enregistre cote instance. Le service
   rouvre donc une session a chaque envoi, avec `MPACER_DEEZER_ARL`.

Le modele de nommage de Deemix est regle sur `%position% - %artist% - %title%`
pour les playlists : les fichiers telecharges portent deja le nom attendu par la
montre, que `mpacer-music` retrouve a l'appariement.

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
| Deezer par `arl` | test ignore `cargo test -p mpacer-api --test deezer_live -- --ignored` : session ouverte avec le cookie, playlists du compte listees, fiche d'une grande playlist lue (pagination), identifiant Deezer de chaque piste |
| Instance Deemix | test ignore `cargo test -p mpacer-api --test deemix_live -- --ignored` : file lue, reference Deezer envoyee (session `loginArl` + `addToQueue`) |
| Application locale | `cargo test -p mpacer-music` : import, liste, suppression, statuts, nom de fichier refuse, index corrompu, endpoints `/api/library` |
| Qualite | `cargo clippy --workspace --all-targets -- -D warnings` et `cargo fmt --all --check` |
| Bout en bout | sans montre : `mpacer-music transfer --target-dir` sur un dossier de test + bibliotheque ; avec montre : `adb push` reel (non exerce ici) |

## 7. Hors perimetre

* lecture de l'audio Deezer (DRM) : jamais ;
* conversion automatique d'un format vers MP3 : la liste indique **le nom attendu**,
  la conversion reste a la charge de l'utilisateur (ffmpeg, etc.) ;
* telechargement des MP3 par M-pacer : le service **remet la reference Deezer**
  a l'instance Deemix de l'utilisateur (bouton « Telecharger dans Deemix ») et
  lit sa file pour afficher la progression, mais il ne telecharge rien lui-meme
  et ne stocke aucun audio ; les fichiers arrivent dans le dossier `downloads`
  de Deemix ;
* synchro Wi-Fi montre <-> serveur : voir section 9.

## 8. Suites possibles

* bouton « Copier la liste » dans le bloc 4 (aujourd'hui l'export `.txt` suffit) ;
* detection automatique d'un dossier de MP3 par la montre pour un import sans
  commande ;
* conversion MP3 assistee dans `mpacer-music` si un encodeur est present ;
* **pousser les MP3 depuis la page `/music` directement vers la montre ou le
  telephone** : deux chemins etudies dans [16 - Pousser des MP3 depuis le front
  end](16-poussee-mp3-front-vers-appareils.md) (passerelle avec `mpacer-music`
  par USB, ou relais serveur puis telechargement par l'appareil).

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

### 10.1 Deezer par cookie `arl` et telechargement Deemix

| Chantier | Livre | Preuve |
|---|---|---|
| Cookie `arl` | `crates/mpacer-api/src/deezer.rs` : `arl_session` (`deezer.getUserData`), `list_user_playlists_arl` (`deezer.pageProfile`), `search_playlists_arl` (`deezer.pageSearch`), `get_playlist_arl` (`deezer.pagePlaylist`) sur l'API privee `gw-light`, reprise du cookie de session `sid` apres chaque reponse (sans lui Deezer repond `VALID_TOKEN_REQUIRED`) | test unitaire `gw_light_errors_are_detected_in_a_successful_body` + charges utiles reelles ; test ignore `tests/deezer_live.rs` contre l'API Deezer (19 playlists du compte, 155 pistes paginees, identifiant Deezer de chaque piste) |
| Conservation des identifiants | migration `0008-deezer-arl.sql` (`music_tracks.deezer_track_id`), `MusicTrack`/`MusicTrackInput`/`insert_music_track` | test unitaire `deemix_links_follow_the_webui_routes` ; test d'integration `deemix_list_exports_the_download_links` |
| Liens Deemix | bloc 4 : lien de playlist `#/playlist/{id}`, lien par piste `#/track/{id}`, export `GET /music/playlists/{id}/deemix` ; `MPACER_DEEMIX_URL` (defaut `https://deemix.p.zacharie.org`) | tests unitaires `deemix_links_follow_the_webui_routes` (page et export) |
| Instance Deemix | `crates/mpacer-api/src/deemix.rs` : `add_to_queue` (`loginArl` + `addToQueue`, cookie de session repris) et `queue` (`getQueue`) ; boutons « Telecharger dans Deemix » / « Envoyer » et bloc « File Deemix » dans le bloc 4 ; `MPACER_DEEMIX_USER`/`_PASSWORD` | tests unitaires `a_successful_add_reports_the_created_entries`, `the_queue_is_read_with_its_progress` ; test ignore `tests/deemix_live.rs` contre l'instance reelle (4 entrees lues, reference envoyee) |
| Numerotation des MP3 | positions **1-based** dans le manifeste (`models::manifest_position`) et dans la liste du bloc 4 : la base numerote depuis 0, la montre depuis 1. Avant, les deux premieres pistes d'une playlist portaient le meme prefixe `01` | tests unitaires `manifest_positions_are_one_based`, `prepared_files_follow_the_watch_scheme` (01 / 02 / 03) |
| Configuration | `MPACER_DEEZER_ARL` prioritaire sur l'OAuth ; `deezer_configured()` vrai avec le seul cookie ; panneau Deezer sans bouton « Connecter » en mode `arl` | tests unitaires `deezer_arl_enables_the_source_without_a_developer_application`, `deemix_url_defaults_to_the_zacharie_instance` |
| Deploiement | `charts/mpacer` : `auth.deezerArl`/`auth.deemixUser`/`auth.deemixPassword` -> Secret, `externalSecrets.keys.deezerArl`/`deemixUser`/`deemixPassword` -> ExternalSecret, `config.deemixUrl` -> ConfigMap ; section 2 de `deploy/GITOPS-ET-SECRETS.md` | `helm lint` + `helm template` |

### 10.2 Livraison v2/v3 (rappel)

| Chantier | Livre | Preuve |
|---|---|---|
| Deezer (backend) | `crates/mpacer-api/src/deezer.rs` (OAuth, recherche, playlists du compte, fiche, jeton JSON ou chaine de requete), configuration `MPACER_DEEZER_*`, migration `0006-sources-musique.sql`, table `deezer_accounts`, routes `/auth/deezer` + `/music/deezer/disconnect` | `cargo test -p mpacer-api` : 71 tests unitaires, 29 verts + 1 ignore en integration |
| Liste des MP3 | bloc 4 « Fichiers a preparer (MP3) » et export `GET /music/playlists/{id}/files` (`.txt`) | test unitaire `prepared_files_follow_the_watch_scheme` ; test d'integration de la page a six blocs (nom `01 - Avicii - Wake me up.mp3`) |
| Application locale | `crates/mpacer-music/src/library.rs`, push `POST /api/library/{id}?name=...`, liste et suppression, statut par fichier, `inspect_with_library`, `--library` | `cargo test -p mpacer-music` : 25 tests unitaires + 12 tests d'integration, dont le passage `a_synchroniser` -> `synchronise` apres une copie reelle |
| Deploiement | cles Deezer dans `charts/mpacer` (values, ConfigMap, Secret, ExternalSecret) et `deploy/GITOPS-ET-SECRETS.md` | `helm lint` + `helm template` : `MPACER_DEEZER_APP_ID`/`_SECRET` dans le Secret et l'ExternalSecret, `MPACER_DEEZER_REDIRECT_URI` dans le ConfigMap |
| Qualite | — | `cargo test --workspace` vert, `cargo clippy --workspace --all-targets -- -D warnings` propre, `cargo fmt --all --check` propre |
