package com.jysusman.order.avisos

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.util.Log
import com.jysusman.order.datos.PreferenciasOrder
import com.jysusman.order.datos.PuenteOrder
import com.jysusman.order.datos.PuenteOrderNativo
import com.jysusman.order.datos.Repositorio
import com.jysusman.order.datos.RutasPrivadas
import com.jysusman.order.ffi.AvisoProgramableFfi
import java.time.LocalDateTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

private const val TAG = "ORDER"

interface DespachadorAlarmas {
    fun puedeExactas(): Boolean
    fun programarExacta(clave: String, triggerAtMillis: Long, aviso: AvisoProgramableFfi)
    fun programarInexacta(clave: String, triggerAtMillis: Long, aviso: AvisoProgramableFfi)
    fun cancelar(clave: String)
}

interface AlmacenClaves {
    fun leer(): Set<String>
    fun guardar(claves: Set<String>)
}

class DespachadorAlarmasAndroid(
    private val context: Context,
) : DespachadorAlarmas {
    private val alarmManager = context.getSystemService(Context.ALARM_SERVICE) as AlarmManager

    override fun puedeExactas(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            alarmManager.canScheduleExactAlarms()
        } else {
            true
        }
    }

    override fun programarExacta(clave: String, triggerAtMillis: Long, aviso: AvisoProgramableFfi) {
        val pi = crearPendingIntent(clave, aviso, PendingIntent.FLAG_UPDATE_CURRENT) ?: return
        try {
            alarmManager.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, triggerAtMillis, pi)
        } catch (err: SecurityException) {
            Log.w(TAG, "Sin permiso de alarma exacta; se programa inexacta (${err.javaClass.simpleName})")
            alarmManager.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, triggerAtMillis, pi)
        }
    }

    override fun programarInexacta(clave: String, triggerAtMillis: Long, aviso: AvisoProgramableFfi) {
        val pi = crearPendingIntent(clave, aviso, PendingIntent.FLAG_UPDATE_CURRENT) ?: return
        alarmManager.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, triggerAtMillis, pi)
    }

    override fun cancelar(clave: String) {
        val intent = crearIntent(clave, null)
        val pi = PendingIntent.getBroadcast(
            context,
            clave.hashCode(),
            intent,
            PendingIntent.FLAG_NO_CREATE or PendingIntent.FLAG_IMMUTABLE,
        )
        if (pi != null) {
            alarmManager.cancel(pi)
            pi.cancel()
        }
    }

    private fun crearPendingIntent(
        clave: String,
        aviso: AvisoProgramableFfi,
        flags: Int,
    ): PendingIntent? = PendingIntent.getBroadcast(
        context,
        clave.hashCode(),
        crearIntent(clave, aviso),
        flags or PendingIntent.FLAG_IMMUTABLE,
    )

    internal fun crearIntent(clave: String, aviso: AvisoProgramableFfi?): Intent {
        return Intent(context, ReceptorAlarma::class.java).apply {
            action = ReceptorAlarma.ACCION_DISPARAR_AVISO
            data = Uri.parse("order://aviso/${Uri.encode(clave)}")
            if (aviso != null) {
                putExtra(ReceptorAlarma.EXTRA_CLAVE, aviso.clave)
                putExtra(ReceptorAlarma.EXTRA_TITULO, aviso.titulo)
                putExtra(ReceptorAlarma.EXTRA_CUERPO, aviso.cuerpo)
                putExtra(ReceptorAlarma.EXTRA_ES_RECORDATORIO, aviso.esRecordatorio)
                putExtra(ReceptorAlarma.EXTRA_ES_PREGUNTA_EVENTO, aviso.esPreguntaEvento)
                aviso.eventoId?.let { putExtra(ReceptorAlarma.EXTRA_EVENTO_ID, it) }
                aviso.tareaId?.let { putExtra(ReceptorAlarma.EXTRA_TAREA_ID, it) }
            }
        }
    }
}

class AlmacenClavesPrefs(
    private val context: Context,
) : AlmacenClaves {
    override fun leer(): Set<String> = PreferenciasOrder.clavesProgramadas(context)
    override fun guardar(claves: Set<String>) = PreferenciasOrder.guardarClavesProgramadas(context, claves)
}

