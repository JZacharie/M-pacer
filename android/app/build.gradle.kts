plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.mozilla.rust-android-gradle.rust-android")
}

android {
    namespace = "com.mpacer.watch"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.mpacer.watch"
        // Wear OS 3+ (Android 11) : Health Services et service de premier plan modernes
        minSdk = 30
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
        externalNativeBuild {
            cmake {
                arguments += listOf("-DANDROID_STL=c++_shared")
            }
        }
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
        }
    }

    buildFeatures {
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    packaging {
        // Les .so Rust sont deja optimises (LTO, panic=abort)
        jniLibs.useLegacyPackaging = false
    }
}

// --- Compilation du coeur Rust -------------------------------------------------
cargo {
    module = "../../crates/mpacer-ffi"
    libname = "mpacer_ffi"
    targets = listOf("arm64", "arm", "x86_64")
    profile = "release"
    // Verbosite utile au premier montage : decommenter
    // verbose = true
}

// Le shim JNI et les .so Rust doivent exister avant toute tache native ou de packaging.
tasks.matching { task ->
    task.name.startsWith("merge") && task.name.endsWith("JniLibFolders") ||
        task.name.startsWith("configure") && task.name.endsWith("NativeBuild") ||
        task.name.startsWith("build") && task.name.endsWith("NativeBuild")
}.configureEach {
    dependsOn("cargoBuildRelease")
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-service:2.8.7")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation(platform("androidx.compose:compose-bom:2024.10.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.wear.compose:compose-material:1.4.0")
    implementation("androidx.wear.compose:compose-foundation:1.4.0")
    // GPS
    implementation("com.google.android.gms:play-services-location:21.3.0")
    // Capteurs / seance systeme (phase 2)
    implementation("androidx.health:health-services-client:1.1.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    debugImplementation("androidx.compose.ui:ui-tooling")
}
