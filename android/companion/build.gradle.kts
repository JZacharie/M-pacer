import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// --- Signature ---------------------------------------------------------------
// Meme fichier android/keystore.properties que :app : le Data Layer Wear OS
// n accepte que deux applications signees par la meme cle.
val keystorePropertiesFile = rootProject.file("keystore.properties")
val keystoreProperties = Properties().apply {
    if (keystorePropertiesFile.exists()) {
        keystorePropertiesFile.inputStream().use { load(it) }
    }
}

android {
    namespace = "com.mpacer.companion"
    compileSdk = 35

    defaultConfig {
        // Prefixe com.mpacer partage avec la montre (voir README pour la contrainte Data Layer).
        applicationId = "com.mpacer.companion"
        minSdk = 26 // Android 8+
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"

        buildConfigField("String", "DEFAULT_API_URL", "\"http://10.0.2.2:8080\"")
        buildConfigField("String", "WATCH_PACKAGE", "\"com.mpacer.watch\"")
        buildConfigField("String", "DEVICE_LABEL", "\"Telephone M-pacer\"")
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
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.activity.compose)

    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.compose.material3)

    // Backend
    implementation(libs.okhttp)
    implementation(libs.okhttp.logging)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.security.crypto)
    // Page /link dans un onglet personnalise
    implementation(libs.androidx.browser)

    // Data Layer : envoi des seances vers la montre
    implementation(libs.google.play.services.wearable)

    debugImplementation(libs.androidx.compose.ui.tooling)
    testImplementation(libs.junit)
}
