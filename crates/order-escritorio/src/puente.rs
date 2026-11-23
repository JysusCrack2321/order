//! Puente entre los datos de ORDER y la ventana Slint.
//!
//! La lógica (semana, pendientes, vencidas) vive en `semana.rs`; aquí solo se
//! traduce a los modelos que dibuja la interfaz y se conectan los botones.
//! El estado de la ventana es de un solo hilo: Slint corre el bucle de eventos
//! en el hilo principal y estas funciones se llaman desde ahí.
//!
//! Todas las escrituras pasan por `Almacen::modificar` y las operaciones de
//! `ops`: si el archivo está roto o el cambio no se puede releer, no se escribe
//! nada y el error sale en el banner rojo mientras el formulario sigue abierto
//! con lo que el usuario había escrito.

use crate::adjuntos;
use crate::grabacion::{self, Grabacion};
use crate::voz_local;
use order_core::agente::acciones::{self, Accion};
use order_core::agente::adjuntos::{self as core_adjuntos, Adjunto, MAX_ADJUNTOS};
use order_core::agente::cliente::{self, ErrorAgente, EstadoConexion};
use order_core::agente::config::{
    self, AgenteConfig, Proveedor, VozProveedor, parece_clave_gemini, textos_conexion_agente,
};
use order_core::agente::contexto;
use order_core::agente::conversacion::Conversacion;
use order_core::agente::voz;
use order_core::almacen::Almacen;
use order_core::atajos::{self, AccionAtajo, AtajosConfig, Combinacion, Modificadores, Tecla};
use order_core::conflictos::{self, Conflicto};
use order_core::filtros::{Filtro, TipoFecha};
use order_core::ical::{self, config::CalendarioVista};
use order_core::modelo::{
    self, Ajustes, Clase, Datos, Dia, ErrorDatos, FORMATO_FECHA, Recordatorio, Tarea,
};
use order_core::moodle::{self, EstadoMoodle};
use order_core::ops;
use order_core::presentacion::formularios::{
    self, codigo_fecha, fecha_del_codigo, interpretar_fecha, interpretar_hora, opcional,
};
use order_core::presentacion::materias::{self, canonica, materias_de, tono_materia};
use order_core::presentacion::textos::{
    self, DIAS_SEMANA, cuando_recordatorio, dias_del_mes, dias_texto, etiqueta_mes,
    etiqueta_semana, fecha_larga, fecha_o_vacio, fecha_tarea, formato_tiempo, hora, hora_clase,
    hora_o_vacio, horas_texto, mes_vecino, minutos, minutos_texto, pie,
};
use order_core::reloj::{self, a_12h, a_24h, etiqueta_hora, hora_texto};
use order_core::semana;
use order_p2p::InfoDispositivo;
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike};
use slint::{ModelRc, SharedString, VecModel, Weak};
use std::cell::RefCell;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

slint::include_modules!();

/// Campos del formulario y de la barra de filtros que abren un selector.
/// Los del formulario usan 1–4 (ver `campo-mal` en app.slint); los filtros de
/// Pendientes van aparte para no confundirse con ellos.
const CAMPO_MATERIA: i32 = 1;
const CAMPO_CAL_MATERIA: i32 = 4;
const CAMPO_FILTRO_MATERIA: i32 = 21;
const CAMPO_FILTRO_DESDE: i32 = 22;
const CAMPO_FILTRO_HASTA: i32 = 23;
const CAMPO_FILTRO_HORA_DESDE: i32 = 24;
const CAMPO_FILTRO_HORA_HASTA: i32 = 25;

/// Qué formulario está abierto y sobre qué elemento trabaja. El `id` vacío
/// significa "crear"; los campos escritos viven en las propiedades de la
/// ventana, no aquí.
#[derive(Clone, PartialEq)]
enum Edicion {
    Ninguna,
    Tarea(String),
    Clase(String),
    Recordatorio(String),
    Ajustes,
    Agente,
    Moodle,
    Calendarios,
    Dispositivos,
    Atajos,
}

/// Copia inicial de los campos del formulario abierto para detectar cambios
/// sin guardar al intentar cerrar.
#[derive(Clone, PartialEq)]
enum FormSnapshot {
    Tarea {
        titulo: String,
        materia: String,
        fecha: String,
        hora: String,
        hecha: bool,
    },
    Clase {
        materia: String,
        dias: [bool; 7],
        inicio: String,
        fin: String,
        lugar: String,
    },
    Ajustes {
        sin_semestre: bool,
        autostart: bool,
        inicio: String,
        fin: String,
        hora: String,
        horas: String,
    },
    Recordatorio {
        texto: String,
        hora: String,
        dias: [bool; 7],
    },
    Agente {
        proveedor: i32,
        url: String,
        modelo: String,
        key: String,
        ve_imagenes: bool,
        limite_contexto: String,
        voz_proveedor: i32,
        v_url: String,
        v_modelo: String,
        v_key: String,
    },
    Moodle {
        sitio: String,
        usuario: String,
        contrasena: String,
    },
    Calendarios {
        url: String,
        nombre: String,
        materia: String,
    },
    Dispositivos {
        nombre: String,
        codigo_entrada: String,
    },
}

/// Qué selector está abierto y qué campo del formulario llena.
#[derive(Clone, Copy, PartialEq)]
enum Selector {
    Fecha,
    Hora,
    Materia,
}

/// Estado del selector abierto. El calendario y la hora se editan aquí y se
/// vuelcan a la ventana; al aceptar se escribe el texto en el campo.
#[derive(Clone)]
struct Seleccion {
    tipo: Selector,
    campo: i32,
    anio: i32,
    mes: u32,
    dia: u32,
    hora: u32,
    minuto: u32,
}

/// Lo que la ventana necesita en cada refresco.
struct Estado {
    datos: Option<Datos>,
    /// Lunes de la semana que se está viendo.
    lunes: NaiveDate,
    /// Fecha que muestra la pestaña Día.
    dia: NaiveDate,
    error: Option<String>,
    advertencias: Vec<String>,
    conflictos: Vec<Conflicto>,
    edicion: Edicion,
    seleccion: Option<Seleccion>,
    /// Barra de filtros de Pendientes: se conserva mientras la app vive.
    filtro: Filtro,
    /// El recordatorio abierto se abrió desde Ajustes: al salir se vuelve allí.
    volver_ajustes: bool,
    // --- Agente ---------------------------------------------------------
    /// Configuración leída de `agente.toml`.
    agente: AgenteConfig,
    /// Propuestas pendientes de confirmar, en el orden en que llegaron.
    acciones: Vec<Accion>,
    /// Texto del modelo para el panel.
    respuesta: String,
    /// Nota junto a la respuesta (p. ej. no se pudo leer una propuesta).
    nota: String,
    pensando: bool,
    pensando_texto: String,
    panel: bool,
    /// Aviso de la barra (clave ausente, por ejemplo).
    aviso: String,
    /// Grabación de voz en curso (el flujo de cpal vive en el hilo principal).
    grabacion: Option<Grabacion>,
    /// Reloj del indicador rojo de la barra.
    temporizador: Option<slint::Timer>,
    /// Adjuntos preparados para el próximo mensaje al agente: `(id, adjunto, es_pdf)`.
    adjuntos: Vec<(i32, Adjunto, bool)>,
    siguiente_adjunto_id: i32,
    leyendo_archivo: bool,
    /// Memoria completa de la conversación con el agente.
    conversacion: Conversacion,
    /// Tarjeta de conexión guiada de Gemini cuando falta la clave.
    conexion_abierta: bool,
    conexion_conectando: bool,
    conexion_estado_texto: String,
    conexion_estado_error: bool,
    /// Cancelación activa de descarga del modelo de voz local.
    descarga_voz_cancelar: Option<Arc<AtomicBool>>,
    descarga_voz_en_curso: bool,
    // --- Moodle ---------------------------------------------------------
    moodle: EstadoMoodle,
    moodle_ocupado: bool,
    moodle_aviso: String,
    // --- Calendarios iCal -----------------------------------------------
    calendarios: Vec<CalendarioVista>,
    ical_ocupado: bool,
    ical_aviso: String,
    ical_oferta: String,
    ical_descartadas: std::collections::HashSet<String>,
    // --- Dispositivos P2P -----------------------------------------------
    p2p_sincronizando: bool,
    p2p_ocupado: bool,
    p2p_aviso: String,
    p2p_dispositivos: Vec<InfoDispositivo>,
    p2p_mostrando_qr: bool,
    p2p_qr_imagen: slint::Image,
    p2p_palabras: String,
    p2p_enlace: String,
    p2p_espera_timer: Option<slint::Timer>,
    p2p_conteo_previo: usize,
    // --- Atajos, ayuda y cambios sin guardar ----------------------------
    form_inicial: Option<FormSnapshot>,
    descartar_abierto: bool,
    pendiente_cerrar_ventana: bool,
    ayuda_abierta: bool,
    atajos: AtajosConfig,
    atajo_capturando: Option<AccionAtajo>,
    atajo_choque_pendiente: Option<(AccionAtajo, Combinacion, AccionAtajo)>,
    sync_global_ocupado: bool,
}

fn ruta_conversacion() -> std::path::PathBuf {
    Conversacion::ruta_junto_a(&config::ruta())
}

impl Estado {
    fn nuevo() -> Estado {
        Estado {
            datos: None,
            lunes: semana::lunes_de(Local::now().date_naive()),
            dia: Local::now().date_naive(),
            error: None,
            advertencias: Vec::new(),
            conflictos: Vec::new(),
            edicion: Edicion::Ninguna,
            seleccion: None,
            filtro: Filtro::default(),
            volver_ajustes: false,
            agente: config::cargar_escritorio(),
            acciones: Vec::new(),
            respuesta: String::new(),
            nota: String::new(),
            pensando: false,
            pensando_texto: "pensando…".to_string(),
            panel: false,
            aviso: String::new(),
            grabacion: None,
            temporizador: None,
            adjuntos: Vec::new(),
            siguiente_adjunto_id: 1,
            leyendo_archivo: false,
            conversacion: Conversacion::cargar_de(&ruta_conversacion()),
            conexion_abierta: false,
            conexion_conectando: false,
            conexion_estado_texto: String::new(),
            conexion_estado_error: false,
            descarga_voz_cancelar: None,
            descarga_voz_en_curso: false,
            moodle: moodle::config::estado_de(&moodle::config::ruta()),
            moodle_ocupado: false,
            moodle_aviso: String::new(),
            calendarios: ical::config::listar_de(&ical::config::ruta()),
            ical_ocupado: false,
            ical_aviso: String::new(),
            ical_oferta: String::new(),
            ical_descartadas: std::collections::HashSet::new(),
            p2p_sincronizando: false,
            p2p_ocupado: false,
            p2p_aviso: String::new(),
            p2p_dispositivos: Vec::new(),
            p2p_mostrando_qr: false,
            p2p_qr_imagen: slint::Image::default(),
            p2p_palabras: String::new(),
            p2p_enlace: String::new(),
            p2p_espera_timer: None,
            p2p_conteo_previo: 0,
            form_inicial: None,
            descartar_abierto: false,
            pendiente_cerrar_ventana: false,
            ayuda_abierta: false,
            atajos: atajos::cargar(),
            atajo_capturando: None,
            atajo_choque_pendiente: None,
            sync_global_ocupado: false,
        }
    }
}

thread_local! {
    static VENTANA: RefCell<Option<AppWindow>> = const { RefCell::new(None) };
    static ESTADO: RefCell<Estado> = RefCell::new(Estado::nuevo());
}

/// Todo lo que se vuelca a las propiedades de la ventana.
struct Vista {
    dias: Vec<DiaUI>,
    etiqueta: String,
    es_actual: bool,
    base: i32,
    fin: i32,
    etiquetas: Vec<SharedString>,
    mostrar_ahora: bool,
    ahora_min: i32,
    hay_tareas: bool,
    tareas: Vec<TareaUI>,
    recordatorios: Vec<RecordatorioUI>,
    inicio_fecha: String,
    inicio_clases: Vec<ClaseInicioUI>,
    inicio_hay_siguiente: bool,
    inicio_siguiente: ClaseInicioUI,
    inicio_vencidas: Vec<TareaUI>,
    inicio_de_hoy: Vec<TareaUI>,
    inicio_proximas: Vec<TareaUI>,
    inicio_recordatorios: Vec<RecordatorioInicioUI>,
    inicio_por_materia: Vec<ConteoMateriaUI>,
    dia_etiqueta: String,
    dia_es_hoy: bool,
    dia_clases: Vec<ClaseInicioUI>,
    dia_tareas: Vec<TareaUI>,
    dia_recordatorios: Vec<RecordatorioInicioUI>,
    filtro_materia: String,
    filtro_fecha: i32,
    filtro_desde: String,
    filtro_hasta: String,
    filtro_hora_desde: String,
    filtro_hora_hasta: String,
    mostrar_hechas: bool,
    filtro_activo: bool,
    resumen: String,
    pie: String,
    error: SharedString,
    advertencias: Vec<SharedString>,
    conflicto_texto: String,
    agente_respuesta: SharedString,
    agente_acciones: Vec<AccionUI>,
    agente_pensando: bool,
    agente_pensando_texto: SharedString,
    agente_panel: bool,
    agente_aviso: SharedString,
    agente_nota: SharedString,
    agente_grabando: bool,
    agente_tiempo: SharedString,
    agente_adjuntos: Vec<AdjuntoUI>,
    agente_leyendo: bool,
    agente_hay_conversacion: bool,
    conexion_abierta: bool,
    conexion_conectando: bool,
    conexion_estado_texto: SharedString,
    conexion_estado_error: bool,
    moodle_banner: SharedString,
    moodle_conectado: bool,
    moodle_requiere_reingreso: bool,
    moodle_estado_texto: SharedString,
    moodle_ocupado: bool,
    moodle_aviso: SharedString,
    calendarios: Vec<CalendarioUI>,
    ical_ocupado: bool,
    ical_aviso: SharedString,
    ical_oferta: SharedString,
    p2p_sincronizando: bool,
    p2p_ocupado: bool,
    p2p_aviso: SharedString,
    p2p_dispositivos: Vec<DispositivoUI>,
    p2p_mostrando_qr: bool,
    p2p_qr_imagen: slint::Image,
    p2p_palabras: SharedString,
    p2p_enlace: SharedString,
    atajos_lista: Vec<AtajoUI>,
    atajo_capturando_id: SharedString,
    atajo_choque_abierto: bool,
    atajo_choque_mensaje: SharedString,
    descartar_abierto: bool,
    ayuda_abierta: bool,
}

// ---------------------------------------------------------------------------
// Interfaz pública (la que consume main.rs y, después, la Tarea 8)
// ---------------------------------------------------------------------------

/// Crea la ventana la primera vez y la muestra. Si ya existe, solo la muestra.
pub fn mostrar_ventana() {
    VENTANA.with(|celda| {
        let mut celda = celda.borrow_mut();
        if celda.is_none() {
            match AppWindow::new() {
                Ok(ui) => {
                    conectar(&ui);
                    *celda = Some(ui);
                }
                Err(error) => {
                    eprintln!("ORDER: no se pudo crear la ventana: {error}");
                    return;
                }
            }
        }
        if let Some(ui) = celda.as_ref() {
            revisar_portapapeles_ical();
            actualizar(ui);
            if let Err(error) = ui.show() {
                eprintln!("ORDER: no se pudo mostrar la ventana: {error}");
            } else {
                ui.invoke_enfocar_ventana();
            }
        }
    });
}

/// Clic en el icono de la bandeja: si la ventana se ve, se oculta; si no, se
/// muestra (creándola la primera vez).
pub fn alternar_ventana() {
    let visible = VENTANA.with(|celda| {
        celda
            .borrow()
            .as_ref()
            .is_some_and(|ui| ui.window().is_visible())
    });
    if visible {
        VENTANA.with(|celda| {
            if let Some(ui) = celda.borrow().as_ref() {
                let _ = ui.hide();
            }
        });
    } else {
        mostrar_ventana();
    }
}

/// Guarda los datos y vuelve a la semana actual. La usa main al arrancar y la
/// usará el observador del archivo al recargar (Tarea 8).
pub fn refrescar(datos: &Datos) {
    let hoy = Local::now().date_naive();
    con_estado(|estado| {
        estado.datos = Some(datos.clone());
        estado.lunes = semana::lunes_de(hoy);
        estado.dia = hoy;
        estado.error = None;
    });
    actualizar_si_existe();
}

/// Recarga que no mueve nada de la ventana: se usa cuando el archivo cambia
/// por fuera (Hermes o a mano). A diferencia de `refrescar`, no vuelve a la
/// semana actual ni cierra el formulario que estuviera abierto.
pub fn recargar(datos: &Datos) {
    con_estado(|estado| {
        estado.datos = Some(datos.clone());
        estado.error = None;
        // Las claves también pueden cambiar desde fuera.
        estado.agente = config::cargar_escritorio();
    });
    actualizar_si_existe();
}

/// Banner rojo: el archivo no se pudo leer. `None` lo esconde.
pub fn mostrar_error(error: Option<&ErrorDatos>) {
    con_estado(|estado| {
        estado.error = error.map(|e| format!("No se pudo leer order.toml — {e}"));
    });
    actualizar_si_existe();
}

/// Banner amarillo: claves que el modelo no conoce. No bloquean la lectura.
pub fn mostrar_advertencias(advertencias: &[ErrorDatos]) {
    const MAXIMO: usize = 4;
    let lineas: Vec<String> = advertencias
        .iter()
        .take(MAXIMO)
        .map(|a| a.to_string())
        .collect();
    let mut lineas = lineas;
    if advertencias.len() > MAXIMO {
        lineas.push(format!("… y {} más", advertencias.len() - MAXIMO));
    }
    con_estado(|estado| estado.advertencias = lineas);
    actualizar_si_existe();
}

/// Banner de conflictos de Syncthing: muestra el primero pendiente, si hay.
pub fn mostrar_conflictos(lista: Vec<Conflicto>) {
    con_estado(|estado| estado.conflictos = lista);
    actualizar_si_existe();
}

/// Relee el estado de Moodle desde `moodle.toml` y actualiza la ventana.
pub fn refrescar_moodle() {
    let moodle = moodle::config::estado_de(&moodle::config::ruta());
    con_estado(|estado| estado.moodle = moodle);
    actualizar_si_existe();
}

/// Relee la lista de calendarios desde `calendarios.toml` y actualiza la ventana.
pub fn refrescar_calendarios() {
    let calendarios = ical::config::listar_de(&ical::config::ruta());
    con_estado(|estado| estado.calendarios = calendarios);
    actualizar_si_existe();
}

/// Actualiza el indicador de sincronización P2P en el encabezado.
pub fn marcar_p2p_sincronizando(activo: bool) {
    con_estado(|estado| estado.p2p_sincronizando = activo);
    actualizar_si_existe();
}

/// Relee la lista de dispositivos emparejados desde `order-p2p`.
pub fn refrescar_dispositivos() {
    let lista = crate::sistema::p2p_servicio()
        .map(|s| s.dispositivos())
        .unwrap_or_default();
    con_estado(|estado| estado.p2p_dispositivos = lista);
    actualizar_si_existe();
}

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

