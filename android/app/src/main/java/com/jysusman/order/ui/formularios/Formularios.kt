package com.jysusman.order.ui.formularios

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.jysusman.order.datos.Repositorio
import com.jysusman.order.ffi.DiaFfi
import com.jysusman.order.ffi.MateriaConTonoFfi
import com.jysusman.order.ffi.RecordatorioListaFfi
import com.jysusman.order.ui.comun.CasillaOrder
import com.jysusman.order.ui.comun.ChipOrder
import com.jysusman.order.ui.selectores.SelectorFechaDialog
import com.jysusman.order.ui.selectores.SelectorHoraDialog
import com.jysusman.order.ui.selectores.SelectorMateriaDialog
import com.jysusman.order.ui.theme.OrderTema
import kotlinx.coroutines.launch

sealed interface FormularioActivo {
    data class Tarea(val id: String?) : FormularioActivo
    data class Clase(val id: String?) : FormularioActivo
    data class Recordatorio(val id: String?, val volverAjustes: Boolean = false) : FormularioActivo
    data object Ajustes : FormularioActivo
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ContenedorFormularioSheet(
    formulario: FormularioActivo,
    repositorio: Repositorio,
    materias: List<MateriaConTonoFfi>,
    carpetaActual: String,
    alCambiarCarpeta: (String) -> Unit,
    avisosActivos: Boolean = true,
    alCambiarAvisosActivos: (Boolean) -> Unit = {},
    alAbrirFormulario: (FormularioActivo) -> Unit,
    alCerrar: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    ModalBottomSheet(
        onDismissRequest = alCerrar,
        sheetState = sheetState,
        containerColor = OrderTema.FondoPanel,
        contentColor = OrderTema.Texto,
    ) {
        when (formulario) {
            is FormularioActivo.Tarea -> HojaTarea(
                id = formulario.id,
                repositorio = repositorio,
                materias = materias,
                alCerrar = alCerrar,
            )

            is FormularioActivo.Clase -> HojaClase(
                id = formulario.id,
                repositorio = repositorio,
                materias = materias,
                alCerrar = alCerrar,
            )

            is FormularioActivo.Recordatorio -> HojaRecordatorio(
                id = formulario.id,
                volverAjustes = formulario.volverAjustes,
                repositorio = repositorio,
                alVolverAjustes = { alAbrirFormulario(FormularioActivo.Ajustes) },
                alCerrar = alCerrar,
            )

            FormularioActivo.Ajustes -> HojaAjustes(
                repositorio = repositorio,
                carpetaActual = carpetaActual,
                alCambiarCarpeta = alCambiarCarpeta,
                avisosActivos = avisosActivos,
                alCambiarAvisosActivos = alCambiarAvisosActivos,
                alNuevoRecordatorio = {
                    alAbrirFormulario(FormularioActivo.Recordatorio(id = null, volverAjustes = true))
                },
                alEditarRecordatorio = { recId ->
                    alAbrirFormulario(FormularioActivo.Recordatorio(id = recId, volverAjustes = true))
                },
                alCerrar = alCerrar,
            )
        }
    }
}

@Composable
private fun HojaTarea(
    id: String?,
    repositorio: Repositorio,
    materias: List<MateriaConTonoFfi>,
    alCerrar: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val edita = !id.isNullOrBlank()
    var listo by remember(id) { mutableStateOf(false) }

    var titulo by remember(id) { mutableStateOf("") }
    var materia by remember(id) { mutableStateOf("") }
    var fechaIso by remember(id) { mutableStateOf("") }
    var hora by remember(id) { mutableStateOf("") }
    var hecha by remember(id) { mutableStateOf(false) }

    var errorMsg by remember(id) { mutableStateOf("") }
    var campoMal by remember(id) { mutableIntStateOf(0) }
    var confirmaBorrado by remember(id) { mutableStateOf(false) }

    var selectorMateria by remember { mutableStateOf(false) }
    var selectorFecha by remember { mutableStateOf(false) }
    var selectorHora by remember { mutableStateOf(false) }

    LaunchedEffect(id) {
        repositorio.obtenerTareaForm(id).onSuccess { f ->
            titulo = f.titulo
            materia = f.materia
            fechaIso = f.fechaIso
            hora = f.horaTexto
            hecha = f.hecha
            listo = true
        }.onFailure { err ->
            errorMsg = err.message ?: "No se pudo abrir la tarea."
            listo = true
        }
    }

    MarcoHoja(
        encabezado = if (edita) "Editar tarea" else "Nueva tarea",
        sub = if (edita) "Los cambios se guardan en order.toml." else "Se agrega al final de order.toml.",
        error = errorMsg,
        puedeBorrar = edita,
        confirmaBorrado = confirmaBorrado,
        preguntaConfirma = "¿Borrar esta tarea del archivo?",
        alPedirBorrar = { confirmaBorrado = true },
        alCancelarBorrado = { confirmaBorrado = false },
        alConfirmarBorrado = {
            if (id != null) {
                scope.launch {
                    if (repositorio.borrarTarea(id)) alCerrar()
                }
            }
        },
        alCancelar = alCerrar,
        alGuardar = {
            if (!listo) return@MarcoHoja
            scope.launch {
                val err = repositorio.guardarTarea(id, titulo, fechaIso, hora, materia, hecha)
                if (err == null) {
                    alCerrar()
                } else {
                    campoMal = err.campo
                    errorMsg = err.mensaje
                }
            }
        },
    ) {
        CampoTexto(
            etiqueta = "Título",
            valor = titulo,
            alCambiar = {
                titulo = it
                if (campoMal == 1) campoMal = 0
            },
            pista = "Taller 2",
            mal = campoMal == 1,
        )

        CampoSelector(
            etiqueta = "Materia (opcional)",
            valor = materia,
            pista = "Elegir materia",
            glifo = "▾",
            onClick = { selectorMateria = true },
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
        ) {
            CampoSelector(
                etiqueta = "Fecha de entrega",
                valor = fechaIso,
                pista = "Elegir fecha",
                glifo = "▦",
                mal = campoMal == 2,
                onClick = { selectorFecha = true },
                modifier = Modifier.weight(1f),
            )
            CampoSelector(
                etiqueta = "Hora",
                valor = hora,
                pista = "Elegir hora",
                glifo = "◷",
                mal = campoMal == 3,
                onClick = { selectorHora = true },
                modifier = Modifier.weight(1f),
            )
        }

        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            modifier = Modifier.clickable { hecha = !hecha },
        ) {
            CasillaOrder(marcada = hecha, onToggle = { hecha = !hecha })
            Text(
                text = "Ya está hecha",
                fontSize = OrderTema.TamanoPequeno,
                color = OrderTema.TextoSuave,
            )
        }
    }

