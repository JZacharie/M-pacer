# 05 - Plan d'action : développement, GitOps et déploiement sur jo3

Suite du [plan général](../README.md#4-plan-daction-complet). Ce document découpe le
travail restant en **lots parallélisables**, avec périmètres disjoints, dépendances et
critères de terminaison. Les conventions sont celles relevées sur le cluster jo3.

---

## 1. Contexte technique relevé sur jo3 (5 octobre 2026)

| Élément | Constat | Conséquence pour M-pacer |
|---|---|---|
| Cluster | k3s, nœuds vm15x/vm16x (amd64) + pi20x (arm64), label cluster=jo3 | nodeSelector kubernetes.io/arch=amd64 par défaut |
| Ingress | traefik (entrypoint websecure), hosts *.p.zacharie.org | Ingress mpacer.p.zacharie.org déjà dans le chart |
| TLS | cert-manager + ClusterIssuer letsencrypt-production-dns (DNS-01 Cloudflare) | Certificate dans le chart |
| Secrets | **HashiCorp Vault** (namespace vault, 3 répliques) + **External Secrets Operator** (kube-system) + Sealed Secrets | Les secrets passent par ExternalSecret → Vault, pas par des valeurs Helm |
| Stores ESO | vault-apps (path apps), vault-backend (path backend), vault-infra, vault-internal, vault-shared | apps/mpacer pour les identifiants Google, global/regcred pour l'image |
| Registre | ghcr.io/jzacharie/* ; regcred synchronisé depuis Vault dans chaque namespace | Image ghcr.io/jzacharie/mpacer |
| GitOps | ArgoCD, **app-of-apps** depuis git@github.com:JZacharie/jo3.git, dossier Applications/&lt;catégorie&gt;, sync automatisée (prune, selfHeal, CreateNamespace, ServerSideApply) | Ajouter Applications/&lt;catégorie&gt;/mpacer.yaml pointant sur le chart M-pacer |
| Base de données | CloudNativePG 1.30 (déjà utilisé : pg-prd, pg-sbx) | Cluster mpacer-pg **déjà déployé et sain** sur jo3 |

## 2. Division du travail en lots

Chaque lot a un **périmètre de fichiers disjoint** et peut avancer en parallèle.

| Lot | Objet | Périmètre | Dépend de | Statut |
|---|---|---|---|---|
| **L1** | Backend : exploitation et durcissement | crates/mpacer-api/src/**, migrations/** | — | ✅ filtres, export .pac, agrégats (14 tests verts) |
| **L2** | Frontend web : tableaux de bord et navigation | routes/web.rs, static/** | — | ✅ page statistiques, navigation, pagination |
| **L3** | Images conteneur et publication | deploy/**, .github/workflows/** | — | 🔶 image construite et testée, **publication bloquée** (jeton sans write:packages) |
| **L4** | Secrets Vault + ExternalSecrets | templates/externalsecret*.yaml, values.yaml | jeton Vault | 🔶 gabarits livrés et conformes, **scellement à faire** |
| **L5** | ArgoCD / GitOps sur jo3 | dépôt JZacharie/jo3, Applications/** | L3 (image publiée) | ✅ application créée, ingress + certificat TLS émis, PostgreSQL adopté |
| **L6** | Application montre (Wear OS) | android/app/** | — | ✅ code livré (23 fichiers Kotlin), **non compilé** |
| **L7** | Application Android compagnon | android/companion/**, android/*.gradle.kts | L1 (API) | ✅ module livré, **non compilé** |
| **L8** | Vérification terrain | — | L1→L7 | 🔲 à venir (nécessite une machine avec le SDK Android) |

**Vérifications automatiques du lot Android** (à défaut de compilation) : catalogue de
versions cohérent (43 alias déclarés, 27 utilisés, aucun introuvable), 23 fichiers
Kotlin dont le package correspond à l'arborescence, deux modules déclarés et présents
(`:app`, `:companion`), points d'entrée appelés conformes à l'API
(`/api/v1/device/code`, `/api/v1/device/token`, `/api/v1/me`, `/api/v1/workouts`),
aucun secret en dur, aucun fichier modifié hors de `android/`.

> L6 et L7 partagent la configuration Gradle : **un seul lot les pilote**, afin d'éviter
> deux écritures concurrentes sur settings.gradle.kts.

## 3. Détail des lots

### L1 - Backend : exploitation et durcissement

| Tâche | Critère d'acceptation |
|---|---|
| Pagination et filtres sur /api/v1/workouts | limit, offset, from, to testés |
| Export .pac (JSON) d'un utilisateur | Réimport possible par une autre montre |
| Limitation de débit sur les routes sensibles | 429 au-delà du seuil, en-tête Retry-After |
| Métriques Prometheus sur /metrics | Format d'exposition, testé |

### L2 - Frontend web

| Tâche | Critère d'acceptation |
|---|---|
| Navigation : séances / statistiques / réglages | Aucune page orpheline |
| Page statistiques : histogramme hebdomadaire, cumul mensuel | SVG généré en Rust, testé |
| Pagination de l'historique | Cohérente avec l'API |
| Export GPX et .pac depuis la page détail | Téléchargement vérifié |

### L3 - Images et publication

| Tâche | Critère d'acceptation |
|---|---|
| Image multi-arch (amd64 + arm64) | Manifeste poussé sur ghcr.io |
| Étiquetage : latest + version + SHA court | podman manifest inspect cohérent |
| CI : construction et publication sur tag | Workflow reproductible |

### L4 - Secrets Vault

| Tâche | Critère d'acceptation |
|---|---|
| ExternalSecret mpacer-secrets (Google + session) | kubectl get externalsecret -n mpacer = SecretSynced |
| ExternalSecret regcred | Reprend le motif global/regcred du cluster |
| auth.existingSecret utilisé par le chart | Aucun secret en clair dans les valeurs Helm |
| Scellement Vault documenté | Commandes vault kv put apps/mpacer fournies |

### L5 - ArgoCD / GitOps

| Tâche | Critère d'acceptation |
|---|---|
| Applications/&lt;catégorie&gt;/mpacer.yaml dans le dépôt jo3 | Application visible dans ArgoCD et Synced |
| Source = chart Helm du dépôt M-pacer + values-jo3.yaml | Rendu identique à helm template |
| CreateNamespace, ServerSideApply, prune, selfHeal | Conforme aux autres applications |
| Secrets hors Git | Identifiants fournis par Vault/ESO |

### L6 - Application montre

| Tâche | Critère d'acceptation |
|---|---|
| Projet Gradle complet (wrapper, catalogue de versions) | gradlew :app:assembleDebug sur une machine avec SDK |
| Écran + service GPS branchés sur mpacer-core | 10 km enregistrés sans téléphone |
| Client de synchronisation intégré | Séance visible dans l'interface web |
| Voix, boutons du casque, mode ambiant | Séance guidée écran éteint |

### L7 - Application Android compagnon

| Tâche | Critère d'acceptation |
|---|---|
| Connexion au backend par code d'appairage | Jeton stocké chiffré |
| Liste et détail des séances | Cohérent avec l'API |
| Relais montre vers backend (Wearable Data Layer) | Séance de la montre présente côté web |
| Installation de l'app montre depuis le téléphone | Procédure documentée et testée |

### L8 - Vérification finale

| Tâche | Critère d'acceptation |
|---|---|
| Bout en bout | Montre → téléphone → backend → web |
| Terrain | 10 km, écart &lt; 3 % avec une montre de référence |
| Batterie | &lt; 25 %/h en séance |

## 4. Ordre d'exécution

| Séance | Contenu |
|---|---|
| 1 (en cours) | L1, L2, L3, L4, L5 en parallèle du lot Android délégué (L6+L7) |
| 2 | Finaliser L4 (scellement Vault), publier l'image (L3), activer l'application ArgoCD (L5) |
| 3 | L6 : premier build sur montre, validation GPS |
| 4 | L7 : relais montre vers backend |
| 5 | L8 : vérification terrain, sauvegardes, supervision |

## 5. Points bloquants connus

| Blocage | Impact | Contournement |
|---|---|---|
| Aucun jeton Vault dans cet environnement | Impossible d'écrire les secrets dans Vault | Les ExternalSecret sont livrés ; le scellement se fait en deux commandes documentées |
| Aucun JDK/SDK/NDK Android | L'app montre et le compagnon ne sont pas compilables ici | Code et configuration livrés, vérification sur machine équipée |
| Identifiants Google OAuth non fournis | La connexion web ne fonctionne pas encore | Mode MPACER_DEV_AUTH pour tester, client OAuth à créer |
| Image non publiée sur ghcr.io | ArgoCD ne peut pas démarrer le pod applicatif | pwsh deploy/build-image.ps1 -Push puis synchronisation ArgoCD |