fn conectar(ui: &AppWindow) {
    let textos = textos_conexion_agente();
    ui.set_conexion_titulo(textos.titulo.into());
    ui.set_conexion_paso_1(textos.paso_1.into());
    ui.set_conexion_paso_2(textos.paso_2.into());
    ui.set_conexion_paso_3(textos.paso_3.into());
    ui.set_conexion_texto_abrir_studio(textos.boton_abrir_studio.into());
    ui.set_conexion_texto_pegar(textos.boton_pegar.into());
    ui.set_conexion_etiqueta_clave(textos.etiqueta_clave.into());
    ui.set_conexion_pista_clave(textos.pista_clave.into());
    ui.set_conexion_texto_conectar(textos.boton_conectar.into());
    ui.set_conexion_texto_conectando(textos.conectando.into());
    ui.set_conexion_texto_otro_proveedor(textos.enlace_otro_proveedor.into());

    // Cerrar con la X oculta la ventana: ORDER sigue en la bandeja. Si hay
    // cambios sin guardar en el formulario, pregunta antes. Si se estaba
    // grabando, el micrófono se suelta.
    let debil = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = debil.upgrade() {
            if formulario_tiene_cambios(&ui) {
                con_estado(|estado| {
                    estado.descartar_abierto = true;
                    estado.pendiente_cerrar_ventana = true;
                });
                actualizar(&ui);
                return slint::CloseRequestResponse::KeepWindowShown;
            }
            soltar_grabacion(&ui);
        }
        slint::CloseRequestResponse::HideWindow
    });

    let debil = ui.as_weak();
    ui.on_semana_anterior(move || mover(&debil, -7));

    let debil = ui.as_weak();
    ui.on_semana_siguiente(move || mover(&debil, 7));

    let debil = ui.as_weak();
    ui.on_ir_hoy(move || {
        let Some(ui) = debil.upgrade() else { return };
        let hoy = Local::now().date_naive();
        con_estado(|estado| estado.lunes = semana::lunes_de(hoy));
        actualizar(&ui);
    });

    // --- Navegación del día ----------------------------------------------
    let debil = ui.as_weak();
    ui.on_dia_anterior(move || mover_dia(&debil, -1));

    let debil = ui.as_weak();
    ui.on_dia_siguiente(move || mover_dia(&debil, 1));

    let debil = ui.as_weak();
    ui.on_dia_hoy(move || {
        let Some(ui) = debil.upgrade() else { return };
        con_estado(|estado| estado.dia = Local::now().date_naive());
        actualizar(&ui);
    });

    // --- Tareas ----------------------------------------------------------
    let debil = ui.as_weak();
    ui.on_nueva_tarea(move || {
        let Some(ui) = debil.upgrade() else { return };
        abrir_tarea(&ui, None);
    });

    let debil = ui.as_weak();
    ui.on_editar_tarea(move |id: SharedString| {
        let Some(ui) = debil.upgrade() else { return };
        match tarea_por_id(&id) {
            Some(tarea) => abrir_tarea(&ui, Some(&tarea)),
            None => aviso(&ui, "Esa tarea ya no está en el archivo."),
        }
    });

    let debil = ui.as_weak();
    ui.on_marcar_hecha(move |id: SharedString, hecha: bool| {
        let Some(ui) = debil.upgrade() else { return };
        marcar(&ui, &id, hecha);
    });

    // --- Filtros de Pendientes -------------------------------------------
    let debil = ui.as_weak();
    ui.on_filtro_fecha(move |opcion: i32| {
        let Some(ui) = debil.upgrade() else { return };
        con_estado(|estado| estado.filtro.tipo_fecha = fecha_del_codigo(opcion));
        actualizar(&ui);
    });

    let debil = ui.as_weak();
    ui.on_filtro_hechas(move |mostrar: bool| {
        let Some(ui) = debil.upgrade() else { return };
        con_estado(|estado| estado.filtro.mostrar_hechas = mostrar);
        actualizar(&ui);
    });

    let debil = ui.as_weak();
    ui.on_filtro_limpiar(move || {
        let Some(ui) = debil.upgrade() else { return };
        con_estado(|estado| {
            // El interruptor de hechas no es un filtro: se queda como esté.
            let mostrar_hechas = estado.filtro.mostrar_hechas;
            estado.filtro = Filtro {
                mostrar_hechas,
                ..Filtro::default()
            };
        });
        actualizar(&ui);
    });

    // --- Clases ----------------------------------------------------------
    let debil = ui.as_weak();
    ui.on_nueva_clase(move || {
        let Some(ui) = debil.upgrade() else { return };
        abrir_clase(&ui, None);
    });

    let debil = ui.as_weak();
    ui.on_editar_clase(move |id: SharedString| {
        let Some(ui) = debil.upgrade() else { return };
        match clase_por_id(&id) {
            Some(clase) => abrir_clase(&ui, Some(&clase)),
            None => aviso(&ui, "Esa clase ya no está en el archivo."),
        }
    });

    // --- Ajustes y recordatorios -----------------------------------------
    let debil = ui.as_weak();
    ui.on_abrir_ajustes(move || {
        let Some(ui) = debil.upgrade() else { return };
        abrir_ajustes(&ui);
    });

    let debil = ui.as_weak();
    ui.on_nuevo_recordatorio(move || {
        let Some(ui) = debil.upgrade() else { return };
        abrir_recordatorio(&ui, None);
    });

    let debil = ui.as_weak();
    ui.on_editar_recordatorio(move |id: SharedString| {
        let Some(ui) = debil.upgrade() else { return };
        match recordatorio_por_id(&id) {
            Some(recordatorio) => abrir_recordatorio(&ui, Some(&recordatorio)),
            None => aviso(&ui, "Ese recordatorio ya no está en el archivo."),
        }
    });

    // --- Formularios -----------------------------------------------------
    let debil = ui.as_weak();
    ui.on_guardar_formulario(move || {
        let Some(ui) = debil.upgrade() else { return };
        guardar(&ui);
    });

    let debil = ui.as_weak();
    ui.on_cancelar_formulario(move || {
        let Some(ui) = debil.upgrade() else { return };
        intentar_cancelar_formulario(&ui);
    });

    let debil = ui.as_weak();
    ui.on_confirmar_borrado(move || {
        let Some(ui) = debil.upgrade() else { return };
        borrar(&ui);
    });

    // --- Selectores de fecha, hora y materia ------------------------------
    let debil = ui.as_weak();
    ui.on_abrir_fecha(move |campo: i32| {
        let Some(ui) = debil.upgrade() else { return };
        abrir_selector_fecha(&ui, campo);
    });

    let debil = ui.as_weak();
    ui.on_abrir_hora(move |campo: i32| {
        let Some(ui) = debil.upgrade() else { return };
        abrir_selector_hora(&ui, campo);
    });

    let debil = ui.as_weak();
    ui.on_abrir_materia(move |campo: i32| {
        let Some(ui) = debil.upgrade() else { return };
        abrir_selector_materia(&ui, campo);
    });

    let debil = ui.as_weak();
    ui.on_selector_cerrar(move || {
        let Some(ui) = debil.upgrade() else { return };
        cerrar_selector(&ui);
    });

    let debil = ui.as_weak();
    ui.on_selector_mes(move |delta: i32| {
        let Some(ui) = debil.upgrade() else { return };
        cambiar_mes(&ui, delta);
    });

    let debil = ui.as_weak();
    ui.on_selector_dia(move |dia: i32| {
        let Some(ui) = debil.upgrade() else { return };
        elegir_dia(&ui, dia);
    });

    let debil = ui.as_weak();
    ui.on_selector_hora12(move |hora12: i32| {
        let Some(ui) = debil.upgrade() else { return };
        elegir_hora12(&ui, hora12);
    });

    let debil = ui.as_weak();
    ui.on_selector_minuto(move |minuto: i32| {
        let Some(ui) = debil.upgrade() else { return };
        elegir_minuto(&ui, minuto);
    });

    let debil = ui.as_weak();
    ui.on_selector_pm(move |tarde: bool| {
        let Some(ui) = debil.upgrade() else { return };
        elegir_pm(&ui, tarde);
    });

    let debil = ui.as_weak();
    ui.on_selector_aceptar(move || {
        let Some(ui) = debil.upgrade() else { return };
        aceptar_selector(&ui);
    });

    let debil = ui.as_weak();
    ui.on_selector_escribir(move || {
        let Some(ui) = debil.upgrade() else { return };
        ui.set_materia_escribiendo(true);
    });

    let debil = ui.as_weak();
    ui.on_selector_materia(move |materia: SharedString| {
        let Some(ui) = debil.upgrade() else { return };
        aplicar_materia(&ui, &materia);
    });

    // --- Agente ----------------------------------------------------------
    let debil = ui.as_weak();
    ui.on_agente_enviar(move || {
        let Some(ui) = debil.upgrade() else { return };
        enviar_agente(&ui);
    });

    let debil = ui.as_weak();
    ui.on_agente_cerrar(move || {
        let Some(ui) = debil.upgrade() else { return };
        con_estado(|estado| {
            estado.panel = false;
            estado.respuesta.clear();
            estado.acciones.clear();
            estado.nota.clear();
        });
        actualizar(&ui);
    });

    let debil = ui.as_weak();
    ui.on_agente_aplicar(move |indice: i32| {
        let Some(ui) = debil.upgrade() else { return };
        aplicar_accion(&ui, indice);
    });

    let debil = ui.as_weak();
    ui.on_agente_descartar(move |indice: i32| {
        let Some(ui) = debil.upgrade() else { return };
        descartar_accion(&ui, indice);
    });

    let debil = ui.as_weak();
    ui.on_agente_aplicar_todas(move || {
        let Some(ui) = debil.upgrade() else { return };
        aplicar_todas(&ui);
    });

    let debil = ui.as_weak();
    ui.on_agente_adjuntar(move || {
        let Some(ui) = debil.upgrade() else { return };
        adjuntar_archivos(&ui);
    });

    let debil = ui.as_weak();
    ui.on_agente_quitar_adjunto(move |id: i32| {
        let Some(ui) = debil.upgrade() else { return };
        con_estado(|estado| {
            estado.adjuntos.retain(|(item_id, _, _)| *item_id != id);
            estado.aviso.clear();
        });
        actualizar(&ui);
    });

    let debil = ui.as_weak();
    ui.on_agente_nueva_conversacion(move || {
        let Some(ui) = debil.upgrade() else { return };
        if con_estado(|estado| estado.pensando) {
            return;
        }
        con_estado(|estado| {
            estado.conversacion = Conversacion::nueva();
            estado.panel = false;
            estado.respuesta.clear();
            estado.acciones.clear();
            estado.nota.clear();
            estado.aviso.clear();
        });
        Conversacion::borrar_de(&ruta_conversacion());
        actualizar(&ui);
    });

    let debil = ui.as_weak();
    ui.on_abrir_agente(move || {
        let Some(ui) = debil.upgrade() else { return };
        abrir_agente(&ui);
    });

    let debil = ui.as_weak();
    ui.on_abrir_moodle(move || {
        let Some(ui) = debil.upgrade() else { return };
        abrir_moodle(&ui);
    });

    let debil = ui.as_weak();
    ui.on_moodle_conectar(move || {
        let Some(ui) = debil.upgrade() else { return };
        conectar_moodle(&ui);
    });

    let debil = ui.as_weak();
    ui.on_moodle_sincronizar(move || {
        let Some(ui) = debil.upgrade() else { return };
        sincronizar_moodle(&ui);
    });

    let debil = ui.as_weak();
    ui.on_moodle_desconectar(move || {
        let Some(ui) = debil.upgrade() else { return };
        desconectar_moodle(&ui);
    });

    let debil = ui.as_weak();
    ui.on_abrir_calendarios(move || {
        let Some(ui) = debil.upgrade() else { return };
        abrir_calendarios(&ui);
    });

    let debil = ui.as_weak();
    ui.on_ical_agregar(move || {
        let Some(ui) = debil.upgrade() else { return };
        agregar_ical(&ui);
    });

    let debil = ui.as_weak();
    ui.on_ical_quitar(move |id: SharedString| {
        let Some(ui) = debil.upgrade() else { return };
        quitar_ical(&ui, &id);
    });

    let debil = ui.as_weak();
    ui.on_ical_sincronizar(move || {
        let Some(ui) = debil.upgrade() else { return };
        sincronizar_todo_desde_calendarios(&ui);
    });

    let debil = ui.as_weak();
    ui.on_ical_oferta_aceptar(move || {
        let Some(ui) = debil.upgrade() else { return };
        aceptar_oferta_ical(&ui);
    });

    let debil = ui.as_weak();
    ui.on_ical_oferta_rechazar(move || {
        let Some(ui) = debil.upgrade() else { return };
        con_estado(|estado| {
            let oferta = std::mem::take(&mut estado.ical_oferta);
            if !oferta.is_empty() {
                estado.ical_descartadas.insert(oferta);
            }
        });
        actualizar(&ui);
    });

    let debil = ui.as_weak();
    ui.on_abrir_dispositivos(move || {
        let Some(ui) = debil.upgrade() else { return };
        abrir_dispositivos(&ui);
    });

    let debil = ui.as_weak();
    ui.on_p2p_guardar_nombre(move || {
        let Some(ui) = debil.upgrade() else { return };
        guardar_nombre_p2p(&ui);
    });

    let debil = ui.as_weak();
    ui.on_p2p_generar_codigo(move || {
        let Some(ui) = debil.upgrade() else { return };
        generar_codigo_p2p(&ui);
    });

    let debil = ui.as_weak();
    ui.on_p2p_cerrar_codigo(move || {
        let Some(ui) = debil.upgrade() else { return };
        cerrar_codigo_p2p(&ui);
    });

    let debil = ui.as_weak();
    ui.on_p2p_copiar_enlace(move || {
        let Some(ui) = debil.upgrade() else { return };
        copiar_enlace_p2p(&ui);
    });

    let debil = ui.as_weak();
    ui.on_p2p_emparejar(move || {
        let Some(ui) = debil.upgrade() else { return };
        emparejar_p2p(&ui);
    });

    let debil = ui.as_weak();
    ui.on_p2p_quitar(move |id: SharedString| {
        let Some(ui) = debil.upgrade() else { return };
        quitar_p2p(&ui, &id);
    });

    let debil = ui.as_weak();
    ui.on_p2p_sincronizar(move || {
        let Some(ui) = debil.upgrade() else { return };
        sincronizar_p2p(&ui);
    });

    let debil = ui.as_weak();
    ui.on_abrir_enlace_tarea(move || {
        let Some(ui) = debil.upgrade() else { return };
        let enlace = ui.get_f_enlace().to_string();
        if !enlace.trim().is_empty() {
            crate::sistema::abrir_en_navegador(enlace.trim());
        }
    });

    let debil = ui.as_weak();
    ui.on_conexion_abrir_studio(move || {
        let Some(_ui) = debil.upgrade() else { return };
        crate::sistema::abrir_en_navegador(&textos_conexion_agente().url_studio);
    });

    let debil = ui.as_weak();
    ui.on_conexion_pegar(move || {
        let Some(ui) = debil.upgrade() else { return };
        pegar_clave_conexion(&ui);
    });

    let debil = ui.as_weak();
    ui.on_conexion_alternar_clave(move || {
        let Some(ui) = debil.upgrade() else { return };
        ui.set_conexion_ver_key(!ui.get_conexion_ver_key());
    });

    let debil = ui.as_weak();
    ui.on_conexion_conectar(move || {
        let Some(ui) = debil.upgrade() else { return };
        conectar_gemini_panel(&ui);
    });

    let debil = ui.as_weak();
    ui.on_conexion_otro_proveedor(move || {
        let Some(ui) = debil.upgrade() else { return };
        con_estado(|estado| {
            estado.conexion_abierta = false;
            estado.conexion_estado_texto.clear();
            estado.conexion_estado_error = false;
        });
        abrir_agente(&ui);
    });

    let debil = ui.as_weak();
    ui.on_conexion_cerrar(move || {
        let Some(ui) = debil.upgrade() else { return };
        con_estado(|estado| {
            estado.conexion_abierta = false;
            estado.conexion_estado_texto.clear();
            estado.conexion_estado_error = false;
        });
        actualizar(&ui);
    });

    let debil = ui.as_weak();
    ui.on_elegir_proveedor(move |codigo: i32| {
        let Some(ui) = debil.upgrade() else { return };
        let proveedor = Proveedor::de_codigo(codigo);
        // La ficha trae su URL y su modelo; quedan ocultos salvo en Avanzado.
        let agente = con_estado(|estado| estado.agente.clone()).con_proveedor(proveedor);
        ui.set_a_proveedor(proveedor.codigo());
        ui.set_a_url(agente.url_base.into());
        ui.set_a_modelo(agente.modelo.into());
        // "Otro" no tiene valores que poner: ahí hace falta Avanzado.
        ui.set_a_avanzado(proveedor == Proveedor::OpenAICompatible);
        ui.set_campo_mal(0);
    });

    let debil = ui.as_weak();
    ui.on_elegir_voz_proveedor(move |codigo: i32| {
        let Some(ui) = debil.upgrade() else { return };
        let proveedor = VozProveedor::de_codigo(codigo);
        let voz = con_estado(|estado| estado.agente.voz.clone()).con_proveedor(proveedor);
        let listo = voz::modelo_local_disponible(&voz::ruta_modelo_local());
        ui.set_a_voz_proveedor(proveedor.codigo());
        ui.set_v_url(voz.url_base.into());
        ui.set_v_modelo(voz.modelo.into());
        ui.set_a_voz_modelo_listo(listo);
        ui.set_campo_mal(0);
        if proveedor == VozProveedor::Local && !listo {
            abrir_dialogo_descarga_voz(&ui);
        }
    });

    let debil = ui.as_weak();
    ui.on_conseguir_clave(move |codigo: i32| {
        let Some(ui) = debil.upgrade() else { return };
        if let Some(url) = Proveedor::de_codigo(codigo).url_clave() {
            crate::sistema::abrir_en_navegador(url);
        }
        ui.set_campo_mal(0);
    });

    let debil = ui.as_weak();
    ui.on_conseguir_clave_voz(move |codigo: i32| {
        let Some(ui) = debil.upgrade() else { return };
        let url = VozProveedor::de_codigo(codigo).url_clave();
        if !url.is_empty() {
            crate::sistema::abrir_en_navegador(url);
        }
        ui.set_campo_mal(0);
    });

    let debil = ui.as_weak();
    ui.on_descarga_voz_abrir(move || {
        let Some(ui) = debil.upgrade() else { return };
        abrir_dialogo_descarga_voz(&ui);
    });

    let debil = ui.as_weak();
    ui.on_descarga_voz_iniciar(move || {
        let Some(ui) = debil.upgrade() else { return };
        iniciar_descarga_voz(&ui);
    });

    let debil = ui.as_weak();
    ui.on_descarga_voz_cancelar(move || {
        let Some(ui) = debil.upgrade() else { return };
        cancelar_descarga_voz(&ui);
    });

    let debil = ui.as_weak();
    ui.on_alternar_avanzado(move || {
        let Some(ui) = debil.upgrade() else { return };
        ui.set_a_avanzado(!ui.get_a_avanzado());
    });

    let debil = ui.as_weak();
    ui.on_alternar_clave(move || {
        let Some(ui) = debil.upgrade() else { return };
        ui.set_a_ver_key(!ui.get_a_ver_key());
    });

    let debil = ui.as_weak();
    ui.on_alternar_clave_voz(move || {
        let Some(ui) = debil.upgrade() else { return };
        ui.set_v_ver_key(!ui.get_v_ver_key());
    });

    let debil = ui.as_weak();
    ui.on_grabar_voz(move || {
        let Some(ui) = debil.upgrade() else { return };
        grabar_voz(&ui);
    });

    let debil = ui.as_weak();
    ui.on_conflicto_quedar_actual(move || {
        let Some(ui) = debil.upgrade() else { return };
        resolver_conflicto(&ui, false);
    });

    let debil = ui.as_weak();
    ui.on_conflicto_usar_otra(move || {
        let Some(ui) = debil.upgrade() else { return };
        resolver_conflicto(&ui, true);
    });

    // --- Atajos, ayuda y confirmación de cambios sin guardar -------------
    let debil = ui.as_weak();
    ui.on_capturar_tecla(
        move |tecla: SharedString, ctrl: bool, shift: bool, alt: bool, meta: bool, escribiendo: bool| {
            let Some(ui) = debil.upgrade() else {
                return false;
            };
            manejar_tecla(&ui, &tecla, ctrl, shift, alt, meta, escribiendo)
        },
    );

    let debil = ui.as_weak();
    ui.on_descartar_seguir(move || {
        let Some(ui) = debil.upgrade() else { return };
        descartar_seguir(&ui);
    });

    let debil = ui.as_weak();
    ui.on_descartar_confirmar(move || {
        let Some(ui) = debil.upgrade() else { return };
        descartar_confirmar(&ui);
    });

    let debil = ui.as_weak();
    ui.on_abrir_atajos(move || {
        let Some(ui) = debil.upgrade() else { return };
        abrir_atajos(&ui);
    });

    let debil = ui.as_weak();
    ui.on_atajo_empezar_captura(move |id: SharedString| {
        let Some(ui) = debil.upgrade() else { return };
        atajo_empezar_captura(&ui, &id);
    });

    let debil = ui.as_weak();
    ui.on_atajo_choque_confirmar(move || {
        let Some(ui) = debil.upgrade() else { return };
        atajo_choque_confirmar(&ui);
    });

    let debil = ui.as_weak();
    ui.on_atajo_choque_cancelar(move || {
        let Some(ui) = debil.upgrade() else { return };
        atajo_choque_cancelar(&ui);
    });

    let debil = ui.as_weak();
    ui.on_atajos_restaurar(move || {
        let Some(ui) = debil.upgrade() else { return };
        atajos_restaurar(&ui);
    });

    let debil = ui.as_weak();
    ui.on_ayuda_cerrar(move || {
        let Some(ui) = debil.upgrade() else { return };
        con_estado(|estado| estado.ayuda_abierta = false);
        actualizar(&ui);
        ui.invoke_enfocar_ventana();
    });
}

fn resolver_conflicto(ui: &AppWindow, usar_otra: bool) {
    let Some(conflicto) = con_estado(|estado| estado.conflictos.first().cloned()) else {
        return;
    };
    let almacen = almacen();
    let resultado = if usar_otra {
        conflictos::usar_la_otra(&almacen.ruta, &conflicto)
    } else {
        conflictos::quedarme_con_actual(&almacen.ruta, &conflicto)
    };

    match resultado {
        Ok(()) => {
            let restantes = conflictos::detectar(&almacen.ruta);
            match almacen.cargar() {
                Ok(datos) => {
                    crate::sistema::publicar(&datos);
                    let adv = std::fs::read_to_string(&almacen.ruta)
                        .map(|texto| modelo::advertencias(&texto))
                        .unwrap_or_default();
                    let lineas: Vec<String> = adv.iter().take(4).map(|a| a.to_string()).collect();
                    con_estado(|estado| {
                        estado.datos = Some(datos);
                        estado.error = None;
                        estado.advertencias = lineas;
                        estado.conflictos = restantes;
                    });
                }
                Err(error) => {
                    con_estado(|estado| {
                        estado.error = Some(format!("No se pudo leer order.toml — {error}"));
                        estado.conflictos = restantes;
                    });
                }
            }
        }
        Err(mensaje) => {
            con_estado(|estado| {
                estado.error = Some(format!("No se pudo resolver el conflicto — {mensaje}"));
            });
        }
    }
    actualizar(ui);
}

