# 10 — Suivi en direct (MQTT)

Pendant une séance, la montre publie sa position sur un **broker MQTT** ; le
service s'y abonne et la page **/live** affiche la trace en temps réel sur un
**fond de carte OpenStreetMap**, avec le **profil de dénivelé** de la séance. Les
proches (ou vous-même, depuis un autre appareil) suivez la course sans que la
montre ait à parler HTTP, et sans que la séance dépende du réseau : le suivi en
direct est un **confort**, l'enregistrement reste local puis synchronisé à la
fin comme avant.

Ce document décrit le contrat entre la montre et le service, le coût réel de la
fonctionnalité, et ce qui n'est pas fait.

---

## 1. Principe

```text
Montre Wear OS                 Broker MQTT            mpacer-api                Navigateur
┌──────────────┐   PUBLISH    ┌──────────┐  SUBSCRIBE ┌──────────────┐   GET    ┌───────────┐
│ GPS 1 Hz     │  QoS 0       │ mosquitto│  QoS 1     │ LiveStore    │  /live   │ carte OSM │
│ LiveTracker  │─────────────▶│ (cluster)│───────────▶│ (mémoire)    │─────────▶│ 10 s      │
│ 1 point/10 s │  ~145 o      │          │            │ 4096 pts max │          │ auto-     │
└──────────────┘              └──────────┘            └──────────────┘          │ rafraîchi │
                                                                               └───────────┘
```

Trois choix structurants :

1. **La montre publie, elle ne sert pas.** Aucun port ouvert, aucun certificat à
   gérer côté montre : elle se connecte au broker, comme elle se connecte au
   backend pour envoyer une séance.
2. **Le suivi est volatil.** Aucune position n'est écrite en base : le magasin
   du service est un anneau en mémoire, purgé après 24 h de silence. La séance,
   elle, arrive par `POST /api/v1/workouts` et reste la source de vérité.
3. **Le coût est borné par construction** — cadence, filtre de précision, file
   bornée, un seul fil, aucune minuterie (§ 5).

---

## 2. Contrat MQTT

### 2.1 Sujet

| Élément | Valeur |
|---|---|
| Sujet publié | `<préfixe>/live/<montre>`, par défaut `mpacer/live/<montre>` |
| Filtre souscrit | `MPACER_MQTT_TOPIC`, par défaut `mpacer/live/+` |
| `<montre>` | nom choisi dans les réglages, sinon les 8 premiers caractères de l'`ANDROID_ID` |

Un nom de montre ne peut contenir que des lettres, chiffres, `-`, `_` et `.`
(les espaces deviennent des `-`) : aucun caractère de sujet MQTT n'est laissé au
hasard.

### 2.2 Charge utile

JSON compact, champs courts — un point pèse **116 octets** (132 avec l'altitude,
152 si le nom de la montre voyage aussi avec lui ; par défaut il n'est que dans
le sujet) :

| Champ | Sens | Exemple |
|---|---|---|
| `t` | horodatage de la mesure (ms epoch) | `1728222000000` |
| `lat`, `lon` | position (6 décimales, ~0,1 m) | `48.856600`, `2.352200` |
| `acc` | précision annoncée par le GPS (m) | `4.0` |
| `alt` | altitude (m) : profil de dénivelé de la page `/live` | `154.3` |
| `dist` | distance parcourue depuis le départ (m) | `1234.5` |
| `pace` | allure lissée du cœur (s/km) | `300.0` |
| `hr` | fréquence cardiaque (bpm) | `142` |
| `bat` | batterie de la montre (%) | `78` |
| `st` | état de la séance : `arm`, `run`, `pause`, `stop` | `run` |
| `dev` | nom de la montre (facultatif, sinon le sujet suffit) | `montre-a1b2` |

Exemple complet :

```json
{"t":1728222000000,"lat":48.856600,"lon":2.352200,"acc":4.0,"alt":154.3,
 "dist":1234.5,"pace":300.0,"hr":142,"bat":78,"st":"run","dev":"montre-a1b2"}
```

