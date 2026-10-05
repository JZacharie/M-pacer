package com.mpacer.watch

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.speech.tts.TextToSpeech
import java.util.Locale

/**
 * Retour vocal : synthese vocale + coexistence avec la musique.
 *
 * Le coeur decide *quand* et *quoi* dire (il fournit un texte deja redige, dans la
 * bonne langue). Cette classe ne fait que le rendre audible, avec la strategie
 * audio choisie : baisser le volume, mettre en pause, ou parler par-dessus.
 */
object VoiceCoach : TextToSpeech.OnInitListener {

    private var tts: TextToSpeech? = null
    private var ready = false
    private var audioManager: AudioManager? = null
    private var focusRequest: AudioFocusRequest? = null
    private var policy: MusicPolicy = MusicPolicy.DUCK
    private var language: VoiceLanguage = VoiceLanguage.FR
    private var enabled = true

    fun initialise(context: Context) {
        if (tts == null) {
            tts = TextToSpeech(context.applicationContext, this)
            audioManager = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        }
    }

    fun configure(config: VoiceConfig) {
        enabled = config.enabled
        policy = config.musicPolicy
        if (language != config.language) {
            language = config.language
            applyLanguage()
        }
    }

    override fun onInit(status: Int) {
        ready = status == TextToSpeech.SUCCESS
        if (ready) applyLanguage()
    }

    private fun applyLanguage() {
        val locale = when (language) {
            VoiceLanguage.FR -> Locale.FRANCE
            VoiceLanguage.EN -> Locale.US
        }
        tts?.language = locale
    }

    /** Prononce un message fourni par le moteur (deja redige, dans la bonne langue). */
    fun speak(text: String) {
        if (!enabled || !ready || text.isBlank()) return
        val engine = tts ?: return
        requestFocus()
        engine.speak(text, TextToSpeech.QUEUE_ADD, null, text.hashCode().toString())
    }

    fun shutdown() {
        tts?.shutdown()
        tts = null
        ready = false
    }

    private fun requestFocus() {
        val manager = audioManager ?: return
        val attributes = AudioAttributes.Builder()
            .setUsage(AudioAttributes.USAGE_ASSISTANCE_NAVIGATION_GUIDANCE)
            .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
            .build()
        val request = AudioFocusRequest.Builder(
            when (policy) {
                MusicPolicy.DUCK -> AudioManager.AUDIOFOCUS_GAIN_TRANSIENT_MAY_DUCK
                MusicPolicy.PAUSE, MusicPolicy.IGNORE_AND_SPEAK -> AudioManager.AUDIOFOCUS_GAIN_TRANSIENT
            }
        ).setAudioAttributes(attributes).build()
        focusRequest = request
        manager.requestAudioFocus(request)
    }
}
