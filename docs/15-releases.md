# 15 - Versions et publications (releases)

M-pacer se publie par **tag Git** : pousser `vX.Y.Z` déclenche la construction de
chaque application et la création de la publication GitHub correspondante.
Aucune archive n'est construite sur le poste du mainteneur.

- Workflow : [`.github/workflows/release.yml`](../.github/workflows/release.yml)
- Notes de version communes : [`.github/release-notes.md`](../.github/release-notes.md)
- Image conteneur : [`.github/workflows/publish.yml`](../.github/workflows/publish.yml)
- Déploiement : [deploy/README.md](../deploy/README.md)

---

## 1. Ce que contient une publication

| Application | Fichier publié | Construction |
|---|---|---|
| Montre Wear OS | `M-pacer-montre-<version>-release.apk` | `:app:assembleRelease` |
| Course au téléphone | `M-pacer-telephone-<version>-release.apk` | `:phone:assembleRelease` |
| Application d'appoint | `M-pacer-compagnon-<version>-release.apk` | `:companion:assembleRelease` |
| Outils et serveur (Linux) | `mpacer-outils-<version>-linux-x86_64.tar.gz` | `cargo build --release` |
| Outils et serveur (Windows) | `mpacer-outils-<version>-windows-x86_64.zip` | `cargo build --release` |
| Empreintes | `SHA256SUMS.txt` | `sha256sum` |

Les archives d'outils contiennent `mpacer-api` (serveur), `mpacer-sim`
(simulateur de séance) et `mpacer-music` (transfert des MP3 vers la montre).

L'**image conteneur** est publiée en parallèle par `publish.yml` :
`ghcr.io/jzacharie/mpacer:vX.Y.Z` (et `latest` pour `main`). Le **chart Helm**
suit la même version (`charts/mpacer/Chart.yaml`).

### Ce qui n'est pas dans la publication

L'application **Garmin Connect IQ** est validée par la CI (compilation
`monkeyc`), mais aucun `.iq` ni `.prg` n'est joint : produire une archive
installable exige les **profils d'appareils** du SDK Manager de Garmin (compte
développeur) et une clé de signature, absents d'une machine d'intégration
éphémère. La construction se fait sur le poste, avec le SDK installé :

```bash
pwsh ./garmin/build.ps1 -Device fr965 -Package   # .iq signé (Connect IQ Store)
pwsh ./garmin/build.ps1 -Device fr965            # .prg à copier dans GARMIN/APPS
```

Voir [garmin/README.md](../garmin/README.md).

---

## 2. Publier une version

1. **Aligner les versions** sur le numéro du tag :

   | Fichier | Champ |
   |---|---|
   | [`Cargo.toml`](../Cargo.toml) | `[workspace.package] version` (les binaires et `mpacer_core::VERSION` suivent) |
   | `android/app/build.gradle.kts` | `versionCode` (+1) et `versionName` |
   | `android/phone/build.gradle.kts` | `versionCode` (+1) et `versionName` |
   | `android/companion/build.gradle.kts` | `versionCode` (+1) et `versionName` |
   | [`charts/mpacer/Chart.yaml`](../charts/mpacer/Chart.yaml) | `version` et `appVersion` |

2. **Vérifier** : `cargo test --workspace`, `cargo fmt --all --check`,
   `cargo clippy --workspace --all-targets -- -D warnings`.

3. **Taguer et pousser** :

   ```bash
   git tag -a v0.2.0 -m "M-pacer 0.2.0"
   git push origin main v0.2.0
   ```

4. La publication se crée seule. Pour rejouer une publication (tag existant) :

   ```bash
   gh workflow run release.yml -f tag=v0.2.0
   ```

Le workflow met à jour la publication existante si elle existe déjà : relancer
le tag ne crée pas de doublon.

---

## 3. Signature des APK

Android n'installe une mise à jour que si elle est signée avec **la même clé**
que la version précédente, et le Data Layer Wear OS exige en plus que la montre
et le téléphone partagent cette clé. Le dépôt ne contient aucune clé
(`android/keystore.properties` et `android/*.jks` sont ignorés par git).

| Secret du dépôt | Contenu |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | magasin de clés `.jks` encodé en base64 |
| `ANDROID_KEYSTORE_PROPERTIES` | contenu de `android/keystore.properties` (`storeFile`, `storePassword`, `keyAlias`, `keyPassword`) |

```bash
# Depot des secrets (une seule fois)
base64 -w0 android/mpacer-release.jks > /tmp/keystore.b64
gh secret set ANDROID_KEYSTORE_BASE64 < /tmp/keystore.b64
gh secret set ANDROID_KEYSTORE_PROPERTIES < android/keystore.properties
```

Sans ces secrets, la CI replie sur la **clé de debug** : les APK s'installent,
mais changer de clé ensuite impose de désinstaller l'application avant de la
réinstaller. Le journal du workflow affiche un avertissement dans ce cas.

### URL du backend inscrite dans les APK

`MPACER_API_URL` (variable de dépôt, `Settings > Secrets and variables >
Actions > Variables`) fixe l'URL par défaut ; à défaut,
`https://mpacer.p.zacharie.org`. Elle est passée à Gradle
(`-Pmpacer.apiUrl=...`) et reste modifiable dans les réglages de l'application.

---

## 4. Équivalent local

Les mêmes constructions existent sur le poste, sans passer par GitHub :

```bash
pwsh ./local-ci.ps1 -Check                       # diagnostic des prérequis
pwsh ./local-ci.ps1 -Target all -Release \
     -ApiUrl https://mpacer.p.zacharie.org       # trois APK release signés
cargo build --release --workspace                # serveur et outils
pwsh ./garmin/build.ps1 -Device fr965 -Package   # montre Garmin
```

Prérequis détaillés (JDK 17, SDK Android 35, NDK r27.2.12479018, CMake 3.22.1,
cargo-ndk) : [android/README.md](../android/README.md).
