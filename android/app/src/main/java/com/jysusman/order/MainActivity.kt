package com.jysusman.order

import android.content.Context
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.CalendarViewWeek
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.Today
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.jysusman.order.datos.ObservadorCarpeta
import com.jysusman.order.datos.PuenteOrderNativo
import com.jysusman.order.datos.Repositorio
import com.jysusman.order.ffi.TipoFechaFfi
import com.jysusman.order.ui.BannersEstado
import com.jysusman.order.ui.PantallaPermisoArchivos
import com.jysusman.order.ui.abrirAjustesPermisoArchivos
import com.jysusman.order.ui.dia.DiaPantalla
import com.jysusman.order.ui.formularios.ContenedorFormularioSheet
import com.jysusman.order.ui.formularios.FormularioActivo
import com.jysusman.order.ui.inicio.InicioPantalla
import com.jysusman.order.ui.pendientes.PendientesPantalla
import com.jysusman.order.ui.selectores.SelectorFechaDialog
import com.jysusman.order.ui.selectores.SelectorHoraDialog
import com.jysusman.order.ui.selectores.SelectorMateriaDialog
import com.jysusman.order.ui.semana.SemanaPantalla
import com.jysusman.order.ui.theme.OrderTema
import com.jysusman.order.ui.theme.OrderTheme
import com.jysusman.order.ui.tienePermisoArchivos
import kotlinx.coroutines.launch
import java.io.File

enum class Pestana(val titulo: String, val icono: ImageVector) {
    Inicio("Inicio", Icons.Outlined.Home),
    Dia("Día", Icons.Outlined.Today),
    Semana("Semana", Icons.Outlined.CalendarViewWeek),
    Pendientes("Pendientes", Icons.Outlined.CheckCircle),
}

private enum class SelectorFiltro {
    Ninguno,
    Materia,
    Desde,
    Hasta,
    HoraDesde,
    HoraHasta,
}

private const val PREFS_ORDER = "order_prefs"
private const val CLAVE_CARPETA = "carpeta_datos"

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val prefs = getSharedPreferences(PREFS_ORDER, Context.MODE_PRIVATE)
        val puente = PuenteOrderNativo()
        val carpetaInicial = prefs.getString(CLAVE_CARPETA, null)
        val repositorio = Repositorio(puente = puente, carpetaInicial = carpetaInicial)

        setContent {
            OrderTheme {
                OrderApp(
                    repositorio = repositorio,
                    alCambiarCarpeta = { nueva ->
                        prefs.edit().putString(CLAVE_CARPETA, nueva).apply()
                    },
                )
            }
        }
    }
}