    if (selectorMateria) {
        SelectorMateriaDialog(
            materias = materias,
            repositorio = repositorio,
            onElegir = { materia = it },
            onCerrar = { selectorMateria = false },
        )
    }
    if (selectorFecha) {
        SelectorFechaDialog(
            fechaInicialIso = fechaIso,
            repositorio = repositorio,
            onAceptar = {
                fechaIso = it
                if (campoMal == 2) campoMal = 0
            },
            onCerrar = { selectorFecha = false },
        )
    }
    if (selectorHora) {
        SelectorHoraDialog(
            horaInicialTexto = hora,
            repositorio = repositorio,
            onAceptar = {
                hora = it
                if (campoMal == 3) campoMal = 0
            },
            onCerrar = { selectorHora = false },
        )
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun HojaClase(
    id: String?,
    repositorio: Repositorio,
    materias: List<MateriaConTonoFfi>,
    alCerrar: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val edita = !id.isNullOrBlank()
    var listo by remember(id) { mutableStateOf(false) }

    var materia by remember(id) { mutableStateOf("") }
    var dias by remember(id) { mutableStateOf<Set<DiaFfi>>(setOf(DiaFfi.LUN)) }
    var horaInicio by remember(id) { mutableStateOf("7:00 a. m.") }
    var horaFin by remember(id) { mutableStateOf("9:00 a. m.") }
    var lugar by remember(id) { mutableStateOf("") }
    var etiquetasDias by remember { mutableStateOf(listOf("Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom")) }

    var errorMsg by remember(id) { mutableStateOf("") }
    var campoMal by remember(id) { mutableIntStateOf(0) }
    var confirmaBorrado by remember(id) { mutableStateOf(false) }

    var selectorMateria by remember { mutableStateOf(false) }
    var selectorHoraCampo by remember { mutableIntStateOf(0) } // 2 = inicio, 3 = fin

    LaunchedEffect(id) {
        etiquetasDias = repositorio.diasSemanaEtiquetas()
        repositorio.obtenerClaseForm(id).onSuccess { f ->
            materia = f.materia
            dias = f.dias.toSet()
            horaInicio = f.horaInicio
            horaFin = f.horaFin
            lugar = f.lugar
            listo = true
        }.onFailure { err ->
            errorMsg = err.message ?: "No se pudo abrir la clase."
            listo = true
        }
    }

    MarcoHoja(
        encabezado = if (edita) "Editar clase" else "Nueva clase",
        sub = "La clase se repite cada semana dentro del semestre.",
        error = errorMsg,
        puedeBorrar = edita,
        confirmaBorrado = confirmaBorrado,
        preguntaConfirma = "¿Borrar esta clase del archivo?",
        alPedirBorrar = { confirmaBorrado = true },
        alCancelarBorrado = { confirmaBorrado = false },
        alConfirmarBorrado = {
            if (id != null) {
                scope.launch {
                    if (repositorio.borrarClase(id)) alCerrar()
                }
            }
        },
        alCancelar = alCerrar,
        alGuardar = {
            if (!listo) return@MarcoHoja
            scope.launch {
                val err = repositorio.guardarClase(
                    id = id,
                    materia = materia,
                    dias = DiaFfi.entries.filter { it in dias },
                    horaInicio = horaInicio,
                    horaFin = horaFin,
                    lugar = lugar,
                )
                if (err == null) {
                    alCerrar()
                } else {
                    campoMal = err.campo
                    errorMsg = err.mensaje
                }
            }
        },
    ) {
        CampoSelector(
            etiqueta = "Materia",
            valor = materia,
            pista = "Elegir materia",
            glifo = "▾",
            mal = campoMal == 1,
            onClick = { selectorMateria = true },
        )

        Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs)) {
            EtiquetaCampo(texto = "Días (uno o varios)", mal = campoMal == 4)
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
            ) {
                DiaFfi.entries.forEachIndexed { idx, diaEnum ->
                    val etiqueta = etiquetasDias.getOrElse(idx) { diaEnum.name }
                    ChipOrder(
                        texto = etiqueta,
                        activo = diaEnum in dias,
                        onClick = {
                            dias = if (diaEnum in dias) dias - diaEnum else dias + diaEnum
                            if (campoMal == 4) campoMal = 0
                        },
                    )
                }
            }
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
        ) {
            CampoSelector(
                etiqueta = "Inicio",
                valor = horaInicio,
                pista = "Elegir hora",
                glifo = "◷",
                mal = campoMal == 2,
                onClick = { selectorHoraCampo = 2 },
                modifier = Modifier.weight(1f),
            )
            CampoSelector(
                etiqueta = "Fin",
                valor = horaFin,
                pista = "Elegir hora",
                glifo = "◷",
                mal = campoMal == 3,
                onClick = { selectorHoraCampo = 3 },
                modifier = Modifier.weight(1f),
            )
        }

