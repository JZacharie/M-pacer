import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// --- Signature ---------------------------------------------------------------
// Le Data Layer Wear OS n accepte que des applications signees par la meme cle
// sur le telephone et sur la montre. On lit android/keystore.properties sil existe
// (fichier non versionne, aucun secret en dur) ; sinon le build de debug utilise
// la cle de debug standard, suffisante pour le developpement.
val keystorePropertiesFile = rootProject.file("keystore.properties")
val keystoreProperties = Properties().apply {
    if (keystorePropertiesFile.exists()) {
        keystorePropertiesFile.inputStream().use { load(it) }
    }
}

android {
    namespace = "com.mpacer.watch"
    compileSdk = 35

    // NDK epingle : la version documentee par local-ci.ps1 (r27) et installee par
    // l'integration continue, qui n'a qu'un seul NDK sur le runner.
    ndkVersion = "27.2.12479018"

    defaultConfig {
        // Identifiant fige : la montre et le telephone partagent le prefixe com.mpacer.
        applicationId = "com.mpacer.watch"
        // Wear OS 3+ (Android 11) : Health Services, foreground service type, TTS modernes.
        minSdk = 30
        targetSdk = 35
        versionCode = 2
        versionName = "0.2.0"

        // ABI portees par les montres Wear OS 3/4 et par l emulateur x86_64.
        // Les .so viennent du module :core, filtres ici a l'empaquetage.
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }

        // Libelle de l'appareil envoye au backend pendant l'appairage.
        buildConfigField("String", "PAIRING_LABEL", "\"Montre M-pacer\"")
    }

    signingConfigs {
        if (keystorePropertiesFile.exists()) {
            create("release") {
                storeFile = rootProject.file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("storePassword")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            // Pas de minification pour l instant : kotlinx.serialization et le pont JNI
            // demanderaient des regles keep explicites (voir proguard-rules.pro).
            isMinifyEnabled = false
            isShrinkResources = false
            signingConfig = signingConfigs.findByName("release")
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

// --- Garde-fou sur les bibliotheques natives ---------------------------------
// :app n'a aucune source native a lui : les .so viennent tous de :core (coeur
// Rust et shim JNI). Des copies locales avaient pourtant survecu dans
// src/main/jniLibs (ignorees par git, datant d'avant l'extraction du socle) et
// elles MASQUAIENT celles de :core lors de la fusion : un symbole ajoute cote
// Rust manquait alors au chargement, et l'application mourait sur
// UnsatisfiedLinkError -- sans que rien ne le signale a la compilation.
// On refuse desormais de compiler tant que ces copies traînent la.
val jniLibsLocaux = layout.projectDirectory.dir("src/main/jniLibs").asFile
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("JniLibFolders") }
    .configureEach {
        doFirst {
            val restes = jniLibsLocaux.walkTopDown().filter { it.extension == "so" }.toList()
            check(restes.isEmpty()) {
                "Copies natives locales dans app/src/main/jniLibs : " +
                    restes.joinToString { it.name } +
                    ". Elles masquent celles de :core ; supprimez ce dossier."
            }
        }
    }

// Le coeur Rust, le shim JNI, la seance, la voix, la synchronisation, le suivi
// MQTT et la musique vivent dans le module :core, partage avec :phone.

dependencies {
    // Coeur Rust, seance, voix, archive, synchronisation, MQTT et musique.
    implementation(project(":core"))

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.activity.compose)

    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.wear.compose.material)
    implementation(libs.androidx.wear.compose.foundation)

    implementation(libs.kotlinx.coroutines.android)

    // Reception des seances envoyees par le telephone (Data Layer)
    implementation(libs.google.play.services.wearable)

    debugImplementation(libs.androidx.compose.ui.tooling)
    testImplementation(libs.junit)
}
