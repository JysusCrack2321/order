package com.jysusman.order.avisos

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.app.NotificationManagerCompat
import com.jysusman.order.datos.PreferenciasOrder
import com.jysusman.order.datos.PuenteOrderNativo
import com.jysusman.order.datos.Repositorio
import com.jysusman.order.datos.RutasPrivadas

class ReceptorAlarma : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            ACCION_DISPARAR_AVISO -> manejarAviso(context, intent)
            ACCION_HECHA_TAREA -> manejarMarcarHecha(context, intent)
        }
    }

    private fun manejarAviso(context: Context, intent: Intent) {
        if (!PreferenciasOrder.avisosActivos(context)) return

        val clave = intent.getStringExtra(EXTRA_CLAVE) ?: return
        val titulo = intent.getStringExtra(EXTRA_TITULO) ?: return
        val cuerpo = intent.getStringExtra(EXTRA_CUERPO).orEmpty()
        val esRecordatorio = intent.getBooleanExtra(EXTRA_ES_RECORDATORIO, false)
        val esPreguntaEvento = intent.getBooleanExtra(EXTRA_ES_PREGUNTA_EVENTO, false)
        val eventoId = intent.getStringExtra(EXTRA_EVENTO_ID)
        val tareaId = intent.getStringExtra(EXTRA_TAREA_ID)

        Canales.mostrarNotificacion(
            context = context,
            clave = clave,
            titulo = titulo,
            cuerpo = cuerpo,
            esRecordatorio = esRecordatorio,
            esPreguntaEvento = esPreguntaEvento,
            eventoId = eventoId,
            tareaId = tareaId,
        )

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

    private fun manejarMarcarHecha(context: Context, intent: Intent) {
        val tareaId = intent.getStringExtra(EXTRA_TAREA_ID)?.takeIf { it.isNotBlank() } ?: return
        val clave = intent.getStringExtra(EXTRA_CLAVE)
        if (!clave.isNullOrBlank()) {
            NotificationManagerCompat.from(context).cancel(clave.hashCode())
        }

        val pendingResult = goAsync()
        Thread {
            try {
                runCatching {
                    val puente = PuenteOrderNativo()
                    val carpeta = PreferenciasOrder.carpetaDatos(context)
                        ?.takeIf { it.isNotBlank() }
                        ?: puente.carpetaPorDefecto()
                    val rutaDatos = Repositorio.rutaEnCarpeta(carpeta)
                    puente.marcarTarea(rutaDatos, tareaId, true)
                    Programador.crear(context.applicationContext).reprogramar()
                }.onFailure { err ->
                    Log.w("ORDER", "No se pudo marcar tarea desde aviso: ${err.javaClass.simpleName}: ${err.message ?: "sin detalle"}")
                }
            } finally {
                pendingResult.finish()
            }
        }.start()
    }

    companion object {
        const val ACCION_DISPARAR_AVISO = "com.jysusman.order.DISPARAR_AVISO"
        const val ACCION_HECHA_TAREA = "com.jysusman.order.HECHA_TAREA_DESDE_EVENTO"
        const val EXTRA_CLAVE = "clave"
        const val EXTRA_TITULO = "titulo"
        const val EXTRA_CUERPO = "cuerpo"
        const val EXTRA_ES_RECORDATORIO = "es_recordatorio"
        const val EXTRA_ES_PREGUNTA_EVENTO = "es_pregunta_evento"
        const val EXTRA_EVENTO_ID = "evento_id"
        const val EXTRA_TAREA_ID = "tarea_id"
    }
}
