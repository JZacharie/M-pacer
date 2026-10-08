# GitOps (ArgoCD) et secrets (Vault) sur jo3

Ce document décrit le câblage retenu sur le cluster **jo3** : ArgoCD synchronise le
chart, les secrets viennent de **HashiCorp Vault** via l'**External Secrets Operator**
(ESO). Aucun identifiant n'est présent dans le dépôt Git.

---

## 1. Chaîne GitOps

```text
JZacharie/jo3                          JZacharie/M-pacer
  Applications/home/mpacer.yaml  ──►   charts/mpacer  +  values-jo3.yaml
        (Application ArgoCD)                    (chart Helm et valeurs)
                 │
                 ▼
   ArgoCD (app-of-apps 00-project-home)
                 │  sync automatisée : prune, selfHeal, CreateNamespace, ServerSideApply
                 ▼
   namespace mpacer : Cluster CNPG, Deployment, Service, Ingress, Certificate,
                      ExternalSecrets
```

Le manifeste complet est dans [argocd/mpacer.yaml](argocd/mpacer.yaml).

```bash
kubectl -n argocd get application mpacer
kubectl -n argocd get application mpacer -o jsonpath='{.status.sync.status} {.status.health.status}'
argocd app sync mpacer       # si la synchronisation automatique est suspendue
argocd app history mpacer
```

### Pourquoi des ignoreDifferences ?

Deux opérateurs modifient les objets après création :

| Opérateur | Objet | Champs ajoutés |
|---|---|---|
| External Secrets | `ExternalSecret` | annotations, finalizers, valeurs par défaut des `remoteRef` |
| CloudNativePG | `Cluster` | annotations, section `monitoring` |

Sans `ignoreDifferences`, l'application resterait éternellement **OutOfSync**. Les
expressions sont alignées sur celles des autres applications du cluster.

## 2. Secrets dans Vault

| Secret Kubernetes | ClusterSecretStore | Clé `remoteRef` | Chemin Vault réel | Contenu |
|---|---|---|---|---|
| `mpacer-secrets` | `vault-apps` | `mpacer` | `apps/data/mpacer` | `MPACER_SESSION_SECRET`, `MPACER_GOOGLE_CLIENT_ID`, `MPACER_GOOGLE_CLIENT_SECRET` — et, si Deezer est active, `MPACER_DEEZER_ARL` (cookie arl, voie recommandee) et/ou `MPACER_DEEZER_APP_ID`, `MPACER_DEEZER_APP_SECRET` ; si l'envoi dans la file de Deemix est active, `MPACER_DEEMIX_USER`, `MPACER_DEEMIX_PASSWORD` ; si le broker MQTT du suivi en direct demande un mot de passe, `MPACER_MQTT_PASSWORD` |

