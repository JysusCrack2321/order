package com.jysusman.order.avisos

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import com.jysusman.order.MainActivity
import com.jysusman.order.R

object Canales {
    const val CANAL_TAREAS = "tareas"
    const val CANAL_RECORDATORIOS = "recordatorios"
    private const val COLOR_ACENTO = 0xFFC7A96B.toInt()

    fun asegurarCanales(context: Context) {
        val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager
            ?: return

        val tareas = NotificationChannel(
            CANAL_TAREAS,
            "Tareas",
            NotificationManager.IMPORTANCE_HIGH,
        ).apply {
            description = "Avisos de tareas por vencer y resumen del día"
        }

        val recordatorios = NotificationChannel(
            CANAL_RECORDATORIOS,
            "Recordatorios",
            NotificationManager.IMPORTANCE_HIGH,
        ).apply {
            description = "Recordatorios recurrentes de ORDER"
        }

        manager.createNotificationChannel(tareas)
        manager.createNotificationChannel(recordatorios)
    }

    fun tienePermisoNotificaciones(context: Context): Boolean {
        val permisoRuntime = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.POST_NOTIFICATIONS,
            ) == PackageManager.PERMISSION_GRANTED
        } else {
            true
        }
        return permisoRuntime && NotificationManagerCompat.from(context).areNotificationsEnabled()
    }

    fun mostrarNotificacion(
        context: Context,
        clave: String,
        titulo: String,
        cuerpo: String,
        esRecordatorio: Boolean,
    ) {
        asegurarCanales(context)
        if (!tienePermisoNotificaciones(context)) return

        val canalId = if (esRecordatorio) CANAL_RECORDATORIOS else CANAL_TAREAS
        val abrirApp = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val tapIntent = PendingIntent.getActivity(
            context,
            0,
            abrirApp,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

        val notificacion = NotificationCompat.Builder(context, canalId)
            .setSmallIcon(R.drawable.ic_notificacion_reloj)
            .setColor(COLOR_ACENTO)
            .setContentTitle(titulo)
            .setContentText(cuerpo)
            .setStyle(NotificationCompat.BigTextStyle().bigText(cuerpo))
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setContentIntent(tapIntent)
            .setAutoCancel(true)
            .build()

        runCatching {
            NotificationManagerCompat.from(context).notify(clave.hashCode(), notificacion)
        }
    }
}