@Composable
private fun OrderApp(
    repositorio: Repositorio,
    alCambiarCarpeta: (String) -> Unit,
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val scope = rememberCoroutineScope()
    val estado by repositorio.estado.collectAsStateWithLifecycle()

    var permisoOk by remember { mutableStateOf(tienePermisoArchivos()) }
    var pestana by rememberSaveable { mutableIntStateOf(0) }
    var formularioActivo by remember { mutableStateOf<FormularioActivo?>(null) }
    var selectorFiltro by remember { mutableStateOf(SelectorFiltro.Ninguno) }

    // Recarga al volver a primer plano (vuelta de Ajustes del permiso o Syncthing).
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                val ahoraTienePermiso = tienePermisoArchivos()
                permisoOk = ahoraTienePermiso
                if (ahoraTienePermiso) {
                    scope.launch { repositorio.recargar() }
                }
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    // Carga inicial al tener permiso.
    LaunchedEffect(permisoOk) {
        if (permisoOk) {
            repositorio.cargarInicial()
        }
    }

    // Observa la carpeta de datos cuando hay permiso.
    DisposableEffect(permisoOk, estado.carpetaDatos) {
        if (!permisoOk || estado.carpetaDatos.isBlank()) {
            onDispose { }
        } else {
            val dir = File(estado.carpetaDatos).apply { mkdirs() }
            val observador = ObservadorCarpeta(dir, scope) {
                repositorio.recargar()
            }
            observador.startWatching()
            onDispose { observador.detener() }
        }
    }

    if (!permisoOk) {
        PantallaPermisoArchivos(
            carpeta = estado.carpetaDatos.ifBlank { "/storage/emulated/0/ORDER" },
            onAbrirAjustes = { abrirAjustesPermisoArchivos(context) },
            onReintentar = {
                permisoOk = tienePermisoArchivos()
                if (permisoOk) {
                    scope.launch { repositorio.cargarInicial() }
                }
            },
        )
        return
    }

    Scaffold(
        containerColor = OrderTema.Fondo,
        topBar = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(OrderTema.FondoPanel)
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = OrderTema.EspacioLg, vertical = OrderTema.EspacioSm),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Column {
                        Text(
                            text = "ORDER",
                            fontFamily = OrderTema.FuenteTitulos,
                            fontWeight = FontWeight.Bold,
                            fontSize = OrderTema.TamanoTitulo,
                            letterSpacing = OrderTema.LetraTitulo,
                            color = OrderTema.Texto,
                        )
                        Text(
                            text = "horario · tareas",
                            fontSize = OrderTema.TamanoHora,
                            letterSpacing = OrderTema.LetraNormal,
                            color = OrderTema.TextoTenue,
                        )
                    }
                    IconButton(onClick = { formularioActivo = FormularioActivo.Ajustes }) {
                        Icon(
                            imageVector = Icons.Outlined.Settings,
                            contentDescription = "Ajustes",
                            tint = OrderTema.TextoSuave,
                        )
                    }
                }
                HorizontalDivider(color = OrderTema.Borde, thickness = OrderTema.Linea)
                BannersEstado(
                    error = estado.error,
                    advertencias = estado.advertencias,
                    conflicto = estado.conflictos.firstOrNull(),
                    onQuedarActual = { scope.launch { repositorio.resolverConflicto(usarOtra = false) } },
                    onUsarOtra = { scope.launch { repositorio.resolverConflicto(usarOtra = true) } },
                )
            }
        },
        floatingActionButton = {
            FloatingActionButton(
                onClick = { formularioActivo = FormularioActivo.Tarea(id = null) },
                containerColor = OrderTema.AcentoTenue,
                contentColor = OrderTema.Acento,
                shape = RoundedCornerShape(OrderTema.RadioTarjeta),
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = OrderTema.EspacioMd),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
                ) {
                    Icon(Icons.Outlined.Add, contentDescription = null)
                    Text(
                        text = "Tarea",
                        fontFamily = OrderTema.FuenteTitulos,
                        fontSize = OrderTema.TamanoPequeno,
                        letterSpacing = OrderTema.LetraNormal,
                    )
                }
            }
        },
        bottomBar = {
            Column(modifier = Modifier.fillMaxWidth()) {
                val pie = estado.resumen?.pie ?: estado.inicio?.pie
                if (!pie.isNullOrBlank()) {
                    HorizontalDivider(color = OrderTema.Borde, thickness = OrderTema.Linea)
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(OrderTema.FondoPanel)
                            .padding(horizontal = OrderTema.EspacioMd, vertical = OrderTema.EspacioXs),
                    ) {
                        Text(
                            text = pie,
                            fontSize = OrderTema.TamanoPie,
                            color = OrderTema.TextoSuave,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                HorizontalDivider(color = OrderTema.Borde, thickness = OrderTema.Linea)
                NavigationBar(
                    containerColor = OrderTema.FondoPanel,
                    tonalElevation = OrderTema.Espacio2xs,
                ) {
                    Pestana.entries.forEachIndexed { idx, item ->
                        NavigationBarItem(
                            selected = pestana == idx,
                            onClick = { pestana = idx },
                            icon = { Icon(item.icono, contentDescription = item.titulo) },
                            label = {
                                Text(
                                    text = item.titulo,
                                    fontFamily = OrderTema.FuenteTitulos,
                                    fontSize = OrderTema.TamanoHora,
                                )
                            },
                            colors = NavigationBarItemDefaults.colors(
                                selectedIconColor = OrderTema.Acento,
                                selectedTextColor = OrderTema.Acento,
                                indicatorColor = OrderTema.AcentoTenue,
                                unselectedIconColor = OrderTema.TextoSuave,
                                unselectedTextColor = OrderTema.TextoSuave,
                            ),
                        )
                    }
                }
            }
        },
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
        ) {
            when (Pestana.entries[pestana]) {
                Pestana.Inicio -> InicioPantalla(
                    vista = estado.inicio,
                    onEditarClase = { id -> formularioActivo = FormularioActivo.Clase(id) },
                    onEditarTarea = { id -> formularioActivo = FormularioActivo.Tarea(id) },
                    onEditarRecordatorio = { id ->
                        formularioActivo = FormularioActivo.Recordatorio(id, volverAjustes = false)
                    },
                )

                Pestana.Dia -> DiaPantalla(
                    vista = estado.dia,
                    onAnterior = { scope.launch { repositorio.moverDia(-1) } },
                    onSiguiente = { scope.launch { repositorio.moverDia(1) } },
                    onIrHoy = { scope.launch { repositorio.irDiaHoy() } },
                    onNuevaClase = { formularioActivo = FormularioActivo.Clase(null) },
                    onEditarClase = { id -> formularioActivo = FormularioActivo.Clase(id) },
                    onEditarTarea = { id -> formularioActivo = FormularioActivo.Tarea(id) },
                    onEditarRecordatorio = { id ->
                        formularioActivo = FormularioActivo.Recordatorio(id, volverAjustes = false)
                    },
                )

                Pestana.Semana -> SemanaPantalla(
                    vista = estado.semana,
                    onAnterior = { scope.launch { repositorio.moverSemana(-7) } },
                    onSiguiente = { scope.launch { repositorio.moverSemana(7) } },
                    onIrHoy = { scope.launch { repositorio.irSemanaHoy() } },
                    onNuevaClase = { formularioActivo = FormularioActivo.Clase(null) },
                    onEditarClase = { id -> formularioActivo = FormularioActivo.Clase(id) },
                )

                Pestana.Pendientes -> PendientesPantalla(
                    vista = estado.pendientes,
                    filtro = estado.filtro,
                    onEditarTarea = { id -> formularioActivo = FormularioActivo.Tarea(id) },
                    onMarcarHecha = { id, hecha -> scope.launch { repositorio.marcarTarea(id, hecha) } },
                    onMostrarHechas = { mostrar ->
                        scope.launch {
                            repositorio.actualizarFiltro { it.copy(mostrarHechas = mostrar) }
                        }
                    },
                    onFiltroFecha = { tipo ->
                        scope.launch {
                            repositorio.actualizarFiltro { it.copy(tipoFecha = tipo) }
                        }
                    },
                    onElegirMateria = { selectorFiltro = SelectorFiltro.Materia },
                    onElegirDesde = { selectorFiltro = SelectorFiltro.Desde },
                    onElegirHasta = { selectorFiltro = SelectorFiltro.Hasta },
                    onElegirHoraDesde = { selectorFiltro = SelectorFiltro.HoraDesde },
                    onElegirHoraHasta = { selectorFiltro = SelectorFiltro.HoraHasta },
                    onQuitarFiltros = { scope.launch { repositorio.limpiarFiltros() } },
                )
            }
        }
    }

    formularioActivo?.let { activo ->
        ContenedorFormularioSheet(
            formulario = activo,
            repositorio = repositorio,
            materias = estado.materias,
            carpetaActual = estado.carpetaDatos,
            alCambiarCarpeta = alCambiarCarpeta,
            alAbrirFormulario = { formularioActivo = it },
            alCerrar = { formularioActivo = null },
        )
    }

    when (selectorFiltro) {
        SelectorFiltro.Ninguno -> {}
        SelectorFiltro.Materia -> SelectorMateriaDialog(
            materias = estado.materias,
            repositorio = repositorio,
            tituloSinMateria = "Todas las materias",
            onElegir = { materia ->
                scope.launch {
                    repositorio.actualizarFiltro {
                        it.copy(materia = materia.ifBlank { null })
                    }
                }
            },
            onCerrar = { selectorFiltro = SelectorFiltro.Ninguno },
        )

        SelectorFiltro.Desde -> SelectorFechaDialog(
            fechaInicialIso = estado.filtro.desdeIso.orEmpty(),
            repositorio = repositorio,
            onAceptar = { fecha ->
                scope.launch {
                    repositorio.actualizarFiltro {
                        it.copy(desdeIso = fecha, tipoFecha = TipoFechaFfi.RANGO)
                    }
                }
            },
            onLimpiar = {
                scope.launch {
                    repositorio.actualizarFiltro { it.copy(desdeIso = null) }
                }
            },
            onCerrar = { selectorFiltro = SelectorFiltro.Ninguno },
        )

        SelectorFiltro.Hasta -> SelectorFechaDialog(
            fechaInicialIso = estado.filtro.hastaIso.orEmpty(),
            repositorio = repositorio,
            onAceptar = { fecha ->
                scope.launch {
                    repositorio.actualizarFiltro {
                        it.copy(hastaIso = fecha, tipoFecha = TipoFechaFfi.RANGO)
                    }
                }
            },
            onLimpiar = {
                scope.launch {
                    repositorio.actualizarFiltro { it.copy(hastaIso = null) }
                }
            },
            onCerrar = { selectorFiltro = SelectorFiltro.Ninguno },
        )

        SelectorFiltro.HoraDesde -> SelectorHoraDialog(
            horaInicialTexto = estado.filtro.horaDesde.orEmpty(),
            repositorio = repositorio,
            onAceptar = { hora ->
                scope.launch {
                    repositorio.actualizarFiltro { it.copy(horaDesde = hora) }
                }
            },
            onLimpiar = {
                scope.launch {
                    repositorio.actualizarFiltro { it.copy(horaDesde = null) }
                }
            },
            onCerrar = { selectorFiltro = SelectorFiltro.Ninguno },
        )

        SelectorFiltro.HoraHasta -> SelectorHoraDialog(
            horaInicialTexto = estado.filtro.horaHasta.orEmpty(),
            repositorio = repositorio,
            onAceptar = { hora ->
                scope.launch {
                    repositorio.actualizarFiltro { it.copy(horaHasta = hora) }
                }
            },
            onLimpiar = {
                scope.launch {
                    repositorio.actualizarFiltro { it.copy(horaHasta = null) }
                }
            },
            onCerrar = { selectorFiltro = SelectorFiltro.Ninguno },
        )
    }
}