        CampoTexto(
            etiqueta = "Lugar (opcional)",
            valor = lugar,
            alCambiar = { lugar = it },
            pista = "453-201",
        )
    }

    if (selectorMateria) {
        SelectorMateriaDialog(
            materias = materias,
            repositorio = repositorio,
            onElegir = {
                materia = it
                if (campoMal == 1) campoMal = 0
            },
            onCerrar = { selectorMateria = false },
        )
    }
    if (selectorHoraCampo != 0) {
        val inicial = if (selectorHoraCampo == 2) horaInicio else horaFin
        SelectorHoraDialog(
            horaInicialTexto = inicial,
            repositorio = repositorio,
            onAceptar = { elegida ->
                if (selectorHoraCampo == 2) {
                    horaInicio = elegida
                    if (campoMal == 2) campoMal = 0
                } else {
                    horaFin = elegida
                    if (campoMal == 3) campoMal = 0
                }
            },
            onCerrar = { selectorHoraCampo = 0 },
        )
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun HojaRecordatorio(
    id: String?,
    volverAjustes: Boolean,
    repositorio: Repositorio,
    alVolverAjustes: () -> Unit,
    alCerrar: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val edita = !id.isNullOrBlank()
    var listo by remember(id) { mutableStateOf(false) }

    var texto by remember(id) { mutableStateOf("") }
    var dias by remember(id) { mutableStateOf<Set<DiaFfi>>(emptySet()) }
    var hora by remember(id) { mutableStateOf("6:00 p. m.") }
    var etiquetasDias by remember { mutableStateOf(listOf("Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom")) }

    var errorMsg by remember(id) { mutableStateOf("") }
    var campoMal by remember(id) { mutableIntStateOf(0) }
    var confirmaBorrado by remember(id) { mutableStateOf(false) }
    var selectorHora by remember { mutableStateOf(false) }

    LaunchedEffect(id) {
        etiquetasDias = repositorio.diasSemanaEtiquetas()
        repositorio.obtenerRecordatorioForm(id).onSuccess { f ->
            texto = f.texto
            dias = f.dias.toSet()
            hora = f.hora
            listo = true
        }.onFailure { err ->
            errorMsg = err.message ?: "No se pudo abrir el recordatorio."
            listo = true
        }
    }

    val salir = {
        if (volverAjustes) alVolverAjustes() else alCerrar()
    }

    MarcoHoja(
        encabezado = if (edita) "Editar recordatorio" else "Nuevo recordatorio",
        sub = "Se avisa a esa hora los días marcados.",
        error = errorMsg,
        puedeBorrar = edita,
        confirmaBorrado = confirmaBorrado,
        preguntaConfirma = "¿Borrar este recordatorio del archivo?",
        alPedirBorrar = { confirmaBorrado = true },
        alCancelarBorrado = { confirmaBorrado = false },
        alConfirmarBorrado = {
            if (id != null) {
                scope.launch {
                    if (repositorio.borrarRecordatorio(id)) salir()
                }
            }
        },
        alCancelar = salir,
        alGuardar = {
            if (!listo) return@MarcoHoja
            scope.launch {
                val err = repositorio.guardarRecordatorio(
                    id = id,
                    texto = texto,
                    hora = hora,
                    dias = DiaFfi.entries.filter { it in dias },
                )
                if (err == null) {
                    salir()
                } else {
                    campoMal = err.campo
                    errorMsg = err.mensaje
                }
            }
        },
    ) {
        CampoTexto(
            etiqueta = "Texto",
            valor = texto,
            alCambiar = {
                texto = it
                if (campoMal == 1) campoMal = 0
            },
            pista = "Revisar Moodle",
            mal = campoMal == 1,
        )

        Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs)) {
            EtiquetaCampo(texto = "Días", mal = campoMal == 4)
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
            ) {
                DiaFfi.entries.forEachIndexed { idx, diaEnum ->
                    val etiqueta = etiquetasDias.getOrElse(idx) { diaEnum.name }
                    ChipOrder(
                        texto = etiqueta,
                        activo = diaEnum in dias,
                        onClick = {
                            dias = if (diaEnum in dias) dias - diaEnum else dias + diaEnum
                            if (campoMal == 4) campoMal = 0
                        },
                    )
                }
            }
        }

        CampoSelector(
            etiqueta = "Hora",
            valor = hora,
            pista = "Elegir hora",
            glifo = "◷",
            mal = campoMal == 2,
            onClick = { selectorHora = true },
        )
    }

    if (selectorHora) {
        SelectorHoraDialog(
            horaInicialTexto = hora,
            repositorio = repositorio,
            onAceptar = {
                hora = it
                if (campoMal == 2) campoMal = 0
            },
            onCerrar = { selectorHora = false },
        )
    }
}