fn mover(debil: &Weak<AppWindow>, delta_dias: i64) {
    let Some(ui) = debil.upgrade() else { return };
    let delta = TimeDelta::days(delta_dias);
    con_estado(|estado| {
        estado.lunes = if delta_dias < 0 {
            estado.lunes - delta
        } else {
            estado.lunes + delta
        };
    });
    actualizar(&ui);
}

/// Mueve la fecha de la pestaña Día un día atrás o adelante.
fn mover_dia(debil: &Weak<AppWindow>, delta_dias: i64) {
    let Some(ui) = debil.upgrade() else { return };
    con_estado(|estado| {
        let delta = TimeDelta::days(delta_dias);
        estado.dia = if delta_dias < 0 {
            estado.dia - delta
        } else {
            estado.dia + delta
        };
    });
    actualizar(&ui);
}

// ---------------------------------------------------------------------------
// Formularios: abrir
// ---------------------------------------------------------------------------

fn abrir_tarea(ui: &AppWindow, tarea: Option<&Tarea>) {
    let (id, titulo, materia, fecha, hora, hecha, enlace) = match tarea {
        Some(t) => (
            t.id.clone(),
            t.titulo.clone(),
            t.materia.clone().unwrap_or_default(),
            t.vence.format(FORMATO_FECHA).to_string(),
            hora_texto(t.vence.time()),
            t.hecha,
            t.enlace.clone().unwrap_or_default(),
        ),
        None => (
            String::new(),
            String::new(),
            String::new(),
            (Local::now().date_naive() + TimeDelta::days(7))
                .format(FORMATO_FECHA)
                .to_string(),
            hora_texto(hora(23, 59)),
            false,
            String::new(),
        ),
    };

    con_estado(|estado| estado.edicion = Edicion::Tarea(id));
    ui.set_form_encabezado(
        if tarea.is_some() {
            "Editar tarea"
        } else {
            "Nueva tarea"
        }
        .into(),
    );
    ui.set_form_edita(tarea.is_some());
    ui.set_f_titulo(titulo.into());
    ui.set_f_materia(materia.into());
    ui.set_f_fecha(fecha.into());
    ui.set_f_hora(hora.into());
    ui.set_f_hecha(hecha);
    ui.set_f_enlace(enlace.into());
    abrir_panel(ui, 1);
}

fn abrir_clase(ui: &AppWindow, clase: Option<&Clase>) {
    let (id, materia, dias, inicio, fin, lugar) = match clase {
        Some(c) => (
            c.id.clone(),
            c.materia.clone(),
            c.dias.clone(),
            c.inicio,
            c.fin,
            c.lugar.clone().unwrap_or_default(),
        ),
        None => (
            String::new(),
            String::new(),
            vec![Dia::Lun],
            hora(7, 0),
            hora(9, 0),
            String::new(),
        ),
    };

    con_estado(|estado| estado.edicion = Edicion::Clase(id));
    ui.set_form_encabezado(
        if clase.is_some() {
            "Editar clase"
        } else {
            "Nueva clase"
        }
        .into(),
    );
    ui.set_form_edita(clase.is_some());
    ui.set_f_materia(materia.into());
    poner_dias_clase(ui, &dias);
    ui.set_f_hora(hora_texto(inicio).into());
    ui.set_f_fin(hora_texto(fin).into());
    ui.set_f_lugar(lugar.into());
    abrir_panel(ui, 2);
}

fn abrir_ajustes(ui: &AppWindow) {
    let datos = con_estado(|estado| estado.datos.clone()).unwrap_or_else(datos_vacios);
    let hoy = Local::now().date_naive();
    let (inicio, fin) = match &datos.semestre {
        Some(semestre) => (semestre.inicio, semestre.fin),
        None => (hoy, hoy + TimeDelta::days(120)),
    };

    con_estado(|estado| estado.edicion = Edicion::Ajustes);
    ui.set_form_encabezado("Ajustes".into());
    ui.set_form_edita(false);
    ui.set_a_sin_semestre(datos.semestre.is_none());
    ui.set_a_autostart(crate::sistema::inicio_automatico_activo());
    ui.set_a_inicio(inicio.format(FORMATO_FECHA).to_string().into());
    ui.set_a_fin(fin.format(FORMATO_FECHA).to_string().into());
    ui.set_a_hora(hora_texto(datos.ajustes.aviso_dia_antes).into());
    ui.set_a_horas(datos.ajustes.aviso_horas_antes.to_string().into());
    abrir_panel(ui, 3);
}

fn abrir_recordatorio(ui: &AppWindow, recordatorio: Option<&Recordatorio>) {
    // Si el panel de ajustes estaba abierto (se pinchó en su lista), al salir
    // se vuelve allí; si se llegó desde Inicio o Día, se cierra sin más.
    let desde_ajustes = con_estado(|estado| matches!(estado.edicion, Edicion::Ajustes));
    con_estado(|estado| estado.volver_ajustes = desde_ajustes);

    let (id, texto, dias, hora) = match recordatorio {
        Some(r) => (r.id.clone(), r.texto.clone(), r.dias.clone(), r.hora),
        None => (String::new(), String::new(), Vec::new(), hora(18, 0)),
    };

    con_estado(|estado| estado.edicion = Edicion::Recordatorio(id));
    ui.set_form_encabezado(
        if recordatorio.is_some() {
            "Editar recordatorio"
        } else {
            "Nuevo recordatorio"
        }
        .into(),
    );
    ui.set_form_edita(recordatorio.is_some());
    ui.set_r_texto(texto.into());
    ui.set_r_hora(hora_texto(hora).into());
    poner_dias_recordatorio(ui, &dias);
    abrir_panel(ui, 4);
}

/// Deja el panel listo para escribir: sin error ni confirmación pendientes.
fn abrir_panel(ui: &AppWindow, cual: i32) {
    let snap = con_estado(|estado| capturar_snapshot(ui, &estado.edicion, estado));
    con_estado(|estado| {
        estado.form_inicial = snap;
        estado.descartar_abierto = false;
        estado.pendiente_cerrar_ventana = false;
        estado.atajo_capturando = None;
        estado.atajo_choque_pendiente = None;
    });
    ui.set_form_error("".into());
    ui.set_campo_mal(0);
    ui.set_form_confirma(false);
    ui.set_formulario(cual);
    actualizar(ui);
    ui.invoke_enfocar_ventana();
}

fn cancelar(ui: &AppWindow) {
    let volver_ajustes = con_estado(|estado| {
        if let Some(timer) = estado.p2p_espera_timer.take() {
            timer.stop();
        }
        estado.p2p_mostrando_qr = false;
        estado.atajo_capturando = None;
        estado.atajo_choque_pendiente = None;
        estado.descartar_abierto = false;
        estado.pendiente_cerrar_ventana = false;
        estado.form_inicial = None;
        estado.volver_ajustes
            && matches!(
                estado.edicion,
                Edicion::Recordatorio(_)
                    | Edicion::Agente
                    | Edicion::Moodle
                    | Edicion::Calendarios
                    | Edicion::Dispositivos
                    | Edicion::Atajos
            )
    });
    ui.set_form_confirma(false);
    ui.set_form_error("".into());
    ui.set_campo_mal(0);
    if volver_ajustes {
        // El recordatorio se edita desde los ajustes: al salir se vuelve allí.
        abrir_ajustes(ui);
    } else {
        con_estado(|estado| estado.edicion = Edicion::Ninguna);
        ui.set_formulario(0);
        actualizar(ui);
        ui.invoke_enfocar_ventana();
    }
}

// ---------------------------------------------------------------------------
// Formularios: guardar
// ---------------------------------------------------------------------------

/// Reparte el guardado según el formulario abierto.
fn guardar(ui: &AppWindow) {
    match con_estado(|estado| estado.edicion.clone()) {
        Edicion::Tarea(_) => guardar_tarea(ui),
        Edicion::Clase(_) => guardar_clase(ui),
        Edicion::Recordatorio(_) => guardar_recordatorio(ui),
        Edicion::Ajustes => guardar_ajustes(ui),
        Edicion::Agente => guardar_agente(ui),
        Edicion::Moodle
        | Edicion::Calendarios
        | Edicion::Dispositivos
        | Edicion::Atajos
        | Edicion::Ninguna => {}
    }
}

fn guardar_tarea(ui: &AppWindow) {
    let nueva = match formularios::validar_tarea(
        &ui.get_f_titulo(),
        &ui.get_f_fecha(),
        &ui.get_f_hora(),
        &ui.get_f_materia(),
        &materias_actuales(),
    ) {
        Ok(nueva) => nueva,
        Err(err) => {
            falta(ui, err.campo, &err.mensaje);
            return;
        }
    };
    let hecha = ui.get_f_hecha();
    let edicion = con_estado(|estado| estado.edicion.clone());
    let hoy = Local::now().date_naive();

    let resultado = almacen().modificar(|doc| match &edicion {
        Edicion::Tarea(id) if !id.is_empty() => {
            // `editar_tarea` no toca `hecha`: se compara con lo que hay en el
            // disco recién leído y solo se marca si de verdad cambió.
            let antes = modelo::parsear(&doc.to_string())
                .ok()
                .and_then(|datos| datos.tareas.into_iter().find(|t| &t.id == id))
                .map(|t| t.hecha);
            ops::editar_tarea(doc, id, &nueva)?;
            if antes.is_some_and(|antes| antes != hecha) {
                ops::marcar_tarea(doc, id, hecha, hoy)?;
            }
            Ok(())
        }
        // Crear siempre por `ops::agregar_tarea` (el futuro cuadro del agente
        // usará la misma función).
        _ => ops::agregar_tarea(doc, &nueva).map(|_| ()),
    });

    terminar(ui, resultado, false);
}

fn guardar_clase(ui: &AppWindow) {
    let id = match con_estado(|estado| estado.edicion.clone()) {
        Edicion::Clase(id) => id,
        _ => String::new(),
    };
    let clase = match formularios::validar_clase(
        id,
        &ui.get_f_materia(),
        dias_de_clase(ui),
        &ui.get_f_hora(),
        &ui.get_f_fin(),
        &ui.get_f_lugar(),
        &materias_actuales(),
    ) {
        Ok(clase) => clase,
        Err(err) => {
            falta(ui, err.campo, &err.mensaje);
            return;
        }
    };

    let resultado = almacen().modificar(|doc| ops::guardar_clase(doc, &clase));
    terminar(ui, resultado, false);
}

fn guardar_recordatorio(ui: &AppWindow) {
    let id = match con_estado(|estado| estado.edicion.clone()) {
        Edicion::Recordatorio(id) => id,
        _ => String::new(),
    };
    let recordatorio = match formularios::validar_recordatorio(
        id,
        &ui.get_r_texto(),
        &ui.get_r_hora(),
        dias_del_recordatorio(ui),
    ) {
        Ok(recordatorio) => recordatorio,
        Err(err) => {
            falta(ui, err.campo, &err.mensaje);
            return;
        }
    };

    let resultado = almacen().modificar(|doc| ops::guardar_recordatorio(doc, &recordatorio));
    // Si se abrió desde Ajustes, al guardar se vuelve a su lista.
    let volver = con_estado(|estado| estado.volver_ajustes);
    terminar(ui, resultado, volver);
}

fn guardar_ajustes(ui: &AppWindow) {
    let (ajustes, semestre) = match formularios::validar_ajustes(
        ui.get_a_sin_semestre(),
        &ui.get_a_inicio(),
        &ui.get_a_fin(),
        &ui.get_a_hora(),
        &ui.get_a_horas(),
    ) {
        Ok(par) => par,
        Err(err) => {
            falta(ui, err.campo, &err.mensaje);
            return;
        }
    };
    let autostart = ui.get_a_autostart();
    let resultado = almacen().modificar(|doc| {
        ops::guardar_ajustes(doc, &ajustes)?;
        ops::guardar_semestre(doc, semestre.as_ref())
    });
    let resultado = match resultado {
        Ok(datos) => match crate::sistema::configurar_inicio_automatico(autostart) {
            Ok(()) => Ok(datos),
            Err(mensaje) => Err(ErrorDatos {
                linea: None,
                mensaje,
            }),
        },
        Err(err) => Err(err),
    };
    terminar(ui, resultado, false);
}

/// Cierra el formulario con los datos que devolvió `modificar`. Si el guardado
/// falló, el error va al banner rojo y no se toca lo escrito.
fn terminar(ui: &AppWindow, resultado: Result<Datos, ErrorDatos>, volver_ajustes: bool) {
    match resultado {
        Ok(datos) => {
            crate::sistema::publicar(&datos);
            con_estado(|estado| {
                estado.datos = Some(datos);
                estado.error = None;
                estado.edicion = Edicion::Ninguna;
                estado.form_inicial = None;
                estado.descartar_abierto = false;
                estado.pendiente_cerrar_ventana = false;
            });
            ui.set_form_confirma(false);
            ui.set_form_error("".into());
            ui.set_campo_mal(0);
            ui.set_formulario(0);
            actualizar(ui);
            if volver_ajustes {
                abrir_ajustes(ui);
            } else {
                ui.invoke_enfocar_ventana();
            }
        }
        Err(error) => {
            con_estado(|estado| {
                estado.error = Some(format!("No se pudo guardar — {error}"));
            });
            ui.set_form_confirma(false);
            actualizar(ui);
        }
    }
}

/// Mensaje dentro del formulario, con el campo marcado.
fn falta(ui: &AppWindow, campo: i32, mensaje: &str) {
    ui.set_campo_mal(campo);
    ui.set_form_error(mensaje.into());
}

/// Aviso breve en el banner rojo (el elemento ya no existe en el archivo).
fn aviso(ui: &AppWindow, mensaje: &str) {
    con_estado(|estado| estado.error = Some(mensaje.to_string()));
    actualizar(ui);
}

// ---------------------------------------------------------------------------
// Borrar y marcar
// ---------------------------------------------------------------------------

fn borrar(ui: &AppWindow) {
    let edicion = con_estado(|estado| estado.edicion.clone());
    let volver_ajustes = con_estado(|estado| estado.volver_ajustes);
    let resultado = almacen().modificar(|doc| match &edicion {
        Edicion::Tarea(id) => ops::borrar_tarea(doc, id),
        Edicion::Clase(id) => ops::borrar_clase(doc, id),
        Edicion::Recordatorio(id) => ops::borrar_recordatorio(doc, id),
        _ => Err("no hay nada que borrar".to_string()),
    });

    ui.set_form_confirma(false);
    match resultado {
        Ok(datos) => {
            let volver_ajustes =
                volver_ajustes && matches!(edicion, Edicion::Recordatorio(_));
            crate::sistema::publicar(&datos);
            con_estado(|estado| {
                estado.datos = Some(datos);
                estado.error = None;
                estado.edicion = Edicion::Ninguna;
                estado.form_inicial = None;
                estado.descartar_abierto = false;
                estado.pendiente_cerrar_ventana = false;
            });
            ui.set_formulario(0);
            actualizar(ui);
            if volver_ajustes {
                abrir_ajustes(ui);
            } else {
                ui.invoke_enfocar_ventana();
            }
        }
        Err(error) => {
            con_estado(|estado| {
                estado.error = Some(format!("No se pudo borrar — {error}"));
            });
            actualizar(ui);
        }
    }
}

/// La casilla de Pendientes: marca (o desmarca) la tarea. Al marcar se anota
/// la fecha de hoy, que es la que usa el archivado a los 30 días.
fn marcar(ui: &AppWindow, id: &str, hecha: bool) {
    let hoy = Local::now().date_naive();
    let resultado = almacen().modificar(|doc| ops::marcar_tarea(doc, id, hecha, hoy));
    match resultado {
        Ok(datos) => {
            crate::sistema::publicar(&datos);
            con_estado(|estado| {
                estado.datos = Some(datos);
                estado.error = None;
            });
        }
        Err(error) => {
            let que = if hecha { "marcar" } else { "desmarcar" };
            con_estado(|estado| estado.error = Some(format!("No se pudo {que} — {error}")));
        }
    }
    actualizar(ui);
}

// ---------------------------------------------------------------------------
// Agente
// ---------------------------------------------------------------------------

/// Abre Ajustes → Agente con lo que hay en `agente.toml`.
fn abrir_agente(ui: &AppWindow) {
    let agente = con_estado(|estado| estado.agente.clone());
    con_estado(|estado| {
        estado.edicion = Edicion::Agente;
        estado.volver_ajustes = true;
    });
    ui.set_form_encabezado("Agente".into());
    ui.set_form_edita(false);
    ui.set_a_proveedor(agente.proveedor.codigo());
    ui.set_a_url(agente.url_base.into());
    ui.set_a_modelo(agente.modelo.into());
    ui.set_a_key(agente.api_key.into());
    ui.set_a_ver_key(false);
    ui.set_a_ve_imagenes(agente.ve_imagenes);
    ui.set_a_limite_contexto(agente.limite_contexto_tokens.to_string().into());
    ui.set_a_voz_proveedor(agente.voz.proveedor.codigo());
    ui.set_v_url(agente.voz.url_base.into());
    ui.set_v_modelo(agente.voz.modelo.into());
    ui.set_v_key(agente.voz.api_key.into());
    ui.set_v_ver_key(false);
    ui.set_a_voz_modelo_listo(voz::modelo_local_disponible(&voz::ruta_modelo_local()));
    // Avanzado arranca cerrado; con "Otro" se abre porque ahí hace falta.
    ui.set_a_avanzado(agente.proveedor == Proveedor::OpenAICompatible);
    ui.set_campo_mal(0);
    ui.set_form_error("".into());
    abrir_panel(ui, 5);
}

/// Guarda la sección del agente; vuelve a Ajustes.
fn guardar_agente(ui: &AppWindow) {
    let mut agente = match formularios::validar_agente(
        ui.get_a_proveedor(),
        &ui.get_a_url(),
        &ui.get_a_modelo(),
        &ui.get_a_key(),
        ui.get_a_voz_proveedor(),
        &ui.get_v_url(),
        &ui.get_v_modelo(),
        &ui.get_v_key(),
    ) {
        Ok(agente) => agente,
        Err(err) => {
            // Si el campo marcado vive en Avanzado, se abre para que se vea.
            if (1..=4).contains(&err.campo) {
                ui.set_a_avanzado(true);
            }
            falta(ui, err.campo, &err.mensaje);
            return;
        }
    };
    let limite_contexto = match formularios::validar_limite_contexto(&ui.get_a_limite_contexto()) {
        Ok(limite) => limite,
        Err(err) => {
            ui.set_a_avanzado(true);
            falta(ui, err.campo, &err.mensaje);
            return;
        }
    };
    agente.ve_imagenes = ui.get_a_ve_imagenes();
    agente.limite_contexto_tokens = limite_contexto;

    match config::guardar(&agente) {
        Ok(()) => {
            con_estado(|estado| {
                estado.agente = agente;
                estado.edicion = Edicion::Ninguna;
                estado.error = None;
                estado.aviso = String::new();
                estado.conexion_abierta = false;
                estado.conexion_estado_texto.clear();
                estado.conexion_estado_error = false;
            });
            ui.set_form_error("".into());
            ui.set_campo_mal(0);
            ui.set_formulario(0);
            abrir_ajustes(ui);
        }
        Err(mensaje) => {
            con_estado(|estado| {
                estado.error = Some(format!("No se pudo guardar agente.toml — {mensaje}"));
            });
            actualizar(ui);
        }
    }
}

// ---------------------------------------------------------------------------
// Moodle
// ---------------------------------------------------------------------------

fn abrir_moodle(ui: &AppWindow) {
    let ruta_moodle = moodle::config::ruta();
    let est = moodle::config::estado_de(&ruta_moodle);
    con_estado(|estado| {
        estado.moodle = est.clone();
        estado.moodle_aviso.clear();
        estado.edicion = Edicion::Moodle;
        estado.volver_ajustes = true;
    });
    ui.set_m_sitio(est.sitio.into());
    ui.set_m_contrasena("".into());
    ui.set_m_ver_pass(false);
    ui.set_form_encabezado("Moodle".into());
    ui.set_form_edita(false);
    abrir_panel(ui, 6);
}

