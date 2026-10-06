# Shim JNI : expose la C ABI de libmpacer_ffi.so a Kotlin.
# Les symboles de la C ABI (mpacer_new, mpacer_command, ...) sont resolus a l'edition
# de liens, puis au chargement par le dynamic linker Android (les deux .so sont
# packages cote a cote dans le meme APK).
cmake_minimum_required(VERSION 3.22.1)
project(mpacer_jni C)

# Ce fichier vit dans app/src/main/cpp : les .so produits par cargo-ndk sont dans
# app/src/main/jniLibs/<abi>/, soit un seul niveau au-dessus.
set(RUST_LIB_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../jniLibs/${ANDROID_ABI}")

add_library(mpacer_ffi SHARED IMPORTED)
set_target_properties(mpacer_ffi PROPERTIES
    IMPORTED_LOCATION "${RUST_LIB_DIR}/libmpacer_ffi.so"
    # La cdylib produite par cargo n'a pas de SONAME : sans cette propriete, CMake
    # transmet le chemin complet au linker, qui l'inscrit tel quel dans DT_NEEDED.
    # L'APK embarque alors une dependance vers un chemin de la machine de build
    # (dtopen echoue sur la montre). Avec IMPORTED_NO_SONAME, seul le nom du
    # fichier est enregistre : la resolution se fait entre bibliotheques de l'APK.
    IMPORTED_NO_SONAME TRUE)

add_library(mpacer_jni SHARED mpacer_jni.c)
target_link_libraries(mpacer_jni mpacer_ffi)
