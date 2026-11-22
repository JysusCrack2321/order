package com.jysusman.order.datos

import com.jysusman.order.ffi.AccionVistaFfi
import com.jysusman.order.ffi.AgenteConfigFfi
import com.jysusman.order.ffi.AjustesFormFfi
import com.jysusman.order.ffi.AnioMesFfi
import com.jysusman.order.ffi.AvisoFfi
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
import com.jysusman.order.ffi.ProveedorInfoFfi
import com.jysusman.order.ffi.RecordatorioFormFfi
import com.jysusman.order.ffi.ResumenArchivoFfi
import com.jysusman.order.ffi.SemanaVistaFfi
import com.jysusman.order.ffi.TareaFormFfi
import com.jysusman.order.ffi.TipoFechaFfi
import com.jysusman.order.ffi.VozProveedorInfoFfi
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
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
    val agenteAviso: String = "",
    val agentePanelAbierto: Boolean = false,
    val agenteRespuesta: String = "",
    val agenteNota: String = "",
    val agenteAcciones: List<AccionVistaFfi> = emptyList(),
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
    private val onDatosActualizados: () -> Unit = {},
) {
    private val mutex = Mutex()

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
        _estado.update { it.copy(agenteTexto = texto, agenteAviso = "") }
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

    suspend fun enviarAlAgente() {
        val actual = _estado.value
        val pregunta = actual.agenteTexto.trim()
        if (pregunta.isEmpty() || actual.agentePensando) return
        if (rutaConfigAgente.isBlank() || actual.rutaArchivo.isBlank()) return

        _estado.update {
            it.copy(
                agentePensando = true,
                agentePanelAbierto = false,
                agenteRespuesta = "",
                agenteNota = "",
                agenteAcciones = emptyList(),
                agenteAviso = "",
            )
        }

        val res = withContext(dispatcherIo) {
            runCatching { puente.enviarAlAgente(actual.rutaArchivo, rutaConfigAgente, pregunta) }
        }

        res.onSuccess { resp ->
            _estado.update {
                it.copy(
                    agentePensando = false,
                    agentePanelAbierto = true,
                    agenteRespuesta = resp.respuesta,
                    agenteAcciones = resp.acciones,
                    agenteNota = resp.nota.orEmpty(),
                    agenteTexto = if (resp.nota == null) "" else it.agenteTexto,
                )
            }
        }.onFailure { err ->
            _estado.update {
                it.copy(
                    agentePensando = false,
                    agentePanelAbierto = true,
                    agenteRespuesta = mensajeDeError(err),
                    agenteAcciones = emptyList(),
                    agenteNota = "",
                )
            }
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
                    vozProveedor,
                    vozUrl,
                    vozModelo,
                    vozKey,
                )
            }
        }
        return res.fold(
            onSuccess = { null },
            onFailure = { err ->
                if (err is ErrorFfi.Validacion) err
                else ErrorFfi.Validacion(0, mensajeDeError(err))
            },
        )
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