fn conectar_moodle(ui: &AppWindow) {
    if con_estado(|estado| estado.moodle_ocupado) {
        return;
    }
    let sitio = ui.get_m_sitio().to_string();
    let usuario = ui.get_m_usuario().to_string();
    let contrasena = ui.get_m_contrasena().to_string();

    if sitio.trim().is_empty() || usuario.trim().is_empty() || contrasena.is_empty() {
        falta(ui, 0, "Escribe sitio, usuario y contraseña.");
        return;
    }

    ui.set_form_error("".into());
    con_estado(|estado| {
        estado.moodle_ocupado = true;
        estado.moodle_aviso = "Conectando y sincronizando…".to_string();
    });
    actualizar(ui);

    let debil = ui.as_weak();
    let ruta_datos = almacen().ruta;
    std::thread::spawn(move || {
        let ruta_moodle = moodle::config::ruta();
        let res_con = moodle::conectar(&ruta_moodle, &sitio, &usuario, &contrasena);
        let resultado = match res_con {
            Ok(_) => {
                let hoy = Local::now().date_naive();
                let sync = moodle::sincronizar_ahora(&ruta_datos, &ruta_moodle, hoy);
                let datos = Almacen { ruta: ruta_datos }.cargar().ok();
                let est = moodle::config::estado_de(&ruta_moodle);
                Ok((est, sync, datos))
            }
            Err(err) => Err(err.to_string()),
        };

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = debil.upgrade() else { return };
            match resultado {
                Ok((est, sync, datos)) => {
                    ui.set_m_contrasena("".into());
                    ui.set_m_ver_pass(false);
                    ui.set_form_error("".into());
                    if let Some(ref d) = datos {
                        crate::sistema::publicar(d);
                    }
                    con_estado(|estado| {
                        estado.moodle_ocupado = false;
                        estado.moodle = est;
                        if let Some(d) = datos {
                            estado.datos = Some(d);
                        }
                        estado.moodle_aviso = match sync {
                            Ok(r) => format!(
                                "Sincronizado: {} creadas, {} actualizadas.",
                                r.creadas, r.actualizadas
                            ),
                            Err(e) => format!("Conectado, pero no se pudo sincronizar: {e}"),
                        };
                        if matches!(estado.edicion, Edicion::Moodle) {
                            estado.form_inicial = capturar_snapshot(&ui, &estado.edicion, estado);
                        }
                    });
                }
                Err(msg) => {
                    con_estado(|estado| {
                        estado.moodle_ocupado = false;
                        estado.moodle_aviso.clear();
                    });
                    ui.set_form_error(msg.into());
                }
            }
            actualizar(&ui);
        });
    });
}

fn sincronizar_moodle(ui: &AppWindow) {
    if con_estado(|estado| estado.moodle_ocupado) {
        return;
    }
    ui.set_form_error("".into());
    con_estado(|estado| {
        estado.moodle_ocupado = true;
        estado.moodle_aviso = "Sincronizando…".to_string();
    });
    actualizar(ui);

    let debil = ui.as_weak();
    let ruta_datos = almacen().ruta;
    std::thread::spawn(move || {
        let ruta_moodle = moodle::config::ruta();
        let hoy = Local::now().date_naive();
        let sync = moodle::sincronizar_ahora(&ruta_datos, &ruta_moodle, hoy);
        let ruta_cal = ical::config::ruta();
        let sync_cal = if !ical::config::cargar_de(&ruta_cal).calendarios.is_empty() {
            Some(ical::sincronizar_calendarios(&ruta_datos, &ruta_cal, hoy))
        } else {
            None
        };
        let datos = if sync.is_ok() || sync_cal.as_ref().is_some_and(|r| r.is_ok()) {
            Almacen { ruta: ruta_datos }.cargar().ok()
        } else {
            None
        };
        let est = moodle::config::estado_de(&ruta_moodle);
        let cals = ical::config::listar_de(&ruta_cal);

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = debil.upgrade() else { return };
            if let Some(ref d) = datos {
                crate::sistema::publicar(d);
            }
            con_estado(|estado| {
                estado.moodle_ocupado = false;
                estado.moodle = est;
                estado.calendarios = cals;
                if let Some(d) = datos {
                    estado.datos = Some(d);
                }
                match sync {
                    Ok(r) => {
                        let extra_cal = match sync_cal {
                            Some(Ok(rc)) => (rc.creadas, rc.actualizadas),
                            _ => (0, 0),
                        };
                        estado.moodle_aviso = format!(
                            "Listo: {} creadas, {} actualizadas, {} completadas.",
                            r.creadas + extra_cal.0,
                            r.actualizadas + extra_cal.1,
                            r.hechas
                        );
                        ui.set_form_error("".into());
                    }
                    Err(err) => {
                        estado.moodle_aviso.clear();
                        ui.set_form_error(err.to_string().into());
                    }
                }
            });
            actualizar(&ui);
        });
    });
}

fn desconectar_moodle(ui: &AppWindow) {
    if con_estado(|estado| estado.moodle_ocupado) {
        return;
    }
    let ruta_moodle = moodle::config::ruta();
    if let Err(err) = moodle::config::borrar_en(&ruta_moodle) {
        ui.set_form_error(err.into());
        return;
    }
    let est = moodle::config::estado_de(&ruta_moodle);
    ui.set_m_contrasena("".into());
    ui.set_m_ver_pass(false);
    ui.set_form_error("".into());
    ui.set_m_sitio(est.sitio.clone().into());
    con_estado(|estado| {
        estado.moodle = est;
        estado.moodle_aviso = "Desconectado de Moodle.".to_string();
    });
    actualizar(ui);
}

// ---------------------------------------------------------------------------
// Calendarios iCal
// ---------------------------------------------------------------------------

fn revisar_portapapeles_ical() {
    let Some(candidato) = crate::sistema::leer_portapapeles() else {
        return;
    };
    if !ical::parece_url_ical(&candidato) {
        return;
    }
    let id = ical::hash_corto_url(&candidato);
    let ruta_cal = ical::config::ruta();
    let ya_existe = ical::config::cargar_de(&ruta_cal)
        .calendarios
        .iter()
        .any(|c| c.id == id);
    if ya_existe {
        return;
    }
    con_estado(|estado| {
        if !estado.ical_descartadas.contains(&candidato) {
            estado.ical_oferta = candidato;
        }
    });
}

fn abrir_calendarios(ui: &AppWindow) {
    let cals = ical::config::listar_de(&ical::config::ruta());
    con_estado(|estado| {
        estado.calendarios = cals;
        estado.ical_aviso.clear();
        estado.edicion = Edicion::Calendarios;
        estado.volver_ajustes = true;
    });
    ui.set_c_url("".into());
    ui.set_c_nombre("".into());
    ui.set_c_materia("".into());
    ui.set_form_encabezado("Calendarios".into());
    ui.set_form_edita(false);
    abrir_panel(ui, 7);
}

fn agregar_ical(ui: &AppWindow) {
    if con_estado(|estado| estado.ical_ocupado) {
        return;
    }
    let url = ui.get_c_url().to_string();
    let nombre = ui.get_c_nombre().to_string();
    let materia = ui.get_c_materia().to_string();
    if url.trim().is_empty() {
        falta(ui, 0, "Pega la dirección iCal (.ics o webcal://).");
        return;
    }

    ui.set_form_error("".into());
    con_estado(|estado| {
        estado.ical_ocupado = true;
        estado.ical_aviso = "Descargando y sincronizando…".to_string();
    });
    actualizar(ui);

    let debil = ui.as_weak();
    let ruta_datos = almacen().ruta;
    std::thread::spawn(move || {
        let ruta_cal = ical::config::ruta();
        let res_add = ical::agregar_calendario(&ruta_cal, &url, Some(&nombre), Some(&materia));
        let resultado = match res_add {
            Ok(_) => {
                let hoy = Local::now().date_naive();
                let sync = ical::sincronizar_calendarios(&ruta_datos, &ruta_cal, hoy);
                let datos = Almacen { ruta: ruta_datos }.cargar().ok();
                let cals = ical::config::listar_de(&ruta_cal);
                Ok((cals, sync, datos))
            }
            Err(err) => Err(err),
        };

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = debil.upgrade() else { return };
            match resultado {
                Ok((cals, sync, datos)) => {
                    ui.set_c_url("".into());
                    ui.set_c_nombre("".into());
                    ui.set_c_materia("".into());
                    ui.set_form_error("".into());
                    if let Some(ref d) = datos {
                        crate::sistema::publicar(d);
                    }
                    con_estado(|estado| {
                        estado.ical_ocupado = false;
                        estado.calendarios = cals;
                        estado.ical_oferta.clear();
                        if let Some(d) = datos {
                            estado.datos = Some(d);
                        }
                        estado.ical_aviso = match sync {
                            Ok(r) => format!(
                                "Calendario agregado ({} tareas creadas).",
                                r.creadas
                            ),
                            Err(e) => format!("Agregado, pero no se pudo sincronizar: {e}"),
                        };
                        if matches!(estado.edicion, Edicion::Calendarios) {
                            estado.form_inicial = capturar_snapshot(&ui, &estado.edicion, estado);
                        }
                    });
                }
                Err(msg) => {
                    con_estado(|estado| {
                        estado.ical_ocupado = false;
                        estado.ical_aviso.clear();
                    });
                    ui.set_form_error(msg.into());
                }
            }
            actualizar(&ui);
        });
    });
}

fn quitar_ical(ui: &AppWindow, id: &str) {
    if con_estado(|estado| estado.ical_ocupado) {
        return;
    }
    let ruta_cal = ical::config::ruta();
    if let Err(err) = ical::quitar_calendario(&ruta_cal, id) {
        ui.set_form_error(err.into());
        return;
    }
    let cals = ical::config::listar_de(&ruta_cal);
    con_estado(|estado| {
        estado.calendarios = cals;
        estado.ical_aviso = "Calendario quitado (las tareas existentes se conservan).".to_string();
    });
    actualizar(ui);
}

fn sincronizar_todo_desde_calendarios(ui: &AppWindow) {
    if con_estado(|estado| estado.ical_ocupado) {
        return;
    }
    ui.set_form_error("".into());
    con_estado(|estado| {
        estado.ical_ocupado = true;
        estado.ical_aviso = "Sincronizando…".to_string();
    });
    actualizar(ui);

    let debil = ui.as_weak();
    let ruta_datos = almacen().ruta;
    std::thread::spawn(move || {
        let hoy = Local::now().date_naive();
        let ruta_cal = ical::config::ruta();
        let ruta_moodle = moodle::config::ruta();

        let sync_cal = ical::sincronizar_calendarios(&ruta_datos, &ruta_cal, hoy);
        let sync_moodle = if moodle::config::cargar_de(&ruta_moodle).conectado() {
            Some(moodle::sincronizar_ahora(&ruta_datos, &ruta_moodle, hoy))
        } else {
            None
        };

        let datos = Almacen { ruta: ruta_datos }.cargar().ok();
        let cals = ical::config::listar_de(&ruta_cal);
        let est_moodle = moodle::config::estado_de(&ruta_moodle);

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = debil.upgrade() else { return };
            if let Some(ref d) = datos {
                crate::sistema::publicar(d);
            }
            con_estado(|estado| {
                estado.ical_ocupado = false;
                estado.calendarios = cals;
                estado.moodle = est_moodle;
                if let Some(d) = datos {
                    estado.datos = Some(d);
                }
                match sync_cal {
                    Ok(rc) => {
                        let (mc, ma, mh) = match sync_moodle {
                            Some(Ok(rm)) => (rm.creadas, rm.actualizadas, rm.hechas),
                            _ => (0, 0, 0),
                        };
                        estado.ical_aviso = format!(
                            "Listo: {} creadas, {} actualizadas, {} completadas.",
                            rc.creadas + mc,
                            rc.actualizadas + ma,
                            rc.hechas + mh
                        );
                        ui.set_form_error("".into());
                    }
                    Err(err) => {
                        estado.ical_aviso.clear();
                        ui.set_form_error(err.into());
                    }
                }
            });
            actualizar(&ui);
        });
    });
}

fn aceptar_oferta_ical(ui: &AppWindow) {
    let url = con_estado(|estado| std::mem::take(&mut estado.ical_oferta));
    if url.is_empty() {
        return;
    }
    abrir_calendarios(ui);
    ui.set_c_url(url.into());
    agregar_ical(ui);
}

// ---------------------------------------------------------------------------
// Dispositivos P2P
// ---------------------------------------------------------------------------

fn abrir_dispositivos(ui: &AppWindow) {
    let (nombre, lista, aviso_inicial) = match crate::sistema::p2p_servicio() {
        Some(srv) => (srv.nombre(), srv.dispositivos(), String::new()),
        None => (
            String::new(),
            Vec::new(),
            "Iniciando servicio P2P…".to_string(),
        ),
    };
    con_estado(|estado| {
        if let Some(timer) = estado.p2p_espera_timer.take() {
            timer.stop();
        }
        estado.p2p_mostrando_qr = false;
        estado.p2p_dispositivos = lista;
        estado.p2p_aviso = aviso_inicial;
        estado.edicion = Edicion::Dispositivos;
        estado.volver_ajustes = true;
    });
    ui.set_p2p_nombre(nombre.into());
    ui.set_p2p_codigo_entrada("".into());
    ui.set_form_encabezado("Dispositivos".into());
    ui.set_form_edita(false);
    abrir_panel(ui, 8);
}

fn guardar_nombre_p2p(ui: &AppWindow) {
    let Some(srv) = crate::sistema::p2p_servicio() else {
        ui.set_form_error("El servicio P2P aún está iniciando.".into());
        return;
    };
    let nombre = ui.get_p2p_nombre().to_string();
    match srv.cambiar_nombre(&nombre) {
        Ok(()) => {
            ui.set_form_error("".into());
            con_estado(|estado| {
                estado.p2p_aviso = "Nombre actualizado.".to_string();
                if matches!(estado.edicion, Edicion::Dispositivos) {
                    estado.form_inicial = capturar_snapshot(ui, &estado.edicion, estado);
                }
            });
            actualizar(ui);
        }
        Err(err) => ui.set_form_error(err.into()),
    }
}

fn generar_codigo_p2p(ui: &AppWindow) {
    let Some(srv) = crate::sistema::p2p_servicio() else {
        ui.set_form_error("El servicio P2P aún está iniciando.".into());
        return;
    };
    match srv.codigo_emparejamiento() {
        Ok(cod) => {
            let img = qr_a_imagen_slint(cod.qr_tamano, &cod.qr_modulos);
            let conteo_inicial = srv.dispositivos().len();
            let debil = ui.as_weak();
            let timer = slint::Timer::default();
            timer.start(
                slint::TimerMode::Repeated,
                Duration::from_millis(1200),
                move || {
                    let Some(ui) = debil.upgrade() else { return };
                    let Some(srv) = crate::sistema::p2p_servicio() else { return };
                    let lista = srv.dispositivos();
                    let emparejo = lista.len() > conteo_inicial;
                    if emparejo {
                        con_estado(|estado| {
                            if let Some(t) = estado.p2p_espera_timer.take() {
                                t.stop();
                            }
                            estado.p2p_mostrando_qr = false;
                            estado.p2p_dispositivos = lista;
                            estado.p2p_aviso = "Dispositivo emparejado y listo.".to_string();
                        });
                        actualizar(&ui);
                    }
                },
            );

            ui.set_form_error("".into());
            con_estado(|estado| {
                if let Some(t) = estado.p2p_espera_timer.take() {
                    t.stop();
                }
                estado.p2p_conteo_previo = conteo_inicial;
                estado.p2p_espera_timer = Some(timer);
                estado.p2p_qr_imagen = img;
                estado.p2p_palabras = cod.palabras;
                estado.p2p_enlace = cod.enlace;
                estado.p2p_mostrando_qr = true;
                estado.p2p_aviso.clear();
            });
            actualizar(ui);
        }
        Err(err) => ui.set_form_error(err.into()),
    }
}

fn cerrar_codigo_p2p(ui: &AppWindow) {
    let lista = crate::sistema::p2p_servicio()
        .map(|s| s.dispositivos())
        .unwrap_or_default();
    con_estado(|estado| {
        if let Some(t) = estado.p2p_espera_timer.take() {
            t.stop();
        }
        estado.p2p_mostrando_qr = false;
        estado.p2p_dispositivos = lista;
    });
    actualizar(ui);
}

fn copiar_enlace_p2p(ui: &AppWindow) {
    let enlace = con_estado(|estado| estado.p2p_enlace.clone());
    if enlace.is_empty() {
        return;
    }
    let msg = if crate::sistema::copiar_al_portapapeles(&enlace) {
        "Enlace copiado al portapapeles.".to_string()
    } else {
        "No se pudo copiar al portapapeles; usa el QR o el código de 8 palabras.".to_string()
    };
    con_estado(|estado| estado.p2p_aviso = msg);
    actualizar(ui);
}

fn emparejar_p2p(ui: &AppWindow) {
    if con_estado(|estado| estado.p2p_ocupado) {
        return;
    }
    let Some(srv) = crate::sistema::p2p_servicio() else {
        ui.set_form_error("El servicio P2P aún está iniciando.".into());
        return;
    };
    let entrada = ui.get_p2p_codigo_entrada().to_string();
    if entrada.trim().is_empty() {
        ui.set_form_error("Escribe el código de 8 palabras o pega el enlace.".into());
        return;
    }
    ui.set_form_error("".into());
    con_estado(|estado| {
        estado.p2p_ocupado = true;
        estado.p2p_aviso = "Conectando con el dispositivo…".to_string();
    });
    actualizar(ui);

    let debil = ui.as_weak();
    let ruta_datos = almacen().ruta;
    std::thread::spawn(move || {
        let res = srv.emparejar(&entrada);
        let lista = srv.dispositivos();
        let datos = if res.is_ok() {
            Almacen { ruta: ruta_datos }.cargar().ok()
        } else {
            None
        };
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = debil.upgrade() else { return };
            if let Some(ref d) = datos {
                crate::sistema::publicar(d);
            }
            con_estado(|estado| {
                estado.p2p_ocupado = false;
                estado.p2p_dispositivos = lista;
                if let Some(d) = datos {
                    estado.datos = Some(d);
                }
                match res {
                    Ok(disp) => {
                        ui.set_p2p_codigo_entrada("".into());
                        ui.set_form_error("".into());
                        estado.p2p_aviso = format!("Emparejado con {}.", disp.nombre);
                        if matches!(estado.edicion, Edicion::Dispositivos) {
                            estado.form_inicial = capturar_snapshot(&ui, &estado.edicion, estado);
                        }
                    }
                    Err(err) => {
                        estado.p2p_aviso.clear();
                        ui.set_form_error(err.into());
                    }
                }
            });
            actualizar(&ui);
        });
    });
}

fn quitar_p2p(ui: &AppWindow, id: &str) {
    let Some(srv) = crate::sistema::p2p_servicio() else {
        return;
    };
    if let Err(err) = srv.quitar(id) {
        ui.set_form_error(err.into());
        return;
    }
    let lista = srv.dispositivos();
    con_estado(|estado| {
        estado.p2p_dispositivos = lista;
        estado.p2p_aviso = "Dispositivo quitado.".to_string();
    });
    actualizar(ui);
}

fn sincronizar_p2p(ui: &AppWindow) {
    if con_estado(|estado| estado.p2p_ocupado) {
        return;
    }
    let Some(srv) = crate::sistema::p2p_servicio() else {
        ui.set_form_error("El servicio P2P aún está iniciando.".into());
        return;
    };
    ui.set_form_error("".into());
    con_estado(|estado| {
        estado.p2p_ocupado = true;
        estado.p2p_sincronizando = true;
        estado.p2p_aviso = "Sincronizando con dispositivos…".to_string();
    });
    actualizar(ui);

    let debil = ui.as_weak();
    let ruta_datos = almacen().ruta;
    std::thread::spawn(move || {
        let resumen = srv.sincronizar_ahora();
        let lista = srv.dispositivos();
        let datos = if resumen.cambios_locales {
            Almacen { ruta: ruta_datos }.cargar().ok()
        } else {
            None
        };
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = debil.upgrade() else { return };
            if let Some(ref d) = datos {
                crate::sistema::publicar(d);
            }
            con_estado(|estado| {
                estado.p2p_ocupado = false;
                estado.p2p_sincronizando = false;
                estado.p2p_dispositivos = lista;
                if let Some(d) = datos {
                    estado.datos = Some(d);
                }
                estado.p2p_aviso = if resumen.intentados == 0 {
                    "No hay dispositivos emparejados.".to_string()
                } else if resumen.exitosos == 0 {
                    "Sin conexión con los otros dispositivos.".to_string()
                } else if resumen.cambios_locales {
                    format!(
                        "Sincronizado con {} dispositivo(s) · agenda actualizada.",
                        resumen.exitosos
                    )
                } else {
                    format!("Sincronizado con {} dispositivo(s).", resumen.exitosos)
                };
            });
            actualizar(&ui);
        });
    });
}

fn qr_a_imagen_slint(tamano: u32, modulos: &[bool]) -> slint::Image {
    if tamano == 0 || modulos.len() != (tamano * tamano) as usize {
        return slint::Image::default();
    }
    let margen = 2u32;
    let escala = 5u32;
    let total_mod = tamano + 2 * margen;
    let px = total_mod * escala;
    let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(px, px);
    let slice = buf.make_mut_slice();
    // Colores del tema: fondo oscuro #101014, módulos claros #e9e4d8 (o viceversa con borde claro).
    // Usamos fondo claro (#e9e4d8) y módulos oscuros (#101014) para máxima compatibilidad con lectores QR.
    let oscuro = slint::Rgba8Pixel::new(0x10, 0x10, 0x14, 0xff);
    let claro = slint::Rgba8Pixel::new(0xe9, 0xe4, 0xd8, 0xff);
    slice.fill(claro);

    for my in 0..tamano {
        for mx in 0..tamano {
            if modulos[(my * tamano + mx) as usize] {
                let x0 = (mx + margen) * escala;
                let y0 = (my + margen) * escala;
                for dy in 0..escala {
                    let fila = ((y0 + dy) * px) as usize;
                    for dx in 0..escala {
                        slice[fila + (x0 + dx) as usize] = oscuro;
                    }
                }
            }
        }
    }
    slint::Image::from_rgba8(buf)
}

