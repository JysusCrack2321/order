package com.jysusman.order.datos

import com.jysusman.order.ffi.AccionVistaFfi
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
import com.jysusman.order.ffi.ErrorFfi
import com.jysusman.order.ffi.EstadoConexionFfi
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
import com.jysusman.order.ffi.TareaFfi
import com.jysusman.order.ffi.TareaFormFfi
import com.jysusman.order.ffi.TextosConexionAgenteFfi
import com.jysusman.order.ffi.VozProveedorInfoFfi
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

    @Test
    fun agenteEnviarAplicarDescartarYErrorLegible() = runTest {
        val fake = FakePuenteOrder()
        var reprogramaciones = 0
        val repo = Repositorio(
            puente = fake,
            carpetaInicial = "/tmp/order-agente",
            dispatcherIo = UnconfinedTestDispatcher(testScheduler),
            asegurarCarpeta = {},
            rutaConfigAgente = "/tmp/agente.toml",
            onDatosActualizados = { reprogramaciones++ },
        )

        repo.cargarInicial()
        val reprogramacionesBase = reprogramaciones

        // Error de red/clave se muestra en el panel del agente.
        fake.fallarAgente = ErrorFfi.Operacion("la clave del agente no es válida (401).")
        repo.actualizarTextoAgente("agrega un taller")
        repo.enviarAlAgente()
        assertTrue(repo.estado.value.agentePanelAbierto)
        assertTrue(repo.estado.value.agenteRespuesta.contains("401"))
        assertTrue(repo.estado.value.agenteAcciones.isEmpty())

        // Respuesta con dos acciones.
        fake.fallarAgente = null
        fake.respuestaAgente = RespuestaAgenteFfi(
            respuesta = "Propongo dos cambios.",
            acciones = listOf(
                AccionVistaFfi("a-1", "Crear tarea 1"),
                AccionVistaFfi("a-2", "Crear tarea 2"),
            ),
            nota = null,
        )
        repo.actualizarTextoAgente("agrega dos talleres")
        repo.enviarAlAgente()
        assertTrue(repo.estado.value.agentePanelAbierto)
        assertEquals("", repo.estado.value.agenteTexto)
        assertEquals(2, repo.estado.value.agenteAcciones.size)

        // Aplicar una acción recarga y reprograma avisos.
        repo.aplicarAccionAgente("a-1")
        assertEquals(1, repo.estado.value.agenteAcciones.size)
        assertEquals(reprogramacionesBase + 1, reprogramaciones)

        // Descartar la última cierra el panel.
        repo.descartarAccionAgente("a-2")
        assertTrue(repo.estado.value.agenteAcciones.isEmpty())
        assertTrue(!repo.estado.value.agentePanelAbierto)
    }

    @Test
    fun nuevaConversacionLlamaAlPuenteYLimpiaEstado() = runTest {
        val fake = FakePuenteOrder()
        val repo = Repositorio(
            puente = fake,
            carpetaInicial = "/tmp/order-conv",
            dispatcherIo = UnconfinedTestDispatcher(testScheduler),
            asegurarCarpeta = {},
            rutaConfigAgente = "/tmp/agente.toml",
        )
        repo.cargarInicial()
        assertTrue(!repo.estado.value.agenteHayConversacion)

        fake.respuestaAgente = RespuestaAgenteFfi(
            respuesta = "Propongo una tarea.",
            acciones = listOf(AccionVistaFfi("a-1", "Crear tarea 1")),
            nota = null,
        )
        repo.actualizarTextoAgente("pon un taller el viernes")
        repo.enviarAlAgente()
        assertTrue(repo.estado.value.agenteHayConversacion)
        assertTrue(repo.estado.value.agentePanelAbierto)

        repo.nuevaConversacion()
        assertEquals(1, fake.llamadasNuevaConversacion)
        assertTrue(!repo.estado.value.agenteHayConversacion)
        assertTrue(!repo.estado.value.agentePanelAbierto)
        assertEquals("", repo.estado.value.agenteRespuesta)
        assertTrue(repo.estado.value.agenteAcciones.isEmpty())
    }

    @Test
    fun vozGrabacionVaciaNoEnviaYTranscripcionPasaAlCampo() = runTest {
        val fake = FakePuenteOrder()
        val repo = Repositorio(
            puente = fake,
            carpetaInicial = "/tmp/order-voz",
            dispatcherIo = UnconfinedTestDispatcher(testScheduler),
            asegurarCarpeta = {},
            rutaConfigAgente = "/tmp/agente.toml",
        )

        // Grabación vacía o muy corta: mensaje legible y NO llama a transcribir.
        repo.transcribirAudio(emptyList(), 16_000)
        assertEquals(0, fake.llamadasTranscribir)
        assertTrue(repo.estado.value.agenteAvisoEsError)
        assertTrue(repo.estado.value.agenteAviso.contains("vacía"))

        repo.transcribirAudio(List(1000) { 0 }, 16_000)
        assertEquals(0, fake.llamadasTranscribir)

        // Grabación válida: convierte a WAV y coloca texto en el campo respetando lo previo.
        repo.actualizarTextoAgente("Por favor")
        fake.transcripcionTexto = "agrega taller para el viernes"
        repo.transcribirAudio(List(8000) { 100 }, 16_000)
        assertEquals(1, fake.llamadasTranscribir)
        assertEquals("Por favor agrega taller para el viernes", repo.estado.value.agenteTexto)
        assertEquals("", repo.estado.value.agenteAviso)
    }

    @Test
    fun historialGuardadoYCompactacionSeReflejanEnEstado() = runTest {
        val fake = FakePuenteOrder().apply {
            hayConversacionActiva = true
            necesitaCompactarFlag = true
        }
        var compactandoDuranteEnvio = false
        val repo = Repositorio(
            puente = fake,
            carpetaInicial = "/tmp/order-hist",
            dispatcherIo = UnconfinedTestDispatcher(testScheduler),
            asegurarCarpeta = {},
            rutaConfigAgente = "/tmp/agente.toml",
        )
        fake.alEnviar = {
            compactandoDuranteEnvio = repo.estado.value.agenteCompactando
        }

        // Tras "reiniciar", cargarInicial detecta la conversación guardada.
        repo.cargarInicial()
        assertTrue(repo.estado.value.agenteHayConversacion)

        // Al enviar con necesitaCompactar = true, activa agenteCompactando durante la llamada y lo apaga al volver.
        repo.actualizarTextoAgente("y el martes?")
        repo.enviarAlAgente()
        assertTrue(compactandoDuranteEnvio)
        assertTrue(!repo.estado.value.agenteCompactando)
        assertTrue(!repo.estado.value.agentePensando)

        // Guardar límite de contexto lo reenvía al puente.
        val err = repo.guardarAgenteConfig(0, "", "", "sk", false, "15000", 0, "", "", "")
        assertNull(err)
        assertEquals("15000", fake.ultimoLimiteGuardado)
    }

    @Test
    fun conexionGuiadaSinClavePortapapelesResumeYReenvio() = runTest {
        val fake = FakePuenteOrder().apply {
            tieneClaveFlag = false
        }
        val repo = Repositorio(
            puente = fake,
            carpetaInicial = "/tmp/order-conexion",
            dispatcherIo = UnconfinedTestDispatcher(testScheduler),
            asegurarCarpeta = {},
            rutaConfigAgente = "/tmp/agente.toml",
        )

        repo.cargarInicial()
        assertTrue(!repo.estado.value.agenteTieneClave)
        assertTrue(!repo.estado.value.conexionAbierta)

        // Al intentar enviar sin clave, abre la tarjeta y NO pierde el mensaje escrito.
        repo.actualizarTextoAgente("pon taller de control el viernes")
        repo.enviarAlAgente()
        assertTrue(repo.estado.value.conexionAbierta)
        assertEquals("pon taller de control el viernes", repo.estado.value.agenteTexto)
        assertEquals(0, fake.llamadasEnviarAgente)

        // Pegar algo que no es clave no lo mete y muestra el aviso de portapapeles.
        repo.pegarClaveDesdePortapapeles("hola mundo")
        assertEquals("", repo.estado.value.conexionKey)
        assertTrue(repo.estado.value.conexionEstadoError)
        assertEquals(
            "Lo que copiaste no parece una clave de Google.",
            repo.estado.value.conexionEstadoTexto,
        )

        // Al volver a la app (onResume) con una key AIza... en portapapeles, la ofrece.
        repo.revisarPortapapelesAlReanudar("AIzaSyPruebaResume123456")
        assertEquals("AIzaSyPruebaResume123456", repo.estado.value.conexionOfertaPortapapeles)
        repo.aceptarClaveDePortapapeles()
        assertEquals("AIzaSyPruebaResume123456", repo.estado.value.conexionKey)
        assertNull(repo.estado.value.conexionOfertaPortapapeles)

        // Clave inválida: muestra error, permite reintentar y no envía.
        fake.resultadoConexion = ResultadoConexionFfi(
            estado = EstadoConexionFfi.CLAVE_INVALIDA,
            mensaje = "Esa clave no funciona; cópiala otra vez.",
            guardada = false,
        )
        repo.conectarGemini()
        assertTrue(repo.estado.value.conexionAbierta)
        assertTrue(repo.estado.value.conexionEstadoError)
        assertEquals(
            "Esa clave no funciona; cópiala otra vez.",
            repo.estado.value.conexionEstadoTexto,
        )
        assertEquals(0, fake.llamadasEnviarAgente)

        // Sin internet: guarda la clave igual y avisa.
        fake.resultadoConexion = ResultadoConexionFfi(
            estado = EstadoConexionFfi.SIN_INTERNET,
            mensaje = "Sin internet; inténtalo luego.",
            guardada = true,
        )
        repo.conectarGemini()
        assertTrue(repo.estado.value.agenteTieneClave)
        assertTrue(repo.estado.value.conexionAbierta)
        assertEquals("Sin internet; inténtalo luego.", repo.estado.value.conexionEstadoTexto)
        assertEquals(0, fake.llamadasEnviarAgente)

        // Conexión exitosa: cierra la tarjeta y envía automáticamente el mensaje que estaba escrito.
        fake.resultadoConexion = ResultadoConexionFfi(
            estado = EstadoConexionFfi.CONECTADO,
            mensaje = "Listo, el agente está conectado.",
            guardada = true,
        )
        fake.tieneClaveFlag = true
        fake.respuestaAgente = RespuestaAgenteFfi("Taller agregado.", emptyList(), null)
        repo.conectarGemini()
        assertTrue(!repo.estado.value.conexionAbierta)
        assertTrue(repo.estado.value.agenteTieneClave)
        assertEquals(1, fake.llamadasEnviarAgente)
        assertTrue(repo.estado.value.agentePanelAbierto)
        assertEquals("Taller agregado.", repo.estado.value.agenteRespuesta)
        assertEquals("", repo.estado.value.agenteTexto)
    }

    @Test
    fun moodleConectarSincronizarSilenciosoYDesconectar() = runTest {
        val fake = FakePuenteOrder()
        var reprogramaciones = 0
        val repo = Repositorio(
            puente = fake,
            carpetaInicial = "/tmp/order-moodle",
            dispatcherIo = UnconfinedTestDispatcher(testScheduler),
            asegurarCarpeta = {},
            rutaConfigMoodle = "/tmp/moodle.toml",
            onDatosActualizados = { reprogramaciones++ },
        )

        repo.cargarInicial()
        assertEquals(false, repo.estado.value.moodleEstado?.conectado)
        val baseReprog = reprogramaciones

        // Credenciales malas dejan error visible y no conectan.
        fake.fallarMoodleConectar = ErrorFfi.Operacion("Usuario o contraseña incorrectos.")
        val okFallo = repo.conectarMoodle("micampus.unal.edu.co", "ana", "mala")
        assertEquals(false, okFallo)
        assertEquals("Usuario o contraseña incorrectos.", repo.estado.value.moodleError)
        assertEquals(false, repo.estado.value.moodleEstado?.conectado)

        // Conexión exitosa sincroniza enseguida y reprograma avisos si hubo cambios.
        fake.fallarMoodleConectar = null
        fake.resumenMoodleSync = ResumenSyncFfi(3u, 0u, 0u)
        val okBien = repo.conectarMoodle("micampus.unal.edu.co", "ana", "buena")
        assertTrue(okBien)
        assertEquals("", repo.estado.value.moodleError)
        assertTrue(repo.estado.value.moodleEstado?.conectado == true)
        assertEquals(baseReprog + 1, reprogramaciones)

        // Sincronización silenciosa sin red no ensucia banners ni errores.
        fake.fallarMoodleSync = ErrorFfi.Operacion("Sin conexión con Moodle")
        repo.sincronizarMoodle(silencioso = true)
        assertNull(repo.estado.value.error)
        assertEquals("", repo.estado.value.moodleError)

        // Token vencido activa requiereReingreso para mostrar banner suave.
        fake.moodleEstadoFake = EstadoMoodleFfi(
            conectado = true,
            sitio = "micampus.unal.edu.co",
            usuario = "Ana",
            textoEstado = "Vuelve a iniciar sesión en Moodle.",
            requiereReingreso = true,
        )
        repo.sincronizarMoodle(silencioso = true)
        assertTrue(repo.estado.value.moodleEstado?.requiereReingreso == true)

        // Desconectar limpia la conexión.
        fake.fallarMoodleSync = null
        repo.desconectarMoodle()
        assertEquals(false, repo.estado.value.moodleEstado?.conectado)
    }
}

