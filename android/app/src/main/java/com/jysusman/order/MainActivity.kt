package com.jysusman.order

import android.Manifest
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
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
import com.jysusman.order.agente.ProcesadorAndroid
import com.jysusman.order.avisos.Canales
import com.jysusman.order.avisos.Programador
import com.jysusman.order.avisos.TrabajoRespaldo
import com.jysusman.order.datos.ObservadorCarpeta
import com.jysusman.order.datos.PreferenciasOrder
import com.jysusman.order.datos.PuenteOrderNativo
import com.jysusman.order.datos.Repositorio
import com.jysusman.order.datos.RutasPrivadas
import com.jysusman.order.ffi.TipoFechaFfi
import com.jysusman.order.ui.BannersEstado
import com.jysusman.order.ui.PantallaPermisoArchivos
import com.jysusman.order.ui.abrirAjustesAlarmaExacta
import com.jysusman.order.ui.abrirAjustesNotificaciones
import com.jysusman.order.ui.abrirAjustesPermisoArchivos
import com.jysusman.order.ui.agente.BarraAgente
import com.jysusman.order.ui.agente.PanelRespuesta
import com.jysusman.order.ui.agente.TarjetaConexionAgente
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
import com.jysusman.order.ui.tienePermisoAlarmaExacta
import com.jysusman.order.ui.tienePermisoArchivos
import com.jysusman.order.voz.Grabadora
import com.jysusman.order.voz.VozAndroid
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
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

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Canales.asegurarCanales(this)
        TrabajoRespaldo.programar(this)

        val puente = PuenteOrderNativo()
        val carpetaInicial = PreferenciasOrder.carpetaDatos(this)
        val programador = Programador.crear(applicationContext)
        val repositorio = Repositorio(
            puente = puente,
            carpetaInicial = carpetaInicial,
            rutaRegistroAvisos = RutasPrivadas.registroAvisos(applicationContext),
            rutaConfigAgente = RutasPrivadas.archivoAgenteConfig(applicationContext),
            rutaConfigMoodle = RutasPrivadas.archivoMoodleConfig(applicationContext),
            rutaConfigCalendarios = RutasPrivadas.archivoCalendariosConfig(applicationContext),
            onDatosActualizados = { programador.reprogramar() },
            procesadorAdjuntos = ProcesadorAndroid(applicationContext),
        )

        setContent {
            OrderTheme {
                OrderApp(
                    repositorio = repositorio,
                    programador = programador,
                    alCambiarCarpeta = { nueva ->
                        PreferenciasOrder.guardarCarpetaDatos(this, nueva)
                    },
                )
            }
        }
    }
}

