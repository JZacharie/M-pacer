package com.mpacer.core

import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.os.SystemClock

/**
 * Capteur de cadence et de pas (TYPE_STEP_DETECTOR avec repli TYPE_STEP_COUNTER).
 *
 * Mesure les pas en temps reel durant la seance pour transmettre la cadence
 * (spm = pas par minute) au moteur Rust via [onCadence].
 * Le moteur combine cette cadence avec la vitesse GPS pour calculer la
 * longueur de foulee (stride_m = speed * 60 / cadence).
 */
class StepCadenceSensor(
    context: Context,
    private val onCadence: (tMs: Long, spm: Double) -> Unit,
) {

    private val manager: SensorManager? =
        context.getSystemService(Context.SENSOR_SERVICE) as? SensorManager

    private val detectorSensor: Sensor? = manager?.getDefaultSensor(Sensor.TYPE_STEP_DETECTOR)
    private val counterSensor: Sensor? = if (detectorSensor == null) {
        manager?.getDefaultSensor(Sensor.TYPE_STEP_COUNTER)
    } else null

    val available: Boolean get() = detectorSensor != null || counterSensor != null

    // Garde les derniers horodatages de pas (ms) pour lisser la cadence sur une fenetre glissante
    private val stepHistoryMs = ArrayDeque<Long>(8)
    private var lastCounterValue: Float = -1f
    private var lastCounterTimeMs: Long = 0L

    private val listener = object : SensorEventListener {
        override fun onSensorChanged(event: SensorEvent) {
            val nowMs = System.currentTimeMillis()
            when (event.sensor.type) {
                Sensor.TYPE_STEP_DETECTOR -> {
                    // Chaque event represente 1 pas
                    recordStep(nowMs)
                }
                Sensor.TYPE_STEP_COUNTER -> {
                    val count = event.values.firstOrNull() ?: return
                    if (lastCounterValue >= 0f) {
                        val stepsDelta = (count - lastCounterValue).toInt()
                        val timeDeltaMs = nowMs - lastCounterTimeMs
                        if (stepsDelta in 1..20 && timeDeltaMs > 0) {
                            val spm = (stepsDelta.toDouble() * 60_000.0) / timeDeltaMs.toDouble()
                            if (spm in 60.0..250.0) {
                                onCadence(nowMs, spm)
                            }
                        }
                    }
                    lastCounterValue = count
                    lastCounterTimeMs = nowMs
                }
            }
        }

        override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) = Unit
    }

    private fun recordStep(nowMs: Long) {
        stepHistoryMs.addLast(nowMs)
        while (stepHistoryMs.size > 8) {
            stepHistoryMs.removeFirst()
        }
        if (stepHistoryMs.size >= 4) {
            val oldest = stepHistoryMs.first()
            val spanMs = nowMs - oldest
            val stepCount = stepHistoryMs.size - 1
            if (spanMs in 1000..8000 && stepCount > 0) {
                val spm = (stepCount.toDouble() * 60_000.0) / spanMs.toDouble()
                if (spm in 60.0..250.0) {
                    onCadence(nowMs, spm)
                }
            }
        }
    }

    fun start() {
        val targetSensor = detectorSensor ?: counterSensor ?: return
        stepHistoryMs.clear()
        lastCounterValue = -1f
        lastCounterTimeMs = 0L
        try {
            manager?.registerListener(listener, targetSensor, SensorManager.SENSOR_DELAY_GAME)
        } catch (security: SecurityException) {
            // Permission ACTIVITY_RECOGNITION refusee : pas de capteur de cadence
        }
    }

    fun stop() {
        manager?.unregisterListener(listener)
        stepHistoryMs.clear()
    }
}