class FakePuenteOrder : PuenteOrder {
    var fallarAlCargar: ErrorFfi? = null
    var fallarAgente: ErrorFfi? = null
    var respuestaAgente: RespuestaAgenteFfi = RespuestaAgenteFfi("Listo.", emptyList(), null)
    var llamadasTranscribir: Int = 0
    var transcripcionTexto: String = "hola"
    var veImagenes: Boolean = false
    var paginasPdfFake: List<String> = listOf("Texto página 1")
    var ultimosAdjuntosEnviados: List<AdjuntoFfi> = emptyList()
    var hayConversacionActiva: Boolean = false
    var necesitaCompactarFlag: Boolean = false
    var ultimoLimiteGuardado: String = ""
    var alEnviar: (() -> Unit)? = null
    var llamadasNuevaConversacion: Int = 0
    var llamadasEnviarAgente: Int = 0
    var tieneClaveFlag: Boolean = true
    var resultadoConexion: ResultadoConexionFfi = ResultadoConexionFfi(
        estado = EstadoConexionFfi.CONECTADO,
        mensaje = "Listo, el agente está conectado.",
        guardada = true,
    )
    var conflictos: List<ConflictoFfi> = emptyList()
    var advertencias: List<String> = emptyList()
    var ultimaRutaCargada: String = ""
    var fallarMoodleConectar: ErrorFfi? = null
    var fallarMoodleSync: ErrorFfi? = null
    var resumenMoodleSync: ResumenSyncFfi = ResumenSyncFfi(0u, 0u, 0u)
    var moodleEstadoFake: EstadoMoodleFfi = EstadoMoodleFfi(false, "micampus.unal.edu.co", "", "", false)

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
        TareaFormFfi("", "", "", "2026-11-29", "11:59 p. m.", false, "")

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

