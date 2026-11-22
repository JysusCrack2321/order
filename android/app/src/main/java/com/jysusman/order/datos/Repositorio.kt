package com.jysusman.order.datos

import com.jysusman.order.agente.AdjuntoPendiente
import com.jysusman.order.agente.FuenteAdjunto
import com.jysusman.order.agente.PreparadorAdjuntos
import com.jysusman.order.agente.ProcesadorPlataforma
import com.jysusman.order.ffi.AccionVistaFfi
import com.jysusman.order.ffi.AgenteConfigFfi
import com.jysusman.order.ffi.AjustesFormFfi
import com.jysusman.order.ffi.AnioMesFfi
import com.jysusman.order.ffi.AvisoFfi
import com.jysusman.order.ffi.CalendarioMesFfi
import com.jysusman.order.ffi.CalendarioVistaFfi
import com.jysusman.order.ffi.ClaseFormFfi
import com.jysusman.order.ffi.CodigoEmparejamientoFfi
import com.jysusman.order.ffi.ConflictoFfi
import com.jysusman.order.ffi.DiaFfi
import com.jysusman.order.ffi.DiaVistaFfi
import com.jysusman.order.ffi.DispositivoSyncFfi
import com.jysusman.order.ffi.ErrorFfi
import com.jysusman.order.ffi.EstadoConexionFfi
import com.jysusman.order.ffi.EstadoMoodleFfi
import com.jysusman.order.ffi.FiltroPendientesFfi
import com.jysusman.order.ffi.Hora12Ffi
import com.jysusman.order.ffi.InicioVistaFfi
import com.jysusman.order.ffi.MateriaConTonoFfi
import com.jysusman.order.ffi.PendientesVistaFfi
import com.jysusman.order.ffi.ProveedorInfoFfi
import com.jysusman.order.ffi.RecordatorioFormFfi
import com.jysusman.order.ffi.ResumenArchivoFfi
import com.jysusman.order.ffi.SemanaVistaFfi
import com.jysusman.order.ffi.TareaFormFfi
import com.jysusman.order.ffi.TextosConexionAgenteFfi
import com.jysusman.order.ffi.TipoFechaFfi
import com.jysusman.order.ffi.VozProveedorInfoFfi
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import java.io.File

data class EstadoOrder(
    val carpetaDatos: String = "",
    val rutaArchivo: String = "",
    val cargando: Boolean = false,
    val resumen: ResumenArchivoFfi? = null,
    val error: String? = null,
    val advertencias: List<String> = emptyList(),
    val conflictos: List<ConflictoFfi> = emptyList(),
    val inicio: InicioVistaFfi? = null,
    val dia: DiaVistaFfi? = null,
    val diaIso: String = "",
    val semana: SemanaVistaFfi? = null,
    val lunesIso: String = "",
    val pendientes: PendientesVistaFfi? = null,
    val filtro: FiltroPendientesFfi = FiltroPendientesFfi(
        materia = null,
        tipoFecha = TipoFechaFfi.TODAS,
        desdeIso = null,
        hastaIso = null,
        horaDesde = null,
        horaHasta = null,
        mostrarHechas = true,
    ),
    val materias: List<MateriaConTonoFfi> = emptyList(),
    val agenteTexto: String = "",
    val agentePensando: Boolean = false,
    val agenteCompactando: Boolean = false,
    val agenteTranscribiendo: Boolean = false,
    val agenteLeyendoArchivo: Boolean = false,
    val agenteAdjuntos: List<AdjuntoPendiente> = emptyList(),
    val agenteAviso: String = "",
    val agenteAvisoEsError: Boolean = false,
    val agentePanelAbierto: Boolean = false,
    val agenteRespuesta: String = "",
    val agenteNota: String = "",
    val agenteAcciones: List<AccionVistaFfi> = emptyList(),
    val agenteHayConversacion: Boolean = false,
    val agenteTieneClave: Boolean = true,
    val conexionAbierta: Boolean = false,
    val conexionKey: String = "",
    val conexionVerKey: Boolean = false,
    val conexionConectando: Boolean = false,
    val conexionEstadoTexto: String = "",
    val conexionEstadoError: Boolean = false,
    val conexionOfertaPortapapeles: String? = null,
    val textosConexion: TextosConexionAgenteFfi? = null,
    val moodleEstado: EstadoMoodleFfi? = null,
    val moodleOcupado: Boolean = false,
    val moodleAviso: String = "",
    val moodleError: String = "",
    val calendarios: List<CalendarioVistaFfi> = emptyList(),
    val calendariosOcupado: Boolean = false,
    val calendariosAviso: String = "",
    val calendariosError: String = "",
    val icalOfertaPortapapeles: String? = null,
    val p2pActivo: Boolean = false,
    val p2pNombre: String = "",
    val p2pSincronizando: Boolean = false,
    val p2pOcupado: Boolean = false,
    val p2pDispositivos: List<DispositivoSyncFfi> = emptyList(),
    val p2pCodigo: CodigoEmparejamientoFfi? = null,
    val p2pAviso: String = "",
    val p2pError: String = "",
)

/**
 * Repositorio único de estado para ORDER en Android.
 * Todas las llamadas al puente FFI corren en `dispatcherIo` bajo un mutex,
 * manteniendo las últimas vistas válidas si el archivo TOML está roto.
 */
