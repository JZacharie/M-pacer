package com.mpacer.phone

import android.content.Context
import android.content.SharedPreferences
import com.mpacer.core.AssistantConfig
import com.mpacer.core.AssistantMode
import com.mpacer.core.MusicPolicy
import com.mpacer.core.VoiceConfig
import com.mpacer.core.VoiceFrequency
import com.mpacer.core.VoiceLanguage
import com.mpacer.core.live.LiveConfig
import com.mpacer.core.live.LiveSettings
import com.mpacer.core.music.MusicConfig

/**
 * Reglages du telephone, persistants.
 *
 * La montre garde ses reglages en memoire (ils sont reperdus au redemarrage) :
 * un telephone, lui, se ferme et se rouvre sans arret. Tout est donc range dans
 * des SharedPreferences — sauf les secrets, qui restent dans le socle
 * (mot de passe MQTT et jeton d'appairage y sont chiffres).
 *
 * Aucun calcul de course ici : ce fichier ne fait que conserver des choix
 * d'interface, ensuite transmis au moteur via SessionConfig et MusicSession.
 */
data class PhoneSettings(
    val assistant: AssistantConfig = AssistantConfig(),
    /** Unites d'affichage (km ou miles). */
    val metric: Boolean = true,
    val voice: VoiceConfig = VoiceConfig(),
    val music: MusicConfig = MusicConfig(),
    val live: LiveConfig = LiveConfig.DEFAULT,
    /** Ceinture cardiaque Bluetooth retenue (adresse MAC, vide = aucune). */
    val strapAddress: String = "",
    val strapName: String = "",
    /** Garder l'ecran allume pendant la seance (telephone a la ceinture). */
    val keepScreenOn: Boolean = true,
) {
    /** Nom affiche de la ceinture, ou null si aucune n'est retenue. */
    val strapLabel: String? get() = strapName.ifBlank { strapAddress }.takeIf { it.isNotBlank() }
}

object PhoneSettingsStore {

    private const val FICHIER = "mpacer_phone"
    private const val KEY_MODE = "assistant_mode"
    private const val KEY_DISTANCE = "race_distance_m"
    private const val KEY_PLANNED = "planned_time_s"
    private const val KEY_NEGATIVE_SPLIT = "negative_split_ratio"
    private const val KEY_METRIC = "metric"
    private const val KEY_VOICE_ENABLED = "voice_enabled"
    private const val KEY_VOICE_FREQUENCY = "voice_frequency"
    private const val KEY_VOICE_LANGUAGE = "voice_language"
    private const val KEY_VOICE_EXTENDED = "voice_extended"
    private const val KEY_VOICE_SHORT = "voice_short"
    private const val KEY_VOICE_POLICY = "voice_music_policy"
    private const val KEY_MUSIC_ENABLED = "music_enabled"
    private const val KEY_MUSIC_REFERENCE_BPM = "music_reference_bpm"
    private const val KEY_MUSIC_ANNOUNCE = "music_announce"
    private const val KEY_STRAP_ADDRESS = "strap_address"
    private const val KEY_STRAP_NAME = "strap_name"
    private const val KEY_KEEP_SCREEN_ON = "keep_screen_on"

    fun load(context: Context): PhoneSettings {
        val prefs = prefs(context)
        return PhoneSettings(
            assistant = AssistantConfig(
                mode = enumOr(prefs.getString(KEY_MODE, null), AssistantMode.entries, AssistantMode.TRACK_PACE),
                raceDistanceM = prefs.getFloat(KEY_DISTANCE, 0f).takeIf { it > 0f }?.toDouble(),
                plannedTimeS = prefs.getFloat(KEY_PLANNED, 0f).takeIf { it > 0f }?.toDouble(),
                negativeSplitRatio = prefs.getFloat(KEY_NEGATIVE_SPLIT, 0f).toDouble(),
            ),
            metric = prefs.getBoolean(KEY_METRIC, true),
            voice = VoiceConfig(
                enabled = prefs.getBoolean(KEY_VOICE_ENABLED, true),
                frequency = enumOr(
                    prefs.getString(KEY_VOICE_FREQUENCY, null),
                    VoiceFrequency.entries,
                    VoiceFrequency.EVERY_2_MINUTES,
                ),
                language = enumOr(prefs.getString(KEY_VOICE_LANGUAGE, null), VoiceLanguage.entries, VoiceLanguage.FR),
                extendedLapInfo = prefs.getBoolean(KEY_VOICE_EXTENDED, false),
                shortForms = prefs.getBoolean(KEY_VOICE_SHORT, false),
                musicPolicy = enumOr(
                    prefs.getString(KEY_VOICE_POLICY, null),
                    MusicPolicy.entries,
                    MusicPolicy.DUCK,
                ),
            ),
            music = MusicConfig(
                enabled = prefs.getBoolean(KEY_MUSIC_ENABLED, true),
                referenceBpm = prefs.getFloat(KEY_MUSIC_REFERENCE_BPM, 170f).toDouble(),
                announce = prefs.getBoolean(KEY_MUSIC_ANNOUNCE, true),
            ),
            // Le suivi en direct (mot de passe chiffre) appartient au socle.
            live = LiveSettings.load(context),
            strapAddress = prefs.getString(KEY_STRAP_ADDRESS, "").orEmpty(),
            strapName = prefs.getString(KEY_STRAP_NAME, "").orEmpty(),
            keepScreenOn = prefs.getBoolean(KEY_KEEP_SCREEN_ON, true),
        )
    }

    fun save(context: Context, settings: PhoneSettings) {
        prefs(context).edit()
            .putString(KEY_MODE, settings.assistant.mode.name)
            .putFloat(KEY_DISTANCE, (settings.assistant.raceDistanceM ?: 0.0).toFloat())
            .putFloat(KEY_PLANNED, (settings.assistant.plannedTimeS ?: 0.0).toFloat())
            .putFloat(KEY_NEGATIVE_SPLIT, settings.assistant.negativeSplitRatio.toFloat())
            .putBoolean(KEY_METRIC, settings.metric)
            .putBoolean(KEY_VOICE_ENABLED, settings.voice.enabled)
            .putString(KEY_VOICE_FREQUENCY, settings.voice.frequency.name)
            .putString(KEY_VOICE_LANGUAGE, settings.voice.language.name)
            .putBoolean(KEY_VOICE_EXTENDED, settings.voice.extendedLapInfo)
            .putBoolean(KEY_VOICE_SHORT, settings.voice.shortForms)
            .putString(KEY_VOICE_POLICY, settings.voice.musicPolicy.name)
            .putBoolean(KEY_MUSIC_ENABLED, settings.music.enabled)
            .putFloat(KEY_MUSIC_REFERENCE_BPM, settings.music.referenceBpm.toFloat())
            .putBoolean(KEY_MUSIC_ANNOUNCE, settings.music.announce)
            .putString(KEY_STRAP_ADDRESS, settings.strapAddress)
            .putString(KEY_STRAP_NAME, settings.strapName)
            .putBoolean(KEY_KEEP_SCREEN_ON, settings.keepScreenOn)
            .apply()
        // Le suivi en direct a sa propre persistance (mot de passe chiffre).
        LiveSettings.save(context, settings.live)
    }

    private fun <T : Enum<T>> enumOr(name: String?, values: List<T>, defaut: T): T =
        values.firstOrNull { it.name == name } ?: defaut

    private fun prefs(context: Context): SharedPreferences =
        context.applicationContext.getSharedPreferences(FICHIER, Context.MODE_PRIVATE)
}