    override fun proveedoresAgente(): List<ProveedorInfoFfi> = emptyList()
    override fun proveedoresVoz(): List<VozProveedorInfoFfi> = emptyList()
    override fun textosConexionAgente(): TextosConexionAgenteFfi = TextosConexionAgenteFfi(
        titulo = "Conecta el agente gratis",
        paso1 = "1. Toca Abrir Google AI Studio y entra con tu cuenta de Google.",
        paso2 = "2. Toca Create API key y copia la clave.",
        paso3 = "3. Vuelve aquí y toca Pegar clave.",
        botonAbrirStudio = "Abrir Google AI Studio",
        urlStudio = "https://aistudio.google.com/apikey",
        botonPegar = "Pegar clave",
        etiquetaClave = "API key",
        pistaClave = "AIza…",
        botonConectar = "Conectar",
        conectando = "Conectando…",
        enlaceOtroProveedor = "Usar otro proveedor",
        portapapelesInvalido = "Lo que copiaste no parece una clave de Google.",
        claveVacia = "Pega o escribe tu clave de Google.",
        ofertaPortapapeles = "¿Usar la clave que copiaste?",
        conectado = "Listo, el agente está conectado.",
        claveInvalida = "Esa clave no funciona; cópiala otra vez.",
        sinInternet = "Sin internet; inténtalo luego.",
    )

