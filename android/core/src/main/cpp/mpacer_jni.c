/* Shim JNI minimal (C) entre Kotlin et la C ABI JSON de mpacer-ffi.
 *
 * Pourquoi ce fichier : Kotlin ne peut appeler que des symboles JNI
 * (Java_<paquet>_<Classe>_<methode>). La C ABI Rust reste stable et testable sur PC,
 * et ce shim de 40 lignes fait la traduction, sans ajouter de dependance au crate Rust.
 */
#include <jni.h>
#include <stdint.h>

/* C ABI exportee par crates/mpacer-ffi/src/lib.rs */
extern void *mpacer_new(void);
extern void mpacer_free(void *handle);
extern char *mpacer_command(void *handle, const char *command);
extern char *mpacer_version(void);
extern void mpacer_string_free(char *text);

static jstring take_string(JNIEnv *env, char *text) {
    if (text == NULL) {
        return (*env)->NewStringUTF(env, "{\"error\":\"reponse nulle\"}");
    }
    jstring result = (*env)->NewStringUTF(env, text);
    mpacer_string_free(text);
    return result;
}

JNIEXPORT jlong JNICALL
Java_com_mpacer_watch_MpacerCore_nativeNew(JNIEnv *env, jclass clazz) {
    (void) env;
    (void) clazz;
    return (jlong) (intptr_t) mpacer_new();
}

JNIEXPORT void JNICALL
Java_com_mpacer_watch_MpacerCore_nativeFree(JNIEnv *env, jclass clazz, jlong handle) {
    (void) env;
    (void) clazz;
    mpacer_free((void *) (intptr_t) handle);
}

JNIEXPORT jstring JNICALL
Java_com_mpacer_watch_MpacerCore_nativeCommand(JNIEnv *env, jclass clazz, jlong handle, jstring command) {
    (void) clazz;
    if (command == NULL) {
        return (*env)->NewStringUTF(env, "{\"error\":\"commande nulle\"}");
    }
    const char *utf = (*env)->GetStringUTFChars(env, command, NULL);
    if (utf == NULL) {
        return (*env)->NewStringUTF(env, "{\"error\":\"commande illisible\"}");
    }
    char *response = mpacer_command((void *) (intptr_t) handle, utf);
    (*env)->ReleaseStringUTFChars(env, command, utf);
    return take_string(env, response);
}

JNIEXPORT jstring JNICALL
Java_com_mpacer_watch_MpacerCore_nativeVersion(JNIEnv *env, jclass clazz) {
    (void) clazz;
    return take_string(env, mpacer_version());
}
