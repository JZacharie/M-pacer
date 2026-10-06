import org.gradle.api.initialization.resolve.RepositoriesMode

// Quatre modules, un seul socle Rust et un seul backend :
//   :core      -> bibliotheque partagee (coeur Rust, seance, voix, sync, MQTT, musique)
//   :app       -> application Wear OS (montre)
//   :phone     -> application telephone : courir avec le telephone
//   :companion -> application telephone d'appoint (liste, detail, envoi vers la montre)
// Les identifiants d'application sont fixes : com.mpacer.watch, com.mpacer.phone
// et com.mpacer.companion.

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    // Les depots sont declares ici uniquement : un module ne peut pas en ajouter.
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "M-pacer"

include(":core")
include(":app")
include(":phone")
include(":companion")