class Programador(
    private val puente: PuenteOrder,
    private val despachador: DespachadorAlarmas,
    private val almacenClaves: AlmacenClaves,
    private val rutaDatos: () -> String,
    private val rutaRegistro: () -> String,
    private val avisosActivos: () -> Boolean,
    private val relojAhora: () -> LocalDateTime = { LocalDateTime.now() },
    private val zona: () -> ZoneId = { ZoneId.systemDefault() },
    private val registrarAviso: (String) -> Unit = { msg -> runCatching { Log.w(TAG, msg) } },
) {
    /**
     * Recalcula los próximos avisos (ventana de 48 h), cancela las alarmas
     * anteriores y programa una alarma por cada clave futura.
     */
    @Synchronized
    fun reprogramar(): Int {
        val anteriores = almacenClaves.leer()
        for (clave in anteriores) {
            runCatching { despachador.cancelar(clave) }
                .onFailure { err ->
                    registrarAviso("No se pudo cancelar alarma previa: ${err.javaClass.simpleName}: ${err.message ?: "sin detalle"}")
                }
        }

        if (!avisosActivos()) {
            almacenClaves.guardar(emptySet())
            return 0
        }

        val ahora = relojAhora()
        val desdeIso = ahora.format(FORMATO_ISO_SEGUNDOS)
        val hastaIso = ahora.plusHours(VENTANA_HORAS).format(FORMATO_ISO_SEGUNDOS)

        val avisos = runCatching {
            puente.proximosAvisos(rutaDatos(), desdeIso, hastaIso, rutaRegistro())
        }.getOrElse { err ->
            registrarAviso("No se pudieron consultar próximos avisos (${err.javaClass.simpleName}): ${err.message ?: "sin detalle"}")
            almacenClaves.guardar(emptySet())
            return 0
        }

        val zoneId = zona()
        val ahoraMillis = ahora.atZone(zoneId).toInstant().toEpochMilli()
        val nuevas = linkedSetOf<String>()
        val usarExactas = despachador.puedeExactas()

        for (aviso in avisos) {
            val momento = runCatching { LocalDateTime.parse(aviso.momentoIso, FORMATO_ISO_LECTURA) }
                .onFailure { err ->
                    registrarAviso("Momento de aviso inválido (${aviso.momentoIso}): ${err.javaClass.simpleName}")
                }
                .getOrNull()
                ?: continue
            val triggerAtMillis = momento.atZone(zoneId).toInstant().toEpochMilli()
            if (triggerAtMillis <= ahoraMillis) continue

            val programado = runCatching {
                if (usarExactas) {
                    despachador.programarExacta(aviso.clave, triggerAtMillis, aviso)
                } else {
                    despachador.programarInexacta(aviso.clave, triggerAtMillis, aviso)
                }
            }.onFailure { err ->
                registrarAviso("No se pudo programar alarma (${err.javaClass.simpleName}): ${err.message ?: "sin detalle"}")
            }.isSuccess
            if (programado) {
                nuevas.add(aviso.clave)
            }
        }

        almacenClaves.guardar(nuevas)
        return nuevas.size
    }

    companion object {
        private const val VENTANA_HORAS = 48L
        private val FORMATO_ISO_SEGUNDOS: DateTimeFormatter =
            DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss", Locale.ROOT)
        private val FORMATO_ISO_LECTURA: DateTimeFormatter = DateTimeFormatter.ISO_LOCAL_DATE_TIME

        fun crear(context: Context): Programador {
            val appCtx = context.applicationContext
            val puente = PuenteOrderNativo()
            return Programador(
                puente = puente,
                despachador = DespachadorAlarmasAndroid(appCtx),
                almacenClaves = AlmacenClavesPrefs(appCtx),
                rutaDatos = {
                    val carpeta = PreferenciasOrder.carpetaDatos(appCtx)
                        ?.takeIf { it.isNotBlank() }
                        ?: puente.carpetaPorDefecto()
                    Repositorio.rutaEnCarpeta(carpeta)
                },
                rutaRegistro = { RutasPrivadas.registroAvisos(appCtx) },
                avisosActivos = { PreferenciasOrder.avisosActivos(appCtx) },
                registrarAviso = { msg -> Log.w(TAG, msg) },
            )
        }
    }
}
