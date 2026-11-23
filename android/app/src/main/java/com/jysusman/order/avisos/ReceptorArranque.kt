package com.jysusman.order.avisos

import android.app.AlarmManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

class ReceptorArranque : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val accion = intent.action ?: return
        if (accion != Intent.ACTION_BOOT_COMPLETED &&
            accion != Intent.ACTION_MY_PACKAGE_REPLACED &&
            accion != AlarmManager.ACTION_SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED
        ) {
            return
        }

        val pendingResult = goAsync()
        val appCtx = context.applicationContext
        Thread {
            try {
                Canales.asegurarCanales(appCtx)
                runCatching { Programador.crear(appCtx).reprogramar() }
                    .onFailure { err ->
                        Log.w("ORDER", "Error al reprogramar avisos en arranque: ${err.javaClass.simpleName}: ${err.message ?: "sin detalle"}")
                    }
                runCatching { TrabajoRespaldo.programar(appCtx) }
                    .onFailure { err ->
                        Log.w("ORDER", "Error al programar respaldo en arranque: ${err.javaClass.simpleName}: ${err.message ?: "sin detalle"}")
                    }
            } finally {
                pendingResult.finish()
            }
        }.start()
    }
}
