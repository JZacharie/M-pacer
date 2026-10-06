# Regles R8 du socle partage (minification desactivee par defaut : voir build.gradle.kts).
# Conservees ici pour documenter ce qu il faudra ajouter le jour ou isMinifyEnabled = true.

# kotlinx.serialization utilise la reflexion sur les serialiseurs generes.
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.**
-keepclassmembers class com.mpacer.core.** {
    *** Companion;
}
-keepclasseswithmembers class com.mpacer.core.** {
    kotlinx.serialization.KSerializer serializer(...);
}

# Le pont JNI est resolu par nom de methode native.
-keepclasseswithmembernames class com.mpacer.core.MpacerCore {
    native <methods>;
}