/// Abre el selector del sistema y prepara los adjuntos en un hilo.
fn adjuntar_archivos(ui: &AppWindow) {
    let ocupado = con_estado(|estado| estado.pensando || estado.leyendo_archivo);
    if ocupado {
        return;
    }
    let actuales = con_estado(|estado| estado.adjuntos.len());
    if actuales >= MAX_ADJUNTOS {
        con_estado(|estado| {
            estado.aviso = format!(
                "Puedes adjuntar como máximo {MAX_ADJUNTOS} archivos por mensaje."
            );
        });
        actualizar(ui);
        return;
    }

    let agente = con_estado(|estado| estado.agente.clone());
    let debil = ui.as_weak();
    std::thread::spawn(move || {
        let rutas = adjuntos::elegir_archivos();
        if rutas.is_empty() {
            return;
        }

        let debil_inicio = debil.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = debil_inicio.upgrade() {
                con_estado(|estado| {
                    estado.leyendo_archivo = true;
                    estado.aviso.clear();
                    estado.error = None;
                });
                actualizar(&ui);
            }
        });

        let disponibles = MAX_ADJUNTOS.saturating_sub(actuales);
        let mut preparados = Vec::new();
        let mut avisos = Vec::new();
        let mut error_lectura = None;

        if rutas.len() > disponibles {
            avisos.push(format!(
                "Solo se agregaron {disponibles} archivos (máximo {MAX_ADJUNTOS} por mensaje)."
            ));
        }

        for ruta in rutas.into_iter().take(disponibles) {
            match adjuntos::preparar_archivo(&ruta, &agente) {
                Ok(res) => {
                    if let Some(av) = res.aviso {
                        avisos.push(av);
                    }
                    preparados.push((res.adjunto, res.es_pdf));
                }
                Err(err) => {
                    error_lectura = Some(err);
                    break;
                }
            }
        }

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = debil.upgrade() {
                con_estado(|estado| {
                    estado.leyendo_archivo = false;
                    for (adjunto, es_pdf) in preparados {
                        let id = estado.siguiente_adjunto_id;
                        estado.siguiente_adjunto_id += 1;
                        estado.adjuntos.push((id, adjunto, es_pdf));
                    }
                    if let Some(err) = error_lectura {
                        estado.aviso = err;
                    } else if !avisos.is_empty() {
                        estado.aviso = avisos.join(" ");
                    }
                });
                actualizar(&ui);
            }
        });
    });
}

/// Lee el portapapeles y rellena el campo solo si parece una clave de Google.
fn pegar_clave_conexion(ui: &AppWindow) {
    let textos = textos_conexion_agente();
    match crate::sistema::leer_portapapeles() {
        Some(texto) if parece_clave_gemini(&texto) => {
            ui.set_conexion_key(texto.trim().into());
            con_estado(|estado| {
                estado.conexion_estado_texto.clear();
                estado.conexion_estado_error = false;
            });
        }
        _ => {
            con_estado(|estado| {
                estado.conexion_estado_texto = textos.portapapeles_invalido;
                estado.conexion_estado_error = true;
            });
        }
    }
    actualizar(ui);
}

/// Guarda la clave de Gemini y la valida en un hilo; si conecta y había un
/// mensaje pendiente, lo envía al instante.
fn conectar_gemini_panel(ui: &AppWindow) {
    if con_estado(|estado| estado.conexion_conectando) {
        return;
    }
    let clave = ui.get_conexion_key().to_string();
    let textos = textos_conexion_agente();
    if clave.trim().is_empty() {
        con_estado(|estado| {
            estado.conexion_estado_texto = textos.clave_vacia;
            estado.conexion_estado_error = true;
        });
        actualizar(ui);
        return;
    }

    con_estado(|estado| {
        estado.conexion_conectando = true;
        estado.conexion_estado_texto.clear();
        estado.conexion_estado_error = false;
    });
    actualizar(ui);

    let debil = ui.as_weak();
    std::thread::spawn(move || {
        let ruta = config::ruta();
        let mut base = config::cargar_escritorio_de(&ruta).con_proveedor(Proveedor::Gemini);
        base.api_key = clave.trim().to_string();
        let resultado = cliente::conectar_con_config(&ruta, base);
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = debil.upgrade() else { return };
            let mut reenviar = false;
            con_estado(|estado| {
                estado.conexion_conectando = false;
                match resultado {
                    Ok(res) => {
                        if res.guardada {
                            estado.agente = config::cargar_escritorio();
                        }
                        match res.estado {
                            EstadoConexion::Conectado => {
                                estado.conexion_abierta = false;
                                estado.conexion_estado_texto.clear();
                                estado.conexion_estado_error = false;
                                estado.aviso = res.mensaje;
                                reenviar = !ui.get_agente_texto().trim().is_empty()
                                    || !estado.adjuntos.is_empty();
                            }
                            EstadoConexion::ClaveInvalida | EstadoConexion::SinInternet => {
                                estado.conexion_estado_texto = res.mensaje;
                                estado.conexion_estado_error = true;
                            }
                        }
                    }
                    Err(err) => {
                        estado.conexion_estado_texto = err;
                        estado.conexion_estado_error = true;
                    }
                }
            });
            if !con_estado(|estado| estado.conexion_abierta) {
                ui.set_conexion_key("".into());
                ui.set_conexion_ver_key(false);
            }
            actualizar(&ui);
            if reenviar {
                enviar_agente(&ui);
            }
        });
    });
}

/// Manda la pregunta al modelo en un hilo y vuelve al bucle de la ventana.
fn enviar_agente(ui: &AppWindow) {
    if con_estado(|estado| estado.pensando || estado.leyendo_archivo) {
        return;
    }
    let agente = con_estado(|estado| estado.agente.clone());
    if !agente.lista() {
        con_estado(|estado| {
            estado.conexion_abierta = true;
            estado.conexion_estado_texto.clear();
            estado.conexion_estado_error = false;
            estado.panel = false;
            estado.aviso.clear();
            estado.error = None;
        });
        actualizar(ui);
        return;
    }
    let pregunta_escrita = ui.get_agente_texto().trim().to_string();
    let adjuntos_enviar: Vec<Adjunto> = con_estado(|estado| {
        estado
            .adjuntos
            .iter()
            .map(|(_, adj, _)| adj.clone())
            .collect()
    });
    if pregunta_escrita.is_empty() && adjuntos_enviar.is_empty() {
        return;
    }
    let pregunta = if pregunta_escrita.is_empty() {
        "Revisa los archivos adjuntos y propón las tareas, clases o recordatorios correspondientes."
            .to_string()
    } else {
        pregunta_escrita
    };

    let datos = con_estado(|estado| estado.datos.clone()).unwrap_or_else(datos_vacios);
    let ahora = Local::now();
    let contexto = contexto::resumen(
        &datos,
        ahora.naive_local(),
        &format!("UTC{}", ahora.format("%:z")),
    );
    let nombres_adjuntos: Vec<String> = adjuntos_enviar
        .iter()
        .map(|adj| adj.nombre.clone())
        .collect();
    let mut conv_snapshot = con_estado(|estado| estado.conversacion.clone());
    let necesita_compactar = conv_snapshot.necesita_compactar(agente.limite_contexto_tokens);
    con_estado(|estado| {
        estado.pensando = true;
        estado.pensando_texto = if necesita_compactar {
            "Compactando conversación…".to_string()
        } else {
            "pensando…".to_string()
        };
        estado.panel = false;
        estado.respuesta.clear();
        estado.acciones.clear();
        estado.nota.clear();
        estado.aviso.clear();
        estado.error = None;
    });
    actualizar(ui);

    let debil = ui.as_weak();
    std::thread::spawn(move || {
        if necesita_compactar {
            if let Ok(resumen) = cliente::compactar(&agente, &conv_snapshot) {
                conv_snapshot.aplicar_compactacion(&resumen);
                let debil_comp = debil.clone();
                let resumen_clon = resumen.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = debil_comp.upgrade() {
                        con_estado(|estado| {
                            estado.conversacion.aplicar_compactacion(&resumen_clon);
                            let _ = estado.conversacion.guardar_en(&ruta_conversacion());
                            estado.pensando_texto = "pensando…".to_string();
                        });
                        actualizar(&ui);
                    }
                });
            } else {
                let debil_comp = debil.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = debil_comp.upgrade() {
                        con_estado(|estado| {
                            estado.pensando_texto = "pensando…".to_string();
                        });
                        actualizar(&ui);
                    }
                });
            }
        }

        let salida = match cliente::consultar(
            &agente,
            &contexto,
            &pregunta,
            &adjuntos_enviar,
            &conv_snapshot,
        ) {
            Ok(texto) => Ok(texto),
            Err(err)
                if cliente::es_error_contexto_largo(&err)
                    && conv_snapshot.texto_para_compactar().is_some() =>
            {
                let debil_comp = debil.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = debil_comp.upgrade() {
                        con_estado(|estado| {
                            estado.pensando_texto = "Compactando conversación…".to_string();
                        });
                        actualizar(&ui);
                    }
                });
                if let Ok(resumen) = cliente::compactar(&agente, &conv_snapshot) {
                    conv_snapshot.aplicar_compactacion(&resumen);
                    let debil_comp = debil.clone();
                    let resumen_clon = resumen.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = debil_comp.upgrade() {
                            con_estado(|estado| {
                                estado.conversacion.aplicar_compactacion(&resumen_clon);
                                let _ = estado.conversacion.guardar_en(&ruta_conversacion());
                                estado.pensando_texto = "pensando…".to_string();
                            });
                            actualizar(&ui);
                        }
                    });
                }
                cliente::consultar(
                    &agente,
                    &contexto,
                    &pregunta,
                    &adjuntos_enviar,
                    &conv_snapshot,
                )
            }
            Err(err) => Err(err),
        };
        let docs = if salida.is_ok() {
            core_adjuntos::documentos_de_adjuntos(&adjuntos_enviar)
        } else {
            Vec::new()
        };
        // Nadie más toca el estado de la ventana: se vuelve al hilo principal.
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = debil.upgrade() {
                recibir_agente(&ui, &pregunta, &nombres_adjuntos, docs, salida);
            }
        });
    });
}

/// Llega la respuesta (o el error) del hilo del cliente.
fn recibir_agente(
    ui: &AppWindow,
    pregunta: &str,
    nombres_adjuntos: &[String],
    documentos: Vec<(String, String)>,
    salida: Result<String, ErrorAgente>,
) {
    con_estado(|estado| {
        estado.pensando = false;
        estado.pensando_texto = "pensando…".to_string();
    });
    match salida {
        Err(error) => {
            con_estado(|estado| {
                estado.panel = true;
                estado.respuesta = error.to_string();
                estado.acciones.clear();
                estado.nota.clear();
            });
        }
        Ok(texto) => match acciones::parsear(&texto) {
            Ok(lectura) => {
                let respuesta = if lectura.respuesta.is_empty() {
                    "(sin comentario)".to_string()
                } else {
                    lectura.respuesta
                };
                let mut retirados = Vec::new();
                con_estado(|estado| {
                    estado.conversacion.tomar_resultados();
                    for (nombre, texto_doc) in documentos {
                        retirados.extend(estado.conversacion.agregar_documento(&nombre, &texto_doc));
                    }
                    estado.conversacion.registrar_turno(
                        pregunta,
                        nombres_adjuntos,
                        &respuesta,
                    );
                    let _ = estado.conversacion.guardar_en(&ruta_conversacion());
                    estado.panel = true;
                    estado.respuesta = respuesta;
                    estado.acciones = lectura.acciones;
                    estado.nota = if retirados.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "Se retiró de la memoria el documento antiguo {} por superar el límite de 200 000 caracteres.",
                            retirados.join(", ")
                        )
                    };
                    estado.adjuntos.clear();
                });
                // El texto ya se envió: el campo queda libre para la siguiente.
                ui.set_agente_texto("".into());
            }
            Err(mensaje) => {
                con_estado(|estado| {
                    estado.conversacion.tomar_resultados();
                    let _ = estado.conversacion.guardar_en(&ruta_conversacion());
                    estado.panel = true;
                    estado.respuesta = texto;
                    estado.acciones.clear();
                    estado.nota = format!("No pude leer la propuesta: {mensaje}");
                });
            }
        },
    }
    actualizar(ui);
}

fn aplicar_accion(ui: &AppWindow, indice: i32) {
    let pendiente = con_estado(|estado| {
        let vacio = datos_vacios();
        let datos = estado.datos.as_ref().unwrap_or(&vacio);
        estado
            .acciones
            .get(indice.max(0) as usize)
            .map(|accion| (accion.clone(), accion.descripcion(datos)))
    });
    let Some((accion, descripcion)) = pendiente else { return };

    match acciones::aplicar(&almacen(), &accion) {
        Ok(datos) => {
            crate::sistema::publicar(&datos);
            con_estado(|estado| {
                estado.datos = Some(datos);
                estado.error = None;
                estado.conversacion.anotar_resultado(&descripcion, true);
                let _ = estado.conversacion.guardar_en(&ruta_conversacion());
                let idx = indice.max(0) as usize;
                if idx < estado.acciones.len() {
                    estado.acciones.remove(idx);
                }
                if estado.acciones.is_empty() {
                    estado.panel = false;
                    estado.respuesta.clear();
                }
            });
            actualizar(ui);
        }
        Err(error) => aviso(ui, &format!("No se pudo aplicar — {error}")),
    }
}

fn descartar_accion(ui: &AppWindow, indice: i32) {
    con_estado(|estado| {
        let indice = indice.max(0) as usize;
        if indice < estado.acciones.len() {
            let accion = estado.acciones.remove(indice);
            let vacio = datos_vacios();
            let datos = estado.datos.as_ref().unwrap_or(&vacio);
            let descripcion = accion.descripcion(datos);
            estado.conversacion.anotar_resultado(&descripcion, false);
            let _ = estado.conversacion.guardar_en(&ruta_conversacion());
        }
        if estado.acciones.is_empty() {
            estado.panel = false;
            estado.respuesta.clear();
        }
    });
    actualizar(ui);
}

/// Aplica en orden; si una falla, se detiene y dice cuántas quedaron hechas.
fn aplicar_todas(ui: &AppWindow) {
    let (pendientes, descripciones) = con_estado(|estado| {
        let vacio = datos_vacios();
        let datos = estado.datos.as_ref().unwrap_or(&vacio);
        let descs: Vec<String> = estado
            .acciones
            .iter()
            .map(|accion| accion.descripcion(datos))
            .collect();
        (estado.acciones.clone(), descs)
    });
    let resultado = acciones::aplicar_todas(&almacen(), &pendientes);
    if let Some(datos) = resultado.ultimos_datos {
        crate::sistema::publicar(&datos);
        con_estado(|estado| {
            estado.datos = Some(datos);
            estado.error = None;
            let quitar = resultado.aplicadas.min(estado.acciones.len());
            for desc in descripciones.iter().take(quitar) {
                estado.conversacion.anotar_resultado(desc, true);
            }
            let _ = estado.conversacion.guardar_en(&ruta_conversacion());
            estado.acciones.drain(..quitar);
        });
    }

    match resultado.error {
        Some(mensaje) => {
            con_estado(|estado| estado.error = Some(mensaje));
        }
        None => {
            con_estado(|estado| {
                estado.panel = false;
                estado.respuesta.clear();
            });
        }
    }
    actualizar(ui);
}

// ---------------------------------------------------------------------------
// Voz
// ---------------------------------------------------------------------------

/// El botón del micrófono: empieza a grabar o detiene y transcribe.
fn grabar_voz(ui: &AppWindow) {
    if con_estado(|estado| estado.grabacion.is_some()) {
        detener_y_transcribir(ui);
        return;
    }
    if con_estado(|estado| estado.pensando) {
        return;
    }

    let config_voz = con_estado(|estado| estado.agente.voz.clone());
    if config_voz.proveedor == VozProveedor::Local {
        if !voz::modelo_local_disponible(&voz::ruta_modelo_local()) {
            abrir_dialogo_descarga_voz(ui);
            return;
        }
    } else if !config_voz.lista() {
        mostrar_error_voz(ui, voz::MSG_FALTA_CLAVE_VOZ);
        return;
    }

    match grabacion::empezar() {
        Ok(grabacion) => {
            // Reloj del indicador rojo mientras se graba.
            let debil = ui.as_weak();
            let temporizador = slint::Timer::default();
            temporizador.start(
                slint::TimerMode::Repeated,
                Duration::from_millis(500),
                move || {
                    if let Some(ui) = debil.upgrade() {
                        let segundos =
                            con_estado(|estado| estado.grabacion.as_ref().map(Grabacion::segundos));
                        if let Some(segundos) = segundos {
                            ui.set_agente_tiempo(formato_tiempo(segundos).into());
                        }
                    }
                },
            );
            con_estado(|estado| {
                estado.grabacion = Some(grabacion);
                estado.temporizador = Some(temporizador);
                estado.aviso.clear();
                estado.error = None;
            });
            ui.set_agente_grabando(true);
            ui.set_agente_tiempo("0:00".into());
            actualizar(ui);
        }
        Err(mensaje) => mostrar_error_voz(ui, &format!("No se pudo usar el micrófono — {mensaje}")),
    }
}

/// Detiene la grabación y lanza la transcripción (local o por API) en un hilo.
fn detener_y_transcribir(ui: &AppWindow) {
    let Some(grabacion) = con_estado(|estado| estado.grabacion.take()) else {
        return;
    };
    con_estado(|estado| {
        if let Some(temporizador) = estado.temporizador.take() {
            temporizador.stop();
        }
    });
    ui.set_agente_grabando(false);
    ui.set_agente_tiempo("".into());

    let (muestras, tasa, canales) = grabacion.detener();
    let segundos = muestras.len() as f32 / canales.max(1) as f32 / tasa.max(1) as f32;
    let hay_algo = muestras.iter().any(|&s| s.abs() >= 1e-6);
    if segundos < 0.4 || !hay_algo {
        mostrar_error_voz(ui, voz::MSG_GRABACION_VACIA);
        return;
    }

    let config_voz = con_estado(|estado| estado.agente.voz.clone());
    if config_voz.proveedor == VozProveedor::Local {
        let ruta_modelo = voz::ruta_modelo_local();
        if !voz::modelo_local_disponible(&ruta_modelo) {
            abrir_dialogo_descarga_voz(ui);
            return;
        }
        con_estado(|estado| {
            estado.pensando = true;
            estado.pensando_texto = "transcribiendo…".to_string();
            estado.aviso = "transcribiendo…".to_string();
        });
        actualizar(ui);
        let debil = ui.as_weak();
        std::thread::spawn(move || {
            let salida = voz_local::transcribir_local(&ruta_modelo, &muestras, tasa, canales);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = debil.upgrade() {
                    recibir_transcripcion(&ui, salida);
                }
            });
        });
        return;
    }

    if !config_voz.lista() {
        mostrar_error_voz(ui, voz::MSG_FALTA_CLAVE_VOZ);
        return;
    }
    let wav = match grabacion::a_wav(&muestras, tasa, canales) {
        Ok(wav) => wav,
        Err(mensaje) => {
            mostrar_error_voz(ui, &format!("No se pudo preparar el audio — {mensaje}"));
            return;
        }
    };

    con_estado(|estado| {
        estado.pensando = true;
        estado.pensando_texto = "transcribiendo…".to_string();
        estado.aviso = "transcribiendo…".to_string();
    });
    actualizar(ui);
    let debil = ui.as_weak();
    std::thread::spawn(move || {
        let salida = voz::transcribir(&config_voz, &wav).map_err(|e| voz::mensaje_error_voz(&e));
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = debil.upgrade() {
                recibir_transcripcion(&ui, salida);
            }
        });
    });
}

fn recibir_transcripcion(ui: &AppWindow, salida: Result<String, String>) {
    match salida {
        Ok(texto) => {
            let limpio = texto.trim();
            if limpio.is_empty() {
                mostrar_error_voz(ui, voz::MSG_GRABACION_VACIA);
                return;
            }
            // Se respeta lo que el usuario ya hubiera escrito.
            let actual = ui.get_agente_texto().trim().to_string();
            let nuevo = if actual.is_empty() {
                limpio.to_string()
            } else {
                format!("{actual} {limpio}")
            };
            ui.set_agente_texto(nuevo.into());
            con_estado(|estado| {
                estado.pensando = false;
                estado.pensando_texto = "pensando…".to_string();
                estado.aviso.clear();
            });
            actualizar(ui);
        }
        Err(mensaje) => {
            mostrar_error_voz(ui, &mensaje);
        }
    }
}

fn mostrar_error_voz(ui: &AppWindow, mensaje: &str) {
    con_estado(|estado| {
        estado.pensando = false;
        estado.pensando_texto = "pensando…".to_string();
        estado.aviso.clear();
        estado.panel = true;
        estado.respuesta = mensaje.to_string();
        estado.acciones.clear();
        estado.nota.clear();
    });
    actualizar(ui);
}

