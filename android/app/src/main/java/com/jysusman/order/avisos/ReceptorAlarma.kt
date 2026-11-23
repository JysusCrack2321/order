package com.jysusman.order.avisos

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import com.jysusman.order.datos.PreferenciasOrder
import com.jysusman.order.datos.PuenteOrderNativo
import com.jysusman.order.datos.RutasPrivadas

class ReceptorAlarma : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != ACCION_DISPARAR_AVISO) return
        if (!PreferenciasOrder.avisosActivos(context)) return

        val clave = intent.getStringExtra(EXTRA_CLAVE) ?: return
        val titulo = intent.getStringExtra(EXTRA_TITULO) ?: return
        val cuerpo = intent.getStringExtra(EXTRA_CUERPO).orEmpty()
        val esRecordatorio = intent.getBooleanExtra(EXTRA_ES_RECORDATORIO, false)

        Canales.mostrarNotificacion(context, clave, titulo, cuerpo, esRecordatorio)

        val pendingResult = goAsync()
        Thread {
            try {
                runCatching {
                    PuenteOrderNativo().anotarAviso(RutasPrivadas.registroAvisos(context), clave)
                }.onFailure { err ->
                    Log.w("ORDER", "No se pudo anotar aviso enviado: ${err.javaClass.simpleName}: ${err.message ?: "sin detalle"}")
                }
            } finally {
                pendingResult.finish()
            }
        }.start()
    }

    companion object {
        const val ACCION_DISPARAR_AVISO = "com.jysusman.order.DISPARAR_AVISO"
        const val EXTRA_CLAVE = "clave"
        const val EXTRA_TITULO = "titulo"
        const val EXTRA_CUERPO = "cuerpo"
        const val EXTRA_ES_RECORDATORIO = "es_recordatorio"
    }
}
