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
   | [`garmin/manifest.xml`](../garmin/manifest.xml) | attribut `version` de `<iq:application>` |
   | [`site/_data/version.yml`](../site/_data/version.yml) | `version` (pied de page du site) |
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

### Version et horodatage visibles dans les applications

Chaque application affiche sa **version** et le **moment de sa compilation**
dans ses réglages (et, pour le site, dans le pied de page). Deux APK portant la
même version restent donc discernables — y compris deux compilations du même
jour, puisque l'horodatage descend à la minute — et un correctif se vérifie à
l'œil.

| Application | Où | Version lue dans | Horodatage lu dans |
|---|---|---|---|
| Montre Wear OS | Réglages, section « Version » | `versionName` de l'APK | `BuildConfig.BUILD_DATE` |
| Course au téléphone | Réglages, section « Version » | `versionName` de l'APK | `BuildConfig.BUILD_DATE` |
| Application d'appoint | En-tête de la liste des séances | `versionName` de l'APK | `BuildConfig.BUILD_DATE` |
| Montre Garmin | Écran Réglages | `version` du manifeste | `MpacerBuildInfo` généré |
| Site web | Réglages, section « Version » | `Cargo.toml` | `build.rs` (`BUILD_DATE`) |
| Documentation | Pied de page | `site/_data/version.yml` | `site.time` (Jekyll) |

Le format stocké est **`AAAA-MM-JJ HH:MM`**, en UTC ; l'affichage le traduit en
`JJ/MM/AAAA HH:MM` (montre, téléphone, compagnon) ou le laisse tel quel (site,
Garmin). Un APK plus ancien, qui ne porte que le jour, reste lisible.

L'horodatage est choisi **à la compilation**, dans cet ordre :

1. `MPACER_BUILD_DATE` (variable d'environnement ou propriété Gradle
   `-Pmpacer.buildDate`) : c'est ce que posent `release.yml` et `local-ci.ps1`, à
   partir du commit tagué — le moment de la release, pas celui du runner qui
   rejoue le build ;
2. `SOURCE_DATE_EPOCH` (secondes UNIX), la variable normalisée de
   reproductibilité ;
3. l'horloge de la machine, en **UTC** (la même heure sur un poste, en CI et dans
   l'image DEBIAN).

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