fn abrir_dialogo_descarga_voz(ui: &AppWindow) {
    if con_estado(|estado| estado.descarga_voz_en_curso) {
        ui.set_descarga_voz_abierta(true);
        return;
    }
    ui.set_descarga_voz_progreso(0.0);
    ui.set_descarga_voz_estado("Listo para descargar (~470 MB).".into());
    ui.set_descarga_voz_error("".into());
    ui.set_descarga_voz_en_curso(false);
    ui.set_descarga_voz_abierta(true);
}

fn iniciar_descarga_voz(ui: &AppWindow) {
    if con_estado(|estado| estado.descarga_voz_en_curso) {
        return;
    }
    let cancelar = Arc::new(AtomicBool::new(false));
    con_estado(|estado| {
        estado.descarga_voz_cancelar = Some(Arc::clone(&cancelar));
        estado.descarga_voz_en_curso = true;
    });
    ui.set_descarga_voz_en_curso(true);
    ui.set_descarga_voz_error("".into());
    ui.set_descarga_voz_progreso(0.0);
    ui.set_descarga_voz_estado("Conectando…".into());

    let debil = ui.as_weak();
    std::thread::spawn(move || {
        let destino = voz::ruta_modelo_local();
        let mut ultimo_aviso = Instant::now() - Duration::from_secs(1);
        let resultado = voz::descargar_modelo(
            voz::URL_MODELO_LOCAL,
            &destino,
            &cancelar,
            |descargados, total| {
                let terminado = total.is_some_and(|t| descargados >= t);
                if !terminado && ultimo_aviso.elapsed() < Duration::from_millis(120) {
                    return;
                }
                ultimo_aviso = Instant::now();
                let debil_prog = debil.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = debil_prog.upgrade() else { return };
                    let (fraccion, texto) = match total {
                        Some(t) if t > 0 => {
                            let pct = (descargados as f64 / t as f64).clamp(0.0, 1.0) as f32;
                            let mb_hecho = descargados as f64 / (1024.0 * 1024.0);
                            let mb_total = t as f64 / (1024.0 * 1024.0);
                            (pct, format!("{mb_hecho:.0} MB / {mb_total:.0} MB"))
                        }
                        _ => {
                            let mb_hecho = descargados as f64 / (1024.0 * 1024.0);
                            (0.0, format!("{mb_hecho:.1} MB descargados…"))
                        }
                    };
                    ui.set_descarga_voz_progreso(fraccion);
                    ui.set_descarga_voz_estado(texto.into());
                });
            },
        );

        let cancelada = cancelar.load(Ordering::Relaxed);
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = debil.upgrade() else { return };
            con_estado(|estado| {
                estado.descarga_voz_en_curso = false;
                estado.descarga_voz_cancelar = None;
            });
            ui.set_descarga_voz_en_curso(false);
            match resultado {
                Ok(()) => {
                    ui.set_a_voz_modelo_listo(true);
                    ui.set_descarga_voz_abierta(false);
                    ui.set_descarga_voz_progreso(1.0);
                    ui.set_descarga_voz_estado("".into());
                    con_estado(|estado| {
                        estado.aviso = "Modelo de voz listo.".to_string();
                    });
                    actualizar(&ui);
                }
                Err(_) if cancelada => {
                    ui.set_descarga_voz_abierta(false);
                }
                Err(mensaje) => {
                    ui.set_descarga_voz_error(mensaje.into());
                }
            }
        });
    });
}

fn cancelar_descarga_voz(ui: &AppWindow) {
    con_estado(|estado| {
        if let Some(flag) = estado.descarga_voz_cancelar.take() {
            flag.store(true, Ordering::Relaxed);
        }
        estado.descarga_voz_en_curso = false;
    });
    ui.set_descarga_voz_en_curso(false);
    ui.set_descarga_voz_abierta(false);
}

// ---------------------------------------------------------------------------
// Volcado a la ventana
// ---------------------------------------------------------------------------

fn actualizar_si_existe() {
    VENTANA.with(|celda| {
        if let Some(ui) = celda.borrow().as_ref() {
            actualizar(ui);
        }
    });
}

fn actualizar(ui: &AppWindow) {
    let vista = construir_vista();
    ui.set_dias(ModelRc::new(VecModel::from(vista.dias)));
    ui.set_etiqueta_semana(vista.etiqueta.into());
    ui.set_es_semana_actual(vista.es_actual);
    ui.set_base_hora(vista.base);
    ui.set_fin_hora(vista.fin);
    ui.set_etiquetas_hora(ModelRc::new(VecModel::from(vista.etiquetas)));
    ui.set_mostrar_ahora(vista.mostrar_ahora);
    ui.set_ahora_min(vista.ahora_min);
    ui.set_hay_tareas(vista.hay_tareas);
    ui.set_tareas(ModelRc::new(VecModel::from(vista.tareas)));
    ui.set_recordatorios(ModelRc::new(VecModel::from(vista.recordatorios)));
    ui.set_inicio_fecha(vista.inicio_fecha.into());
    ui.set_inicio_clases(ModelRc::new(VecModel::from(vista.inicio_clases)));
    ui.set_inicio_hay_siguiente(vista.inicio_hay_siguiente);
    ui.set_inicio_siguiente(vista.inicio_siguiente);
    ui.set_inicio_vencidas(ModelRc::new(VecModel::from(vista.inicio_vencidas)));
    ui.set_inicio_de_hoy(ModelRc::new(VecModel::from(vista.inicio_de_hoy)));
    ui.set_inicio_proximas(ModelRc::new(VecModel::from(vista.inicio_proximas)));
    ui.set_inicio_recordatorios(ModelRc::new(VecModel::from(vista.inicio_recordatorios)));
    ui.set_inicio_por_materia(ModelRc::new(VecModel::from(vista.inicio_por_materia)));
    ui.set_dia_etiqueta(vista.dia_etiqueta.into());
    ui.set_dia_es_hoy(vista.dia_es_hoy);
    ui.set_dia_clases(ModelRc::new(VecModel::from(vista.dia_clases)));
    ui.set_dia_tareas(ModelRc::new(VecModel::from(vista.dia_tareas)));
    ui.set_dia_recordatorios(ModelRc::new(VecModel::from(vista.dia_recordatorios)));
    ui.set_resumen_pendientes(vista.resumen.into());
    ui.set_pm_materia(vista.filtro_materia.into());
    ui.set_pm_fecha(vista.filtro_fecha);
    ui.set_pm_desde(vista.filtro_desde.into());
    ui.set_pm_hasta(vista.filtro_hasta.into());
    ui.set_pm_hora_desde(vista.filtro_hora_desde.into());
    ui.set_pm_hora_hasta(vista.filtro_hora_hasta.into());
    ui.set_pm_mostrar_hechas(vista.mostrar_hechas);
    ui.set_pm_filtro_activo(vista.filtro_activo);
    ui.set_pie(vista.pie.into());
    ui.set_error(vista.error);
    ui.set_advertencias(ModelRc::new(VecModel::from(vista.advertencias)));
    ui.set_conflicto_texto(vista.conflicto_texto.into());
    ui.set_agente_respuesta(vista.agente_respuesta);
    ui.set_agente_acciones(ModelRc::new(VecModel::from(vista.agente_acciones)));
    ui.set_agente_pensando(vista.agente_pensando);
    ui.set_agente_pensando_texto(vista.agente_pensando_texto);
    ui.set_agente_panel(vista.agente_panel);
    ui.set_agente_aviso(vista.agente_aviso);
    ui.set_agente_nota(vista.agente_nota);
    ui.set_agente_grabando(vista.agente_grabando);
    ui.set_agente_tiempo(vista.agente_tiempo);
    ui.set_agente_adjuntos(ModelRc::new(VecModel::from(vista.agente_adjuntos)));
    ui.set_agente_leyendo(vista.agente_leyendo);
    ui.set_agente_hay_conversacion(vista.agente_hay_conversacion);
    ui.set_conexion_abierta(vista.conexion_abierta);
    ui.set_conexion_conectando(vista.conexion_conectando);
    ui.set_conexion_estado_texto(vista.conexion_estado_texto);
    ui.set_conexion_estado_error(vista.conexion_estado_error);
    ui.set_moodle_banner(vista.moodle_banner);
    ui.set_moodle_conectado(vista.moodle_conectado);
    ui.set_moodle_requiere_reingreso(vista.moodle_requiere_reingreso);
    ui.set_moodle_estado_texto(vista.moodle_estado_texto);
    ui.set_moodle_ocupado(vista.moodle_ocupado);
    ui.set_moodle_aviso(vista.moodle_aviso);
    ui.set_calendarios(ModelRc::new(VecModel::from(vista.calendarios)));
    ui.set_ical_ocupado(vista.ical_ocupado);
    ui.set_ical_aviso(vista.ical_aviso);
    ui.set_ical_oferta(vista.ical_oferta);
    ui.set_p2p_sincronizando(vista.p2p_sincronizando);
    ui.set_p2p_ocupado(vista.p2p_ocupado);
    ui.set_p2p_aviso(vista.p2p_aviso);
    ui.set_p2p_dispositivos(ModelRc::new(VecModel::from(vista.p2p_dispositivos)));
    ui.set_p2p_mostrando_qr(vista.p2p_mostrando_qr);
    ui.set_p2p_qr_imagen(vista.p2p_qr_imagen);
    ui.set_p2p_palabras(vista.p2p_palabras);
    ui.set_p2p_enlace(vista.p2p_enlace);
    ui.set_atajos_lista(ModelRc::new(VecModel::from(vista.atajos_lista)));
    ui.set_atajo_capturando_id(vista.atajo_capturando_id);
    ui.set_atajo_choque_abierto(vista.atajo_choque_abierto);
    ui.set_atajo_choque_mensaje(vista.atajo_choque_mensaje);
    ui.set_descartar_abierto(vista.descartar_abierto);
    ui.set_ayuda_abierta(vista.ayuda_abierta);
}

fn construir_vista() -> Vista {
    ESTADO.with(|celda| {
        let estado = celda.borrow();
        let vacio = datos_vacios();
        let datos = estado.datos.as_ref().unwrap_or(&vacio);
        let ahora = Local::now().naive_local();
        let dias_vista = semana::semana(datos, estado.lunes);

        // Rango de la rejilla: siempre al menos 07:00–19:00.
        let mut base = 7;
        let mut fin = 19;
        for dia in &dias_vista {
            for bloque in &dia.bloques {
                base = base.min(bloque.inicio.hour() as i32);
                let termina = bloque.fin.hour() as i32 + i32::from(bloque.fin.minute() > 0);
                fin = fin.max(termina);
            }
        }

        let hoy = ahora.date();
        let resumen_inicio = semana::inicio(datos, ahora);
        let dia_vista = semana::dia(datos, estado.dia);

        let ahora_min = ahora.hour() as i32 * 60 + ahora.minute() as i32;
        let es_actual = estado.lunes == semana::lunes_de(ahora.date());
        let mostrar_ahora = es_actual && ahora_min >= base * 60 && ahora_min <= fin * 60;

        let dias: Vec<DiaUI> = dias_vista.iter().map(|dia| dia_ui(dia)).collect();
        let hay_tareas = dias_vista.iter().any(|dia| !dia.tareas.is_empty());

        // El pie cuenta lo pendiente de verdad; la lista, lo que pasa los
        // filtros de la barra de Pendientes.
        let pendientes = semana::pendientes(datos);
        let visibles = estado.filtro.aplicar(&datos.tareas, ahora.date());
        let tareas: Vec<TareaUI> = visibles
            .iter()
            .map(|tarea| tarea_a_ui(tarea, ahora))
            .collect();

        Vista {
            dias,
            etiqueta: etiqueta_semana(estado.lunes, &dias_vista),
            es_actual,
            base,
            fin,
            etiquetas: (base..=fin)
                .map(|hora| SharedString::from(etiqueta_hora(hora as u32)))
                .collect(),
            mostrar_ahora,
            ahora_min,
            hay_tareas,
            inicio_fecha: fecha_larga(hoy),
            inicio_clases: resumen_inicio
                .clases
                .iter()
                .map(|bloque| clase_ui(bloque, hoy, hoy))
                .collect(),
            inicio_hay_siguiente: resumen_inicio.siguiente.is_some(),
            inicio_siguiente: resumen_inicio
                .siguiente
                .as_ref()
                .map(|(fecha, bloque)| clase_ui(bloque, *fecha, hoy))
                .unwrap_or_default(),
            inicio_vencidas: resumen_inicio
                .vencidas
                .iter()
                .map(|tarea| tarea_a_ui(tarea, ahora))
                .collect(),
            inicio_de_hoy: resumen_inicio
                .de_hoy
                .iter()
                .map(|tarea| tarea_a_ui(tarea, ahora))
                .collect(),
            inicio_proximas: resumen_inicio
                .proximos
                .iter()
                .map(|tarea| tarea_a_ui(tarea, ahora))
                .collect(),
            inicio_recordatorios: resumen_inicio
                .recordatorios
                .iter()
                .map(|recordatorio| recordatorio_ui(recordatorio, hoy))
                .collect(),
            inicio_por_materia: resumen_inicio
                .por_materia
                .iter()
                .map(|(materia, cuantas)| ConteoMateriaUI {
                    materia: materia.clone().into(),
                    cuantas: *cuantas as i32,
                    tono: tono_materia(materia),
                })
                .collect(),
            dia_etiqueta: fecha_larga(estado.dia),
            dia_es_hoy: estado.dia == hoy,
            dia_clases: dia_vista
                .bloques
                .iter()
                .map(|bloque| clase_ui(bloque, estado.dia, hoy))
                .collect(),
            dia_tareas: dia_vista
                .tareas
                .iter()
                .map(|tarea| tarea_a_ui(tarea, ahora))
                .collect(),
            dia_recordatorios: semana::recordatorios_de(datos, estado.dia)
                .iter()
                .map(|recordatorio| recordatorio_ui(recordatorio, hoy))
                .collect(),
            resumen: {
                let hechas: Vec<bool> = tareas.iter().map(|tarea| tarea.hecha).collect();
                textos::resumen(&hechas, estado.filtro.activo())
            },
            pie: pie(datos, pendientes.len(), ahora),
            tareas,
            recordatorios: recordatorios_ui(datos),
            filtro_materia: estado.filtro.materia.clone().unwrap_or_default(),
            filtro_fecha: codigo_fecha(estado.filtro.tipo_fecha),
            filtro_desde: fecha_o_vacio(estado.filtro.desde),
            filtro_hasta: fecha_o_vacio(estado.filtro.hasta),
            filtro_hora_desde: hora_o_vacio(estado.filtro.hora_desde),
            filtro_hora_hasta: hora_o_vacio(estado.filtro.hora_hasta),
            mostrar_hechas: estado.filtro.mostrar_hechas,
            filtro_activo: estado.filtro.activo(),
            error: estado.error.clone().unwrap_or_default().into(),
            advertencias: estado
                .advertencias
                .iter()
                .cloned()
                .map(SharedString::from)
                .collect(),
            conflicto_texto: estado
                .conflictos
                .first()
                .map(conflictos::mensaje_banner)
                .unwrap_or_default(),
            agente_respuesta: estado.respuesta.clone().into(),
            agente_acciones: estado
                .acciones
                .iter()
                .enumerate()
                .map(|(indice, accion)| AccionUI {
                    indice: indice as i32,
                    descripcion: accion.descripcion(datos).into(),
                })
                .collect(),
            agente_pensando: estado.pensando,
            agente_pensando_texto: estado.pensando_texto.clone().into(),
            agente_panel: estado.panel,
            agente_aviso: estado.aviso.clone().into(),
            agente_nota: estado.nota.clone().into(),
            agente_grabando: estado.grabacion.is_some(),
            agente_tiempo: estado
                .grabacion
                .as_ref()
                .map(|grabacion| formato_tiempo(grabacion.segundos()))
                .unwrap_or_default()
                .into(),
            agente_adjuntos: estado
                .adjuntos
                .iter()
                .map(|(id, adj, es_pdf)| AdjuntoUI {
                    id: *id,
                    nombre: adj.nombre.clone().into(),
                    es_pdf: *es_pdf,
                })
                .collect(),
            agente_leyendo: estado.leyendo_archivo,
            agente_hay_conversacion: !estado.conversacion.esta_vacia(),
            conexion_abierta: estado.conexion_abierta,
            conexion_conectando: estado.conexion_conectando,
            conexion_estado_texto: estado.conexion_estado_texto.clone().into(),
            conexion_estado_error: estado.conexion_estado_error,
            moodle_banner: if estado.moodle.requiere_reingreso {
                "Vuelve a iniciar sesión en Moodle (Ajustes → Moodle).".into()
            } else {
                "".into()
            },
            moodle_conectado: estado.moodle.conectado,
            moodle_requiere_reingreso: estado.moodle.requiere_reingreso,
            moodle_estado_texto: estado.moodle.texto_estado.clone().into(),
            moodle_ocupado: estado.moodle_ocupado,
            moodle_aviso: estado.moodle_aviso.clone().into(),
            calendarios: estado
                .calendarios
                .iter()
                .map(|c| CalendarioUI {
                    id: c.id.clone().into(),
                    nombre: c.nombre.clone().into(),
                    materia: c.materia.clone().into(),
                    tono: c.tono,
                    ultima: c.ultima_texto.clone().into(),
                })
                .collect(),
            ical_ocupado: estado.ical_ocupado,
            ical_aviso: estado.ical_aviso.clone().into(),
            ical_oferta: estado.ical_oferta.clone().into(),
            p2p_sincronizando: estado.p2p_sincronizando || estado.sync_global_ocupado,
            p2p_ocupado: estado.p2p_ocupado,
            p2p_aviso: estado.p2p_aviso.clone().into(),
            p2p_dispositivos: estado
                .p2p_dispositivos
                .iter()
                .map(|d| DispositivoUI {
                    id: d.id.clone().into(),
                    nombre: d.nombre.clone().into(),
                    estado: d.texto_estado.clone().into(),
                    en_linea: d.en_linea,
                })
                .collect(),
            p2p_mostrando_qr: estado.p2p_mostrando_qr,
            p2p_qr_imagen: estado.p2p_qr_imagen.clone(),
            p2p_palabras: estado.p2p_palabras.clone().into(),
            p2p_enlace: estado.p2p_enlace.clone().into(),
            atajos_lista: atajos_ui(&estado.atajos),
            atajo_capturando_id: estado
                .atajo_capturando
                .map(|a| a.id())
                .unwrap_or_default()
                .into(),
            atajo_choque_abierto: estado.atajo_choque_pendiente.is_some(),
            atajo_choque_mensaje: estado
                .atajo_choque_pendiente
                .map(|(_, _, otra)| format!("Ya la usa «{}». ¿Cambiarla?", otra.nombre()))
                .unwrap_or_default()
                .into(),
            descartar_abierto: estado.descartar_abierto,
            ayuda_abierta: estado.ayuda_abierta,
        }
    })
}

/// Una clase para las vistas de resumen. Si no es de hoy, la hora lleva delante
/// el día ("jue 26 nov · 09:00–11:00").
fn clase_ui(bloque: &semana::Bloque, fecha: NaiveDate, hoy: NaiveDate) -> ClaseInicioUI {
    ClaseInicioUI {
        id: bloque.clase_id.clone().into(),
        materia: bloque.materia.clone().into(),
        lugar: bloque.lugar.clone().unwrap_or_default().into(),
        hora: hora_clase(bloque, fecha, hoy).into(),
        tono: tono_materia(&bloque.materia),
    }
}

/// Una tarea tal como la dibujan las tres listas (Pendientes, Inicio y Día).
fn tarea_a_ui(tarea: &Tarea, ahora: NaiveDateTime) -> TareaUI {
    TareaUI {
        id: tarea.id.clone().into(),
        titulo: tarea.titulo.clone().into(),
        materia: tarea.materia.clone().unwrap_or_default().into(),
        fecha: fecha_tarea(tarea.vence).into(),
        vencida: semana::vencida(tarea, ahora),
        hecha: tarea.hecha,
        tono: tono_materia(tarea.materia.as_deref().unwrap_or("")),
    }
}

/// Un recordatorio con su cuándo escrito: "hoy · 6:00 p. m.", "mañana · 9:00
/// a. m." o "lun 30 nov · 9:00 a. m.".
fn recordatorio_ui(recordatorio: &semana::RecordatorioVisto, hoy: NaiveDate) -> RecordatorioInicioUI {
    RecordatorioInicioUI {
        id: recordatorio.id.clone().into(),
        texto: recordatorio.texto.clone().into(),
        cuando: cuando_recordatorio(recordatorio, hoy).into(),
    }
}

