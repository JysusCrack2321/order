package com.jysusman.order.avisos

import android.app.AlarmManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

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
                Programador.crear(appCtx).reprogramar()
                TrabajoRespaldo.programar(appCtx)
            } finally {
                pendingResult.finish()
            }
        }.start()
    }
}
