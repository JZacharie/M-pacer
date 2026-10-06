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

    defaultConfig {
        // Identifiant fige : la montre et le telephone partagent le prefixe com.mpacer.
        applicationId = "com.mpacer.watch"
        // Wear OS 3+ (Android 11) : Health Services, foreground service type, TTS modernes.
        minSdk = 30
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"

        // ABI portees par les montres Wear OS 3/4 et par l emulateur x86_64.
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }

        externalNativeBuild {
            cmake {
                arguments += listOf("-DANDROID_STL=c++_shared")
            }
        }

        // Backend auto-heberge. Surchargeable :
        //   a la compilation : ./gradlew assembleDebug -Pmpacer.apiUrl=http://192.168.0.152:8080
        //   a l'execution    : adb shell am start -n com.mpacer.watch/.MainActivity --es api_url http://hote:8080
        val apiUrlDefaut = (findProperty("mpacer.apiUrl") as String?) ?: "http://10.0.2.2:8080"
        buildConfigField("String", "DEFAULT_API_URL", "\"$apiUrlDefaut\"")
        buildConfigField("String", "PAIRING_LABEL", "\"Montre M-pacer\"")
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
        }
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
        // Les .so Rust sont deja optimises (LTO, panic=abort).
        jniLibs.useLegacyPackaging = false
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

// --- Coeur Rust via cargo-ndk ------------------------------------------------
// Prerequis sur la machine de build : cargo install cargo-ndk et les cibles
// rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android.
// La tache produit app/src/main/jniLibs/<abi>/libmpacer_ffi.so, que CMake importe
// et que l APK empaquette.
//   -Pmpacer.buildRust=false   -> ne pas appeler cargo (reutilise des .so existants)
//   -Pmpacer.cargoProfile=debug -> profil cargo debug
val buildRust = (findProperty("mpacer.buildRust") as String?)?.toBoolean() ?: true
val cargoProfile = (findProperty("mpacer.cargoProfile") as String?) ?: "release"
val rustWorkspaceDir = rootProject.projectDir.parentFile // racine du depot Rust
val jniLibsDir = layout.projectDirectory.dir("src/main/jniLibs").asFile

val cargoNdkBuild = tasks.register<Exec>("cargoNdkBuild") {
    group = "build"
    description = "Compile mpacer-ffi pour arm64-v8a, armeabi-v7a et x86_64 (cargo-ndk)."
    workingDir = rustWorkspaceDir
    val arguments = mutableListOf<String>()
    arguments += listOf("cargo", "ndk", "-t", "arm64-v8a", "-t", "armeabi-v7a", "-t", "x86_64")
    arguments += listOf("-o", jniLibsDir.absolutePath, "build")
    if (cargoProfile == "release") arguments += "--release"
    arguments += listOf("-p", "mpacer-ffi")
    commandLine(*arguments.toTypedArray())
    inputs.dir(File(rustWorkspaceDir, "crates/mpacer-ffi/src"))
    inputs.file(File(rustWorkspaceDir, "Cargo.toml"))
    outputs.dir(jniLibsDir)
    onlyIf { buildRust }
}

// Impossible d assembler un APK sans les .so : on branche la compilation Rust en amont.
tasks.matching { it.name == "preBuild" }.configureEach { dependsOn(cargoNdkBuild) }
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("JniLibFolders") }
    .configureEach { dependsOn(cargoNdkBuild) }
tasks.matching { it.name.startsWith("configure") && it.name.endsWith("NativeBuild") }
    .configureEach { dependsOn(cargoNdkBuild) }
tasks.matching { it.name.startsWith("build") && it.name.endsWith("NativeBuild") }
    .configureEach { dependsOn(cargoNdkBuild) }

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.service)
    implementation(libs.androidx.activity.compose)

    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.wear.compose.material)
    implementation(libs.androidx.wear.compose.foundation)

    // GPS haute precision
    implementation(libs.google.play.services.location)
    // Capteurs / seance systeme (Health Services)
    implementation(libs.androidx.health.services.client)

    // Lecture audio hors ligne (Media3/ExoPlayer) et session medias du lecteur
    implementation(libs.androidx.media3.common)
    implementation(libs.androidx.media3.exoplayer)
    implementation(libs.androidx.media3.session)

    // Synchronisation vers le backend (SyncClient)
    implementation(libs.okhttp)
    implementation(libs.okhttp.logging)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.security.crypto)

    // Reception des seances envoyees par le telephone (Data Layer)
    implementation(libs.google.play.services.wearable)

    debugImplementation(libs.androidx.compose.ui.tooling)
    testImplementation(libs.junit)
}
