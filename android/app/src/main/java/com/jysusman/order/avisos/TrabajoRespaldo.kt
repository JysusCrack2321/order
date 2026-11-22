package com.jysusman.order.avisos

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit

class TrabajoRespaldo(
    appContext: Context,
    params: WorkerParameters,
) : CoroutineWorker(appContext, params) {

    override suspend fun doWork(): Result {
        Canales.asegurarCanales(applicationContext)
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
