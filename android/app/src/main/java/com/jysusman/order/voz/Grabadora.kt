package com.jysusman.order.voz

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import androidx.core.content.ContextCompat
import java.util.concurrent.atomic.AtomicBoolean

class Grabadora {
    private val grabando = AtomicBoolean(false)
    private var audioRecord: AudioRecord? = null
    private var hiloLectura: Thread? = null
    private val muestras = ArrayList<Short>()
    private var inicioMs: Long = 0L

    fun estaGrabando(): Boolean = grabando.get()

    fun segundosTranscurridos(): Long {
        if (!grabando.get()) return 0L
        return ((System.currentTimeMillis() - inicioMs).coerceAtLeast(0L)) / 1000L
    }

    @SuppressLint("MissingPermission")
    fun empezar(context: Context): Result<Unit> {
        if (grabando.get()) return Result.success(Unit)
        if (!tienePermisoMicrofono(context)) {
            return Result.failure(IllegalStateException("Permiso de micrófono denegado."))
        }

        val minBuf = AudioRecord.getMinBufferSize(
            TASA_MUESTREO,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_16BIT,
        )
        if (minBuf <= 0) {
            return Result.failure(IllegalStateException("El dispositivo no soporta grabación a 16 kHz."))
        }
        val tamanoBuffer = maxOf(minBuf, 4096)

        val recorder = try {
            AudioRecord(
                MediaRecorder.AudioSource.MIC,
                TASA_MUESTREO,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
                tamanoBuffer,
            )
        } catch (e: SecurityException) {
            return Result.failure(IllegalStateException("Permiso de micrófono denegado."))
        } catch (e: IllegalArgumentException) {
            return Result.failure(IllegalStateException("No se pudo abrir el micrófono."))
        }

        if (recorder.state != AudioRecord.STATE_INITIALIZED) {
            recorder.release()
            return Result.failure(IllegalStateException("No se pudo inicializar el micrófono."))
        }

        synchronized(muestras) {
            muestras.clear()
        }

        try {
            recorder.startRecording()
        } catch (e: IllegalStateException) {
            recorder.release()
            return Result.failure(IllegalStateException("No se pudo empezar a grabar."))
        }

        audioRecord = recorder
        inicioMs = System.currentTimeMillis()
        grabando.set(true)

        val bloque = ShortArray(tamanoBuffer / 2)
        hiloLectura = Thread {
            while (grabando.get()) {
                val leidos = recorder.read(bloque, 0, bloque.size)
                if (leidos > 0) {
                    synchronized(muestras) {
                        for (i in 0 until leidos) {
                            muestras.add(bloque[i])
                        }
                    }
                }
            }
        }.apply {
            name = "order-grabadora"
            start()
        }

        return Result.success(Unit)
    }

    fun detener(): List<Short> {
        if (!grabando.getAndSet(false)) {
            return emptyList()
        }
        val recorder = audioRecord
        audioRecord = null
        runCatching { recorder?.stop() }
        runCatching { hiloLectura?.join(500) }
        hiloLectura = null
        runCatching { recorder?.release() }

        return synchronized(muestras) {
            val copia = muestras.toList()
            muestras.clear()
            copia
        }
    }

    fun cancelar() {
        detener()
    }

    companion object {
        const val TASA_MUESTREO = 16_000

        fun tienePermisoMicrofono(context: Context): Boolean =
            ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.RECORD_AUDIO,
            ) == PackageManager.PERMISSION_GRANTED

        fun formatoTiempo(segundos: Long): String {
            val min = segundos / 60L
            val seg = segundos % 60L
            return "%d:%02d".format(min, seg)
        }
    }
}
