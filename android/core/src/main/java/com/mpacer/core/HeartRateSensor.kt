package com.mpacer.core

import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager

/**
 * Capteur de frequence cardiaque de la montre (TYPE_HEART_RATE).
 *
 * Aucun calcul ici : chaque mesure part telle quelle dans le moteur Rust, qui la
 * rattache a la seance (zones, derive cardiaque, graphique et export GPX).
 *
 * Le capteur n'est ecoute que pendant une seance : hors course, il ne servirait
 * a rien et couterait de la batterie.
 */
class HeartRateSensor(
    context: Context,
    private val onSample: (tMs: Long, bpm: Int) -> Unit,
) {

    private val manager: SensorManager? =
        context.getSystemService(Context.SENSOR_SERVICE) as? SensorManager

    private val sensor: Sensor? = manager?.getDefaultSensor(Sensor.TYPE_HEART_RATE)

    /** Vrai si la montre possede un capteur : l'ecran peut alors afficher la FC. */
    val available: Boolean get() = sensor != null

    private val listener = object : SensorEventListener {
        override fun onSensorChanged(event: SensorEvent) {
            if (event.sensor.type != Sensor.TYPE_HEART_RATE) return
            val bpm = event.values.firstOrNull()?.toInt() ?: return
            // Le capteur annonce 0 tant qu'il n'a pas accroche le pouls.
            if (bpm <= 0) return
            onSample(System.currentTimeMillis(), bpm)
        }

        override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) = Unit
    }

    /**
     * Demarre l'ecoute. Sans capteur, ou sans la permission BODY_SENSORS, il ne se
     * passe rien : la seance reste parfaitement utilisable sans cardio.
     */
    fun start() {
        val sensor = sensor ?: return
        try {
            manager?.registerListener(listener, sensor, SensorManager.SENSOR_DELAY_NORMAL)
        } catch (security: SecurityException) {
            // Permission refusee : le cardio est simplement absent de la seance.
        }
    }

    fun stop() {
        manager?.unregisterListener(listener)
    }
}

/**
 * Source de frequence cardiaque supplementaire.
 *
 * [HeartRateSensor] couvre le capteur integre de l'appareil (montre, ou de rares
 * telephones). Le telephone, lui, lit sa frequence sur une ceinture Bluetooth :
 * l'application y enregistre cette source dans [HeartRateSources.external], et la
 * seance demarre les deux sans rien connaitre de Bluetooth.
 */
interface HeartRateSource {

    /** Nom affiche dans les reglages ("Ceinture BLE"). */
    val label: String

    /** Demarre l'ecoute ; chaque mesure part telle quelle dans le moteur. */
    fun start(onSample: (tMs: Long, bpm: Int) -> Unit)

    fun stop()
}

/** Registre des sources cardiaques externes (une seule a la fois). */
object HeartRateSources {

    @Volatile
    var external: HeartRateSource? = null
}