fn dia_ui(dia: &semana::DiaVista) -> DiaUI {
    let mut tareas: Vec<TareaDiaUI> = dia
        .tareas
        .iter()
        .map(|tarea| TareaDiaUI {
            titulo: tarea.titulo.clone().into(),
            hora: hora_texto(tarea.vence.time()).into(),
            hecha: tarea.hecha,
        })
        .collect();
    let extra = if tareas.len() > 2 {
        let resto = tareas.len() - 2;
        tareas.truncate(2);
        format!("+{resto} más")
    } else {
        String::new()
    };

    DiaUI {
        nombre: textos::nombre_dia(dia.fecha).to_string().into(),
        fecha: textos::fecha_corta(dia.fecha).into(),
        es_hoy: dia.fecha == Local::now().date_naive(),
        bloques: ModelRc::new(VecModel::from(
            dia.bloques
                .iter()
                .map(|bloque| BloqueUI {
                    id: bloque.clase_id.clone().into(),
                    materia: bloque.materia.clone().into(),
                    lugar: bloque.lugar.clone().unwrap_or_default().into(),
                    hora: reloj::rango_texto(bloque.inicio, bloque.fin).into(),
                    inicio: minutos(bloque.inicio),
                    fin: minutos(bloque.fin),
                    tono: tono_materia(&bloque.materia),
                })
                .collect::<Vec<_>>(),
        )),
        tareas: ModelRc::new(VecModel::from(tareas)),
        extra: extra.into(),
    }
}

fn recordatorios_ui(datos: &Datos) -> Vec<RecordatorioUI> {
    datos
        .recordatorios
        .iter()
        .map(|recordatorio| RecordatorioUI {
            id: recordatorio.id.clone().into(),
            texto: recordatorio.texto.clone().into(),
            dias: dias_texto(&recordatorio.dias).into(),
            hora: hora_texto(recordatorio.hora).into(),
        })
        .collect()
}


// ---------------------------------------------------------------------------
// Selectores de fecha, hora y materia
// ---------------------------------------------------------------------------
// Las fechas y las horas no se escriben a mano: se eligen. El calendario y la
// lista de horas se calculan aquí (y se prueban aquí); el .slint solo dibuja.

fn abrir_selector_fecha(ui: &AppWindow, campo: i32) {
    let fecha = fecha_del_campo(ui, campo).unwrap_or_else(|| Local::now().date_naive());
    con_estado(|estado| {
        estado.seleccion = Some(Seleccion {
            tipo: Selector::Fecha,
            campo,
            anio: fecha.year(),
            mes: fecha.month(),
            dia: fecha.day(),
            hora: 0,
            minuto: 0,
        });
    });
    ui.set_sel_titulo("Fecha".into());
    ui.set_selector_tipo(1);
    pintar_calendario(ui);
}

fn abrir_selector_hora(ui: &AppWindow, campo: i32) {
    let hora = hora_del_campo(ui, campo).unwrap_or_else(|| hora(23, 59));
    con_estado(|estado| {
        estado.seleccion = Some(Seleccion {
            tipo: Selector::Hora,
            campo,
            anio: 0,
            mes: 0,
            dia: 0,
            hora: hora.hour(),
            minuto: hora.minute(),
        });
    });
    ui.set_sel_horas(modelo_textos(&horas_texto()));
    ui.set_sel_minutos(modelo_textos(&minutos_texto()));
    ui.set_sel_titulo("Hora".into());
    ui.set_selector_tipo(2);
    pintar_hora(ui);
}

fn abrir_selector_materia(ui: &AppWindow, campo: i32) {
    let materias = materias_actuales();
    con_estado(|estado| {
        estado.seleccion = Some(Seleccion {
            tipo: Selector::Materia,
            campo,
            anio: 0,
            mes: 0,
            dia: 0,
            hora: 0,
            minuto: 0,
        });
    });
    ui.set_sel_materias(ModelRc::new(VecModel::from(materias_ui(&materias))));
    ui.set_materia_nueva("".into());
    ui.set_materia_escribiendo(false);
    ui.set_sel_titulo("Materia".into());
    ui.set_selector_tipo(3);
}

fn cerrar_selector(ui: &AppWindow) {
    con_estado(|estado| estado.seleccion = None);
    ui.set_selector_tipo(0);
    ui.set_materia_nueva("".into());
    ui.set_materia_escribiendo(false);
    ui.invoke_enfocar_ventana();
}

fn cambiar_mes(ui: &AppWindow, delta: i32) {
    let Some(mut seleccion) = con_estado(|estado| estado.seleccion.clone()) else {
        return;
    };
    if seleccion.tipo != Selector::Fecha {
        return;
    }
    let (anio, mes) = mes_vecino(seleccion.anio, seleccion.mes, delta);
    seleccion.anio = anio;
    seleccion.mes = mes;
    seleccion.dia = seleccion.dia.min(dias_del_mes(anio, mes) as u32).max(1);
    con_estado(|estado| estado.seleccion = Some(seleccion));
    pintar_calendario(ui);
}

fn elegir_dia(ui: &AppWindow, dia: i32) {
    let Some(mut seleccion) = con_estado(|estado| estado.seleccion.clone()) else {
        return;
    };
    if seleccion.tipo != Selector::Fecha || dia < 1 {
        return;
    }
    seleccion.dia = dia as u32;
    con_estado(|estado| estado.seleccion = Some(seleccion));
    pintar_calendario(ui);
}

/// La rueda de horas da 1–12 (el índice 0 es la 1); se guarda en 24 h
/// conservando a. m. / p. m.
fn elegir_hora12(ui: &AppWindow, indice: i32) {
    let Some(mut seleccion) = con_estado(|estado| estado.seleccion.clone()) else {
        return;
    };
    if seleccion.tipo != Selector::Hora || !(0..12).contains(&indice) {
        return;
    }
    let (_, tarde) = a_12h(seleccion.hora);
    seleccion.hora = a_24h(indice as u32 + 1, tarde);
    con_estado(|estado| estado.seleccion = Some(seleccion));
    pintar_hora(ui);
}

/// La rueda de minutos da 00–59, de uno en uno.
fn elegir_minuto(ui: &AppWindow, minuto: i32) {
    let Some(mut seleccion) = con_estado(|estado| estado.seleccion.clone()) else {
        return;
    };
    if seleccion.tipo != Selector::Hora || !(0..60).contains(&minuto) {
        return;
    }
    seleccion.minuto = minuto as u32;
    con_estado(|estado| estado.seleccion = Some(seleccion));
    pintar_hora(ui);
}

/// a. m. / p. m. cambia la mitad del día sin mover la hora de la rueda.
fn elegir_pm(ui: &AppWindow, tarde: bool) {
    let Some(mut seleccion) = con_estado(|estado| estado.seleccion.clone()) else {
        return;
    };
    if seleccion.tipo != Selector::Hora {
        return;
    }
    let (hora12, _) = a_12h(seleccion.hora);
    seleccion.hora = a_24h(hora12, tarde);
    con_estado(|estado| estado.seleccion = Some(seleccion));
    pintar_hora(ui);
}

fn aceptar_selector(ui: &AppWindow) {
    let Some(seleccion) = con_estado(|estado| estado.seleccion.clone()) else {
        return;
    };
    match seleccion.tipo {
        Selector::Fecha => {
            if let Some(fecha) =
                NaiveDate::from_ymd_opt(seleccion.anio, seleccion.mes, seleccion.dia)
            {
                poner_fecha_en_campo(ui, seleccion.campo, fecha);
            }
            cerrar_selector(ui);
        }
        Selector::Hora => {
            if let Some(hora) =
                NaiveTime::from_hms_opt(seleccion.hora, seleccion.minuto, 0)
            {
                poner_hora_en_campo(ui, seleccion.campo, hora);
            }
            cerrar_selector(ui);
        }
        Selector::Materia => {
            if ui.get_materia_escribiendo() {
                let texto = ui.get_materia_nueva().to_string();
                aplicar_materia(ui, &texto);
            } else {
                cerrar_selector(ui);
            }
        }
    }
    // Los campos del filtro no viven en el formulario: hay que repintar.
    actualizar(ui);
}

/// Elige una materia de la lista (o vacía con "Sin materia"). Se guarda con la
/// grafía que ya existe en el archivo aunque se haya escrito distinto.
fn aplicar_materia(ui: &AppWindow, texto: &str) {
    let materia = canonica(&materias_actuales(), texto);
    let campo = con_estado(|estado| estado.seleccion.as_ref().map(|s| s.campo));
    match campo {
        Some(CAMPO_MATERIA) => ui.set_f_materia(materia.into()),
        Some(CAMPO_CAL_MATERIA) => ui.set_c_materia(materia.into()),
        Some(CAMPO_FILTRO_MATERIA) => {
            con_estado(|estado| estado.filtro.materia = opcional(&materia));
        }
        _ => {}
    }
    cerrar_selector(ui);
    actualizar(ui);
}

fn pintar_calendario(ui: &AppWindow) {
    let Some(seleccion) = con_estado(|estado| estado.seleccion.clone()) else {
        return;
    };
    ui.set_sel_mes_texto(etiqueta_mes(seleccion.anio, seleccion.mes).into());
    let hoy = Local::now().date_naive();
    let celdas: Vec<CeldaDia> = textos::celdas_mes(seleccion.anio, seleccion.mes, seleccion.dia, hoy)
        .into_iter()
        .map(|celda| CeldaDia {
            dia: celda.dia,
            fila: celda.fila,
            columna: celda.columna,
            hoy: celda.hoy,
            elegida: celda.elegida,
        })
        .collect();
    ui.set_sel_celdas(ModelRc::new(VecModel::from(celdas)));
}

fn pintar_hora(ui: &AppWindow) {
    let Some(seleccion) = con_estado(|estado| estado.seleccion.clone()) else {
        return;
    };
    let (hora12, tarde) = a_12h(seleccion.hora);
    ui.set_sel_hora12(hora12 as i32 - 1);
    ui.set_sel_minuto(seleccion.minuto as i32);
    ui.set_sel_tarde(tarde);
    ui.set_sel_hora_texto(hora_texto(hora(seleccion.hora, seleccion.minuto)).into());
}

/// La fecha que muestra el campo que se va a llenar, si ya tiene una válida.
fn fecha_del_campo(ui: &AppWindow, campo: i32) -> Option<NaiveDate> {
    match campo {
        CAMPO_FILTRO_DESDE => con_estado(|estado| estado.filtro.desde),
        CAMPO_FILTRO_HASTA => con_estado(|estado| estado.filtro.hasta),
        _ => match con_estado(|estado| estado.edicion.clone()) {
            Edicion::Tarea(_) if campo == 2 => interpretar_fecha(&ui.get_f_fecha()),
            Edicion::Ajustes if campo == 1 => interpretar_fecha(&ui.get_a_inicio()),
            Edicion::Ajustes if campo == 2 => interpretar_fecha(&ui.get_a_fin()),
            _ => None,
        },
    }
}

/// La hora que muestra el campo que se va a llenar, si ya tiene una válida.
fn hora_del_campo(ui: &AppWindow, campo: i32) -> Option<NaiveTime> {
    match campo {
        CAMPO_FILTRO_HORA_DESDE => con_estado(|estado| estado.filtro.hora_desde),
        CAMPO_FILTRO_HORA_HASTA => con_estado(|estado| estado.filtro.hora_hasta),
        _ => match con_estado(|estado| estado.edicion.clone()) {
            Edicion::Tarea(_) if campo == 3 => interpretar_hora(&ui.get_f_hora()),
            Edicion::Clase(_) if campo == 2 => interpretar_hora(&ui.get_f_hora()),
            Edicion::Clase(_) if campo == 3 => interpretar_hora(&ui.get_f_fin()),
            Edicion::Recordatorio(_) if campo == 2 => interpretar_hora(&ui.get_r_hora()),
            Edicion::Ajustes if campo == 3 => interpretar_hora(&ui.get_a_hora()),
            _ => None,
        },
    }
}

fn poner_fecha_en_campo(ui: &AppWindow, campo: i32, fecha: NaiveDate) {
    let texto = SharedString::from(fecha.format(FORMATO_FECHA).to_string());
    match campo {
        // Elegir una fecha del rango enciende el filtro de rango.
        CAMPO_FILTRO_DESDE => con_estado(|estado| {
            estado.filtro.desde = Some(fecha);
            estado.filtro.tipo_fecha = TipoFecha::Rango;
        }),
        CAMPO_FILTRO_HASTA => con_estado(|estado| {
            estado.filtro.hasta = Some(fecha);
            estado.filtro.tipo_fecha = TipoFecha::Rango;
        }),
        _ => match con_estado(|estado| estado.edicion.clone()) {
            Edicion::Tarea(_) if campo == 2 => ui.set_f_fecha(texto),
            Edicion::Ajustes if campo == 1 => ui.set_a_inicio(texto),
            Edicion::Ajustes if campo == 2 => ui.set_a_fin(texto),
            _ => {}
        },
    }
}

fn poner_hora_en_campo(ui: &AppWindow, campo: i32, hora: NaiveTime) {
    let texto = SharedString::from(hora_texto(hora));
    match campo {
        CAMPO_FILTRO_HORA_DESDE => con_estado(|estado| estado.filtro.hora_desde = Some(hora)),
        CAMPO_FILTRO_HORA_HASTA => con_estado(|estado| estado.filtro.hora_hasta = Some(hora)),
        _ => match con_estado(|estado| estado.edicion.clone()) {
            Edicion::Tarea(_) if campo == 3 => ui.set_f_hora(texto),
            Edicion::Clase(_) if campo == 2 => ui.set_f_hora(texto),
            Edicion::Clase(_) if campo == 3 => ui.set_f_fin(texto),
            Edicion::Recordatorio(_) if campo == 2 => ui.set_r_hora(texto),
            Edicion::Ajustes if campo == 3 => ui.set_a_hora(texto),
            _ => {}
        },
    }
}

fn modelo_textos(textos: &[String]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        textos
            .iter()
            .map(|texto| SharedString::from(texto.as_str()))
            .collect::<Vec<_>>(),
    ))
}

fn materias_actuales() -> Vec<String> {
    con_estado(|estado| {
        estado
            .datos
            .as_ref()
            .map(materias_de)
            .unwrap_or_default()
    })
}

/// Las materias del archivo con el tono que les toca, para el selector.
fn materias_ui(materias: &[String]) -> Vec<MateriaUI> {
    materias::materias_con_tono(materias)
        .into_iter()
        .map(|materia| MateriaUI {
            nombre: materia.nombre.into(),
            tono: materia.tono,
        })
        .collect()
}

/// Los siete días marcados en el formulario de recordatorio.
fn dias_del_recordatorio(ui: &AppWindow) -> Vec<Dia> {
    let marcas = [
        ui.get_r_lun(),
        ui.get_r_mar(),
        ui.get_r_mie(),
        ui.get_r_jue(),
        ui.get_r_vie(),
        ui.get_r_sab(),
        ui.get_r_dom(),
    ];
    DIAS_SEMANA
        .iter()
        .zip(marcas)
        .filter(|(_, marcado)| *marcado)
        .map(|(dia, _)| *dia)
        .collect()
}

fn poner_dias_recordatorio(ui: &AppWindow, dias: &[Dia]) {
    ui.set_r_lun(dias.contains(&Dia::Lun));
    ui.set_r_mar(dias.contains(&Dia::Mar));
    ui.set_r_mie(dias.contains(&Dia::Mie));
    ui.set_r_jue(dias.contains(&Dia::Jue));
    ui.set_r_vie(dias.contains(&Dia::Vie));
    ui.set_r_sab(dias.contains(&Dia::Sab));
    ui.set_r_dom(dias.contains(&Dia::Dom));
}

/// Los días marcados en el formulario de clase (puede haber varios).
fn dias_de_clase(ui: &AppWindow) -> Vec<Dia> {
    let marcas = [
        ui.get_f_lun(),
        ui.get_f_mar(),
        ui.get_f_mie(),
        ui.get_f_jue(),
        ui.get_f_vie(),
        ui.get_f_sab(),
        ui.get_f_dom(),
    ];
    DIAS_SEMANA
        .iter()
        .zip(marcas)
        .filter(|(_, marcado)| *marcado)
        .map(|(dia, _)| *dia)
        .collect()
}

fn poner_dias_clase(ui: &AppWindow, dias: &[Dia]) {
    ui.set_f_lun(dias.contains(&Dia::Lun));
    ui.set_f_mar(dias.contains(&Dia::Mar));
    ui.set_f_mie(dias.contains(&Dia::Mie));
    ui.set_f_jue(dias.contains(&Dia::Jue));
    ui.set_f_vie(dias.contains(&Dia::Vie));
    ui.set_f_sab(dias.contains(&Dia::Sab));
    ui.set_f_dom(dias.contains(&Dia::Dom));
}

fn almacen() -> Almacen {
    Almacen::por_defecto()
}

fn tarea_por_id(id: &str) -> Option<Tarea> {
    let datos = con_estado(|estado| estado.datos.clone())?;
    datos.tareas.into_iter().find(|tarea| tarea.id == id)
}

fn clase_por_id(id: &str) -> Option<Clase> {
    let datos = con_estado(|estado| estado.datos.clone())?;
    datos.clases.into_iter().find(|clase| clase.id == id)
}

fn recordatorio_por_id(id: &str) -> Option<Recordatorio> {
    let datos = con_estado(|estado| estado.datos.clone())?;
    datos
        .recordatorios
        .into_iter()
        .find(|recordatorio| recordatorio.id == id)
}

fn con_estado<F, R>(f: F) -> R
where
    F: FnOnce(&mut Estado) -> R,
{
    ESTADO.with(|celda| f(&mut celda.borrow_mut()))
}

/// Datos sin contenido, para que la ventana se dibuje aunque el TOML esté roto.
fn datos_vacios() -> Datos {
    Datos {
        ajustes: Ajustes::default(),
        semestre: None,
        clases: Vec::new(),
        tareas: Vec::new(),
        recordatorios: Vec::new(),
    }
}

/// Suelta el micrófono si estaba grabando.
fn soltar_grabacion(ui: &AppWindow) {
    if let Some(grabacion) = con_estado(|estado| estado.grabacion.take()) {
        drop(grabacion);
        con_estado(|estado| {
            if let Some(temporizador) = estado.temporizador.take() {
                temporizador.stop();
            }
        });
        ui.set_agente_grabando(false);
        ui.set_agente_tiempo("".into());
    }
}

fn capturar_snapshot(ui: &AppWindow, edicion: &Edicion, estado: &Estado) -> Option<FormSnapshot> {
    match edicion {
        Edicion::Tarea(_) => Some(FormSnapshot::Tarea {
            titulo: ui.get_f_titulo().to_string(),
            materia: ui.get_f_materia().to_string(),
            fecha: ui.get_f_fecha().to_string(),
            hora: ui.get_f_hora().to_string(),
            hecha: ui.get_f_hecha(),
        }),
        Edicion::Clase(_) => Some(FormSnapshot::Clase {
            materia: ui.get_f_materia().to_string(),
            dias: [
                ui.get_f_lun(),
                ui.get_f_mar(),
                ui.get_f_mie(),
                ui.get_f_jue(),
                ui.get_f_vie(),
                ui.get_f_sab(),
                ui.get_f_dom(),
            ],
            inicio: ui.get_f_hora().to_string(),
            fin: ui.get_f_fin().to_string(),
            lugar: ui.get_f_lugar().to_string(),
        }),
        Edicion::Ajustes => Some(FormSnapshot::Ajustes {
            sin_semestre: ui.get_a_sin_semestre(),
            autostart: ui.get_a_autostart(),
            inicio: ui.get_a_inicio().to_string(),
            fin: ui.get_a_fin().to_string(),
            hora: ui.get_a_hora().to_string(),
            horas: ui.get_a_horas().to_string(),
        }),
        Edicion::Recordatorio(_) => Some(FormSnapshot::Recordatorio {
            texto: ui.get_r_texto().to_string(),
            hora: ui.get_r_hora().to_string(),
            dias: [
                ui.get_r_lun(),
                ui.get_r_mar(),
                ui.get_r_mie(),
                ui.get_r_jue(),
                ui.get_r_vie(),
                ui.get_r_sab(),
                ui.get_r_dom(),
            ],
        }),
        Edicion::Agente => Some(FormSnapshot::Agente {
            proveedor: ui.get_a_proveedor(),
            url: ui.get_a_url().to_string(),
            modelo: ui.get_a_modelo().to_string(),
            key: ui.get_a_key().to_string(),
            ve_imagenes: ui.get_a_ve_imagenes(),
            limite_contexto: ui.get_a_limite_contexto().to_string(),
            voz_proveedor: ui.get_a_voz_proveedor(),
            v_url: ui.get_v_url().to_string(),
            v_modelo: ui.get_v_modelo().to_string(),
            v_key: ui.get_v_key().to_string(),
        }),
        Edicion::Moodle => {
            if estado.moodle.conectado && !estado.moodle.requiere_reingreso {
                None
            } else {
                Some(FormSnapshot::Moodle {
                    sitio: ui.get_m_sitio().to_string(),
                    usuario: ui.get_m_usuario().to_string(),
                    contrasena: ui.get_m_contrasena().to_string(),
                })
            }
        }
        Edicion::Calendarios => Some(FormSnapshot::Calendarios {
            url: ui.get_c_url().to_string(),
            nombre: ui.get_c_nombre().to_string(),
            materia: ui.get_c_materia().to_string(),
        }),
        Edicion::Dispositivos => Some(FormSnapshot::Dispositivos {
            nombre: ui.get_p2p_nombre().to_string(),
            codigo_entrada: ui.get_p2p_codigo_entrada().to_string(),
        }),
        Edicion::Atajos | Edicion::Ninguna => None,
    }
}

