// Plugins declares ici, appliques dans :app
plugins {
    id("com.android.application") version "8.7.3" apply false
    id("org.jetbrains.kotlin.android") version "2.1.0" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.1.0" apply false
    // Voie maintenue en 2026 pour compiler du Rust dans un projet Android
    id("org.mozilla.rust-android-gradle.rust-android") version "0.10.0" apply false
}