    override fun pareceClaveGemini(texto: String): Boolean {
        val limpio = texto.trim()
        return limpio.startsWith("AIza") && limpio.length >= 10 && limpio.none { it.isWhitespace() }
    }

    override fun agenteTieneClave(rutaConfig: String): Boolean = tieneClaveFlag

    override fun conectarGemini(rutaConfig: String, apiKey: String): ResultadoConexionFfi {
        if (resultadoConexion.guardada) {
            tieneClaveFlag = true
        }
        return resultadoConexion
    }

    override fun leerAgenteConfig(rutaConfig: String): AgenteConfigFfi =
        AgenteConfigFfi(0, "https://generativelanguage.googleapis.com/v1beta/openai", "gemini-flash-latest", "", veImagenes, 60000u, 0, "https://api.openai.com/v1", "whisper-1", "")

    override fun guardarAgenteConfig(
        rutaConfig: String,
        proveedor: Int,
        url: String,
        modelo: String,
        apiKey: String,
        veImagenes: Boolean,
        limiteContexto: String,
        vozProveedor: Int,
        vozUrl: String,
        vozModelo: String,
        vozKey: String,
    ): AgenteConfigFfi {
        ultimoLimiteGuardado = limiteContexto
        return leerAgenteConfig(rutaConfig)
    }

