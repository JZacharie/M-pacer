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

| Secret Kubernetes | Chemin Vault | ClusterSecretStore | Contenu |
|---|---|---|---|
| `mpacer-secrets` | `apps/mpacer` | `vault-apps` | `MPACER_SESSION_SECRET`, `MPACER_GOOGLE_CLIENT_ID`, `MPACER_GOOGLE_CLIENT_SECRET` |
| `regcred` | `global/regcred` | `vault-backend` | `.dockerconfigjson` (tirage de l'image ghcr.io) |
| `mpacer-pg-app` | — | — | généré par l'opérateur CloudNativePG |

### 2.1 Scellement (une seule fois)

Le montage KV v2 étant `secret/`, le chemin réel est `secret/data/apps/mpacer` :

```bash
export VAULT_ADDR=https://vault.zacharie.org     # ou http://vault-active.vault.svc:8200

vault kv put secret/apps/mpacer \
  MPACER_SESSION_SECRET="$(openssl rand -base64 48)" \
  MPACER_GOOGLE_CLIENT_ID="<ID>.apps.googleusercontent.com" \
  MPACER_GOOGLE_CLIENT_SECRET="GOCSPX-<secret>"
```

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
| Secrets scellés dans Vault | ❌ **à faire** (aucun jeton Vault dans l'environnement de développement) |
| Image publiée sur ghcr.io | ❌ **à faire** (le jeton disponible n'a pas le scope `write:packages`) |