> Le champ `lap` (numéro de tour) est prévu par le format côté service mais
> n'est pas encore publié : le cœur n'expose pas de tour courant dans
> `EngineOutput`, et la montre ne calcule rien elle-même.

### 2.3 Qualité de service et retenue

| Sens | QoS | Retenue (`retain`) |
|---|---|---|
| Montre → broker | 0 | oui |
| Broker → service | 1 | — (le service accuse réception) |

La montre publie en **QoS 0** : un point perdu toutes les dix secondes est
invisible sur une trace, et l'accusé de réception coûterait de la radio et de la
mémoire. Le message est **retenu** : un proche qui ouvre la page en cours de
route voit immédiatement la dernière position connue, sans attendre le point
suivant. Le dernier message d'une séance porte `"st":"stop"` : la page affiche
alors « séance terminée » et cesse de se rafraîchir.

---

## 3. Côté montre (Wear OS)

| Fichier | Rôle |
|---|---|
| `.../watch/live/LiveConfig.kt` | réglages, analyse de l'adresse du broker (`mqtt://`, `mqtts://`) |
| `.../watch/live/LivePolicy.kt` | **politique de cadence** : une publication par période, filtre de précision |
| `.../watch/live/LivePayload.kt` | charge utile JSON compacte (locale `ROOT`, texte assaini) |
| `.../watch/live/MqttCodec.kt` | sérialisation MQTT 3.1.1 (CONNECT, PUBLISH, PINGREQ, DISCONNECT) |
| `.../watch/live/LiveTracker.kt` | un fil de fond : connexion, reconnexion, décharge en rafale, compteurs |
| `.../watch/live/LiveQueue.kt` | la file bornée (20 min / 120 points) : fenêtre, compression, compteurs |
| `.../watch/live/LiveSettings.kt` | persistance (mot de passe en `EncryptedSharedPreferences`) |
| `.../watch/live/LiveProbe.kt` | **test de connexion** demandé depuis la montre (CONNACK + point de test) |
| `.../watch/TrackingService.kt` | branche le suivi au départ et à l'arrêt, tend chaque position |
| `.../watch/ui/SettingsScreen.kt` | activation, cadence, état de la liaison, accès au menu MQTT |
| `.../watch/ui/LiveSettingsScreen.kt` | **menu MQTT** : adresse, identifiants, sujet, cadence, test |

Règles tenues :

- **Aucune bibliothèque MQTT embarquée** : 4 paquets suffisent, testés octet par
  octet. Pas de client complet avec ses tampons, ses fils et ses `ScheduledExecutor`.
- **Aucun réveil supplémentaire** : le fil attend sur une condition et se
  réveille quand une position est mise en file (ou à l'expiration du délai de
  `PINGREQ`). Rien ne tourne quand la séance est arrêtée.
- **La montre ne calcule rien** : distance, allure et cardio viennent déjà du
  cœur Rust ; `LiveTracker` ne fait que transmettre, comme le reste du shell.
- **La séance ne dépend pas du réseau** : si le broker est injoignable, la
  montre publie ce qu'elle peut et la séance se termine normalement.

### Réglages

| Réglage | Défaut | Effet |
|---|---|---|
| URL du broker | vide | vide = suivi **désactivé**, coût nul. Les identifiants peuvent y figurer : `mqtt://joseph:secret@192.168.0.115:1883` |
| Préfixe de sujet | `mpacer` | sujet publié : `<préfixe>/live/<montre>` |
| Nom de la montre | `ANDROID_ID` (8 car.) | dernier segment du sujet |
| Identifiant / mot de passe | vide | authentification du broker (facultative) |
| Cadence en course | 10 s | 5 s à 5 min |
| Cadence en pause | 60 s | la montre ne bouge plus |
| Précision minimale | 50 m | en dessous, le point n'est pas publié |

Tout se règle **depuis la montre** : Réglages ▸ *Broker MQTT*. L'écran ouvre le
clavier Wear pour l'adresse (`mqtt://hote:1883`), l'utilisateur, le mot de passe,
le préfixe de sujet et le nom de la montre, et propose la cadence en course comme
en pause. Deux commodités pour un écran rond :