@Composable
private fun OrderApp(
    repositorio: Repositorio,
    programador: Programador,
    alCambiarCarpeta: (String) -> Unit,
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val scope = rememberCoroutineScope()
    val estado by repositorio.estado.collectAsStateWithLifecycle()

    var permisoOk by remember { mutableStateOf(tienePermisoArchivos()) }
    var avisosActivos by remember { mutableStateOf(PreferenciasOrder.avisosActivos(context)) }
    var permisoNotifOk by remember { mutableStateOf(Canales.tienePermisoNotificaciones(context)) }
    var permisoAlarmaExactaOk by remember { mutableStateOf(tienePermisoAlarmaExacta(context)) }
    var notifSolicitada by rememberSaveable { mutableStateOf(false) }

    var pestana by rememberSaveable { mutableIntStateOf(0) }
    var formularioActivo by remember { mutableStateOf<FormularioActivo?>(null) }
    var selectorFiltro by remember { mutableStateOf(SelectorFiltro.Ninguno) }

    val grabadora = remember { Grabadora() }
    val vozAndroid = remember(context) { VozAndroid(context) }
    var grabandoVoz by remember { mutableStateOf(false) }
    var grabandoConSistema by remember { mutableStateOf(false) }
    var tiempoGrabacion by remember { mutableStateOf("0:00") }

    fun iniciarGrabacionVoz() {
        val usarSistema = PreferenciasOrder.usarVozAndroid(context)
        if (usarSistema) {
            grabandoConSistema = true
            grabandoVoz = true
            tiempoGrabacion = "escuchando…"
            repositorio.mostrarAvisoAgente("", esError = false)
            vozAndroid.empezar(
                textoActual = estado.agenteTexto,
                alActualizarTexto = { nuevo -> repositorio.actualizarTextoAgente(nuevo) },
                alTerminar = {
                    grabandoVoz = false
                    grabandoConSistema = false
                    tiempoGrabacion = ""
                },
                alError = { msg ->
                    grabandoVoz = false
                    grabandoConSistema = false
                    tiempoGrabacion = ""
                    repositorio.mostrarAvisoAgente(msg, esError = true)
                },
            )
        } else {
            grabandoConSistema = false
            grabadora.empezar(context)
                .onSuccess {
                    grabandoVoz = true
                    tiempoGrabacion = "0:00"
                    repositorio.mostrarAvisoAgente("", esError = false)
                }
                .onFailure { err ->
                    grabandoVoz = false
                    repositorio.mostrarAvisoAgente(err.message ?: "No se pudo usar el micrófono.", esError = true)
                }
        }
    }

    val lanzadorMicrofono = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { concedido ->
        if (concedido) {
            iniciarGrabacionVoz()
        } else {
            repositorio.mostrarAvisoAgente(
                "Permiso de micrófono denegado; puedes usar el micrófono de Gboard.",
                esError = true,
            )
        }
    }

    var fotoPendienteUri by remember { mutableStateOf<Uri?>(null) }
    var fotoPendienteArchivo by remember { mutableStateOf<File?>(null) }

    val lanzadorDocumento = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri ->
        if (uri != null) {
            scope.launch {
                val lim = repositorio.limitesAdjuntos()
                repositorio.adjuntarDesde {
                    ProcesadorAndroid.leerFuenteDesdeUri(
                        context = context,
                        uri = uri,
                        ladoLargo = lim.ladoLargoImagen.toInt(),
                        calidadJpeg = lim.calidadJpeg.toInt(),
                    )
                }
            }
        }
    }

    val lanzadorFoto = rememberLauncherForActivityResult(
        ActivityResultContracts.TakePicture()
    ) { ok ->
        val uri = fotoPendienteUri
        val archivo = fotoPendienteArchivo
        fotoPendienteUri = null
        fotoPendienteArchivo = null
        if (ok && uri != null) {
            scope.launch {
                val lim = repositorio.limitesAdjuntos()
                try {
                    repositorio.adjuntarDesde {
                        ProcesadorAndroid.leerFuenteDesdeUri(
                            context = context,
                            uri = uri,
                            ladoLargo = lim.ladoLargoImagen.toInt(),
                            calidadJpeg = lim.calidadJpeg.toInt(),
                            nombreForzado = "foto.jpg",
                        )
                    }
                } finally {
                    archivo?.delete()
                }
            }
        } else {
            archivo?.delete()
        }
    }

    fun abrirCamara() {
        runCatching {
            val dir = File(context.cacheDir, "fotos").apply { mkdirs() }
            val archivo = File.createTempFile("foto-", ".jpg", dir)
            val uri = FileProvider.getUriForFile(
                context,
                "${context.packageName}.fileprovider",
                archivo,
            )
            fotoPendienteArchivo = archivo
            fotoPendienteUri = uri
            lanzadorFoto.launch(uri)
        }.onFailure { err ->
            repositorio.mostrarAvisoAgente(
                err.message ?: "No se pudo abrir la cámara.",
                esError = true,
            )
        }
    }

    val lanzadorCamaraPermiso = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { concedido ->
        if (concedido) {
            abrirCamara()
        } else {
            repositorio.mostrarAvisoAgente(
                "Permiso de cámara denegado; puedes elegir una foto desde Archivo.",
                esError = true,
            )
        }
    }

    LaunchedEffect(grabandoVoz, grabandoConSistema) {
        while (grabandoVoz && !grabandoConSistema) {
            tiempoGrabacion = Grabadora.formatoTiempo(grabadora.segundosTranscurridos())
            delay(300)
        }
    }

    DisposableEffect(grabadora, vozAndroid) {
        onDispose {
            grabadora.cancelar()
            vozAndroid.cancelar()
        }
    }

    val lanzadorNotif = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { concedido ->
        permisoNotifOk = concedido && Canales.tienePermisoNotificaciones(context)
    }

    suspend fun mostrarResumenSiToca() {
        if (!avisosActivos || !Canales.tienePermisoNotificaciones(context)) return
        val resumen = repositorio.resumenDelDia() ?: return
        Canales.mostrarNotificacion(
            context = context,
            clave = resumen.clave,
            titulo = resumen.titulo,
            cuerpo = resumen.cuerpo,
            esRecordatorio = false,
        )
    }

    // Recarga al volver a primer plano (vuelta de Ajustes del permiso o Syncthing).
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                val ahoraTienePermiso = tienePermisoArchivos()
                permisoOk = ahoraTienePermiso
                permisoNotifOk = Canales.tienePermisoNotificaciones(context)
                permisoAlarmaExactaOk = tienePermisoAlarmaExacta(context)
                if (ahoraTienePermiso) {
                    scope.launch {
                        repositorio.recargar()
                        mostrarResumenSiToca()
                    }
                }
                if (repositorio.estado.value.conexionAbierta) {
                    scope.launch {
                        repositorio.revisarPortapapelesAlReanudar(leerPortapapeles(context))
                    }
                }
                if (ahoraTienePermiso) {
                    scope.launch {
                        repositorio.revisarPortapapelesIcal(leerPortapapeles(context))
                    }
                }
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    // Carga inicial al tener permiso y petición única de POST_NOTIFICATIONS.
    LaunchedEffect(permisoOk) {
        if (permisoOk) {
            if (!notifSolicitada &&
                Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                !Canales.tienePermisoNotificaciones(context)
            ) {
                notifSolicitada = true
                lanzadorNotif.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
            repositorio.cargarInicial()
            mostrarResumenSiToca()
            repositorio.revisarPortapapelesIcal(leerPortapapeles(context))
            repositorio.sincronizarTodoSilencioso()
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
                    faltaPermisoNotificaciones = avisosActivos && !permisoNotifOk,
                    onAbrirAjustesNotificaciones = { abrirAjustesNotificaciones(context) },
                    faltaPermisoAlarmaExacta = avisosActivos && !permisoAlarmaExactaOk,
                    onAbrirAjustesAlarmaExacta = { abrirAjustesAlarmaExacta(context) },
                    moodleRequiereReingreso = estado.moodleEstado?.requiereReingreso == true,
                    onAbrirMoodle = { formularioActivo = FormularioActivo.Moodle },
                    icalOferta = estado.icalOfertaPortapapeles,
                    onAceptarIcal = { scope.launch { repositorio.aceptarOfertaIcal() } },
                    onRechazarIcal = { repositorio.rechazarOfertaIcal() },
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
                val textosConexion = estado.textosConexion
                if (estado.conexionAbierta && textosConexion != null) {
                    TarjetaConexionAgente(
                        textos = textosConexion,
                        clave = estado.conexionKey,
                        verClave = estado.conexionVerKey,
                        conectando = estado.conexionConectando,
                        estadoTexto = estado.conexionEstadoTexto,
                        estadoError = estado.conexionEstadoError,
                        ofertaPortapapeles = estado.conexionOfertaPortapapeles,
                        alCambiarClave = { repositorio.actualizarClaveConexion(it) },
                        alAlternarVerClave = { repositorio.alternarVerClaveConexion() },
                        alAbrirStudio = {
                            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(textosConexion.urlStudio))
                            runCatching { context.startActivity(intent) }
                        },
                        alPegarClave = {
                            scope.launch {
                                repositorio.pegarClaveDesdePortapapeles(leerPortapapeles(context))
                            }
                        },
                        alAceptarOferta = { repositorio.aceptarClaveDePortapapeles() },
                        alConectar = { scope.launch { repositorio.conectarGemini() } },
                        alOtroProveedor = {
                            repositorio.cerrarConexionGuiada()
                            formularioActivo = FormularioActivo.Agente
                        },
                        alCerrar = { repositorio.cerrarConexionGuiada() },
                    )
                } else if (estado.agentePanelAbierto) {
                    PanelRespuesta(
                        respuesta = estado.agenteRespuesta,
                        nota = estado.agenteNota,
                        acciones = estado.agenteAcciones,
                        alAplicar = { id -> scope.launch { repositorio.aplicarAccionAgente(id) } },
                        alDescartar = { id -> scope.launch { repositorio.descartarAccionAgente(id) } },
                        alAplicarTodas = { scope.launch { repositorio.aplicarTodasAgente() } },
                        alCerrar = { repositorio.cerrarPanelAgente() },
                    )
                }
                BarraAgente(
                    texto = estado.agenteTexto,
                    pensando = estado.agentePensando,
                    compactando = estado.agenteCompactando,
                    transcribiendo = estado.agenteTranscribiendo,
                    leyendoArchivo = estado.agenteLeyendoArchivo,
                    grabando = grabandoVoz,
                    tiempoGrabacion = tiempoGrabacion,
                    aviso = estado.agenteAviso,
                    avisoEsError = estado.agenteAvisoEsError,
                    adjuntos = estado.agenteAdjuntos,
                    hayConversacion = estado.agenteHayConversacion,
                    tieneClave = estado.agenteTieneClave,
                    alCambiarTexto = { repositorio.actualizarTextoAgente(it) },
                    alGrabar = {
                        if (grabandoVoz) {
                            if (grabandoConSistema) {
                                vozAndroid.detener()
                                grabandoVoz = false
                                grabandoConSistema = false
                                tiempoGrabacion = ""
                            } else {
                                grabandoVoz = false
                                tiempoGrabacion = ""
                                val muestras = grabadora.detener()
                                scope.launch { repositorio.transcribirAudio(muestras) }
                            }
                        } else if (!Grabadora.tienePermisoMicrofono(context)) {
                            lanzadorMicrofono.launch(Manifest.permission.RECORD_AUDIO)
                        } else {
                            iniciarGrabacionVoz()
                        }
                    },
                    alElegirArchivo = {
                        lanzadorDocumento.launch(arrayOf("application/pdf", "image/*"))
                    },
                    alTomarFoto = {
                        val tieneCamara = ContextCompat.checkSelfPermission(
                            context,
                            Manifest.permission.CAMERA,
                        ) == PackageManager.PERMISSION_GRANTED
                        if (tieneCamara) {
                            abrirCamara()
                        } else {
                            lanzadorCamaraPermiso.launch(Manifest.permission.CAMERA)
                        }
                    },
                    alQuitarAdjunto = { id -> repositorio.quitarAdjunto(id) },
                    alNuevaConversacion = { scope.launch { repositorio.nuevaConversacion() } },
                    alAbrirConexion = { scope.launch { repositorio.abrirConexionGuiada() } },
                    alEnviar = { scope.launch { repositorio.enviarAlAgente() } },
                )
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
            avisosActivos = avisosActivos,
            alCambiarAvisosActivos = { activos ->
                avisosActivos = activos
                PreferenciasOrder.guardarAvisosActivos(context, activos)
                scope.launch {
                    withContext(Dispatchers.IO) { programador.reprogramar() }
                }
            },
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

private fun leerPortapapeles(context: Context): String? {
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager ?: return null
    val clip = clipboard.primaryClip ?: return null
    if (clip.itemCount == 0) return null
    return clip.getItemAt(0).coerceToText(context)?.toString()
}
