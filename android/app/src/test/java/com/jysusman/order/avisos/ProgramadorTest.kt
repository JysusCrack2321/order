package com.jysusman.order.avisos

import com.jysusman.order.datos.PuenteOrder
import com.jysusman.order.ffi.AdjuntoFfi
import com.jysusman.order.ffi.AgenteConfigFfi
import com.jysusman.order.ffi.AjustesFormFfi
import com.jysusman.order.ffi.AnioMesFfi
import com.jysusman.order.ffi.AvisoFfi
import com.jysusman.order.ffi.AvisoProgramableFfi
import com.jysusman.order.ffi.CalendarioMesFfi
import com.jysusman.order.ffi.ClaseFormFfi
import com.jysusman.order.ffi.ConflictoFfi
import com.jysusman.order.ffi.DiaFfi
import com.jysusman.order.ffi.DiaVistaFfi
import com.jysusman.order.ffi.EstadoMoodleFfi
import com.jysusman.order.ffi.FiltroPendientesFfi
import com.jysusman.order.ffi.Hora12Ffi
import com.jysusman.order.ffi.InicioVistaFfi
import com.jysusman.order.ffi.LimitesAdjuntosFfi
import com.jysusman.order.ffi.MateriaConTonoFfi
import com.jysusman.order.ffi.PendientesVistaFfi
import com.jysusman.order.ffi.ProveedorInfoFfi
import com.jysusman.order.ffi.RecordatorioFormFfi
import com.jysusman.order.ffi.RespuestaAgenteFfi
import com.jysusman.order.ffi.ResultadoAplicarTodasFfi
import com.jysusman.order.ffi.ResultadoConexionFfi
import com.jysusman.order.ffi.ResumenArchivoFfi
import com.jysusman.order.ffi.ResumenSyncFfi
import com.jysusman.order.ffi.SemanaVistaFfi
import com.jysusman.order.ffi.TareaFormFfi
import com.jysusman.order.ffi.TextosConexionAgenteFfi
import com.jysusman.order.ffi.VozProveedorInfoFfi
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDateTime
import java.time.ZoneId

class ProgramadorTest {

    @Test
    fun reprogramarCancelaAnterioresYProgramaPorClave() {
        val puente = FakePuenteAvisos(
            avisos = listOf(
                AvisoProgramableFfi(
                    clave = "t:t-1:dia:2026-11-28",
                    momentoIso = "2026-11-27T20:00:00",
                    titulo = "Vence mañana: Parcial",
                    cuerpo = "Control · vence el 28/11 a las 11:59 p. m.",
                    esRecordatorio = false,
                ),
                AvisoProgramableFfi(
                    clave = "r:r-1:2026-11-28",
                    momentoIso = "2026-11-28T18:00:00",
                    titulo = "Revisar Moodle",
                    cuerpo = "Recordatorio de las 6:00 p. m.",
                    esRecordatorio = true,
                ),
                // Aviso pasado: no debe programarse.
                AvisoProgramableFfi(
                    clave = "t:t-vieja:dia:2026-11-26",
                    momentoIso = "2026-11-26T20:00:00",
                    titulo = "Pasada",
                    cuerpo = "No debe sonar",
                    esRecordatorio = false,
                ),
            )
        )
        val despachador = FakeDespachadorAlarmas(exactas = true)
        val almacen = FakeAlmacenClaves(mutableSetOf("clave-vieja-1", "t:t-1:dia:2026-11-28"))
        val ahora = LocalDateTime.of(2026, 11, 27, 8, 0, 0)
        var activos = true

        val programador = Programador(
            puente = puente,
            despachador = despachador,
            almacenClaves = almacen,
            rutaDatos = { "/tmp/order.toml" },
            rutaRegistro = { "/tmp/enviados.txt" },
            avisosActivos = { activos },
            relojAhora = { ahora },
            zona = { ZoneId.of("UTC") },
        )

        val total = programador.reprogramar()
        assertEquals(2, total)
        assertEquals(listOf("clave-vieja-1", "t:t-1:dia:2026-11-28"), despachador.canceladas)
        assertEquals(
            listOf("t:t-1:dia:2026-11-28", "r:r-1:2026-11-28"),
            despachador.exactasProgramadas.map { it.first },
        )
        assertTrue(despachador.inexactasProgramadas.isEmpty())
        assertEquals(setOf("t:t-1:dia:2026-11-28", "r:r-1:2026-11-28"), almacen.leer())
        assertEquals("2026-11-27T08:00:00", puente.ultimoDesde)
        assertEquals("2026-11-29T08:00:00", puente.ultimoHasta)

        // Sin permiso de alarma exacta usa respaldo inexacto.
        despachador.exactas = false
        despachador.limpiar()
        val totalInexactas = programador.reprogramar()
        assertEquals(2, totalInexactas)
        assertEquals(2, despachador.canceladas.size)
        assertTrue(despachador.exactasProgramadas.isEmpty())
        assertEquals(2, despachador.inexactasProgramadas.size)

        // Con interruptor apagado cancela todas y no programa ninguna.
        activos = false
        despachador.limpiar()
        val totalApagado = programador.reprogramar()
        assertEquals(0, totalApagado)
        assertEquals(setOf("t:t-1:dia:2026-11-28", "r:r-1:2026-11-28"), despachador.canceladas.toSet())
        assertTrue(almacen.leer().isEmpty())
        assertTrue(despachador.exactasProgramadas.isEmpty())
        assertTrue(despachador.inexactasProgramadas.isEmpty())
    }
}