- **Coller l'adresse** reprend le presse-papiers : une URL complète
  (`mqtt://joseph:secret@192.168.0.115:1883`) peut ainsi arriver du téléphone,
  et les identifiants qu'elle contient sont analysés (y compris encodés :
  `%40` pour `@`) ;
- **Tester la connexion** ouvre une connexion avec les valeurs affichées —
  brouillon non enregistré — vérifie le `CONNACK` puis publie un point de test
  sur `<préfixe>/test/<montre>`, **hors** du filtre `<préfixe>/live/+` souscrit
  par le service : la page `/live` n'est jamais polluée par un essai de réglages.
  Le message d'erreur distingue une adresse illisible, un broker injoignable et
  un refus d'authentification (codes 1 à 5 du CONNACK).

Le mot de passe est enregistré dans `EncryptedSharedPreferences` (clé AES256-GCM
du Keystore Android), jamais en clair, et les réglages prennent effet **à la
prochaine séance** : la séance en cours garde la configuration avec laquelle elle
a démarré.

Le réglage par `adb` reste disponible pour les tests automatisés ; l'adresse
transmise écrase celle enregistrée :

```bash
adb shell am start -n com.mpacer.watch/.MainActivity --es mqtt_url "mqtt://joseph:motdepasse@192.168.0.115:1883"
```

Les réglages explicites de l'écran MQTT restent prioritaires sur les identifiants
de l'URL, ce qui permet de corriger un seul mot de passe sans retaper l'adresse.
Les valeurs locales vivent dans un fichier `.env` **non versionné** (gabarit :
[`.env.example`](../.env.example)).

---

## 4. Côté service

| Fichier | Rôle |
|---|---|
| `crates/mpacer-api/src/mqtt.rs` | client MQTT minimal (TCP ou TLS), reconnexion 2 s → 60 s, `PINGREQ` toutes les 30 s |
| `crates/mpacer-api/src/live.rs` | magasin en mémoire (`LiveStore`), purge, insertion triée par `t_ms`, résumé de trace |
| `crates/mpacer-api/src/routes/web.rs` | page `/live` (SVG, sans JavaScript) et `/live.json` |
| `crates/mpacer-api/src/state.rs` | `AppState.live` partagé par les gestionnaires |
| `crates/mpacer-api/src/main.rs` | démarrage de l'abonnement si `MPACER_MQTT_URL` est défini |

### Variables d'environnement

| Variable | Défaut | Rôle |
|---|---|---|
| `MPACER_MQTT_URL` | *(vide)* | `mqtt://hote:1883` ou `mqtts://hote:8883`. Vide = suivi désactivé, aucune connexion ouverte |
| `MPACER_MQTT_TOPIC` | `mpacer/live/+` | filtre souscrit |
| `MPACER_MQTT_USERNAME` | *(vide)* | identifiant du broker |
| `MPACER_MQTT_PASSWORD` | *(vide)* | mot de passe (dans le Secret Helm, jamais dans le ConfigMap) |

Le chart Helm expose `config.mqttUrl`, `config.mqttTopic`,
`config.mqttUsername` et `auth.mqttPassword`.

### Routes

| Route | Accès | Contenu |
|---|---|---|
| `GET /live` | session (cookie) | carte OpenStreetMap, profil de dénivelé (D+ / D-), trace SVG de repli, chiffres clés, rafraîchissement automatique de 10 s |
| `GET /live.json` | session (cookie) | même contenu en JSON, pour un tableau de bord ou un script (la carte du site le relit toutes les 10 s) |

Chaque session porte aussi le **parcours planifié** publié par le coureur
(`PUT /api/v1/live/route`, voir [13](13-amis-partage-position.md)) et
l'avancement calculé par le service : la carte dessine le parcours en bleu sous
la trace, et le marqueur affiche « 42 % ».

La page dessine **deux lectures complémentaires** de la trace en cours :

1. une **carte** : le fond de tuiles OpenStreetMap est affiché par le script
   maison `/static/map.js` (le même que les pages Amis et la fiche de séance,
   sans Leaflet ni CDN), la trace et la position courante viennent de
   `/live.json` ; c'est la réponse à « où en est-il ? » ;