@Composable
private fun HojaAjustes(
    repositorio: Repositorio,
    carpetaActual: String,
    alCambiarCarpeta: (String) -> Unit,
    avisosActivos: Boolean,
    alCambiarAvisosActivos: (Boolean) -> Unit,
    alNuevoRecordatorio: () -> Unit,
    alEditarRecordatorio: (String) -> Unit,
    alCerrar: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var listo by remember { mutableStateOf(false) }

    var carpetaTexto by remember(carpetaActual) { mutableStateOf(carpetaActual) }
    var avisosLocal by remember(avisosActivos) { mutableStateOf(avisosActivos) }
    var sinSemestre by remember { mutableStateOf(false) }
    var inicioIso by remember { mutableStateOf("") }
    var finIso by remember { mutableStateOf("") }
    var horaAviso by remember { mutableStateOf("") }
    var horasAntes by remember { mutableStateOf("") }
    var recordatorios by remember { mutableStateOf<List<RecordatorioListaFfi>>(emptyList()) }

    var errorMsg by remember { mutableStateOf("") }
    var campoMal by remember { mutableIntStateOf(0) }
    var selectorFechaCampo by remember { mutableIntStateOf(0) } // 1 = inicio, 2 = fin
    var selectorHora by remember { mutableStateOf(false) }

    LaunchedEffect(carpetaActual) {
        repositorio.obtenerAjustesForm().onSuccess { f ->
            sinSemestre = f.sinSemestre
            inicioIso = f.semestreInicioIso
            finIso = f.semestreFinIso
            horaAviso = f.avisoDiaAntes
            horasAntes = f.avisoHorasAntes
            recordatorios = f.recordatorios
            listo = true
        }.onFailure { err ->
            errorMsg = err.message ?: "No se pudieron leer los ajustes."
            listo = true
        }
    }

    MarcoHoja(
        encabezado = "Ajustes",
        sub = "Semestre, avisos, carpeta y recordatorios fijos.",
        error = errorMsg,
        puedeBorrar = false,
        confirmaBorrado = false,
        alCancelar = alCerrar,
        alGuardar = {
            if (!listo) return@MarcoHoja
            scope.launch {
                val carpetaLimpia = carpetaTexto.trim().trimEnd('/')
                if (carpetaLimpia.isNotEmpty() && carpetaLimpia != carpetaActual) {
                    alCambiarCarpeta(carpetaLimpia)
                    repositorio.cambiarCarpeta(carpetaLimpia)
                }
                if (avisosLocal != avisosActivos) {
                    alCambiarAvisosActivos(avisosLocal)
                }
                val err = repositorio.guardarAjustes(
                    sinSemestre = sinSemestre,
                    inicioIso = inicioIso,
                    finIso = finIso,
                    horaAviso = horaAviso,
                    horasAntes = horasAntes,
                )
                if (err == null) {
                    alCerrar()
                } else {
                    campoMal = err.campo
                    errorMsg = err.mensaje
                }
            }
        },
    ) {
        CampoTexto(
            etiqueta = "Carpeta de datos (order.toml)",
            valor = carpetaTexto,
            alCambiar = { carpetaTexto = it },
            pista = "/storage/emulated/0/ORDER",
        )

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { avisosLocal = !avisosLocal },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "Avisos en este dispositivo",
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.Texto,
                )
                Text(
                    text = "Muestra notificaciones programadas de tareas y recordatorios.",
                    fontSize = OrderTema.TamanoHora,
                    color = OrderTema.TextoTenue,
                )
            }
            Switch(
                checked = avisosLocal,
                onCheckedChange = { avisosLocal = it },
                colors = SwitchDefaults.colors(
                    checkedThumbColor = OrderTema.Fondo,
                    checkedTrackColor = OrderTema.Acento,
                    uncheckedThumbColor = OrderTema.TextoSuave,
                    uncheckedTrackColor = OrderTema.FondoCampo,
                    uncheckedBorderColor = OrderTema.Borde,
                ),
            )
        }

        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            modifier = Modifier.clickable { sinSemestre = !sinSemestre },
        ) {
            CasillaOrder(marcada = sinSemestre, onToggle = { sinSemestre = !sinSemestre })
            Text(
                text = "Sin semestre (las clases valen todo el año)",
                fontSize = OrderTema.TamanoPequeno,
                color = OrderTema.TextoSuave,
            )
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
        ) {
            CampoSelector(
                etiqueta = "Inicio del semestre",
                valor = inicioIso,
                pista = "Elegir fecha",
                glifo = "▦",
                mal = campoMal == 1,
                apagado = sinSemestre,
                onClick = { selectorFechaCampo = 1 },
                modifier = Modifier.weight(1f),
            )
            CampoSelector(
                etiqueta = "Fin del semestre",
                valor = finIso,
                pista = "Elegir fecha",
                glifo = "▦",
                mal = campoMal == 2,
                apagado = sinSemestre,
                onClick = { selectorFechaCampo = 2 },
                modifier = Modifier.weight(1f),
            )
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
        ) {
            CampoSelector(
                etiqueta = "Aviso del día antes",
                valor = horaAviso,
                pista = "Elegir hora",
                glifo = "◷",
                mal = campoMal == 3,
                onClick = { selectorHora = true },
                modifier = Modifier.weight(1f),
            )
            CampoTexto(
                etiqueta = "Horas antes de vencer",
                valor = horasAntes,
                alCambiar = {
                    horasAntes = it
                    if (campoMal == 4) campoMal = 0
                },
                pista = "3",
                mal = campoMal == 4,
                keyboardType = KeyboardType.Number,
                modifier = Modifier.weight(1f),
            )
        }

        EtiquetaCampo(texto = "RECORDATORIOS")

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(max = 160.dp)
                .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                .background(OrderTema.FondoCampo)
                .border(OrderTema.Linea, OrderTema.BordeCampo, RoundedCornerShape(OrderTema.RadioPequeno))
                .verticalScroll(rememberScrollState()),
        ) {
            if (recordatorios.isEmpty()) {
                Text(
                    text = "Sin recordatorios.",
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoTenue,
                    modifier = Modifier.padding(OrderTema.EspacioSm),
                )
            } else {
                recordatorios.forEachIndexed { idx, rec ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { alEditarRecordatorio(rec.id) }
                            .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioSm),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                    ) {
                        Text(
                            text = rec.texto,
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.Texto,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        Text(
                            text = "${rec.diasTexto} · ${rec.horaTexto}",
                            fontSize = OrderTema.TamanoHora,
                            color = OrderTema.TextoSuave,
                            maxLines = 1,
                        )
                    }
                    if (idx < recordatorios.lastIndex) {
                        HorizontalDivider(color = OrderTema.BordeSuave, thickness = OrderTema.Linea)
                    }
                }
            }
        }

        OutlinedButton(
            onClick = alNuevoRecordatorio,
            shape = RoundedCornerShape(OrderTema.Radio),
            border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(
                text = "Agregar recordatorio",
                fontSize = OrderTema.TamanoPequeno,
                color = OrderTema.Acento,
            )
        }
    }

    if (selectorFechaCampo != 0) {
        val inicial = if (selectorFechaCampo == 1) inicioIso else finIso
        SelectorFechaDialog(
            fechaInicialIso = inicial,
            repositorio = repositorio,
            onAceptar = { elegida ->
                if (selectorFechaCampo == 1) {
                    inicioIso = elegida
                    if (campoMal == 1) campoMal = 0
                } else {
                    finIso = elegida
                    if (campoMal == 2) campoMal = 0
                }
            },
            onCerrar = { selectorFechaCampo = 0 },
        )
    }
    if (selectorHora) {
        SelectorHoraDialog(
            horaInicialTexto = horaAviso,
            repositorio = repositorio,
            onAceptar = {
                horaAviso = it
                if (campoMal == 3) campoMal = 0
            },
            onCerrar = { selectorHora = false },
        )
    }
}

