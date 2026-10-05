# Regles R8 (minification desactivee par defaut : voir build.gradle.kts).
# Conservees ici pour documenter ce qu il faudra ajouter le jour ou isMinifyEnabled = true.

# kotlinx.serialization utilise la reflexion sur les serialiseurs generes.
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.**
-keepclassmembers class com.mpacer.watch.** {
    *** Companion;
}
-keepclasseswithmembers class com.mpacer.watch.** {
    kotlinx.serialization.KSerializer serializer(...);
}

# Le pont JNI est resolu par nom de methode native.
-keepclasseswithmembernames class com.mpacer.watch.MpacerCore {
    native <methods>;
}
