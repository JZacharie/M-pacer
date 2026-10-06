import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// --- Signature ---------------------------------------------------------------
// Meme fichier android/keystore.properties que :app et :companion. Une signature
// commune n'est pas exigee pour courir avec le telephone, mais elle evite d'avoir
// deux jeux de cles a gerer sur une meme machine.
val keystorePropertiesFile = rootProject.file("keystore.properties")
val keystoreProperties = Properties().apply {
    if (keystorePropertiesFile.exists()) {
        keystorePropertiesFile.inputStream().use { load(it) }
    }
}

android {
    namespace = "com.mpacer.phone"
    compileSdk = 35

    // NDK epingle : la version documentee par local-ci.ps1 (r27) et installee par
    // l'integration continue, qui n'a qu'un seul NDK sur le runner.
    ndkVersion = "27.2.12479018"

    defaultConfig {
        applicationId = "com.mpacer.phone"
        minSdk = 26 // Android 8+
        targetSdk = 35
        versionCode = 2
        versionName = "0.2.0"

        // ABI des telephones recents, des appareils plus anciens et de l'emulateur.
        // Les .so viennent du module :core, filtres ici a l'empaquetage.
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }

        // Libelle de l'appareil envoye au backend pendant l'appairage.
        buildConfigField("String", "PAIRING_LABEL", "\"Telephone M-pacer\"")
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

dependencies {
    // Coeur Rust, seance, voix, archive, synchronisation, MQTT et musique.
    implementation(project(":core"))

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.activity.compose)

    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.compose.material3)

    implementation(libs.kotlinx.coroutines.android)
    // Ouverture de la page /link dans un onglet personnalise pendant l'appairage.
    implementation(libs.androidx.browser)

    debugImplementation(libs.androidx.compose.ui.tooling)
    testImplementation(libs.junit)
}