@Composable
private fun MarcoHoja(
    encabezado: String,
    sub: String,
    error: String,
    puedeBorrar: Boolean,
    confirmaBorrado: Boolean,
    preguntaConfirma: String = "¿Borrar esto?",
    alPedirBorrar: () -> Unit = {},
    alCancelarBorrado: () -> Unit = {},
    alConfirmarBorrado: () -> Unit = {},
    alCancelar: () -> Unit,
    alGuardar: () -> Unit,
    contenido: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = OrderTema.EspacioLg, vertical = OrderTema.EspacioMd),
        verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(OrderTema.Espacio2xs)) {
            Text(
                text = encabezado,
                fontFamily = OrderTema.FuenteTitulos,
                fontWeight = FontWeight.Bold,
                fontSize = OrderTema.TamanoSemana,
                letterSpacing = OrderTema.LetraNormal,
                color = OrderTema.Texto,
            )
            if (sub.isNotEmpty()) {
                Text(
                    text = sub,
                    fontSize = OrderTema.TamanoHora,
                    color = OrderTema.TextoTenue,
                )
            }
        }

        if (error.isNotEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                    .background(OrderTema.FondoError)
                    .border(OrderTema.Linea, OrderTema.BordeError, RoundedCornerShape(OrderTema.RadioPequeno))
                    .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioSm),
            ) {
                Text(
                    text = error,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoError,
                )
            }
        }

        contenido()

        Spacer(modifier = Modifier.height(OrderTema.EspacioXs))

        if (confirmaBorrado) {
            Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm)) {
                Text(
                    text = preguntaConfirma,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.VencidaTexto,
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    OutlinedButton(
                        onClick = alCancelarBorrado,
                        shape = RoundedCornerShape(OrderTema.Radio),
                        border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
                    ) {
                        Text(
                            text = "No",
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.TextoSuave,
                        )
                    }
                    Spacer(modifier = Modifier.padding(horizontal = OrderTema.EspacioXs))
                    Button(
                        onClick = alConfirmarBorrado,
                        shape = RoundedCornerShape(OrderTema.Radio),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = OrderTema.FondoError,
                            contentColor = OrderTema.VencidaTexto,
                        ),
                        border = BorderStroke(OrderTema.Linea, OrderTema.Vencida),
                    ) {
                        Text(
                            text = "Sí, borrar",
                            fontSize = OrderTema.TamanoPequeno,
                        )
                    }
                }
            }
        } else {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                if (puedeBorrar) {
                    OutlinedButton(
                        onClick = alPedirBorrar,
                        shape = RoundedCornerShape(OrderTema.Radio),
                        border = BorderStroke(OrderTema.Linea, OrderTema.BordeError),
                    ) {
                        Text(
                            text = "Borrar",
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.VencidaTexto,
                        )
                    }
                }
                Spacer(modifier = Modifier.weight(1f))
                OutlinedButton(
                    onClick = alCancelar,
                    shape = RoundedCornerShape(OrderTema.Radio),
                    border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
                ) {
                    Text(
                        text = "Cancelar",
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.TextoSuave,
                    )
                }
                Button(
                    onClick = alGuardar,
                    shape = RoundedCornerShape(OrderTema.Radio),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = OrderTema.AcentoTenue,
                        contentColor = OrderTema.Acento,
                    ),
                    border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
                ) {
                    Text(
                        text = "Guardar",
                        fontFamily = OrderTema.FuenteTitulos,
                        fontSize = OrderTema.TamanoPequeno,
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(OrderTema.EspacioMd))
    }
}