    override fun agenteVeImagenes(rutaConfig: String): Boolean = veImagenes

    override fun textoDePdf(bytes: ByteArray): List<String> = paginasPdfFake

    override fun paginasSinTexto(paginas: List<String>): List<UInt> =
        paginas.mapIndexedNotNull { idx, texto ->
            if (texto.filterNot { it.isWhitespace() }.length < 5) idx.toUInt() else null
        }

    override fun limitesAdjuntos(): LimitesAdjuntosFfi =
        LimitesAdjuntosFfi(
            maxAdjuntos = 5u,
            maxPaginasPdf = 20u,
            maxBytesTotal = (20 * 1024 * 1024).toULong(),
            ladoLargoImagen = 1568u,
            calidadJpeg = 85u,
        )

    override fun enviarAlAgente(
        rutaDatos: String,
        rutaConfig: String,
        texto: String,
        adjuntos: List<AdjuntoFfi>,
    ): RespuestaAgenteFfi {
        llamadasEnviarAgente++
        alEnviar?.invoke()
        ultimosAdjuntosEnviados = adjuntos
        fallarAgente?.let { throw it }
        if (respuestaAgente.nota == null) {
            hayConversacionActiva = true
        }
        return respuestaAgente
    }

    override fun aplicarAccion(rutaDatos: String, accionId: String): ResumenArchivoFfi =
        cargar(rutaDatos)

    override fun descartarAccion(accionId: String) {}

    override fun aplicarTodas(
        rutaDatos: String,
        accionIds: List<String>,
    ): ResultadoAplicarTodasFfi =
        ResultadoAplicarTodasFfi(accionIds.size.toUInt(), cargar(rutaDatos), null)

    override fun necesitaCompactar(rutaConfig: String): Boolean = necesitaCompactarFlag

    override fun nuevaConversacion() {
        llamadasNuevaConversacion++
        hayConversacionActiva = false
    }

    override fun hayConversacion(): Boolean = hayConversacionActiva

    override fun hayConversacionEn(rutaConfig: String): Boolean = hayConversacionActiva

    override fun wavDePcm16(muestras: List<Short>, frecuencia: UInt): ByteArray =
        ByteArray(44 + muestras.size * 2)

    override fun transcribir(rutaConfig: String, wav: ByteArray): String {
        llamadasTranscribir++
        return transcripcionTexto
    }

    override fun moodleConectar(
        rutaConfig: String,
        sitio: String,
        usuario: String,
        contrasena: String,
    ): EstadoMoodleFfi {
        fallarMoodleConectar?.let { throw it }
        moodleEstadoFake = EstadoMoodleFfi(
            conectado = true,
            sitio = sitio,
            usuario = usuario,
            textoEstado = "Conectado como $usuario · sin sincronizar aún",
            requiereReingreso = false,
        )
        return moodleEstadoFake
    }

    override fun moodleEstado(rutaConfig: String): EstadoMoodleFfi = moodleEstadoFake

    override fun moodleSincronizar(rutaDatos: String, rutaConfig: String): ResumenSyncFfi {
        fallarMoodleSync?.let { throw it }
        return resumenMoodleSync
    }

    override fun moodleDesconectar(rutaConfig: String) {
        moodleEstadoFake = EstadoMoodleFfi(false, "micampus.unal.edu.co", "", "", false)
    }
}
