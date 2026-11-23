package com.jysusman.order.avisos

import android.content.Context
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.jysusman.order.datos.PreferenciasOrder
import com.jysusman.order.datos.PuenteOrderNativo
import com.jysusman.order.datos.Repositorio
import com.jysusman.order.datos.RutasPrivadas
import java.util.concurrent.TimeUnit

class TrabajoRespaldo(
    appContext: Context,
    params: WorkerParameters,
) : CoroutineWorker(appContext, params) {

    override suspend fun doWork(): Result {
        Canales.asegurarCanales(applicationContext)
        val puente = PuenteOrderNativo()
        val rutaMoodle = RutasPrivadas.archivoMoodleConfig(applicationContext)
        val rutaCal = RutasPrivadas.archivoCalendariosConfig(applicationContext)
        val rutaSync = RutasPrivadas.archivoSyncConfig(applicationContext)
        val conectado = runCatching { puente.moodleEstado(rutaMoodle).conectado }.getOrDefault(false)
        val hayCals = runCatching { puente.calendariosListar(rutaCal).isNotEmpty() }.getOrDefault(false)
        val carpeta = PreferenciasOrder.carpetaDatos(applicationContext)
            ?.takeIf { it.isNotBlank() }
            ?: runCatching { puente.carpetaPorDefecto() }.getOrNull().orEmpty()
        if (carpeta.isNotBlank()) {
            val rutaDatos = Repositorio.rutaEnCarpeta(carpeta)
            if (conectado) {
                runCatching { puente.moodleSincronizar(rutaDatos, rutaMoodle) }
                    .onFailure { err ->
                        Log.w(TAG, "Error en respaldo Moodle: ${err.javaClass.simpleName}: ${err.message ?: "sin detalle"}")
                    }
            }
            if (hayCals) {
                runCatching { puente.calendariosSincronizar(rutaDatos, rutaCal) }
                    .onFailure { err ->
                        Log.w(TAG, "Error en respaldo iCal: ${err.javaClass.simpleName}: ${err.message ?: "sin detalle"}")
                    }
            }
            val estabaActivo = runCatching { puente.syncEstado().activo }.getOrDefault(false)
            val iniciadoAqui = if (!estabaActivo) {
                runCatching { puente.syncIniciar(rutaDatos, rutaSync) }
                    .onFailure { err ->
                        Log.w(TAG, "No se pudo iniciar sync P2P en respaldo: ${err.javaClass.simpleName}")
                    }
                    .isSuccess
            } else {
                false
            }
            if (estabaActivo || iniciadoAqui) {
                val hayEmparejados = runCatching {
                    puente.syncEstado().totalEmparejados > 0u
                }.getOrDefault(false)
                if (hayEmparejados) {
                    runCatching { puente.syncAhora() }
                        .onFailure { err ->
                            Log.w(TAG, "Error en respaldo P2P: ${err.javaClass.simpleName}: ${err.message ?: "sin detalle"}")
                        }
                }
                if (iniciadoAqui) {
                    runCatching { puente.syncDetener() }
                        .onFailure { err ->
                            Log.w(TAG, "No se pudo detener sync P2P en respaldo: ${err.javaClass.simpleName}")
                        }
                }
            }
        } else {
            Log.w(TAG, "Respaldo sin carpeta de datos disponible; se intenta reprogramar con ruta por defecto")
        }
        Programador.crear(applicationContext).reprogramar()
        return Result.success()
    }

    companion object {
        private const val TAG = "ORDER"
        private const val NOMBRE_TRABAJO = "order_reprogramar_avisos"

        fun programar(context: Context) {
            val request = PeriodicWorkRequestBuilder<TrabajoRespaldo>(6, TimeUnit.HOURS).build()
            WorkManager.getInstance(context.applicationContext).enqueueUniquePeriodicWork(
                NOMBRE_TRABAJO,
                ExistingPeriodicWorkPolicy.KEEP,
                request,
            )
        }
    }
}