private class FakeDespachadorAlarmas(var exactas: Boolean) : DespachadorAlarmas {
    val canceladas = mutableListOf<String>()
    val exactasProgramadas = mutableListOf<Pair<String, Long>>()
    val inexactasProgramadas = mutableListOf<Pair<String, Long>>()

    override fun puedeExactas(): Boolean = exactas

    override fun programarExacta(clave: String, triggerAtMillis: Long, aviso: AvisoProgramableFfi) {
        exactasProgramadas.add(clave to triggerAtMillis)
    }

    override fun programarInexacta(clave: String, triggerAtMillis: Long, aviso: AvisoProgramableFfi) {
        inexactasProgramadas.add(clave to triggerAtMillis)
    }

    override fun cancelar(clave: String) {
        canceladas.add(clave)
    }

    fun limpiar() {
        canceladas.clear()
        exactasProgramadas.clear()
        inexactasProgramadas.clear()
    }
}

private class FakeAlmacenClaves(
    private var claves: MutableSet<String> = mutableSetOf(),
) : AlmacenClaves {
    override fun leer(): Set<String> = claves.toSet()
    override fun guardar(claves: Set<String>) {
        this.claves = claves.toMutableSet()
    }
}

private class FakePuenteAvisos(
    private val avisos: List<AvisoProgramableFfi>,
) : PuenteOrder {
    var ultimoDesde: String = ""
    var ultimoHasta: String = ""

    override fun proximosAvisos(
        ruta: String,
        desdeIso: String,
        hastaIso: String,
        registroRuta: String,
    ): List<AvisoProgramableFfi> {
        ultimoDesde = desdeIso
        ultimoHasta = hastaIso
        return avisos
    }

    override fun anotarAviso(registroRuta: String, clave: String) {}
    override fun resumenDelDia(ruta: String, ahoraIso: String, registroRuta: String): AvisoFfi? = null

    override fun rutaPorDefecto(): String = ""
    override fun carpetaPorDefecto(): String = ""
    override fun cargar(ruta: String): ResumenArchivoFfi = throw UnsupportedOperationException()
    override fun archivar(ruta: String): ResumenArchivoFfi = throw UnsupportedOperationException()
    override fun detectarConflictos(ruta: String): List<ConflictoFfi> = emptyList()
    override fun resolverConflicto(ruta: String, rutaConflicto: String, usarOtra: Boolean) {}
    override fun vistaInicio(ruta: String): InicioVistaFfi = throw UnsupportedOperationException()
    override fun vistaDia(ruta: String, fechaIso: String?): DiaVistaFfi = throw UnsupportedOperationException()
    override fun vistaSemana(ruta: String, lunesIso: String?): SemanaVistaFfi = throw UnsupportedOperationException()
    override fun vistaPendientes(ruta: String, filtro: FiltroPendientesFfi): PendientesVistaFfi = throw UnsupportedOperationException()
    override fun materiasConTono(ruta: String): List<MateriaConTonoFfi> = emptyList()
    override fun canonicaMateria(ruta: String, texto: String): String = texto
    override fun obtenerTareaForm(ruta: String, id: String?): TareaFormFfi = throw UnsupportedOperationException()
    override fun obtenerClaseForm(ruta: String, id: String?): ClaseFormFfi = throw UnsupportedOperationException()
    override fun obtenerRecordatorioForm(ruta: String, id: String?): RecordatorioFormFfi = throw UnsupportedOperationException()
    override fun obtenerAjustesForm(ruta: String): AjustesFormFfi = throw UnsupportedOperationException()
    override fun guardarTarea(ruta: String, id: String?, titulo: String, fechaIso: String, hora: String, materia: String, hecha: Boolean): ResumenArchivoFfi = throw UnsupportedOperationException()
    override fun marcarTarea(ruta: String, id: String, hecha: Boolean): ResumenArchivoFfi = throw UnsupportedOperationException()
    override fun borrarTarea(ruta: String, id: String): ResumenArchivoFfi = throw UnsupportedOperationException()
    override fun guardarClase(ruta: String, id: String?, materia: String, dias: List<DiaFfi>, horaInicio: String, horaFin: String, lugar: String): ResumenArchivoFfi = throw UnsupportedOperationException()
    override fun borrarClase(ruta: String, id: String): ResumenArchivoFfi = throw UnsupportedOperationException()
    override fun guardarRecordatorio(ruta: String, id: String?, texto: String, hora: String, dias: List<DiaFfi>): ResumenArchivoFfi = throw UnsupportedOperationException()
    override fun borrarRecordatorio(ruta: String, id: String): ResumenArchivoFfi = throw UnsupportedOperationException()
    override fun guardarAjustes(ruta: String, sinSemestre: Boolean, inicioIso: String, finIso: String, horaAviso: String, horasAntes: String): ResumenArchivoFfi = throw UnsupportedOperationException()
    override fun fechaHoyIso(): String = ""
    override fun lunesActualIso(): String = ""
    override fun sumarDiasIso(fechaIso: String, delta: Int): String = ""
    override fun calendarioMes(anio: Int, mes: Int, elegido: Int): CalendarioMesFfi = throw UnsupportedOperationException()
    override fun mesVecino(anio: Int, mes: Int, delta: Int): AnioMesFfi = throw UnsupportedOperationException()
    override fun diasDelMes(anio: Int, mes: Int): Int = 30
    override fun horas12Texto(): List<String> = emptyList()
    override fun minutos60Texto(): List<String> = emptyList()
    override fun formatearHora12(hora12: Int, minuto: Int, tarde: Boolean): String = ""
    override fun parsearHora12(texto: String): Hora12Ffi = throw UnsupportedOperationException()
    override fun diasSemanaEtiquetas(): List<String> = emptyList()
    override fun proveedoresAgente(): List<ProveedorInfoFfi> = emptyList()
    override fun proveedoresVoz(): List<VozProveedorInfoFfi> = emptyList()
    override fun textosConexionAgente(): TextosConexionAgenteFfi = throw UnsupportedOperationException()
    override fun pareceClaveGemini(texto: String): Boolean = false
    override fun agenteTieneClave(rutaConfig: String): Boolean = false
    override fun conectarGemini(rutaConfig: String, apiKey: String): ResultadoConexionFfi = throw UnsupportedOperationException()
    override fun leerAgenteConfig(rutaConfig: String): AgenteConfigFfi = throw UnsupportedOperationException()
    override fun guardarAgenteConfig(rutaConfig: String, proveedor: Int, url: String, modelo: String, apiKey: String, veImagenes: Boolean, limiteContexto: String, vozProveedor: Int, vozUrl: String, vozModelo: String, vozKey: String): AgenteConfigFfi = throw UnsupportedOperationException()
    override fun agenteVeImagenes(rutaConfig: String): Boolean = false
    override fun textoDePdf(bytes: ByteArray): List<String> = emptyList()
    override fun paginasSinTexto(paginas: List<String>): List<UInt> = emptyList()
    override fun limitesAdjuntos(): LimitesAdjuntosFfi = throw UnsupportedOperationException()
    override fun enviarAlAgente(rutaDatos: String, rutaConfig: String, texto: String, adjuntos: List<AdjuntoFfi>): RespuestaAgenteFfi = throw UnsupportedOperationException()
    override fun aplicarAccion(rutaDatos: String, accionId: String): ResumenArchivoFfi = throw UnsupportedOperationException()
    override fun descartarAccion(accionId: String) {}
    override fun aplicarTodas(rutaDatos: String, accionIds: List<String>): ResultadoAplicarTodasFfi = throw UnsupportedOperationException()
    override fun necesitaCompactar(rutaConfig: String): Boolean = false
    override fun nuevaConversacion() {}
    override fun hayConversacion(): Boolean = false
    override fun hayConversacionEn(rutaConfig: String): Boolean = false
    override fun wavDePcm16(muestras: List<Short>, frecuencia: UInt): ByteArray = ByteArray(0)
    override fun transcribir(rutaConfig: String, wav: ByteArray): String = ""
    override fun moodleConectar(rutaConfig: String, sitio: String, usuario: String, contrasena: String): EstadoMoodleFfi = throw UnsupportedOperationException()
    override fun moodleEstado(rutaConfig: String): EstadoMoodleFfi = EstadoMoodleFfi(false, "micampus.unal.edu.co", "", "", false)
    override fun moodleSincronizar(rutaDatos: String, rutaConfig: String): ResumenSyncFfi = ResumenSyncFfi(0u, 0u, 0u)
    override fun moodleDesconectar(rutaConfig: String) {}
}
