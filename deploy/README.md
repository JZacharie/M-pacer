# Deploiement sur le cluster k3s « jo3 »

Guide complet : construction de l'image, configuration Google, deploiement Helm,
verification, sauvegarde et mise a jour.

---

## 1. Ce qui est deploye

| Ressource | Role |
|---|---|
| `Cluster` CloudNativePG `mpacer-pg` | **PostgreSQL 18** dedie (1 instance, 2 Gi), avec ses services `-rw`/`-ro`/`-r` et son secret applicatif |
| `Deployment` (1 replica, `RollingUpdate`) | binaire Rust `mpacer-api` : API + interface web |
| `Service` ClusterIP (8080) | acces interne |
| `Ingress` traefik (entrypoint `websecure`) | `https://mpacer.p.zacharie.org` |
| `Ingress` cloudflare-tunnel (optionnel) | `https://mpacer.zacharie.org` pour la montre hors du domicile |
| `Certificate` cert-manager (DNS-01 Cloudflare) | TLS automatique dans le secret `mpacer-tls` |
| `Secret` `mpacer-credentials` | secret de session (genere) + identifiants Google |
| `ConfigMap` `mpacer` | configuration non sensible |

L'application est **sans etat** : toutes les donnees vivent dans PostgreSQL, gere par
l'operateur CloudNativePG deja present sur jo3. Consequence : mise a jour sans coupure
(`RollingUpdate`), montee en replicas possible, et sauvegardes geree par l'operateur.
Le detail de la configuration est en section 4 bis.

## 1 bis. PostgreSQL (CloudNativePG)

Le chart cree lui-meme le cluster de base de donnees :

```yaml
postgresql:
  enabled: true
  mode: cnpg            # cnpg | external
  instances: 1          # 3 pour du HA (basculement automatique)
  database: mpacer
  username: mpacer
  storage:
    size: 2Gi
    storageClass: local-path-retain
  backup:
    enabled: false      # sauvegardes continues vers un stockage S3
```

Ce qui est cree : un `Cluster` `mpacer-pg`, un PVC par instance, les services
`mpacer-pg-rw` (ecriture), `mpacer-pg-ro` (lecture), `mpacer-pg-r` (lecture seule) et
le secret applicatif `mpacer-pg-app` (cles `host`, `port`, `dbname`, `username`,
`password`, `uri`).

L'application lit **directement** ces cles via `secretKeyRef` : aucun mot de passe
n'apparait dans les valeurs Helm ni dans le ConfigMap.

```bash
# Etat du cluster
kubectl -n mpacer get clusters.postgresql.cnpg.io mpacer-pg
kubectl -n mpacer get pods,pvc -l cnpg.io/cluster=mpacer-pg

# Mot de passe applicatif (si besoin de se connecter depuis l'exterieur)
kubectl -n mpacer get secret mpacer-pg-app -o jsonpath='{.data.password}' | base64 -d

# Session psql dans le pod
kubectl -n mpacer exec -it mpacer-pg-1 -- psql -U postgres -d mpacer
```

> Attention : sur jo3, KubeBlocks installe aussi un type `Cluster`. Si
> `kubectl get cluster mpacer-pg` repond « not found », utilisez la forme complete
> `clusters.postgresql.cnpg.io`.

**Utiliser un PostgreSQL existant** (par exemple `cluster-pg-rw` du namespace
`pg-prd`) au lieu d'en creer un :

```bash
helm upgrade mpacer charts/mpacer -n mpacer -f charts/mpacer/values-jo3.yaml \
  --set postgresql.mode=external \
  --set postgresql.external.host=cluster-pg-rw.pg-prd.svc.cluster.local \
  --set postgresql.external.database=mpacer \
  --set postgresql.external.username=mpacer \
  --set postgresql.external.existingSecret=mpacer-db-password \
  --set postgresql.external.sslmode=require
```

(Le secret `existingSecret` doit contenir la cle `password`.)

## 1 ter. Migration depuis SQLite

La version SQLite stockait la base dans un fichier `/data/mpacer.db`. Pour reprendre
un historique existant : demarrer un PostgreSQL, puis rejouer les exports depuis les
montres (ou importer les fichiers `.pac`) — l'API est idempotente, un renvoi remplace
la seance existante sans creer de doublon.

