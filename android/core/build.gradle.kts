import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// --- Socle commun ------------------------------------------------------------
// Ce module porte tout ce qui n'est pas propre a un ecran : le pont JNI vers
// mpacer-core, la seance (TrackingService), la voix, l'archive locale, la
// synchronisation, le suivi MQTT et la musique. Les applications :app (montre)
// et :phone (telephone) n'ajoutent que leur interface et leurs permissions.
//
// Regle du depot : aucun calcul de course ici. Le module transporte les
// commandes et l'etat publies par le coeur Rust.
android {
    namespace = "com.mpacer.core"
    compileSdk = 35

    // NDK epingle : la version documentee par local-ci.ps1 (r27) et installee par
    // l'integration continue, qui n'a qu'un seul NDK sur le runner.
    ndkVersion = "27.2.12479018"

    defaultConfig {
        // Le plan le plus bas des deux applications (Android 8 pour le telephone).
        minSdk = 26

        // ABI portees par les montres Wear OS 3/4, les telephones et l'emulateur.
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }

        externalNativeBuild {
            cmake {
                arguments += listOf("-DANDROID_STL=c++_shared")
            }
        }

        // Backend auto-heberge utilise tant que l'utilisateur n'a rien regle.
        //   ./gradlew assembleDebug -Pmpacer.apiUrl=http://192.168.0.152:8080
        val apiUrlDefaut = (findProperty("mpacer.apiUrl") as String?) ?: "http://10.0.2.2:8080"
        buildConfigField("String", "DEFAULT_API_URL", "\"$apiUrlDefaut\"")

        // Le module est consomme par deux applications : ce sont les regles des
        // applications qui s'appliquent a l'assemblage final.
        consumerProguardFiles("proguard-rules.pro")
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
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
// La tache produit core/src/main/jniLibs/<abi>/libmpacer_ffi.so, que CMake importe
// et que l'APK empaquette.
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

// Impossible d'assembler un APK sans les .so : on branche la compilation Rust en amont.
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

    // Palette et voyant GPS partages par les deux interfaces.
    api(platform(libs.androidx.compose.bom))
    api(libs.androidx.compose.ui)
    api(libs.androidx.compose.foundation)

    // GPS haute precision de la seance
    implementation(libs.google.play.services.location)

    // Lecture audio hors ligne (Media3/ExoPlayer) et session medias du lecteur
    implementation(libs.androidx.media3.common)
    implementation(libs.androidx.media3.exoplayer)
    implementation(libs.androidx.media3.session)

    // Synchronisation vers le backend
    implementation(libs.okhttp)
    implementation(libs.okhttp.logging)
    api(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.security.crypto)

    testImplementation(libs.junit)
}
