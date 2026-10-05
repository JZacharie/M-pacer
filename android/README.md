# Shell Wear OS (Kotlin + Compose)

Ce dossier contient le squelette de l'application montre. Il **n'a pas ete compile**
dans l'environnement de developpement initial (ni JDK, ni SDK/NDK Android) : c'est un
point de depart structure, a valider sur une machine equipee (voir phase 0 de
[docs/03-plan-action.md](../docs/03-plan-action.md)).

## Prerequis

- JDK 17+, Android Studio recente, SDK Android 35+, Wear OS SDK
- NDK r27+ et `cargo-ndk`
- `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`

## Compiler

```bash
# 1. Bibliotheque Rust pour chaque ABI (produit app/src/main/jniLibs/<abi>/libmpacer_ffi.so)
cd ..
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o android/app/src/main/jniLibs build --release -p mpacer-ffi

# 2. Application montre
cd android
./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

## Comment le code est organise

| Fichier | Role |
|---|---|
| `app/src/main/cpp/mpacer_jni.c` | Shim JNI (40 lignes) : expose la C ABI Rust a Kotlin. Evite d'ajouter la dependance `jni` au crate Rust. |
| `app/src/main/java/com/mpacer/watch/MpacerCore.kt` | Chargement de `.so`, cycle de vie du moteur, envoi de commandes JSON, lecture de l'etat |
| `.../TrackingService.kt` | Service de premier plan (`location\|health`), boucle GPS 1 Hz, notification |
| `.../VoiceCoach.kt` | `TextToSpeech` + focus audio (duck / pause / ignorer) |
| `.../MainActivity.kt` | Permissions, liaison au service, navigation entre ecrans |
| `.../ui/MainScreen.kt` | Ecran rond : allure, distance, temps, feu GPS, panneau d'assistant |
| `.../ui/SettingsScreen.kt` | Mode d'assistant, unites, retour vocal |

## Regle d'or

Toute la logique de course vit dans `mpacer-core`. Le code Kotlin ne doit jamais
recalculer une allure, un tour ou un ecart au plan : il pousse des positions, envoie
des commandes et affiche `EngineOutput`. Si une regle metier doit changer, elle change
en Rust et gagne un test.

## Variante sans shim C

Si vous preferez supprimer le fichier C et CMake, ajoutez au crate `mpacer-ffi` :

```toml
[target.'cfg(target_os = "android")'.dependencies]
jni = "0.22"
```

puis implementez les symboles `Java_com_mpacer_watch_MpacerCore_native*` directement
en Rust. C'est la voie recommandee par UniFFI/JNI en 2026, au prix d'une dependance
supplementaire ; la C ABI JSON livree ici reste la meme dans les deux cas.