## 2. Prerequis

- Acces au cluster : `kubectl config use-context <contexte-jo3>` (les nœuds portent le label `cluster=jo3`).
- `helm` 3.x, `podman` (ou Docker), un jeton GitHub avec `write:packages` pour pousser sur ghcr.io.
- Le secret `regcred` (dockerconfigjson ghcr.io) est deja present dans la plupart des
  namespaces du cluster ; sinon, le creer :

```bash
kubectl -n mpacer create secret docker-registry regcred \
  --docker-server=ghcr.io \
  --docker-username=<utilisateur-github> \
  --docker-password=<PAT-avec-write:packages>
```

## 3. Construire et publier l'image

```powershell
# Image pour l'architecture courante
pwsh deploy/build-image.ps1

# Pousser sur ghcr.io (demande le PAT)
pwsh deploy/build-image.ps1 -Push

# Manifeste multi-arch (le cluster melange amd64 vm15x/vm16x et arm64 pi20x)
pwsh deploy/build-image.ps1 -MultiArch -Push
```

Le `Dockerfile` (multi-etapes) produit une image `debian:bookworm-slim` avec le seul
binaire, un utilisateur non privilegie (uid 10001) et un `HEALTHCHECK` sur `/healthz`.

> Verifie le 5 octobre 2026 : image construite avec podman (succes), conteneur demarre
> en uid 10001, base SQLite creee sur `/data`, et reponses correctes de `/healthz`,
> `/readyz`, `/` et `/login` depuis l'interieur du conteneur. Le `HEALTHCHECK` exige
> `--format docker` (podman l'ignore en format OCI), d'ou l'option dans le script.

> Le chart epingle par defaut `nodeSelector: kubernetes.io/arch=amd64`. Pour utiliser
> les nœuds Raspberry Pi, publier une image multi-arch et vider ce selector dans
> `values-jo3.yaml`.

## 4. Creer le client OAuth Google

1. Console Google Cloud > **APIs et services** > **Ecran de consentement OAuth** :
   type *Externe*, nom « M-pacer », portee `email`, `profile`, `openid`.
2. **Identifiants** > **Creer des identifiants** > **ID client OAuth** > type
   **Application Web**.
3. **URI de redirection autorisee** :
   ```text
   https://mpacer.p.zacharie.org/auth/google/callback
   https://mpacer.zacharie.org/auth/google/callback     (si l'ingress public est active)
   ```
4. Recopier l'ID client et le secret : ils seront passes a Helm (section suivante).

Le service verifie l'`id_token` contre les cles publiques de Google (JWKS), controle
l'audience, l'emetteur, la signature et `email_verified`, puis ouvre une session
signee (JWT HS256) dans un cookie `HttpOnly` + `SameSite=Lax` + `Secure`.

## 5. Deployer

```bash
helm upgrade --install mpacer charts/mpacer \
  --namespace mpacer --create-namespace \
  -f charts/mpacer/values-jo3.yaml \
  --set auth.googleClientId="<ID>.apps.googleusercontent.com" \
  --set auth.googleClientSecret="<SECRET>"
```

Pour exposer aussi la montre hors du domicile :

```bash
helm upgrade mpacer charts/mpacer -n mpacer -f charts/mpacer/values-jo3.yaml \
  --set cloudflareIngress.enabled=true \
  --set config.publicUrl=https://mpacer.zacharie.org
```

> `config.publicUrl` doit correspondre exactement a l'URL de redirection declaree chez
> Google : c'est elle qui construit `/auth/google/callback`.

### Verifier sans deployer

```bash
helm template mpacer charts/mpacer -n mpacer -f charts/mpacer/values-jo3.yaml | less
helm install mpacer charts/mpacer -n mpacer -f charts/mpacer/values-jo3.yaml --dry-run=server
```

## 6. Verifications apres deploiement

```bash
kubectl -n mpacer rollout status deploy/mpacer
kubectl -n mpacer get ingress,certificate,pvc
kubectl -n mpacer logs deploy/mpacer --tail=30
kubectl -n mpacer get certificate mpacer -o jsonpath='{.status.conditions[*].message}'
```

Puis, depuis un poste du reseau :

```bash
curl -s https://mpacer.p.zacharie.org/healthz     # {"status":"ok",...}
curl -s https://mpacer.p.zacharie.org/readyz      # {"status":"ready","database":true}
```

Enfin, dans le navigateur : se connecter avec Google, puis **Appairer** la montre avec
le code affiche par l'application.

## 7. Appairer la montre et synchroniser

1. Sur la montre (ou avec le simulateur), demander un code d'appairage :
   ```bash
   cargo run -p mpacer-sim -- --mode plan --distance 5000 --time 1500 \
     --api-url https://mpacer.p.zacharie.org
   ```
   Le simulateur affiche un code du type `BCDF-GHJK`.
2. Se connecter sur `https://mpacer.p.zacharie.org/link` et saisir ce code.
3. La montre recoit un jeton d'appareil et le conserve : les seances suivantes
   partent toutes seules (le simulateur le fait a la fin de chaque seance).

## 8. Sauvegarde et restauration

### 8.1 Sauvegarde logique a la demande

```bash
kubectl -n mpacer exec mpacer-pg-1 -- pg_dump -U postgres -Fc mpacer > mpacer-$(date +%F).dump
```

Restauration (dans une base vide ou existante) :

```bash
kubectl -n mpacer exec -i mpacer-pg-1 -- pg_restore -U postgres -d mpacer --clean --if-exists \
  < mpacer-2026-10-05.dump
```

### 8.2 Sauvegardes continues (recommande)

Avec un stockage compatible S3 :

```bash
helm upgrade mpacer charts/mpacer -n mpacer -f charts/mpacer/values-jo3.yaml \
  --set postgresql.backup.enabled=true \
  --set postgresql.backup.destinationPath=s3://mon-bucket/mpacer \
  --set postgresql.backup.credentialsSecret=mpacer-backup-s3
```

Cela active WAL archiving + une `ScheduledBackup` quotidienne (02:30 par defaut).
Restauration à un instant précis : voir la documentation CloudNativePG
(`kubectl -n mpacer get backups.postgresql.cnpg.io`).

En dernier recours, l'API etant idempotente, une montre conserve toujours ses seances
et peut les renvoyer.

## 9. Mise a jour et retour arriere

```bash
# Nouvelle image
pwsh deploy/build-image.ps1 -Push -Tag 0.2.0
helm upgrade mpacer charts/mpacer -n mpacer -f charts/mpacer/values-jo3.yaml --set image.tag=0.2.0

# Retour arriere
helm rollback mpacer -n mpacer
```

Le secret de session est conserve entre les upgrades (`lookup` + `resource-policy: keep`) :
les utilisateurs deja connectes ne sont pas deconnectes.

## 10. Depannage

| Symptome | Cause probable | Action |
|---|---|---|
| `ImagePullBackOff` | image absente du registre ou `regcred` manquant/invalide | verifier `kubectl -n mpacer describe pod`, recreer `regcred` |
| `CrashLoopBackOff` au demarrage | `MPACER_SESSION_SECRET` manquant ou mot de passe de base refuse | verifier le secret `mpacer-credentials` (>= 32 caracteres) et les logs |
| `readyz` en echec | base PostgreSQL injoignable | `kubectl -n mpacer get clusters.postgresql.cnpg.io mpacer-pg` et `kubectl -n mpacer logs mpacer-pg-1` |
| Pod applicatif en `CreateContainerConfigError` | secret `mpacer-pg-app` pas encore cree (bootstrap en cours) | attendre la fin du bootstrap CNPG, le pod demarre tout seul |
| « relation … does not exist » | schema non applique | verifier que l'application a bien demarre une fois sur la base cible |
| « Connexion impossible » apres Google | redirect URI differente de `publicUrl` | aligner `config.publicUrl` et la console Google |
| Certificat jamais emis | ClusterIssuer DNS non resolu | `kubectl -n mpacer describe certificate mpacer` |
| La montre ne synchronise pas hors du domicile | ingress public desactive | activer `cloudflareIngress.enabled` et pointer le tunnel sur le service |

## 11. Desinstallation

```bash
helm uninstall mpacer -n mpacer
kubectl -n mpacer delete pvc mpacer     # supprime definitivement les donnees
kubectl delete namespace mpacer
```