2. un **profil de dénivelé** : l'altitude publiée (`alt`) est tracée en fonction
   de la distance, avec le cumul D+ / D- (variations sous 1 m ignorées, c'est du
   bruit GPS) ; c'est la réponse à « qu'a-t-il grimpé ? ».

Sans altitude dans la charge utile (ancienne montre, ou GPS muet), la carte
reste seule : rien n'est inventé. La trace SVG est conservée en repli, pour
lire la forme du parcours même sans JavaScript. Un lien ouvre OpenStreetMap sur
la position exacte.

---

## 5. Budget de ressources

### 5.1 Montre

| Poste | Valeur | Détail |
|---|---|---|
| Publications | 360 / h | une toutes les 10 s en course, une toutes les 60 s en pause |
| Trafic montant | ~65 ko / h | 360 × (116 o de charge + ~24 o d'en-têtes MQTT + ~40 o d'IP/TCP) ≈ 18 o/s |
| `PINGREQ` | 2 o / 30 s | garde la liaison ouverte, évite une reconnexion par point |
| Fils | 1 | priorité `THREAD_PRIORITY_BACKGROUND`, endormi sur condition |
| Wake locks | 0 | aucun |
| Demandes GPS | 0 en plus | la boucle 1 Hz du service existe déjà, elle est seulement observée |
| Mémoire | ~20 ko | file de 120 points au maximum (20 minutes), compressée si la coupure dure |
| Batterie (estimation) | < 1 %/h | le GPS 1 Hz reste le poste dominant ; la radio est réveillée 6 fois par minute pendant ~50 ms |
| Suivi désactivé | 0 | aucun fil, aucune connexion, aucun objet créé |

Si le broker tombe : la file **garde la trace** — bornée à 120 points et à
20 minutes après le dernier point — et la reconnexion retente 2 s, 4 s, 8 s…
jusqu'à 60 s. À la reconnexion, les positions accumulées **repartent en rafale**
(espacées de 50 ms) : les proches ne voient pas de trou de plusieurs minutes.
Au-delà de vingt minutes de coupure, l'historique est **compressé de moitié**
(un point sur deux) plutôt que tronqué : le tracé garde sa forme et la position
courante survit toujours. Une écriture interrompue par la perte du lien remet le
point en tête de file : rien n'est perdu au moment précis de la coupure. À l'arrêt
de la séance, seule la position courante est gardée, pour que le message `stop`
parte sans attendre.

### 5.2 Service

| Poste | Valeur | Détail |
|---|---|---|
| Connexions | 1 | une socket vers le broker, un task Tokio |
| Mémoire par montre | ~600 ko | anneau de 4096 points de ~144 o (11 h à 10 s), purgé après 24 h de silence |
| Rendu d'une page | ~10 ko de HTML | trace sous-échantillonnée à 700 points, 4 montres au maximum |
| Écritures en base | 0 | le suivi en direct ne touche pas PostgreSQL |

### 5.3 Comment le vérifier

- **Tests unitaires de la politique** (`LivePolicyTest`) : la cadence est
  vérifiée à la seconde près, y compris le passage en pause.
- **Tests du magasin** (`live.rs`) : l'anneau reste borné à 4096 points, la
  trace rendue à 700, les montres silencieuses sont oubliées.
- **Compteurs sur la montre** : l'écran Réglages affiche l'état de la liaison,
  le nombre de points publiés et le nombre de points jetés.
- **Mesure sur le terrain** : comparer une séance d'une heure avec et sans le
  suivi en direct :

  ```bash
  adb shell dumpsys batterystats --reset          # avant la séance
  # … séance d'une heure …
  adb shell dumpsys batterystats | Select-String "com.mpacer.watch|Mobile radio|Wifi"
  ```

  Le critère du projet reste « batterie < 25 %/h » : le suivi en direct doit
  rester dans le bruit de mesure de cette comparaison.

---

## 6. Sécurité et vie privée