> **Attention au piège** : dans un `ClusterSecretStore`, `provider.vault.path` désigne le
> **montage** KV, pas un préfixe de chemin. Sur jo3, `vault-apps` est monté sur `apps/`
> et `vault-backend` sur `secret/`. La clé `remoteRef` est donc le chemin *dans* ce
> montage (`mpacer`), et non `apps/mpacer` — cette erreur produit un
> `SecretSyncedError: could not get secret data from provider`.
| `regcred` | `global/regcred` | `vault-backend` | `.dockerconfigjson` (tirage de l'image ghcr.io) |
| `mpacer-pg-app` | — | — | généré par l'opérateur CloudNativePG |

### 2.1 Scellement (une seule fois)

```bash
export VAULT_ADDR=https://vault.zacharie.org     # ou http://vault-active.vault.svc:8200

# 'apps/' est le montage KV du store vault-apps : la cle est 'mpacer'
vault kv put apps/mpacer \
  MPACER_SESSION_SECRET="$(openssl rand -base64 48)" \
  MPACER_GOOGLE_CLIENT_ID="<ID>.apps.googleusercontent.com" \
  MPACER_GOOGLE_CLIENT_SECRET="GOCSPX-<secret>" \
  MPACER_DEEZER_APP_ID="<Application ID Deezer>" \
  MPACER_DEEZER_APP_SECRET="<Secret Key Deezer>" \
  MPACER_DEEZER_ARL="<cookie arl Deezer>" \
  MPACER_DEEMIX_USER="<utilisateur Deemix>" \
  MPACER_DEEMIX_PASSWORD="<mot de passe Deemix>" \
  MPACER_MQTT_PASSWORD="<mot de passe du broker MQTT>"
```

> **Deezer par cookie `arl` (voie recommandee).** Elle ne demande **aucune**
> application developpeur et rend lisibles les playlists du compte, y compris
> privees. Le cookie se recupere dans le navigateur connecte a deezer.com
> (DevTools > Application > Cookies > `arl`) et change rarement ; quand Deezer le
> renouvelle, `/music` affiche « Le cookie Deezer (ARL) a ete refuse » et il
> suffit de resceller la nouvelle valeur :
>
> ```bash
> vault kv patch apps/mpacer MPACER_DEEZER_ARL="<cookie arl>"
> kubectl -n mpacer annotate externalsecret mpacer-secrets force-sync="$(date +%s)" --overwrite
> ```
>
> Reportez la meme cle dans `externalSecrets.keys.deezerArl: MPACER_DEEZER_ARL`
> (voir [values-jo3.yaml](../charts/mpacer/values-jo3.yaml)) : un `remoteRef` vers
> une propriete absente fait echouer l'ExternalSecret. Quand le cookie `arl` est
> present, le mode OAuth Deezer n'est plus utilise : la page `/music` accede
> directement aux playlists du compte, sans bouton « Connecter ».
>
> Les MP3 listes par `/music` se telechargent depuis l'instance Deemix
> (`MPACER_DEEMIX_URL`, par defaut `https://deemix.p.zacharie.org`) : la page
> produit un lien par piste (`#/track/<id>`) et un lien pour la playlist entiere
> (`#/playlist/<id>`). Voir
> [docs/11](../docs/11-playlists-multi-sources-et-synchro-mp3.md).
>
> **Envoi dans la file de Deemix.** Avec `MPACER_DEEMIX_USER` et
> `MPACER_DEEMIX_PASSWORD` (l'authentification HTTP basique de l'instance,
> Traefik), `/music` ne se contente plus des liens : le bouton « Telecharger dans
> Deemix » remet la playlist dans la file de l'instance et `File Deemix` affiche
> la progression. Le service ouvre la session Deezer de Deemix avec le cookie
> `arl` deja scelle, a chaque envoi. Scellez les deux valeurs puis reportez-les
> dans `externalSecrets.keys.deemixUser` / `deemixPassword` :
>
> ```bash
> vault kv patch apps/mpacer \
>   MPACER_DEEMIX_USER="joseph" \
>   MPACER_DEEMIX_PASSWORD="<mot de passe>"
> kubectl -n mpacer annotate externalsecret mpacer-secrets force-sync="$(date +%s)" --overwrite
> ```

> **Suivi en direct (MQTT).** `MPACER_MQTT_PASSWORD` n'est utile que si le broker
> demande une authentification ; l'adresse et l'identifiant vivent dans les
> valeurs non sensibles (`config.mqttUrl`, `config.mqttUsername`). Reportez la
> meme cle dans `externalSecrets.keys.mqttPassword: MPACER_MQTT_PASSWORD` : un
> `remoteRef` qui pointe vers une propriete absente fait echouer la
> synchronisation de l'ExternalSecret. Sur un broker interne sans mot de passe,
> laissez la cle vide des deux cotes. Detail : [docs/10](../docs/10-suivi-temps-reel.md).

> **Deezer est optionnel.** Si les cles d'un service ne sont pas
> scellees dans Vault, laisser les cles correspondantes **vides** dans les valeurs
> (`externalSecrets.keys.deezerAppId`/`...AppSecret`) : un `remoteRef` qui pointe
> vers une propriete absente fait echouer la synchronisation de l'ExternalSecret
> (`SecretSyncedError`). La page `/music` reste alors utilisable pour les fichiers
> personnels et affiche que la source concernee n'est pas configuree.

### 2.1.1 Musique : aucun stockage cote serveur

La musique n'est **pas** stockee par le service : les fichiers audio restent sur
le disque de l'utilisateur et sont copies sur la montre par USB avec l'outil local
`mpacer-music` (voir [docs/07](../docs/07-musique-bpm-et-playlists.md)). Le
backend ne conserve que des **metadonnees** (playlists, titres, BPM, manifeste de
transfert) : le volume applicatif du chart reste donc desactive sur jo3, et
aucune variable `MPACER_MEDIA_DIR` n'est necessaire.

Puis forcer la synchronisation et vérifier :

```bash
kubectl -n mpacer annotate externalsecret mpacer-secrets force-sync="$(date +%s)" --overwrite
kubectl -n mpacer get externalsecret
# NAME              STORE           STATUS        READY
# mpacer-secrets    vault-apps      SecretSynced  True
# regcred           vault-backend   SecretSynced  True
```

### 2.2 Sans jeton Vault (démarrage rapide)

Pour démarrer sans attendre le scellement, désactiver ESO et créer les secrets à la
main (le chart ne les écrase pas) :

```bash
helm upgrade mpacer charts/mpacer -n mpacer -f charts/mpacer/values-jo3.yaml \
  --set externalSecrets.enabled=false \
  --set auth.existingSecret=""

kubectl -n mpacer create secret generic mpacer-credentials \
  --from-literal=MPACER_SESSION_SECRET="$(openssl rand -base64 48)" \
  --from-literal=MPACER_GOOGLE_CLIENT_ID="..." \
  --from-literal=MPACER_GOOGLE_CLIENT_SECRET="..."
```

Le motif `regcred` reste recommandé (il est déjà présent dans la plupart des
namespaces du cluster) :

```bash
kubectl -n mpacer create secret docker-registry regcred \
  --docker-server=ghcr.io --docker-username=<compte> --docker-password=<PAT write:packages>
```

## 3. Ordre de mise en service

1. **Publier l'image** : `pwsh deploy/build-image.ps1 -Push` (PAT avec `write:packages`).
2. **Sceller les secrets** dans Vault (section 2.1) — ou la variante 2.2.
3. **Synchroniser** : ArgoCD le fait seul ; sinon `argocd app sync mpacer`.
4. **Vérifier** : `kubectl -n mpacer rollout status deploy/mpacer` puis
   `curl -s https://mpacer.p.zacharie.org/readyz`.

## 4. Ce qui est vérifié et ce qui ne l'est pas

| Élément | État |
|---|---|
| Application ArgoCD créée dans jo3 et reprise par ArgoCD | ✅ vérifié (`mpacer` Synced, ressources créées) |
| Cluster PostgreSQL CNPG | ✅ déployé et sain (`mpacer-pg`, PostgreSQL 18.6) |
| ExternalSecrets générés par le chart | ✅ rendus conformes aux stores du cluster |
| Secrets scellés dans Vault | ✅ scellés dans `apps/mpacer` (jeton root lu dans le secret `vault-unseal-keys`), ESO en `SecretSynced` |
| Image publiée sur ghcr.io | ✅ publiée par GitHub Actions (`.github/workflows/publish.yml`) avec le `GITHUB_TOKEN` — aucun PAT requis |
| Identifiants Google réels | ❌ **à faire** : la valeur scellée est un marqueur `REMPLACER-...` |