fn formulario_tiene_cambios(ui: &AppWindow) -> bool {
    con_estado(|estado| {
        let Some(ref inicial) = estado.form_inicial else {
            return false;
        };
        let actual = capturar_snapshot(ui, &estado.edicion, estado);
        actual.as_ref().is_some_and(|a| a != inicial)
    })
}

fn intentar_cancelar_formulario(ui: &AppWindow) {
    if formulario_tiene_cambios(ui) {
        con_estado(|estado| {
            estado.descartar_abierto = true;
            estado.pendiente_cerrar_ventana = false;
        });
        actualizar(ui);
    } else {
        cancelar(ui);
    }
}

fn descartar_seguir(ui: &AppWindow) {
    con_estado(|estado| {
        estado.descartar_abierto = false;
        estado.pendiente_cerrar_ventana = false;
    });
    actualizar(ui);
    ui.invoke_enfocar_ventana();
}

fn descartar_confirmar(ui: &AppWindow) {
    let cerrar_ventana = con_estado(|estado| {
        estado.descartar_abierto = false;
        std::mem::take(&mut estado.pendiente_cerrar_ventana)
    });
    if cerrar_ventana {
        con_estado(|estado| {
            if let Some(timer) = estado.p2p_espera_timer.take() {
                timer.stop();
            }
            estado.p2p_mostrando_qr = false;
            estado.atajo_capturando = None;
            estado.atajo_choque_pendiente = None;
            estado.form_inicial = None;
            estado.edicion = Edicion::Ninguna;
        });
        ui.set_form_confirma(false);
        ui.set_form_error("".into());
        ui.set_campo_mal(0);
        ui.set_formulario(0);
        soltar_grabacion(ui);
        actualizar(ui);
        let _ = ui.hide();
    } else {
        cancelar(ui);
    }
}

fn atajos_ui(cfg: &AtajosConfig) -> Vec<AtajoUI> {
    cfg.entradas()
        .into_iter()
        .map(|(accion, comb)| AtajoUI {
            id: accion.id().into(),
            nombre: accion.nombre().into(),
            tecla: comb.formatear().into(),
            es_defecto: comb == accion.por_defecto(),
        })
        .collect()
}

fn abrir_atajos(ui: &AppWindow) {
    con_estado(|estado| {
        estado.edicion = Edicion::Atajos;
        estado.volver_ajustes = true;
        estado.atajo_capturando = None;
        estado.atajo_choque_pendiente = None;
    });
    ui.set_form_encabezado("Atajos de teclado".into());
    ui.set_form_edita(false);
    abrir_panel(ui, 9);
}

fn atajo_empezar_captura(ui: &AppWindow, id: &str) {
    let Some(accion) = AccionAtajo::desde_id(id) else {
        return;
    };
    con_estado(|estado| {
        estado.atajo_capturando = Some(accion);
        estado.atajo_choque_pendiente = None;
    });
    ui.set_form_error("".into());
    actualizar(ui);
    ui.invoke_enfocar_ventana();
}

fn aplicar_y_guardar_atajo(ui: &AppWindow, accion: AccionAtajo, combinacion: Combinacion) {
    let resultado = con_estado(|estado| {
        estado.atajos.asignar(accion, combinacion);
        atajos::guardar(&estado.atajos)
    });
    if let Err(err) = resultado {
        ui.set_form_error(format!("No se pudo guardar atajos.toml: {err}").into());
    } else {
        ui.set_form_error("".into());
    }
    actualizar(ui);
}

fn atajo_choque_confirmar(ui: &AppWindow) {
    let pendiente = con_estado(|estado| estado.atajo_choque_pendiente.take());
    if let Some((accion, combinacion, _)) = pendiente {
        aplicar_y_guardar_atajo(ui, accion, combinacion);
    } else {
        actualizar(ui);
    }
    ui.invoke_enfocar_ventana();
}

fn atajo_choque_cancelar(ui: &AppWindow) {
    con_estado(|estado| estado.atajo_choque_pendiente = None);
    actualizar(ui);
    ui.invoke_enfocar_ventana();
}

fn atajos_restaurar(ui: &AppWindow) {
    let resultado = con_estado(|estado| {
        estado.atajo_capturando = None;
        estado.atajo_choque_pendiente = None;
        estado.atajos.restaurar_por_defecto();
        atajos::guardar(&estado.atajos)
    });
    if let Err(err) = resultado {
        ui.set_form_error(format!("No se pudo guardar atajos.toml: {err}").into());
    } else {
        ui.set_form_error("".into());
    }
    actualizar(ui);
    ui.invoke_enfocar_ventana();
}

/// Capa superior que debe cerrarse con `Esc`, respetando el orden de la pila.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapaEsc {
    ChoqueAtajo,
    DescartarCambios,
    DescargaVoz,
    Selector,
    ConfirmarBorrado,
    AyudaAtajos,
    Formulario,
    ConexionAgente,
    PanelAgente,
    QuitarFocoTexto,
    Nada,
}

#[allow(clippy::too_many_arguments)]
fn capa_superior_esc(
    choque_atajo: bool,
    descartar_abierto: bool,
    descarga_voz_abierta: bool,
    selector_abierto: bool,
    confirma_borrado: bool,
    ayuda_abierta: bool,
    formulario_abierto: bool,
    conexion_abierta: bool,
    agente_panel_abierto: bool,
    escribiendo: bool,
) -> CapaEsc {
    if choque_atajo {
        CapaEsc::ChoqueAtajo
    } else if descartar_abierto {
        CapaEsc::DescartarCambios
    } else if descarga_voz_abierta {
        CapaEsc::DescargaVoz
    } else if selector_abierto {
        CapaEsc::Selector
    } else if confirma_borrado {
        CapaEsc::ConfirmarBorrado
    } else if ayuda_abierta {
        CapaEsc::AyudaAtajos
    } else if formulario_abierto {
        CapaEsc::Formulario
    } else if conexion_abierta {
        CapaEsc::ConexionAgente
    } else if agente_panel_abierto {
        CapaEsc::PanelAgente
    } else if escribiendo {
        CapaEsc::QuitarFocoTexto
    } else {
        CapaEsc::Nada
    }
}

fn manejar_tecla(
    ui: &AppWindow,
    tecla: &str,
    ctrl: bool,
    shift: bool,
    alt: bool,
    meta: bool,
    escribiendo: bool,
) -> bool {
    let mods = Modificadores::new(ctrl, shift, alt, meta);

    // Modo captura de nueva combinación en Ajustes → Atajos.
    let capturando = con_estado(|estado| {
        if estado.atajo_choque_pendiente.is_some() {
            None
        } else {
            estado.atajo_capturando
        }
    });
    if let Some(objetivo) = capturando {
        let Some(combinacion) = Combinacion::desde_evento(tecla, mods) else {
            // Ignoramos pulsaciones de modificadores solos mientras se espera la tecla.
            return true;
        };
        if combinacion == Combinacion::simple(Tecla::Esc) {
            con_estado(|estado| estado.atajo_capturando = None);
            actualizar(ui);
            ui.invoke_enfocar_ventana();
            return true;
        }
        let choque = con_estado(|estado| {
            estado.atajo_capturando = None;
            estado.atajos.choque(objetivo, combinacion)
        });
        if let Some(otra) = choque {
            con_estado(|estado| {
                estado.atajo_choque_pendiente = Some((objetivo, combinacion, otra));
            });
            actualizar(ui);
        } else {
            aplicar_y_guardar_atajo(ui, objetivo, combinacion);
        }
        return true;
    }

    let accion = con_estado(|estado| estado.atajos.accion_para(tecla, mods, escribiendo));
    let Some(accion) = accion else {
        return false;
    };
    ejecutar_accion(ui, accion, escribiendo)
}

fn ejecutar_accion(ui: &AppWindow, accion: AccionAtajo, escribiendo: bool) -> bool {
    let (
        choque_atajo,
        descartar_abierto,
        ayuda_abierta,
        formulario_abierto,
        conexion_abierta,
        panel_abierto,
    ) = con_estado(|estado| {
        (
            estado.atajo_choque_pendiente.is_some(),
            estado.descartar_abierto,
            estado.ayuda_abierta,
            !matches!(estado.edicion, Edicion::Ninguna),
            estado.conexion_abierta,
            estado.panel,
        )
    });
    let descarga_voz_abierta = ui.get_descarga_voz_abierta();
    let selector_abierto = ui.get_selector_tipo() != 0;
    let confirma_borrado = ui.get_form_confirma();

    if accion == AccionAtajo::Cerrar {
        let capa = capa_superior_esc(
            choque_atajo,
            descartar_abierto,
            descarga_voz_abierta,
            selector_abierto,
            confirma_borrado,
            ayuda_abierta,
            formulario_abierto,
            conexion_abierta,
            panel_abierto,
            escribiendo,
        );
        match capa {
            CapaEsc::ChoqueAtajo => atajo_choque_cancelar(ui),
            CapaEsc::DescartarCambios => descartar_seguir(ui),
            CapaEsc::DescargaVoz => cancelar_descarga_voz(ui),
            CapaEsc::Selector => cerrar_selector(ui),
            CapaEsc::ConfirmarBorrado => ui.set_form_confirma(false),
            CapaEsc::AyudaAtajos => {
                con_estado(|estado| estado.ayuda_abierta = false);
                actualizar(ui);
                ui.invoke_enfocar_ventana();
            }
            CapaEsc::Formulario => intentar_cancelar_formulario(ui),
            CapaEsc::ConexionAgente => {
                con_estado(|estado| {
                    estado.conexion_abierta = false;
                    estado.conexion_estado_texto.clear();
                    estado.conexion_estado_error = false;
                });
                actualizar(ui);
                ui.invoke_enfocar_ventana();
            }
            CapaEsc::PanelAgente => {
                con_estado(|estado| {
                    estado.panel = false;
                    estado.respuesta.clear();
                    estado.acciones.clear();
                    estado.nota.clear();
                });
                actualizar(ui);
                ui.invoke_enfocar_ventana();
            }
            CapaEsc::QuitarFocoTexto => {
                ui.invoke_enfocar_ventana();
            }
            CapaEsc::Nada => return false,
        }
        return true;
    }

    let bloqueo_modal = choque_atajo || descartar_abierto || descarga_voz_abierta || selector_abierto;
    if bloqueo_modal {
        return false;
    }

    if accion == AccionAtajo::Guardar {
        if !ayuda_abierta
            && con_estado(|estado| {
                matches!(
                    estado.edicion,
                    Edicion::Tarea(_)
                        | Edicion::Clase(_)
                        | Edicion::Recordatorio(_)
                        | Edicion::Ajustes
                        | Edicion::Agente
                )
            })
        {
            guardar(ui);
            return true;
        }
        return false;
    }

    if accion == AccionAtajo::AyudaAtajos {
        if formulario_abierto {
            return false;
        }
        con_estado(|estado| estado.ayuda_abierta = !estado.ayuda_abierta);
        actualizar(ui);
        ui.invoke_enfocar_ventana();
        return true;
    }

    if ayuda_abierta || formulario_abierto {
        return false;
    }

    match accion {
        AccionAtajo::NuevaTarea => abrir_tarea(ui, None),
        AccionAtajo::IrInicio => {
            ui.set_pestana(0);
            ui.invoke_enfocar_ventana();
        }
        AccionAtajo::IrDia => {
            ui.set_pestana(1);
            ui.invoke_enfocar_ventana();
        }
        AccionAtajo::IrSemana => {
            ui.set_pestana(2);
            ui.invoke_enfocar_ventana();
        }
        AccionAtajo::IrPendientes => {
            ui.set_pestana(3);
            ui.invoke_enfocar_ventana();
        }
        AccionAtajo::Anterior => match ui.get_pestana() {
            1 => mover_dia(&ui.as_weak(), -1),
            2 => mover(&ui.as_weak(), -7),
            _ => return false,
        },
        AccionAtajo::Siguiente => match ui.get_pestana() {
            1 => mover_dia(&ui.as_weak(), 1),
            2 => mover(&ui.as_weak(), 7),
            _ => return false,
        },
        AccionAtajo::Hoy => match ui.get_pestana() {
            1 => {
                con_estado(|estado| estado.dia = Local::now().date_naive());
                actualizar(ui);
            }
            2 => {
                let hoy = Local::now().date_naive();
                con_estado(|estado| estado.lunes = semana::lunes_de(hoy));
                actualizar(ui);
            }
            _ => return false,
        },
        AccionAtajo::Agente => ui.invoke_enfocar_agente(),
        AccionAtajo::Voz => grabar_voz(ui),
        AccionAtajo::Sincronizar => sincronizar_todo_global(ui),
        AccionAtajo::Ajustes => abrir_ajustes(ui),
        AccionAtajo::Cerrar | AccionAtajo::Guardar | AccionAtajo::AyudaAtajos => {}
    }
    true
}

fn sincronizar_todo_global(ui: &AppWindow) {
    let ocupado = con_estado(|estado| {
        estado.sync_global_ocupado || estado.moodle_ocupado || estado.ical_ocupado || estado.p2p_ocupado
    });
    if ocupado {
        return;
    }
    let ruta_datos = almacen().ruta;
    let ruta_moodle = moodle::config::ruta();
    let ruta_cal = ical::config::ruta();
    let moodle_activo = moodle::config::cargar_de(&ruta_moodle).conectado();
    let cal_activo = !ical::config::cargar_de(&ruta_cal).calendarios.is_empty();
    let srv = crate::sistema::p2p_servicio();
    let p2p_activo = srv.as_ref().is_some_and(|s| !s.dispositivos().is_empty());

    if !moodle_activo && !cal_activo && !p2p_activo {
        return;
    }

    con_estado(|estado| estado.sync_global_ocupado = true);
    actualizar(ui);

    let debil = ui.as_weak();
    std::thread::spawn(move || {
        let hoy = Local::now().date_naive();
        if moodle_activo {
            let _ = moodle::sincronizar_ahora(&ruta_datos, &ruta_moodle, hoy);
        }
        if cal_activo {
            let _ = ical::sincronizar_calendarios(&ruta_datos, &ruta_cal, hoy);
        }
        if let Some(ref s) = srv {
            if p2p_activo {
                let _ = s.sincronizar_ahora();
            }
        }
        let datos = Almacen { ruta: ruta_datos }.cargar().ok();
        let est_moodle = moodle::config::estado_de(&ruta_moodle);
        let cals = ical::config::listar_de(&ruta_cal);
        let disps = srv.map(|s| s.dispositivos());

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = debil.upgrade() else { return };
            if let Some(ref d) = datos {
                crate::sistema::publicar(d);
            }
            con_estado(|estado| {
                estado.sync_global_ocupado = false;
                estado.moodle = est_moodle;
                estado.calendarios = cals;
                if let Some(lista) = disps {
                    estado.p2p_dispositivos = lista;
                }
                if let Some(d) = datos {
                    estado.datos = Some(d);
                }
            });
            actualizar(&ui);
        });
    });
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn fecha(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).unwrap()
    }

    fn momento(anio: i32, mes: u32, dia: u32, h: u32, m: u32) -> NaiveDateTime {
        fecha(anio, mes, dia).and_hms_opt(h, m, 0).unwrap()
    }

    /// Las tareas, clases y selector de materias llegan a la interfaz con el tono de su materia.
    #[test]
    fn las_listas_llevan_el_tono_de_la_materia() {
        let ahora = momento(2026, 11, 25, 8, 0);
        let tarea = Tarea {
            id: "t-uno".into(),
            titulo: "Taller".into(),
            materia: Some("Robótica".into()),
            vence: momento(2026, 11, 28, 23, 59),
            hecha: false,
            hecha_en: None,
            origen: None,
            enlace: None,
        };
        assert_eq!(tarea_a_ui(&tarea, ahora).tono, tono_materia("Robótica"));

        let sin_materia = Tarea {
            materia: None,
            ..tarea
        };
        assert_eq!(
            tarea_a_ui(&sin_materia, ahora).tono,
            materias::TONO_SIN_MATERIA
        );

        let bloque = semana::Bloque {
            clase_id: "c-uno".into(),
            materia: "Cálculo".into(),
            inicio: NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            fin: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            lugar: None,
        };
        assert_eq!(
            clase_ui(&bloque, ahora.date(), ahora.date()).tono,
            tono_materia("Cálculo")
        );

        let lista = materias_ui(&["Robótica".to_string(), "Control".to_string()]);
        assert_eq!(lista[0].nombre, "Robótica");
        assert_eq!(lista[0].tono, tono_materia("Robótica"));
        assert_eq!(lista[1].tono, tono_materia("CONTROL"));
    }

    /// Una clase de otro día lleva el día delante; la de hoy no.
    #[test]
    fn clase_ui_lleva_el_dia_solo_si_no_es_hoy() {
        let bloque = semana::Bloque {
            clase_id: "c-uno".into(),
            materia: "Control".into(),
            inicio: NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            fin: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            lugar: Some("453-201".into()),
        };
        let hoy = fecha(2026, 11, 25);
        let de_hoy = clase_ui(&bloque, hoy, hoy);
        assert_eq!(de_hoy.hora, "7:00–9:00 a. m.");
        assert_eq!(de_hoy.materia, "Control");

        let otro = clase_ui(&bloque, fecha(2026, 11, 26), hoy);
        assert_eq!(otro.hora, "jue 26 nov · 7:00–9:00 a. m.");
    }

    /// El cuándo de un recordatorio usa "hoy" y "mañana" cuando toca.
    #[test]
    fn cuando_del_recordatorio() {
        let hoy = fecha(2026, 11, 25);
        let recordatorio = |dia: NaiveDate| semana::RecordatorioVisto {
            id: "r-uno".into(),
            texto: "Revisar Moodle".into(),
            fecha: dia,
            hora: NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
        };
        assert_eq!(
            recordatorio_ui(&recordatorio(hoy), hoy).cuando,
            "hoy · 6:00 p. m."
        );
        assert_eq!(
            recordatorio_ui(&recordatorio(fecha(2026, 11, 26)), hoy).cuando,
            "mañana · 6:00 p. m."
        );
        assert_eq!(
            recordatorio_ui(&recordatorio(fecha(2026, 11, 30)), hoy).cuando,
            "lun 30 nov · 6:00 p. m."
        );
    }

    #[test]
    fn qr_se_convierte_en_imagen_slint() {
        let (tam, modulos) =
            order_p2p::generar_qr_matriz("order://emparejar?n=abc&s=01020304&d=PC").unwrap();
        let img = qr_a_imagen_slint(tam, &modulos);
        assert_eq!(img.size().width, (tam + 4) * 5);
        assert_eq!(img.size().height, (tam + 4) * 5);
    }

    #[test]
    fn pila_esc_cierra_solo_la_capa_superior() {
        assert_eq!(
            capa_superior_esc(true, true, true, true, true, true, true, true, true, true),
            CapaEsc::ChoqueAtajo
        );
        assert_eq!(
            capa_superior_esc(false, true, true, true, true, true, true, true, true, true),
            CapaEsc::DescartarCambios
        );
        assert_eq!(
            capa_superior_esc(false, false, true, true, true, true, true, true, true, true),
            CapaEsc::DescargaVoz
        );
        assert_eq!(
            capa_superior_esc(false, false, false, true, true, true, true, true, true, true),
            CapaEsc::Selector
        );
        assert_eq!(
            capa_superior_esc(false, false, false, false, true, true, true, true, true, true),
            CapaEsc::ConfirmarBorrado
        );
        assert_eq!(
            capa_superior_esc(false, false, false, false, false, true, true, true, true, true),
            CapaEsc::AyudaAtajos
        );
        assert_eq!(
            capa_superior_esc(false, false, false, false, false, false, true, true, true, true),
            CapaEsc::Formulario
        );
        assert_eq!(
            capa_superior_esc(false, false, false, false, false, false, false, true, true, true),
            CapaEsc::ConexionAgente
        );
        assert_eq!(
            capa_superior_esc(false, false, false, false, false, false, false, false, true, true),
            CapaEsc::PanelAgente
        );
        assert_eq!(
            capa_superior_esc(false, false, false, false, false, false, false, false, false, true),
            CapaEsc::QuitarFocoTexto
        );
        assert_eq!(
            capa_superior_esc(false, false, false, false, false, false, false, false, false, false),
            CapaEsc::Nada
        );
    }

    #[test]
    fn lista_atajos_ui_refleja_personalizaciones() {
        let mut cfg = AtajosConfig::default();
        let ui_def = atajos_ui(&cfg);
        assert_eq!(ui_def.len(), AccionAtajo::TODAS.len());
        assert!(ui_def.iter().all(|item| item.es_defecto));

        cfg.asignar(
            AccionAtajo::NuevaTarea,
            Combinacion::parsear("Ctrl+Shift+N").unwrap(),
        );
        let ui_mod = atajos_ui(&cfg);
        let nueva = ui_mod.iter().find(|i| i.id == "nueva_tarea").unwrap();
        assert_eq!(nueva.tecla, "Ctrl+Shift+N");
        assert!(!nueva.es_defecto);
    }
}
