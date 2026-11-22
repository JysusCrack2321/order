package com.jysusman.order.datos

import com.jysusman.order.ffi.AjustesFormFfi
import com.jysusman.order.ffi.AnioMesFfi
import com.jysusman.order.ffi.AvisoFfi
import com.jysusman.order.ffi.AvisoProgramableFfi
import com.jysusman.order.ffi.CalendarioMesFfi
import com.jysusman.order.ffi.ClaseFormFfi
import com.jysusman.order.ffi.ConflictoFfi
import com.jysusman.order.ffi.DiaFfi
import com.jysusman.order.ffi.DiaVistaFfi
import com.jysusman.order.ffi.ErrorFfi
import com.jysusman.order.ffi.FiltroPendientesFfi
import com.jysusman.order.ffi.Hora12Ffi
import com.jysusman.order.ffi.InicioVistaFfi
import com.jysusman.order.ffi.MateriaConTonoFfi
import com.jysusman.order.ffi.PendientesVistaFfi
import com.jysusman.order.ffi.RecordatorioFormFfi
import com.jysusman.order.ffi.ResumenArchivoFfi
import com.jysusman.order.ffi.SemanaVistaFfi
import com.jysusman.order.ffi.TareaFfi
import com.jysusman.order.ffi.TareaFormFfi
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class RepositorioTest {

    @Test
    fun cargaInicialYRecargaConservaVistasAnteError() = runTest {
        val fake = FakePuenteOrder()
        val repo = Repositorio(
            puente = fake,
            carpetaInicial = "/tmp/order-test",
            dispatcherIo = UnconfinedTestDispatcher(testScheduler),
            asegurarCarpeta = {},
        )

        repo.cargarInicial()
        val estado1 = repo.estado.value
        assertNull(estado1.error)
        assertNotNull(estado1.inicio)
        assertEquals(2u, estado1.resumen?.numTareas)
        assertEquals(1, estado1.inicio?.deHoy?.size)

        // Simulamos un archivo roto escrito por Syncthing.
        fake.fallarAlCargar = ErrorFfi.Datos(linea = 4u, mensaje = "hora no válida")
        fake.conflictos = listOf(
            ConflictoFfi(
                ruta = "/tmp/order-test/order.sync-conflict-20261122-010000-AA.toml",
                mensaje = "Hay una copia en conflicto (22 nov · 1:00 a. m.)",
            )
        )

        repo.recargar()
        val estado2 = repo.estado.value
        assertTrue(estado2.error?.contains("línea 4") == true)
        assertNotNull("Las vistas válidas anteriores sobreviven", estado2.inicio)
        assertEquals(1, estado2.conflictos.size)

        // Resolvemos el conflicto y restauramos el archivo válido.
        fake.fallarAlCargar = null
        repo.resolverConflicto(usarOtra = false)
        val estado3 = repo.estado.value
        assertNull(estado3.error)
        assertTrue(estado3.conflictos.isEmpty())
    }

    @Test
    fun cambiarCarpetaActualizaRutaYRecarga() = runTest {
        val fake = FakePuenteOrder()
        val repo = Repositorio(
            puente = fake,
            carpetaInicial = "/tmp/uno",
            dispatcherIo = UnconfinedTestDispatcher(testScheduler),
            asegurarCarpeta = {},
        )

        repo.cargarInicial()
        assertEquals("/tmp/uno/order.toml", repo.estado.value.rutaArchivo)

        repo.cambiarCarpeta("/tmp/dos/")
        assertEquals("/tmp/dos", repo.estado.value.carpetaDatos)
        assertEquals("/tmp/dos/order.toml", repo.estado.value.rutaArchivo)
        assertEquals("/tmp/dos/order.toml", fake.ultimaRutaCargada)
    }
}

private class FakePuenteOrder : PuenteOrder {
    var fallarAlCargar: ErrorFfi? = null
    var conflictos: List<ConflictoFfi> = emptyList()
    var advertencias: List<String> = emptyList()
    var ultimaRutaCargada: String = ""

    private val tareaEjemplo = TareaFfi(
        id = "t-1",
        titulo = "Taller",
        materia = "Control",
        fechaTexto = "dom 22 nov · 11:59 p. m.",
        fechaIso = "2026-11-22",
        horaTexto = "11:59 p. m.",
        vencida = false,
        hecha = false,
        tono = 3,
    )

    override fun rutaPorDefecto(): String = "/storage/emulated/0/ORDER/order.toml"
    override fun carpetaPorDefecto(): String = "/storage/emulated/0/ORDER"

    override fun cargar(ruta: String): ResumenArchivoFfi {
        ultimaRutaCargada = ruta
        fallarAlCargar?.let { throw it }
        return ResumenArchivoFfi(
            ruta = ruta,
            numClases = 1u,
            numTareas = 2u,
            numTareasPendientes = 1u,
            numRecordatorios = 1u,
            advertencias = advertencias,
            conflictos = conflictos,
            pie = "Hoy: 1 tarea pendiente",
        )
    }

    override fun archivar(ruta: String): ResumenArchivoFfi = cargar(ruta)

    override fun detectarConflictos(ruta: String): List<ConflictoFfi> = conflictos

    override fun resolverConflicto(ruta: String, rutaConflicto: String, usarOtra: Boolean) {
        conflictos = conflictos.filterNot { it.ruta == rutaConflicto }
    }