class Repositorio(
    private val puente: PuenteOrder = PuenteOrderNativo(),
    carpetaInicial: String? = null,
    private val dispatcherIo: CoroutineDispatcher = Dispatchers.IO,
    private val asegurarCarpeta: (File) -> Unit = { dir -> dir.mkdirs() },
    private val rutaRegistroAvisos: String = "",
    private val rutaConfigAgente: String = "",
    private val rutaConfigMoodle: String = "",
    private val rutaConfigCalendarios: String = "",
    private val rutaConfigSync: String = "",
    private val onDatosActualizados: () -> Unit = {},
    procesadorAdjuntos: ProcesadorPlataforma? = null,
    private val demoraSyncLocalMs: Long = 2_000L,
) {
    private val mutex = Mutex()
    private val mutexP2p = Mutex()
    private val scopeP2p = CoroutineScope(SupervisorJob() + dispatcherIo)
    private val lockSyncLocal = Any()
    private var jobSyncLocal: Job? = null
    private var ultimaSecuenciaP2p: ULong = 0uL
    private var conteoAlGenerarCodigoP2p: Int = 0
    private val preparadorAdjuntos = procesadorAdjuntos?.let { PreparadorAdjuntos(puente, it) }
    private val ofertasIcalDescartadas = mutableSetOf<String>()

    private val carpetaInicialLimpia = carpetaInicial?.trim()?.trimEnd('/').orEmpty()

    private val _estado = MutableStateFlow(
        EstadoOrder(
            carpetaDatos = carpetaInicialLimpia,
            rutaArchivo = if (carpetaInicialLimpia.isNotEmpty()) rutaEnCarpeta(carpetaInicialLimpia) else "",
        )
    )
    val estado: StateFlow<EstadoOrder> = _estado.asStateFlow()

    suspend fun cargarInicial() {
        cargarTodo(archivarViejas = true)
    }

    suspend fun recargar() {
        cargarTodo(archivarViejas = false)
    }

    suspend fun cambiarCarpeta(nuevaCarpeta: String) {
        val limpia = nuevaCarpeta.trim().trimEnd('/')
        if (limpia.isEmpty()) {
            _estado.update { it.copy(error = "La carpeta de datos no puede quedar vacía.") }
            return
        }
        _estado.update {
            it.copy(
                carpetaDatos = limpia,
                rutaArchivo = rutaEnCarpeta(limpia),
            )
        }
        cargarTodo(archivarViejas = true)
    }

    private suspend fun cargarTodo(archivarViejas: Boolean) {
        mutex.withLock {
            _estado.update { it.copy(cargando = true) }
            val actualInicial = _estado.value
            val carpetaEfectiva = actualInicial.carpetaDatos.ifBlank {
                withContext(dispatcherIo) { puente.carpetaPorDefecto().trimEnd('/') }
            }
            val rutaEfectiva = rutaEnCarpeta(carpetaEfectiva)
            val diaEfectivo = actualInicial.diaIso.ifBlank {
                withContext(dispatcherIo) { puente.fechaHoyIso() }
            }
            val lunesEfectivo = actualInicial.lunesIso.ifBlank {
                withContext(dispatcherIo) { puente.lunesActualIso() }
            }
            if (actualInicial.carpetaDatos != carpetaEfectiva ||
                actualInicial.diaIso != diaEfectivo ||
                actualInicial.lunesIso != lunesEfectivo
            ) {
                _estado.update {
                    it.copy(
                        carpetaDatos = carpetaEfectiva,
                        rutaArchivo = rutaEfectiva,
                        diaIso = diaEfectivo,
                        lunesIso = lunesEfectivo,
                    )
                }
            }
            val actual = _estado.value
            val resultado = withContext(dispatcherIo) {
                runCatching {
                    val archivo = File(actual.rutaArchivo)
                    archivo.parentFile?.let { asegurarCarpeta(it) }
                    val res = if (archivarViejas) {
                        puente.cargar(actual.rutaArchivo)
                        puente.archivar(actual.rutaArchivo)
                    } else {
                        puente.cargar(actual.rutaArchivo)
                    }
                    leerVistas(actual.rutaArchivo, actual.diaIso, actual.lunesIso, actual.filtro, res)
                }
            }

            resultado.onSuccess { lectura ->
                val hayConvGuardada = if (rutaConfigAgente.isNotBlank()) {
                    withContext(dispatcherIo) {
                        runCatching { puente.hayConversacionEn(rutaConfigAgente) }
                            .getOrDefault(_estado.value.agenteHayConversacion)
                    }
                } else {
                    _estado.value.agenteHayConversacion
                }
                val tieneClave = if (rutaConfigAgente.isNotBlank()) {
                    withContext(dispatcherIo) {
                        runCatching { puente.agenteTieneClave(rutaConfigAgente) }
                            .getOrDefault(_estado.value.agenteTieneClave)
                    }
                } else {
                    _estado.value.agenteTieneClave
                }
                val textos = _estado.value.textosConexion ?: withContext(dispatcherIo) {
                    runCatching { puente.textosConexionAgente() }.getOrNull()
                }
                val moodleEst = if (rutaConfigMoodle.isNotBlank()) {
                    withContext(dispatcherIo) {
                        runCatching { puente.moodleEstado(rutaConfigMoodle) }.getOrNull()
                    }
                } else {
                    _estado.value.moodleEstado
                }
                val listaCal = if (rutaConfigCalendarios.isNotBlank()) {
                    withContext(dispatcherIo) {
                        runCatching { puente.calendariosListar(rutaConfigCalendarios) }.getOrNull()
                    }
                } else {
                    null
                }
                _estado.update {
                    it.copy(
                        cargando = false,
                        resumen = lectura.resumen,
                        error = null,
                        advertencias = lectura.resumen.advertencias,
                        conflictos = lectura.resumen.conflictos,
                        inicio = lectura.inicio,
                        dia = lectura.dia,
                        semana = lectura.semana,
                        pendientes = lectura.pendientes,
                        materias = lectura.materias,
                        agenteHayConversacion = hayConvGuardada,
                        agenteTieneClave = tieneClave,
                        textosConexion = textos,
                        moodleEstado = moodleEst,
                        calendarios = listaCal ?: it.calendarios,
                    )
                }
                withContext(dispatcherIo) { onDatosActualizados() }
            }.onFailure { err ->
                val conflictosActuales = withContext(dispatcherIo) {
                    runCatching { puente.detectarConflictos(actual.rutaArchivo) }
                        .getOrDefault(emptyList())
                }
                val mensaje = mensajeDeError(err)
                _estado.update {
                    it.copy(
                        cargando = false,
                        error = "No se pudo leer order.toml — $mensaje",
                        conflictos = conflictosActuales,
                    )
                }
            }
        }
    }

    suspend fun resolverConflicto(usarOtra: Boolean) {
        val actual = _estado.value
        val conflicto = actual.conflictos.firstOrNull() ?: return
        mutex.withLock {
            val res = withContext(dispatcherIo) {
                runCatching {
                    puente.resolverConflicto(actual.rutaArchivo, conflicto.ruta, usarOtra)
                }
            }
            res.onFailure { err ->
                _estado.update {
                    it.copy(error = "No se pudo resolver el conflicto — ${mensajeDeError(err)}")
                }
                return
            }
        }
        recargar()
        dispararSyncTrasCambioLocal()
    }

    // --- Navegación de Día y Semana ----------------------------------------

    suspend fun moverDia(delta: Int) {
        val nuevoDia = withContext(dispatcherIo) {
            puente.sumarDiasIso(_estado.value.diaIso, delta)
        }
        actualizarDia(nuevoDia)
    }

    suspend fun irDiaHoy() {
        val hoy = withContext(dispatcherIo) { puente.fechaHoyIso() }
        actualizarDia(hoy)
    }

    private suspend fun actualizarDia(nuevoDiaIso: String) {
        _estado.update { it.copy(diaIso = nuevoDiaIso) }
        mutex.withLock {
            val actual = _estado.value
            val res = withContext(dispatcherIo) {
                runCatching { puente.vistaDia(actual.rutaArchivo, nuevoDiaIso) }
            }
            res.onSuccess { dia ->
                _estado.update { it.copy(dia = dia) }
            }.onFailure { err ->
                _estado.update { it.copy(error = "No se pudo cargar el día — ${mensajeDeError(err)}") }
            }
        }
    }

    suspend fun moverSemana(deltaDias: Int) {
        val nuevoLunes = withContext(dispatcherIo) {
            puente.sumarDiasIso(_estado.value.lunesIso, deltaDias)
        }
        actualizarSemana(nuevoLunes)
    }

    suspend fun irSemanaHoy() {
        val lunes = withContext(dispatcherIo) { puente.lunesActualIso() }
        actualizarSemana(lunes)
    }

    private suspend fun actualizarSemana(nuevoLunesIso: String) {
        _estado.update { it.copy(lunesIso = nuevoLunesIso) }
        mutex.withLock {
            val actual = _estado.value
            val res = withContext(dispatcherIo) {
                runCatching { puente.vistaSemana(actual.rutaArchivo, nuevoLunesIso) }
            }
            res.onSuccess { semana ->
                _estado.update { it.copy(semana = semana) }
            }.onFailure { err ->
                _estado.update { it.copy(error = "No se pudo cargar la semana — ${mensajeDeError(err)}") }
            }
        }
    }

    // --- Filtros de Pendientes ---------------------------------------------

    suspend fun actualizarFiltro(transformar: (FiltroPendientesFfi) -> FiltroPendientesFfi) {
        val nuevoFiltro = transformar(_estado.value.filtro)
        _estado.update { it.copy(filtro = nuevoFiltro) }
        mutex.withLock {
            val actual = _estado.value
            val res = withContext(dispatcherIo) {
                runCatching { puente.vistaPendientes(actual.rutaArchivo, nuevoFiltro) }
            }
            res.onSuccess { pend ->
                _estado.update { it.copy(pendientes = pend) }
            }.onFailure { err ->
                _estado.update { it.copy(error = "No se pudo filtrar — ${mensajeDeError(err)}") }
            }
        }
    }

    suspend fun limpiarFiltros() {
        actualizarFiltro { actual ->
            FiltroPendientesFfi(
                materia = null,
                tipoFecha = TipoFechaFfi.TODAS,
                desdeIso = null,
                hastaIso = null,
                horaDesde = null,
                horaHasta = null,
                mostrarHechas = actual.mostrarHechas,
            )
        }
    }

    // --- Escrituras y formularios ------------------------------------------

    suspend fun marcarTarea(id: String, hecha: Boolean) {
        ejecutarEscritura("marcar") { ruta -> puente.marcarTarea(ruta, id, hecha) }
    }

    suspend fun obtenerTareaForm(id: String?): Result<TareaFormFfi> = withContext(dispatcherIo) {
        runCatching { puente.obtenerTareaForm(_estado.value.rutaArchivo, id) }
    }

    suspend fun obtenerClaseForm(id: String?): Result<ClaseFormFfi> = withContext(dispatcherIo) {
        runCatching { puente.obtenerClaseForm(_estado.value.rutaArchivo, id) }
    }

    suspend fun obtenerRecordatorioForm(id: String?): Result<RecordatorioFormFfi> =
        withContext(dispatcherIo) {
            runCatching { puente.obtenerRecordatorioForm(_estado.value.rutaArchivo, id) }
        }

    suspend fun obtenerAjustesForm(): Result<AjustesFormFfi> = withContext(dispatcherIo) {
        runCatching { puente.obtenerAjustesForm(_estado.value.rutaArchivo) }
    }

    suspend fun guardarTarea(
        id: String?,
        titulo: String,
        fechaIso: String,
        hora: String,
        materia: String,
        hecha: Boolean,
    ): ErrorFfi.Validacion? = ejecutarFormulario { ruta ->
        puente.guardarTarea(ruta, id, titulo, fechaIso, hora, materia, hecha)
    }

    suspend fun borrarTarea(id: String): Boolean =
        ejecutarEscritura("borrar") { ruta -> puente.borrarTarea(ruta, id) }

    suspend fun guardarClase(
        id: String?,
        materia: String,
        dias: List<DiaFfi>,
        horaInicio: String,
        horaFin: String,
        lugar: String,
    ): ErrorFfi.Validacion? = ejecutarFormulario { ruta ->
        puente.guardarClase(ruta, id, materia, dias, horaInicio, horaFin, lugar)
    }

    suspend fun borrarClase(id: String): Boolean =
        ejecutarEscritura("borrar") { ruta -> puente.borrarClase(ruta, id) }

    suspend fun guardarRecordatorio(
        id: String?,
        texto: String,
        hora: String,
        dias: List<DiaFfi>,
    ): ErrorFfi.Validacion? = ejecutarFormulario { ruta ->
        puente.guardarRecordatorio(ruta, id, texto, hora, dias)
    }

    suspend fun borrarRecordatorio(id: String): Boolean =
        ejecutarEscritura("borrar") { ruta -> puente.borrarRecordatorio(ruta, id) }

    suspend fun guardarAjustes(
        sinSemestre: Boolean,
        inicioIso: String,
        finIso: String,
        horaAviso: String,
        horasAntes: String,
    ): ErrorFfi.Validacion? = ejecutarFormulario { ruta ->
        puente.guardarAjustes(ruta, sinSemestre, inicioIso, finIso, horaAviso, horasAntes)
    }

    // --- Selectores (en Dispatchers.IO / FFI) ------------------------------

    suspend fun calendarioMes(anio: Int, mes: Int, elegido: Int): CalendarioMesFfi =
        withContext(dispatcherIo) { puente.calendarioMes(anio, mes, elegido) }

    suspend fun mesVecino(anio: Int, mes: Int, delta: Int): AnioMesFfi =
        withContext(dispatcherIo) { puente.mesVecino(anio, mes, delta) }

    suspend fun diasDelMes(anio: Int, mes: Int): Int =
        withContext(dispatcherIo) { puente.diasDelMes(anio, mes) }

    suspend fun horas12Texto(): List<String> = withContext(dispatcherIo) { puente.horas12Texto() }

    suspend fun minutos60Texto(): List<String> =
        withContext(dispatcherIo) { puente.minutos60Texto() }

    suspend fun formatearHora12(hora12: Int, minuto: Int, tarde: Boolean): String =
        withContext(dispatcherIo) { puente.formatearHora12(hora12, minuto, tarde) }

    suspend fun parsearHora12(texto: String): Hora12Ffi =
        withContext(dispatcherIo) { puente.parsearHora12(texto) }

    suspend fun diasSemanaEtiquetas(): List<String> =
        withContext(dispatcherIo) { puente.diasSemanaEtiquetas() }

    suspend fun canonicaMateria(texto: String): String =
        withContext(dispatcherIo) {
            runCatching { puente.canonicaMateria(_estado.value.rutaArchivo, texto) }
                .getOrDefault(texto.trim())
        }

    suspend fun resumenDelDia(): AvisoFfi? {
        val rutaDatos = _estado.value.rutaArchivo
        if (rutaDatos.isBlank() || rutaRegistroAvisos.isBlank()) return null
        return withContext(dispatcherIo) {
            runCatching { puente.resumenDelDia(rutaDatos, "", rutaRegistroAvisos) }.getOrNull()
        }
    }

    // --- Agente ------------------------------------------------------------

    fun actualizarTextoAgente(texto: String) {
        _estado.update { it.copy(agenteTexto = texto, agenteAviso = "", agenteAvisoEsError = false) }
    }

    fun mostrarAvisoAgente(mensaje: String, esError: Boolean = true) {
        _estado.update { it.copy(agenteAviso = mensaje, agenteAvisoEsError = esError) }
    }

    suspend fun transcribirAudio(muestras: List<Short>, frecuencia: Int = 16_000) {
        val minimoMuestras = (frecuencia * 0.4).toInt()
        if (muestras.isEmpty() || muestras.size < minimoMuestras) {
            mostrarAvisoAgente("La grabación quedó vacía; revisa el micrófono.", esError = true)
            return
        }
        if (rutaConfigAgente.isBlank()) return

        _estado.update {
            it.copy(
                agenteTranscribiendo = true,
                agenteAviso = "transcribiendo…",
                agenteAvisoEsError = false,
            )
        }

        val res = withContext(dispatcherIo) {
            runCatching {
                val wav = puente.wavDePcm16(muestras, frecuencia.toUInt())
                if (wav.isEmpty()) {
                    throw IllegalStateException("La grabación quedó vacía; revisa el micrófono.")
                }
                puente.transcribir(rutaConfigAgente, wav)
            }
        }

        res.onSuccess { transcripcion ->
            _estado.update { actual ->
                val previo = actual.agenteTexto.trim()
                val combinado = if (previo.isEmpty()) {
                    transcripcion
                } else {
                    "$previo $transcripcion"
                }
                actual.copy(
                    agenteTranscribiendo = false,
                    agenteTexto = combinado,
                    agenteAviso = "",
                    agenteAvisoEsError = false,
                )
            }
        }.onFailure { err ->
            _estado.update {
                it.copy(
                    agenteTranscribiendo = false,
                    agenteAviso = "No se pudo transcribir — ${mensajeDeError(err)}",
                    agenteAvisoEsError = true,
                )
            }
        }
    }

    fun cerrarPanelAgente() {
        _estado.update {
            it.copy(
                agentePanelAbierto = false,
                agenteRespuesta = "",
                agenteNota = "",
                agenteAcciones = emptyList(),
            )
        }
    }

    suspend fun adjuntar(fuente: FuenteAdjunto) {
        adjuntarDesde { fuente }
    }

    suspend fun adjuntarDesde(leerFuente: suspend () -> FuenteAdjunto) {
        val preparador = preparadorAdjuntos ?: return
        val actual = _estado.value
        if (actual.agenteLeyendoArchivo || actual.agentePensando) return
        if (rutaConfigAgente.isBlank()) return

        _estado.update {
            it.copy(
                agenteLeyendoArchivo = true,
                agenteAviso = "leyendo archivo…",
                agenteAvisoEsError = false,
            )
        }

        val res = withContext(dispatcherIo) {
            runCatching {
                val fuente = leerFuente()
                preparador.preparar(rutaConfigAgente, fuente, actual.agenteAdjuntos.size)
            }
        }

        res.onSuccess { resultado ->
            _estado.update { estado ->
                estado.copy(
                    agenteLeyendoArchivo = false,
                    agenteAdjuntos = estado.agenteAdjuntos + resultado.adjunto,
                    agenteAviso = resultado.aviso.orEmpty(),
                    agenteAvisoEsError = false,
                )
            }
        }.onFailure { err ->
            _estado.update { estado ->
                estado.copy(
                    agenteLeyendoArchivo = false,
                    agenteAviso = mensajeDeError(err),
                    agenteAvisoEsError = true,
                )
            }
        }
    }

    fun quitarAdjunto(id: String) {
        _estado.update { estado ->
            estado.copy(
                agenteAdjuntos = estado.agenteAdjuntos.filterNot { it.id == id },
                agenteAviso = "",
                agenteAvisoEsError = false,
            )
        }
    }

    suspend fun limitesAdjuntos() = withContext(dispatcherIo) { puente.limitesAdjuntos() }

    suspend fun abrirConexionGuiada() {
        val textos = _estado.value.textosConexion ?: withContext(dispatcherIo) {
            runCatching { puente.textosConexionAgente() }.getOrNull()
        }
        _estado.update {
            it.copy(
                conexionAbierta = true,
                conexionEstadoTexto = "",
                conexionEstadoError = false,
                agentePanelAbierto = false,
                agenteAviso = "",
                agenteAvisoEsError = false,
                textosConexion = textos,
            )
        }
    }

    fun cerrarConexionGuiada() {
        _estado.update {
            it.copy(
                conexionAbierta = false,
                conexionEstadoTexto = "",
                conexionEstadoError = false,
                conexionOfertaPortapapeles = null,
            )
        }
    }

    fun actualizarClaveConexion(clave: String) {
        _estado.update {
            it.copy(
                conexionKey = clave,
                conexionEstadoTexto = "",
                conexionEstadoError = false,
            )
        }
    }

    fun alternarVerClaveConexion() {
        _estado.update { it.copy(conexionVerKey = !it.conexionVerKey) }
    }

    suspend fun pegarClaveDesdePortapapeles(textoPortapapeles: String?) {
        val textos = _estado.value.textosConexion ?: withContext(dispatcherIo) {
            runCatching { puente.textosConexionAgente() }.getOrNull()
        }
        val candidato = textoPortapapeles.orEmpty().trim()
        val esValida = candidato.isNotEmpty() && withContext(dispatcherIo) {
            runCatching { puente.pareceClaveGemini(candidato) }.getOrDefault(false)
        }
        if (esValida) {
            _estado.update {
                it.copy(
                    conexionKey = candidato,
                    conexionEstadoTexto = "",
                    conexionEstadoError = false,
                    conexionOfertaPortapapeles = null,
                    textosConexion = textos,
                )
            }
        } else {
            _estado.update {
                it.copy(
                    conexionEstadoTexto = textos?.portapapelesInvalido
                        ?: "Lo que copiaste no parece una clave de Google.",
                    conexionEstadoError = true,
                    textosConexion = textos,
                )
            }
        }
    }

    suspend fun revisarPortapapelesAlReanudar(textoPortapapeles: String?) {
        val actual = _estado.value
        if (!actual.conexionAbierta) return
        val candidato = textoPortapapeles.orEmpty().trim()
        val esValida = candidato.isNotEmpty() && withContext(dispatcherIo) {
            runCatching { puente.pareceClaveGemini(candidato) }.getOrDefault(false)
        }
        _estado.update {
            it.copy(
                conexionOfertaPortapapeles = if (esValida && candidato != it.conexionKey.trim()) {
                    candidato
                } else {
                    null
                }
            )
        }
    }

    fun aceptarClaveDePortapapeles() {
        val oferta = _estado.value.conexionOfertaPortapapeles ?: return
        _estado.update {
            it.copy(
                conexionKey = oferta,
                conexionOfertaPortapapeles = null,
                conexionEstadoTexto = "",
                conexionEstadoError = false,
            )
        }
    }

    suspend fun conectarGemini() {
        val actual = _estado.value
        if (actual.conexionConectando) return
        val textos = actual.textosConexion ?: withContext(dispatcherIo) {
            runCatching { puente.textosConexionAgente() }.getOrNull()
        }
        val clave = actual.conexionKey.trim()
        if (clave.isEmpty()) {
            _estado.update {
                it.copy(
                    conexionEstadoTexto = textos?.claveVacia ?: "Pega o escribe tu clave de Google.",
                    conexionEstadoError = true,
                    textosConexion = textos,
                )
            }
            return
        }
        if (rutaConfigAgente.isBlank()) return

        _estado.update {
            it.copy(
                conexionConectando = true,
                conexionEstadoTexto = "",
                conexionEstadoError = false,
                conexionOfertaPortapapeles = null,
                textosConexion = textos,
            )
        }

        val res = withContext(dispatcherIo) {
            runCatching { puente.conectarGemini(rutaConfigAgente, clave) }
        }

        var reenviar = false
        res.onSuccess { resultado ->
            _estado.update { estado ->
                val nuevaTieneClave = resultado.guardada || estado.agenteTieneClave
                when (resultado.estado) {
                    EstadoConexionFfi.CONECTADO -> {
                        reenviar = estado.agenteTexto.isNotBlank() || estado.agenteAdjuntos.isNotEmpty()
                        estado.copy(
                            agenteTieneClave = nuevaTieneClave,
                            conexionConectando = false,
                            conexionAbierta = false,
                            conexionKey = "",
                            conexionVerKey = false,
                            conexionEstadoTexto = "",
                            conexionEstadoError = false,
                            agenteAviso = resultado.mensaje,
                            agenteAvisoEsError = false,
                        )
                    }
                    EstadoConexionFfi.CLAVE_INVALIDA,
                    EstadoConexionFfi.SIN_INTERNET -> estado.copy(
                        agenteTieneClave = nuevaTieneClave,
                        conexionConectando = false,
                        conexionEstadoTexto = resultado.mensaje,
                        conexionEstadoError = true,
                    )
                }
            }
        }.onFailure { err ->
            _estado.update {
                it.copy(
                    conexionConectando = false,
                    conexionEstadoTexto = mensajeDeError(err),
                    conexionEstadoError = true,
                )
            }
        }

        if (reenviar) {
            enviarAlAgente()
        }
    }

    suspend fun enviarAlAgente() {
        val actual = _estado.value
        if (actual.agentePensando || actual.agenteLeyendoArchivo) return
        if (rutaConfigAgente.isBlank() || actual.rutaArchivo.isBlank()) return

        val tieneClave = withContext(dispatcherIo) {
            runCatching { puente.agenteTieneClave(rutaConfigAgente) }.getOrDefault(actual.agenteTieneClave)
        }
        if (!tieneClave) {
            _estado.update { it.copy(agenteTieneClave = false) }
            abrirConexionGuiada()
            return
        }

        val pregunta = actual.agenteTexto.trim()
        val adjuntosFfi = actual.agenteAdjuntos.map { it.adjuntoFfi }
        if (pregunta.isEmpty() && adjuntosFfi.isEmpty()) return

        val compactando = withContext(dispatcherIo) {
            runCatching { puente.necesitaCompactar(rutaConfigAgente) }.getOrDefault(false)
        }

        _estado.update {
            it.copy(
                agentePensando = true,
                agenteCompactando = compactando,
                agentePanelAbierto = false,
                agenteRespuesta = "",
                agenteNota = "",
                agenteAcciones = emptyList(),
                agenteAviso = "",
            )
        }

        val res = withContext(dispatcherIo) {
            runCatching {
                val resp = puente.enviarAlAgente(actual.rutaArchivo, rutaConfigAgente, pregunta, adjuntosFfi)
                val hayConv = puente.hayConversacionEn(rutaConfigAgente)
                resp to hayConv
            }
        }

        res.onSuccess { (resp, hayConv) ->
            _estado.update {
                it.copy(
                    agentePensando = false,
                    agenteCompactando = false,
                    agentePanelAbierto = true,
                    agenteRespuesta = resp.respuesta,
                    agenteAcciones = resp.acciones,
                    agenteNota = resp.nota.orEmpty(),
                    agenteTexto = if (resp.nota == null) "" else it.agenteTexto,
                    agenteAdjuntos = if (resp.nota == null) emptyList() else it.agenteAdjuntos,
                    agenteHayConversacion = hayConv,
                )
            }
        }.onFailure { err ->
            _estado.update {
                it.copy(
                    agentePensando = false,
                    agenteCompactando = false,
                    agentePanelAbierto = true,
                    agenteRespuesta = mensajeDeError(err),
                    agenteAcciones = emptyList(),
                    agenteNota = "",
                )
            }
        }
    }

    suspend fun nuevaConversacion() {
        if (_estado.value.agentePensando) return
        withContext(dispatcherIo) {
            runCatching { puente.nuevaConversacion() }
        }
        _estado.update {
            it.copy(
                agenteHayConversacion = false,
                agentePanelAbierto = false,
                agenteRespuesta = "",
                agenteNota = "",
                agenteAcciones = emptyList(),
                agenteAviso = "",
                agenteAvisoEsError = false,
            )
        }
    }

    suspend fun aplicarAccionAgente(accionId: String) {
        mutex.withLock {
            val actual = _estado.value
            val res = withContext(dispatcherIo) {
                runCatching {
                    val resumen = puente.aplicarAccion(actual.rutaArchivo, accionId)
                    leerVistas(
                        actual.rutaArchivo,
                        actual.diaIso,
                        actual.lunesIso,
                        actual.filtro,
                        resumen,
                    )
                }
            }
            res.onSuccess { lectura ->
                _estado.update { estado ->
                    val restantes = estado.agenteAcciones.filterNot { it.id == accionId }
                    estado.copy(
                        resumen = lectura.resumen,
                        error = null,
                        advertencias = lectura.resumen.advertencias,
                        conflictos = lectura.resumen.conflictos,
                        inicio = lectura.inicio,
                        dia = lectura.dia,
                        semana = lectura.semana,
                        pendientes = lectura.pendientes,
                        materias = lectura.materias,
                        agenteAcciones = restantes,
                        agentePanelAbierto = restantes.isNotEmpty(),
                        agenteRespuesta = if (restantes.isEmpty()) "" else estado.agenteRespuesta,
                        agenteNota = "",
                    )
                }
                withContext(dispatcherIo) { onDatosActualizados() }
                dispararSyncTrasCambioLocal()
            }.onFailure { err ->
                _estado.update {
                    it.copy(agenteNota = "No se pudo aplicar — ${mensajeDeError(err)}")
                }
            }
        }
    }

    suspend fun descartarAccionAgente(accionId: String) {
        withContext(dispatcherIo) {
            runCatching { puente.descartarAccion(accionId) }
        }
        _estado.update { estado ->
            val restantes = estado.agenteAcciones.filterNot { it.id == accionId }
            estado.copy(
                agenteAcciones = restantes,
                agentePanelAbierto = restantes.isNotEmpty(),
                agenteRespuesta = if (restantes.isEmpty()) "" else estado.agenteRespuesta,
            )
        }
    }

    suspend fun aplicarTodasAgente() {
        val ids = _estado.value.agenteAcciones.map { it.id }
        if (ids.isEmpty()) return

        mutex.withLock {
            val actual = _estado.value
            val res = withContext(dispatcherIo) {
                runCatching {
                    val resultado = puente.aplicarTodas(actual.rutaArchivo, ids)
                    val lectura = resultado.resumen?.let { resumen ->
                        leerVistas(
                            actual.rutaArchivo,
                            actual.diaIso,
                            actual.lunesIso,
                            actual.filtro,
                            resumen,
                        )
                    }
                    resultado to lectura
                }
            }

            res.onSuccess { (resultado, lectura) ->
                _estado.update { estado ->
                    val aplicadas = resultado.aplicadas.toInt().coerceIn(0, estado.agenteAcciones.size)
                    val restantes = estado.agenteAcciones.drop(aplicadas)
                    val cerrar = resultado.error == null
                    estado.copy(
                        resumen = lectura?.resumen ?: estado.resumen,
                        error = null,
                        advertencias = lectura?.resumen?.advertencias ?: estado.advertencias,
                        conflictos = lectura?.resumen?.conflictos ?: estado.conflictos,
                        inicio = lectura?.inicio ?: estado.inicio,
                        dia = lectura?.dia ?: estado.dia,
                        semana = lectura?.semana ?: estado.semana,
                        pendientes = lectura?.pendientes ?: estado.pendientes,
                        materias = lectura?.materias ?: estado.materias,
                        agenteAcciones = if (cerrar) emptyList() else restantes,
                        agentePanelAbierto = !cerrar,
                        agenteRespuesta = if (cerrar) "" else estado.agenteRespuesta,
                        agenteNota = resultado.error.orEmpty(),
                    )
                }
                if (lectura != null) {
                    withContext(dispatcherIo) { onDatosActualizados() }
                    dispararSyncTrasCambioLocal()
                }
            }.onFailure { err ->
                _estado.update {
                    it.copy(agenteNota = "No se pudo aplicar — ${mensajeDeError(err)}")
                }
            }
        }
    }

    suspend fun proveedoresAgente(): List<ProveedorInfoFfi> =
        withContext(dispatcherIo) { puente.proveedoresAgente() }

    suspend fun proveedoresVoz(): List<VozProveedorInfoFfi> =
        withContext(dispatcherIo) { puente.proveedoresVoz() }

    suspend fun leerAgenteConfig(): Result<AgenteConfigFfi> =
        withContext(dispatcherIo) {
            runCatching { puente.leerAgenteConfig(rutaConfigAgente) }
        }

    suspend fun guardarAgenteConfig(
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
    ): ErrorFfi.Validacion? {
        val res = withContext(dispatcherIo) {
            runCatching {
                puente.guardarAgenteConfig(
                    rutaConfigAgente,
                    proveedor,
                    url,
                    modelo,
                    apiKey,
                    veImagenes,
                    limiteContexto,
                    vozProveedor,
                    vozUrl,
                    vozModelo,
                    vozKey,
                )
            }
        }
        return res.fold(
            onSuccess = {
                _estado.update { estado ->
                    estado.copy(
                        agenteTieneClave = apiKey.isNotBlank(),
                        conexionAbierta = false,
                        conexionEstadoTexto = "",
                        conexionEstadoError = false,
                    )
                }
                null
            },
            onFailure = { err ->
                if (err is ErrorFfi.Validacion) err
                else ErrorFfi.Validacion(0, mensajeDeError(err))
            },
        )
    }

    // --- Moodle ------------------------------------------------------------

    suspend fun cargarEstadoMoodle() {
        if (rutaConfigMoodle.isBlank()) return
        val est = withContext(dispatcherIo) {
            runCatching { puente.moodleEstado(rutaConfigMoodle) }.getOrNull()
        }
        if (est != null) {
            _estado.update { it.copy(moodleEstado = est, moodleError = "", moodleAviso = "") }
        }
    }

    suspend fun conectarMoodle(sitio: String, usuario: String, contrasena: String): Boolean {
        if (rutaConfigMoodle.isBlank() || _estado.value.moodleOcupado) return false
        if (sitio.isBlank() || usuario.isBlank() || contrasena.isEmpty()) {
            _estado.update { it.copy(moodleError = "Escribe sitio, usuario y contraseña.") }
            return false
        }
        _estado.update {
            it.copy(
                moodleOcupado = true,
                moodleError = "",
                moodleAviso = "Conectando y sincronizando…",
            )
        }
        val resCon = withContext(dispatcherIo) {
            runCatching { puente.moodleConectar(rutaConfigMoodle, sitio, usuario, contrasena) }
        }
        resCon.onFailure { err ->
            _estado.update {
                it.copy(
                    moodleOcupado = false,
                    moodleAviso = "",
                    moodleError = mensajeDeError(err),
                )
            }
            return false
        }
        sincronizarMoodle(silencioso = false, yaMarcadoOcupado = true)
        return true
    }

    suspend fun sincronizarMoodle(
        silencioso: Boolean = false,
        yaMarcadoOcupado: Boolean = false,
    ) {
        if (rutaConfigMoodle.isBlank()) return
        val rutaDatos = _estado.value.rutaArchivo
        if (rutaDatos.isBlank()) return

        val conectado = withContext(dispatcherIo) {
            runCatching { puente.moodleEstado(rutaConfigMoodle).conectado }.getOrDefault(false)
        }
        if (!conectado) {
            if (yaMarcadoOcupado) {
                _estado.update { it.copy(moodleOcupado = false, moodleAviso = "") }
            }
            return
        }

        if (!silencioso && !yaMarcadoOcupado) {
            _estado.update {
                it.copy(
                    moodleOcupado = true,
                    moodleError = "",
                    moodleAviso = "Sincronizando…",
                )
            }
        }

        val resSync = withContext(dispatcherIo) {
            runCatching { puente.moodleSincronizar(rutaDatos, rutaConfigMoodle) }
        }
        val nuevoEstado = withContext(dispatcherIo) {
            runCatching { puente.moodleEstado(rutaConfigMoodle) }.getOrNull()
        }

        resSync.onSuccess { resumenSync ->
            val huboCambios =
                resumenSync.creadas > 0u || resumenSync.actualizadas > 0u || resumenSync.hechas > 0u
            if (huboCambios) {
                mutex.withLock {
                    val actual = _estado.value
                    val lectura = withContext(dispatcherIo) {
                        runCatching {
                            val res = puente.cargar(actual.rutaArchivo)
                            leerVistas(
                                actual.rutaArchivo,
                                actual.diaIso,
                                actual.lunesIso,
                                actual.filtro,
                                res,
                            )
                        }
                    }
                    lectura.onSuccess { lec ->
                        _estado.update {
                            it.copy(
                                resumen = lec.resumen,
                                error = null,
                                advertencias = lec.resumen.advertencias,
                                conflictos = lec.resumen.conflictos,
                                inicio = lec.inicio,
                                dia = lec.dia,
                                semana = lec.semana,
                                pendientes = lec.pendientes,
                                materias = lec.materias,
                            )
                        }
                        withContext(dispatcherIo) { onDatosActualizados() }
                        dispararSyncTrasCambioLocal()
                    }
                }
            }
            _estado.update {
                it.copy(
                    moodleEstado = nuevoEstado ?: it.moodleEstado,
                    moodleOcupado = false,
                    moodleError = "",
                    moodleAviso = if (silencioso) {
                        it.moodleAviso
                    } else {
                        "Listo: ${resumenSync.creadas} creadas, ${resumenSync.actualizadas} actualizadas, ${resumenSync.hechas} completadas."
                    },
                )
            }
        }.onFailure { err ->
            _estado.update {
                it.copy(
                    moodleEstado = nuevoEstado ?: it.moodleEstado,
                    moodleOcupado = false,
                    moodleAviso = if (silencioso) it.moodleAviso else "",
                    moodleError = if (silencioso) it.moodleError else mensajeDeError(err),
                )
            }
        }
    }

    suspend fun desconectarMoodle() {
        if (rutaConfigMoodle.isBlank() || _estado.value.moodleOcupado) return
        val res = withContext(dispatcherIo) {
            runCatching {
                puente.moodleDesconectar(rutaConfigMoodle)
                puente.moodleEstado(rutaConfigMoodle)
            }
        }
        res.onSuccess { est ->
            _estado.update {
                it.copy(
                    moodleEstado = est,
                    moodleError = "",
                    moodleAviso = "Desconectado de Moodle.",
                )
            }
        }.onFailure { err ->
            _estado.update { it.copy(moodleError = mensajeDeError(err)) }
        }
    }

    // --- Calendarios iCal --------------------------------------------------

    suspend fun sincronizarTodoSilencioso() {
        sincronizarMoodle(silencioso = true)
        sincronizarCalendarios(silencioso = true, incluirMoodle = false)
    }

    suspend fun cargarCalendarios() {
        if (rutaConfigCalendarios.isBlank()) return
        val lista = withContext(dispatcherIo) {
            runCatching { puente.calendariosListar(rutaConfigCalendarios) }.getOrNull()
        }
        if (lista != null) {
            _estado.update {
                it.copy(
                    calendarios = lista,
                    calendariosError = "",
                    calendariosAviso = "",
                )
            }
        }
    }

    suspend fun revisarPortapapelesIcal(textoPortapapeles: String?) {
        val candidato = textoPortapapeles.orEmpty().trim()
        if (candidato.isEmpty() || candidato in ofertasIcalDescartadas) return
        val esIcal = withContext(dispatcherIo) {
            runCatching { puente.pareceUrlIcal(candidato) }.getOrDefault(false)
        }
        if (esIcal) {
            _estado.update { it.copy(icalOfertaPortapapeles = candidato) }
        }
    }

    suspend fun aceptarOfertaIcal() {
        val url = _estado.value.icalOfertaPortapapeles ?: return
        _estado.update { it.copy(icalOfertaPortapapeles = null) }
        ofertasIcalDescartadas.add(url)
        agregarCalendario(url, "", "")
    }

    fun rechazarOfertaIcal() {
        val url = _estado.value.icalOfertaPortapapeles ?: return
        ofertasIcalDescartadas.add(url)
        _estado.update { it.copy(icalOfertaPortapapeles = null) }
    }

    suspend fun agregarCalendario(url: String, nombre: String, materia: String): Boolean {
        if (rutaConfigCalendarios.isBlank() || _estado.value.calendariosOcupado) return false
        if (url.isBlank()) {
            _estado.update { it.copy(calendariosError = "Pega la dirección iCal (.ics o webcal://).") }
            return false
        }
        _estado.update {
            it.copy(
                calendariosOcupado = true,
                calendariosError = "",
                calendariosAviso = "Descargando y sincronizando…",
            )
        }
        val resAdd = withContext(dispatcherIo) {
            runCatching { puente.calendarioAgregar(rutaConfigCalendarios, url, nombre, materia) }
        }
        resAdd.onFailure { err ->
            _estado.update {
                it.copy(
                    calendariosOcupado = false,
                    calendariosAviso = "",
                    calendariosError = mensajeDeError(err),
                )
            }
            return false
        }
        sincronizarCalendarios(silencioso = false, yaMarcadoOcupado = true, incluirMoodle = false)
        return true
    }

    suspend fun quitarCalendario(id: String) {
        if (rutaConfigCalendarios.isBlank() || _estado.value.calendariosOcupado) return
        val res = withContext(dispatcherIo) {
            runCatching {
                puente.calendarioQuitar(rutaConfigCalendarios, id)
                puente.calendariosListar(rutaConfigCalendarios)
            }
        }
        res.onSuccess { lista ->
            _estado.update {
                it.copy(
                    calendarios = lista,
                    calendariosError = "",
                    calendariosAviso = "Calendario quitado (las tareas existentes se conservan).",
                )
            }
        }.onFailure { err ->
            _estado.update { it.copy(calendariosError = mensajeDeError(err)) }
        }
    }

    suspend fun sincronizarCalendarios(
        silencioso: Boolean = false,
        yaMarcadoOcupado: Boolean = false,
        incluirMoodle: Boolean = true,
    ) {
        if (incluirMoodle && rutaConfigMoodle.isNotBlank()) {
            sincronizarMoodle(silencioso = silencioso)
        }
        if (rutaConfigCalendarios.isBlank()) {
            if (yaMarcadoOcupado) {
                _estado.update { it.copy(calendariosOcupado = false, calendariosAviso = "") }
            }
            return
        }
        val rutaDatos = _estado.value.rutaArchivo
        if (rutaDatos.isBlank()) return

        val lista = withContext(dispatcherIo) {
            runCatching { puente.calendariosListar(rutaConfigCalendarios) }.getOrDefault(emptyList())
        }
        if (lista.isEmpty()) {
            if (yaMarcadoOcupado) {
                _estado.update { it.copy(calendarios = emptyList(), calendariosOcupado = false, calendariosAviso = "") }
            }
            return
        }

        if (!silencioso && !yaMarcadoOcupado) {
            _estado.update {
                it.copy(
                    calendariosOcupado = true,
                    calendariosError = "",
                    calendariosAviso = "Sincronizando…",
                )
            }
        }

        val resSync = withContext(dispatcherIo) {
            runCatching { puente.calendariosSincronizar(rutaDatos, rutaConfigCalendarios) }
        }
        val nuevaLista = withContext(dispatcherIo) {
            runCatching { puente.calendariosListar(rutaConfigCalendarios) }.getOrNull()
        }

        resSync.onSuccess { r ->
            val huboCambios = r.creadas > 0u || r.actualizadas > 0u
            if (huboCambios) {
                mutex.withLock {
                    val actual = _estado.value
                    val lectura = withContext(dispatcherIo) {
                        runCatching {
                            val res = puente.cargar(actual.rutaArchivo)
                            leerVistas(
                                actual.rutaArchivo,
                                actual.diaIso,
                                actual.lunesIso,
                                actual.filtro,
                                res,
                            )
                        }
                    }
                    lectura.onSuccess { lec ->
                        _estado.update {
                            it.copy(
                                resumen = lec.resumen,
                                error = null,
                                advertencias = lec.resumen.advertencias,
                                conflictos = lec.resumen.conflictos,
                                inicio = lec.inicio,
                                dia = lec.dia,
                                semana = lec.semana,
                                pendientes = lec.pendientes,
                                materias = lec.materias,
                            )
                        }
                        withContext(dispatcherIo) { onDatosActualizados() }
                        dispararSyncTrasCambioLocal()
                    }
                }
            }
            _estado.update {
                it.copy(
                    calendarios = nuevaLista ?: it.calendarios,
                    calendariosOcupado = false,
                    calendariosError = "",
                    calendariosAviso = if (silencioso) {
                        it.calendariosAviso
                    } else {
                        "Listo: ${r.creadas} creadas, ${r.actualizadas} actualizadas."
                    },
                )
            }
            }.onFailure { err ->
                _estado.update {
                    it.copy(
                        calendarios = nuevaLista ?: it.calendarios,
                        calendariosOcupado = false,
                        calendariosAviso = if (silencioso) it.calendariosAviso else "",
                        calendariosError = if (silencioso) it.calendariosError else mensajeDeError(err),
                    )
                }
            }
    }

    // --- Sincronización P2P (iroh) -----------------------------------------

    suspend fun iniciarSincronizacionP2p() {
        if (rutaConfigSync.isBlank()) return
        val rutaDatos = _estado.value.rutaArchivo
        if (rutaDatos.isBlank()) return

        val inicio = withContext(dispatcherIo) {
            runCatching {
                puente.syncIniciar(rutaDatos, rutaConfigSync)
                val est = puente.syncEstado()
                val lista = puente.syncDispositivos()
                est to lista
            }
        }
        inicio.onSuccess { (est, lista) ->
            ultimaSecuenciaP2p = est.secuenciaCambio
            _estado.update {
                it.copy(
                    p2pActivo = est.activo,
                    p2pNombre = est.nombre,
                    p2pSincronizando = est.sincronizando,
                    p2pDispositivos = lista,
                    p2pError = "",
                )
            }
            if (lista.isNotEmpty()) {
                sincronizarP2p(silencioso = true)
            }
        }.onFailure { err ->
            _estado.update { it.copy(p2pError = mensajeDeError(err)) }
        }
    }

    fun detenerSincronizacionP2p() {
        synchronized(lockSyncLocal) {
            jobSyncLocal?.cancel()
            jobSyncLocal = null
        }
        if (rutaConfigSync.isBlank()) return
        runCatching { puente.syncDetener() }
        _estado.update { it.copy(p2pActivo = false, p2pSincronizando = false) }
    }

    suspend fun verificarCambiosRemotosP2p() {
        if (rutaConfigSync.isBlank() || !_estado.value.p2pActivo) return
        val lectura = withContext(dispatcherIo) {
            runCatching {
                val est = puente.syncEstado()
                val lista = puente.syncDispositivos()
                est to lista
            }
        }.getOrNull() ?: return

        val (est, lista) = lectura
        val secuenciaNueva = est.secuenciaCambio > ultimaSecuenciaP2p
        if (secuenciaNueva) {
            ultimaSecuenciaP2p = est.secuenciaCambio
            recargar()
        }
        val codigoAbierto = _estado.value.p2pCodigo != null
        val emparejoNuevo = codigoAbierto && lista.size > conteoAlGenerarCodigoP2p
        _estado.update {
            it.copy(
                p2pNombre = est.nombre,
                p2pSincronizando = est.sincronizando || it.p2pOcupado,
                p2pDispositivos = lista,
                p2pCodigo = if (emparejoNuevo) null else it.p2pCodigo,
                p2pAviso = if (emparejoNuevo) "Dispositivo emparejado y listo." else it.p2pAviso,
            )
        }
    }

    suspend fun refrescarDispositivosP2p() {
        if (rutaConfigSync.isBlank()) return
        val lectura = withContext(dispatcherIo) {
            runCatching {
                puente.syncEstado() to puente.syncDispositivos()
            }
        }.getOrNull() ?: return
        _estado.update {
            it.copy(
                p2pActivo = lectura.first.activo,
                p2pNombre = lectura.first.nombre,
                p2pSincronizando = lectura.first.sincronizando,
                p2pDispositivos = lectura.second,
            )
        }
    }

    suspend fun cambiarNombreDispositivoP2p(nombre: String) {
        if (rutaConfigSync.isBlank()) return
        val res = withContext(dispatcherIo) {
            runCatching {
                puente.syncNombre(nombre)
                puente.syncEstado()
            }
        }
        res.onSuccess { est ->
            _estado.update {
                it.copy(
                    p2pNombre = est.nombre,
                    p2pError = "",
                    p2pAviso = "Nombre actualizado.",
                )
            }
        }.onFailure { err ->
            _estado.update { it.copy(p2pError = mensajeDeError(err)) }
        }
    }

    suspend fun generarCodigoEmparejamientoP2p() {
        if (rutaConfigSync.isBlank()) return
        if (!_estado.value.p2pActivo) {
            iniciarSincronizacionP2p()
        }
        val res = withContext(dispatcherIo) {
            runCatching {
                val lista = puente.syncDispositivos()
                val codigo = puente.syncCodigoEmparejamiento()
                lista.size to codigo
            }
        }
        res.onSuccess { (conteo, codigo) ->
            conteoAlGenerarCodigoP2p = conteo
            _estado.update {
                it.copy(
                    p2pCodigo = codigo,
                    p2pError = "",
                    p2pAviso = "",
                )
            }
        }.onFailure { err ->
            _estado.update { it.copy(p2pError = mensajeDeError(err)) }
        }
    }

    fun cerrarCodigoEmparejamientoP2p() {
        _estado.update { it.copy(p2pCodigo = null) }
    }

    fun anotarErrorP2p(mensaje: String) {
        _estado.update { it.copy(p2pError = mensaje) }
    }

    suspend fun emparejarP2p(enlaceOPalabras: String): Boolean {
        if (rutaConfigSync.isBlank() || _estado.value.p2pOcupado) return false
        if (!_estado.value.p2pActivo) {
            iniciarSincronizacionP2p()
        }
        _estado.update {
            it.copy(
                p2pOcupado = true,
                p2pSincronizando = true,
                p2pError = "",
                p2pAviso = "Conectando con el dispositivo…",
            )
        }
        val res = withContext(dispatcherIo) {
            runCatching {
                val disp = puente.syncEmparejar(enlaceOPalabras)
                val lista = puente.syncDispositivos()
                val est = puente.syncEstado()
                Triple(disp, lista, est)
            }
        }
        return res.fold(
            onSuccess = { (disp, lista, est) ->
                ultimaSecuenciaP2p = est.secuenciaCambio
                recargar()
                _estado.update {
                    it.copy(
                        p2pOcupado = false,
                        p2pSincronizando = false,
                        p2pDispositivos = lista,
                        p2pCodigo = null,
                        p2pError = "",
                        p2pAviso = "Emparejado con ${disp.nombre}.",
                    )
                }
                true
            },
            onFailure = { err ->
                _estado.update {
                    it.copy(
                        p2pOcupado = false,
                        p2pSincronizando = false,
                        p2pAviso = "",
                        p2pError = mensajeDeError(err),
                    )
                }
                false
            },
        )
    }

    suspend fun quitarDispositivoP2p(id: String) {
        if (rutaConfigSync.isBlank() || _estado.value.p2pOcupado) return
        val res = withContext(dispatcherIo) {
            runCatching {
                puente.syncQuitar(id)
                puente.syncDispositivos()
            }
        }
        res.onSuccess { lista ->
            _estado.update {
                it.copy(
                    p2pDispositivos = lista,
                    p2pError = "",
                    p2pAviso = "Dispositivo quitado.",
                )
            }
        }.onFailure { err ->
            _estado.update { it.copy(p2pError = mensajeDeError(err)) }
        }
    }

    suspend fun sincronizarP2p(silencioso: Boolean = false) {
        if (rutaConfigSync.isBlank() || !_estado.value.p2pActivo) return
        mutexP2p.withLock {
            val dispositivosActuales = _estado.value.p2pDispositivos
            if (dispositivosActuales.isEmpty()) {
                if (!silencioso) {
                    _estado.update { it.copy(p2pAviso = "No hay dispositivos emparejados.", p2pError = "") }
                }
                return
            }
            _estado.update {
                it.copy(
                    p2pSincronizando = true,
                    p2pOcupado = !silencioso,
                    p2pError = if (silencioso) it.p2pError else "",
                    p2pAviso = if (silencioso) it.p2pAviso else "Sincronizando con dispositivos…",
                )
            }
            val res = withContext(dispatcherIo) {
                runCatching {
                    val resumen = puente.syncAhora()
                    val lista = puente.syncDispositivos()
                    val est = puente.syncEstado()
                    Triple(resumen, lista, est)
                }
            }
            res.onSuccess { (resumen, lista, est) ->
                ultimaSecuenciaP2p = est.secuenciaCambio
                if (resumen.cambiosLocales) {
                    recargar()
                }
                _estado.update {
                    it.copy(
                        p2pSincronizando = false,
                        p2pOcupado = false,
                        p2pDispositivos = lista,
                        p2pAviso = if (silencioso) {
                            it.p2pAviso
                        } else if (resumen.exitosos == 0u) {
                            "Sin conexión con los otros dispositivos."
                        } else if (resumen.cambiosLocales) {
                            "Sincronizado con ${resumen.exitosos} dispositivo(s) · agenda actualizada."
                        } else {
                            "Sincronizado con ${resumen.exitosos} dispositivo(s)."
                        },
                    )
                }
            }.onFailure { err ->
                _estado.update {
                    it.copy(
                        p2pSincronizando = false,
                        p2pOcupado = false,
                        p2pError = if (silencioso) it.p2pError else mensajeDeError(err),
                    )
                }
            }
        }
    }

    private fun dispararSyncTrasCambioLocal() {
        if (rutaConfigSync.isBlank()) return
        synchronized(lockSyncLocal) {
            jobSyncLocal?.cancel()
            jobSyncLocal = scopeP2p.launch {
                if (demoraSyncLocalMs > 0L) {
                    delay(demoraSyncLocalMs)
                }
                if (_estado.value.p2pActivo && _estado.value.p2pDispositivos.isNotEmpty()) {
                    sincronizarP2p(silencioso = true)
                }
            }
        }
    }

    // --- Ayudantes internos ------------------------------------------------

    private suspend fun ejecutarEscritura(
        verbo: String,
        bloque: (String) -> ResumenArchivoFfi,
    ): Boolean {
        mutex.withLock {
            val actual = _estado.value
            val res = withContext(dispatcherIo) {
                runCatching {
                    val resumen = bloque(actual.rutaArchivo)
                    leerVistas(
                        actual.rutaArchivo,
                        actual.diaIso,
                        actual.lunesIso,
                        actual.filtro,
                        resumen,
                    )
                }
            }
            return res.fold(
                onSuccess = { lectura ->
                    _estado.update {
                        it.copy(
                            resumen = lectura.resumen,
                            error = null,
                            advertencias = lectura.resumen.advertencias,
                            conflictos = lectura.resumen.conflictos,
                            inicio = lectura.inicio,
                            dia = lectura.dia,
                            semana = lectura.semana,
                            pendientes = lectura.pendientes,
                            materias = lectura.materias,
                        )
                    }
                    withContext(dispatcherIo) { onDatosActualizados() }
                    dispararSyncTrasCambioLocal()
                    true
                },
                onFailure = { err ->
                    _estado.update {
                        it.copy(error = "No se pudo $verbo — ${mensajeDeError(err)}")
                    }
                    false
                },
            )
        }
    }

    private suspend fun ejecutarFormulario(
        bloque: (String) -> ResumenArchivoFfi,
    ): ErrorFfi.Validacion? {
        mutex.withLock {
            val actual = _estado.value
            val res = withContext(dispatcherIo) {
                runCatching {
                    val resumen = bloque(actual.rutaArchivo)
                    leerVistas(
                        actual.rutaArchivo,
                        actual.diaIso,
                        actual.lunesIso,
                        actual.filtro,
                        resumen,
                    )
                }
            }
            return res.fold(
                onSuccess = { lectura ->
                    _estado.update {
                        it.copy(
                            resumen = lectura.resumen,
                            error = null,
                            advertencias = lectura.resumen.advertencias,
                            conflictos = lectura.resumen.conflictos,
                            inicio = lectura.inicio,
                            dia = lectura.dia,
                            semana = lectura.semana,
                            pendientes = lectura.pendientes,
                            materias = lectura.materias,
                        )
                    }
                    withContext(dispatcherIo) { onDatosActualizados() }
                    dispararSyncTrasCambioLocal()
                    null
                },
                onFailure = { err ->
                    if (err is ErrorFfi.Validacion) {
                        err
                    } else {
                        val msg = mensajeDeError(err)
                        _estado.update {
                            it.copy(error = "No se pudo guardar — $msg")
                        }
                        ErrorFfi.Validacion(0, msg)
                    }
                },
            )
        }
    }

    private fun mensajeDeError(err: Throwable): String = when (err) {
        is ErrorFfi.Datos -> {
            val l = err.linea
            if (l != null) "línea $l: ${err.mensaje}" else err.mensaje
        }
        is ErrorFfi.Operacion -> err.mensaje
        is ErrorFfi.Validacion -> err.mensaje
        else -> err.message ?: err.toString()
    }

    private fun leerVistas(
        ruta: String,
        diaIso: String,
        lunesIso: String,
        filtro: FiltroPendientesFfi,
        resumen: ResumenArchivoFfi,
    ): LecturaCompleta {
        return LecturaCompleta(
            resumen = resumen,
            inicio = puente.vistaInicio(ruta),
            dia = puente.vistaDia(ruta, diaIso),
            semana = puente.vistaSemana(ruta, lunesIso),
            pendientes = puente.vistaPendientes(ruta, filtro),
            materias = puente.materiasConTono(ruta),
        )
    }

    private data class LecturaCompleta(
        val resumen: ResumenArchivoFfi,
        val inicio: InicioVistaFfi,
        val dia: DiaVistaFfi,
        val semana: SemanaVistaFfi,
        val pendientes: PendientesVistaFfi,
        val materias: List<MateriaConTonoFfi>,
    )

    companion object {
        fun rutaEnCarpeta(carpeta: String): String =
            carpeta.trimEnd('/') + "/order.toml"
    }
}