- **Aucun secret dans le message** : la charge utile ne contient que des
  mesures ; l'identifiant de montre n'est pas un secret.
- **Le mot de passe du broker** est rangé dans `EncryptedSharedPreferences`
  (clé AES256-GCM du Keystore Android), comme le jeton d'appairage.
- **TLS** : `mqtts://` utilise une socket TLS avec vérification du certificat
  et du nom d'hôte (autorités racines publiques). Un broker à certificat
  auto-signé n'est pas accepté — dans ce cas, gardez le broker sur le réseau
  interne du cluster, ce qui est la configuration recommandée.
- **La page `/live` demande une session** (connexion Google) : elle n'est pas
  publique.
- **Partage entre amis** : le suivi peut être montré à un cercle fermé, et
  seulement pendant la séance, via la page `/amis` (voir
  [13 - Amis et partage de la position en direct](13-amis-partage-position.md)) :
  l'appareil revendique alors son nom MQTT auprès du backend
  (`POST /api/v1/live/register`), qui refuse deux comptes sous le même nom.
- **Le broker ne doit pas être exposé sur Internet** sans mot de passe ni TLS :
  un sujet MQTT est lisible par quiconque peut s'abonner.

---

## 7. Ce qui n'est pas fait

| Limite | Raison |
|---|---|
| **Garmin (Connect IQ)** ne publie pas en MQTT | Connect IQ n'expose aucune socket TCP, seulement `Communications.makeWebRequest` (HTTP). Un relais par le backend est possible plus tard, mais la trace complète vit déjà dans le FIT Garmin |
| Aucun historique du suivi | choix assumé : la base reçoit la séance à la fin ; le direct est volatil |
| Aucun lien de partage public | la page demande une session ; le cercle d'amis ([13](13-amis-partage-position.md)) répond au besoin sans lien ouvert |
| Pas de « dernier point reçu » dans l'en-tête de la page de séance | la page `/live` suffit pour l'instant |
| Le fond de carte `/live` charge des tuiles publiques `tile.openstreetmap.org` | c'est le seul appel sortant de la page ; aucune position n'est envoyée au serveur de tuiles, seules les coordonnées des tuiles visibles le sont. Hors ligne, la trace SVG reste dessinée |
| Pas de profil de dénivelé pour une montre qui ne publie pas `alt` | le champ est facultatif : sans lui, rien n'est inventé, la carte reste seule |

---

## 8. Vérification

```bash
# Service : sérialisation MQTT, magasin en direct, page /live (62 tests unitaires)
cargo test -p mpacer-api

# Un faux broker dans les tests : connexion, PUBLISH QoS 1, PUBACK
cargo test -p mpacer-api mqtt::

# Socle partage : politique de cadence, charge utile, paquets MQTT
cd android && ./gradlew :core:testDebugUnitTest
```

Essai de bout en bout, sans montre :

```bash
# 1. Un broker local
docker run --rm -p 1883:1883 eclipse-mosquitto:2 \
  mosquitto -c /mosquitto-no-auth.conf

# 2. Le service abonné
MPACER_MQTT_URL=mqtt://127.0.0.1:1883 cargo run -p mpacer-api

# 3. Un point publié à la main, tel que la montre l'envoie
mosquitto_pub -h 127.0.0.1 -t mpacer/live/montre-test -r \
  -m '{"t":1728222000000,"lat":48.8566,"lon":2.3522,"acc":4.0,"st":"run"}'

# 4. La page
#    http://localhost:8080/live  (session ouverte) doit afficher la trace
```

Avec un broker deja en place, le meme controle se rejoue depuis le depot, sans
qu'aucun identifiant n'entre dans le code (les valeurs viennent de
l'environnement, par exemple du fichier `.env` non versionne) :

```bash
MPACER_MQTT_TEST_URL="mqtt://utilisateur:motdepasse@192.168.0.115:1883" \\
  cargo test -p mpacer-api --test mqtt_live -- --ignored --nocapture
```

Le test se connecte avec le client de production, s'abonne a un sujet de test,
y publie une position au format de la montre, verifie qu'elle revient intacte,
puis efface le message retenu.
