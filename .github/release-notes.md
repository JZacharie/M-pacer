Applications et outils M-pacer pour ce tag.

### Applications Android

| Application | Fichier |
|---|---|
| Montre Wear OS | `M-pacer-montre-<version>-release.apk` |
| Course au telephone | `M-pacer-telephone-<version>-release.apk` |
| Application d'appoint | `M-pacer-compagnon-<version>-release.apk` |

Installation : `adb install -r <fichier>.apk`. La montre et le telephone
partagent la meme cle de signature : le Data Layer Wear OS l'exige.

- un fichier `...-release.apk` est signe avec la **cle du projet** : les mises
  a jour s'installent par-dessus la version precedente ;
- un fichier `...-debug.apk` (produit par la CI quand le secret
  `ANDROID_KEYSTORE_BASE64` n'est pas configure) est signe avec la cle de
  debug : il s'installe, mais changer de signature impose de desinstaller
  l'application avant de la remettre.

### Montre Garmin

L'application Connect IQ vit dans `garmin/` : le code est dans le tag, mais
aucune archive n'est jointe ici. Un `.iq` (Connect IQ Store) ou un `.prg`
(installation par USB) exige les profils d'appareils du **SDK Manager** de
Garmin (compte developpeur) et une cle de signature : deux choses qu'une
machine d'integration ephemere n'a pas. Voir `garmin/README.md` :

```
pwsh ./garmin/build.ps1 -Device fr965 -Package
```

### Serveur et outils

- `mpacer-outils-<version>-linux-x86_64.tar.gz` et
  `mpacer-outils-<version>-windows-x86_64.zip` : `mpacer-api` (serveur),
  `mpacer-sim` (simulateur de seance) et `mpacer-music` (transfert des MP3
  vers la montre).
- Image conteneur : `ghcr.io/jzacharie/mpacer:<tag>` (voir `deploy/README.md`).
- Chart Helm : `charts/mpacer`, version alignee sur le tag.

`SHA256SUMS.txt` contient l'empreinte de chaque fichier.