@Composable
private fun EtiquetaCampo(
    texto: String,
    mal: Boolean = false,
    apagada: Boolean = false,
) {
    Text(
        text = texto,
        fontSize = OrderTema.TamanoPequeno,
        letterSpacing = OrderTema.LetraNormal,
        color = when {
            mal -> OrderTema.TextoError
            apagada -> OrderTema.TextoTenue
            else -> OrderTema.TextoSuave
        },
    )
}

@Composable
private fun CampoTexto(
    etiqueta: String,
    valor: String,
    alCambiar: (String) -> Unit,
    pista: String,
    mal: Boolean = false,
    keyboardType: KeyboardType = KeyboardType.Text,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier,
        verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
    ) {
        EtiquetaCampo(texto = etiqueta, mal = mal)
        OutlinedTextField(
            value = valor,
            onValueChange = alCambiar,
            placeholder = {
                Text(
                    text = pista,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoTenue,
                )
            },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
            colors = OutlinedTextFieldDefaults.colors(
                focusedContainerColor = OrderTema.FondoCampo,
                unfocusedContainerColor = OrderTema.FondoCampo,
                focusedBorderColor = if (mal) OrderTema.Vencida else OrderTema.AcentoSuave,
                unfocusedBorderColor = if (mal) OrderTema.Vencida else OrderTema.BordeCampo,
                focusedTextColor = OrderTema.Texto,
                unfocusedTextColor = OrderTema.Texto,
                cursorColor = OrderTema.Acento,
            ),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

@Composable
private fun CampoSelector(
    etiqueta: String,
    valor: String,
    pista: String,
    glifo: String,
    mal: Boolean = false,
    apagado: Boolean = false,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier,
        verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
    ) {
        EtiquetaCampo(texto = etiqueta, mal = mal, apagada = apagado)
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(42.dp)
                .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                .background(if (apagado) OrderTema.FondoPanel else OrderTema.FondoCampo)
                .border(
                    OrderTema.Linea,
                    if (mal) OrderTema.Vencida else OrderTema.BordeCampo,
                    RoundedCornerShape(OrderTema.RadioPequeno),
                )
                .clickable(enabled = !apagado, onClick = onClick)
                .padding(horizontal = OrderTema.EspacioSm),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
        ) {
            Text(
                text = valor.ifEmpty { pista },
                fontSize = OrderTema.TamanoPequeno,
                color = if (valor.isEmpty()) OrderTema.TextoTenue else OrderTema.Texto,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Text(
                text = glifo,
                fontSize = OrderTema.TamanoHora,
                color = OrderTema.TextoTenue,
            )
        }
    }
}
