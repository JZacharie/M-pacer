// Build racine : les plugins sont declares (et non appliques) ici.
// Chaque module choisit ceux dont il a besoin via le catalogue gradle/libs.versions.toml.
plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.android) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.kotlin.serialization) apply false
}
