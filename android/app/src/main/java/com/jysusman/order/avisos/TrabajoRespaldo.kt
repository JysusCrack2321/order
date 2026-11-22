package com.jysusman.order.avisos

import android.content.Context
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
        val conectado = runCatching { puente.moodleEstado(rutaMoodle).conectado }.getOrDefault(false)
        val hayCals = runCatching { puente.calendariosListar(rutaCal).isNotEmpty() }.getOrDefault(false)
        if (conectado || hayCals) {
            val carpeta = PreferenciasOrder.carpetaDatos(applicationContext)
                ?: runCatching { puente.carpetaPorDefecto() }.getOrNull().orEmpty()
            if (carpeta.isNotBlank()) {
                val rutaDatos = Repositorio.rutaEnCarpeta(carpeta)
                if (conectado) {
                    runCatching { puente.moodleSincronizar(rutaDatos, rutaMoodle) }
                }
                if (hayCals) {
                    runCatching { puente.calendariosSincronizar(rutaDatos, rutaCal) }
                }
            }
        }
        Programador.crear(applicationContext).reprogramar()
        return Result.success()
    }

    companion object {
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