    override fun vistaInicio(ruta: String): InicioVistaFfi = InicioVistaFfi(
        fechaLarga = "domingo 22 de noviembre",
        clases = emptyList(),
        siguiente = null,
        vencidas = emptyList(),
        deHoy = listOf(tareaEjemplo),
        proximas = emptyList(),
        recordatorios = emptyList(),
        porMateria = emptyList(),
        pie = "Hoy: 1 tarea pendiente",
    )

    override fun vistaDia(ruta: String, fechaIso: String?): DiaVistaFfi = DiaVistaFfi(
        fechaIso = fechaIso ?: "2026-11-22",
        etiqueta = "domingo 22 de noviembre",
        esHoy = true,
        clases = emptyList(),
        tareas = listOf(tareaEjemplo),
        recordatorios = emptyList(),
    )

    override fun vistaSemana(ruta: String, lunesIso: String?): SemanaVistaFfi = SemanaVistaFfi(
        lunesIso = lunesIso ?: "2026-11-16",
        etiqueta = "16 – 20 de noviembre de 2026",
        esActual = true,
        baseHora = 7,
        finHora = 19,
        etiquetasHora = listOf("7 a. m.", "8 a. m."),
        mostrarAhora = false,
        ahoraMin = 600,
        hayTareas = true,
        dias = emptyList(),
    )

    override fun vistaPendientes(ruta: String, filtro: FiltroPendientesFfi): PendientesVistaFfi =
        PendientesVistaFfi(
            tareas = listOf(tareaEjemplo),
            resumen = "1 tarea pendiente",
            filtroActivo = false,
            filtroDesdeTexto = "",
            filtroHastaTexto = "",
            filtroHoraDesdeTexto = "",
            filtroHoraHastaTexto = "",
        )

    override fun materiasConTono(ruta: String): List<MateriaConTonoFfi> =
        listOf(MateriaConTonoFfi("Control", 3))

    override fun canonicaMateria(ruta: String, texto: String): String = texto.trim()

    override fun obtenerTareaForm(ruta: String, id: String?): TareaFormFfi =
        TareaFormFfi("", "", "", "2026-11-29", "11:59 p. m.", false)

    override fun obtenerClaseForm(ruta: String, id: String?): ClaseFormFfi =
        ClaseFormFfi("", "", listOf(DiaFfi.LUN), "7:00 a. m.", "9:00 a. m.", "")

    override fun obtenerRecordatorioForm(ruta: String, id: String?): RecordatorioFormFfi =
        RecordatorioFormFfi("", "", emptyList(), "6:00 p. m.")

    override fun obtenerAjustesForm(ruta: String): AjustesFormFfi =
        AjustesFormFfi(false, "2026-08-03", "2026-12-05", "8:00 p. m.", "3", emptyList())

    override fun guardarTarea(
        ruta: String,
        id: String?,
        titulo: String,
        fechaIso: String,
        hora: String,
        materia: String,
        hecha: Boolean,
    ): ResumenArchivoFfi = cargar(ruta)

    override fun marcarTarea(ruta: String, id: String, hecha: Boolean): ResumenArchivoFfi =
        cargar(ruta)

    override fun borrarTarea(ruta: String, id: String): ResumenArchivoFfi = cargar(ruta)

    override fun guardarClase(
        ruta: String,
        id: String?,
        materia: String,
        dias: List<DiaFfi>,
        horaInicio: String,
        horaFin: String,
        lugar: String,
    ): ResumenArchivoFfi = cargar(ruta)

    override fun borrarClase(ruta: String, id: String): ResumenArchivoFfi = cargar(ruta)

    override fun guardarRecordatorio(
        ruta: String,
        id: String?,
        texto: String,
        hora: String,
        dias: List<DiaFfi>,
    ): ResumenArchivoFfi = cargar(ruta)

    override fun borrarRecordatorio(ruta: String, id: String): ResumenArchivoFfi = cargar(ruta)

    override fun guardarAjustes(
        ruta: String,
        sinSemestre: Boolean,
        inicioIso: String,
        finIso: String,
        horaAviso: String,
        horasAntes: String,
    ): ResumenArchivoFfi = cargar(ruta)

    override fun fechaHoyIso(): String = "2026-11-22"
    override fun lunesActualIso(): String = "2026-11-16"
    override fun sumarDiasIso(fechaIso: String, delta: Int): String = "2026-11-23"
    override fun calendarioMes(anio: Int, mes: Int, elegido: Int): CalendarioMesFfi =
        CalendarioMesFfi(anio, mes.toUInt(), "noviembre de 2026", emptyList())

    override fun mesVecino(anio: Int, mes: Int, delta: Int): AnioMesFfi =
        AnioMesFfi(anio, mes.toUInt())

    override fun diasDelMes(anio: Int, mes: Int): Int = 30
    override fun horas12Texto(): List<String> = (1..12).map { it.toString() }
    override fun minutos60Texto(): List<String> = (0..59).map { "%02d".format(it) }
    override fun formatearHora12(hora12: Int, minuto: Int, tarde: Boolean): String = "7:00 a. m."
    override fun parsearHora12(texto: String): Hora12Ffi = Hora12Ffi(7u, 0u, false, "7:00 a. m.")
    override fun diasSemanaEtiquetas(): List<String> =
        listOf("Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom")

    override fun proximosAvisos(
        ruta: String,
        desdeIso: String,
        hastaIso: String,
        registroRuta: String,
    ): List<AvisoProgramableFfi> = emptyList()

    override fun anotarAviso(registroRuta: String, clave: String) {}

    override fun resumenDelDia(ruta: String, ahoraIso: String, registroRuta: String): AvisoFfi? =
        null
}
