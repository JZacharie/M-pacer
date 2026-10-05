import org.gradle.api.initialization.resolve.RepositoriesMode

// Deux modules independants, partageant le meme socle Rust et le meme backend :
//   :app       -> application Wear OS (montre)
//   :companion -> application telephone (liste, detail, envoi vers la montre)
// Les identifiants d'application sont fixes : com.mpacer.watch et com.mpacer.companion.

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

include(":app")
include(":companion")
