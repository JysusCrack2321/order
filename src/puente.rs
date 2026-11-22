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

use crate::almacen::Almacen;
use crate::filtros::{Filtro, TipoFecha};
use crate::modelo::{
    self, Ajustes, Clase, Datos, Dia, ErrorDatos, FORMATO_FECHA, FORMATO_HORA, Recordatorio,
    Semestre, Tarea,
};
use crate::ops::{self, NuevaTarea};
use crate::reloj::{self, a_12h, a_24h, etiqueta_hora, hora_texto};
use crate::semana;
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike, Weekday};
use slint::{ModelRc, SharedString, VecModel, Weak};
use std::cell::RefCell;

slint::include_modules!();

const MESES: [&str; 12] = [
    "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
];
const DIAS_LARGOS: [&str; 7] = [
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
    "domingo",
];
const MESES_LARGOS: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

/// Los días en el mismo orden que las fichas del formulario (lun → dom).
const DIAS_SEMANA: [Dia; 7] = [
    Dia::Lun,
    Dia::Mar,
    Dia::Mie,
    Dia::Jue,
    Dia::Vie,
    Dia::Sab,
    Dia::Dom,
];

/// Campos del formulario y de la barra de filtros que abren un selector.
/// Los del formulario usan 1–4 (ver `campo-mal` en app.slint); los filtros de
/// Pendientes van aparte para no confundirse con ellos.
const CAMPO_MATERIA: i32 = 1;
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
    edicion: Edicion,
    seleccion: Option<Seleccion>,
    /// Barra de filtros de Pendientes: se conserva mientras la app vive.
    filtro: Filtro,
    /// El recordatorio abierto se abrió desde Ajustes: al salir se vuelve allí.
    volver_ajustes: bool,
}

impl Estado {
    fn nuevo() -> Estado {
        Estado {
            datos: None,
            lunes: semana::lunes_de(Local::now().date_naive()),
            dia: Local::now().date_naive(),
            error: None,
            advertencias: Vec::new(),
            edicion: Edicion::Ninguna,
            seleccion: None,
            filtro: Filtro::default(),
            volver_ajustes: false,
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
            actualizar(ui);
            if let Err(error) = ui.show() {
                eprintln!("ORDER: no se pudo mostrar la ventana: {error}");
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

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

fn conectar(ui: &AppWindow) {
    // Cerrar con la X oculta la ventana: ORDER sigue en la bandeja.
    ui.window()
        .on_close_requested(|| slint::CloseRequestResponse::HideWindow);

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
        cancelar(&ui);
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
    let (id, titulo, materia, fecha, hora, hecha) = match tarea {
        Some(t) => (
            t.id.clone(),
            t.titulo.clone(),
            t.materia.clone().unwrap_or_default(),
            t.vence.format(FORMATO_FECHA).to_string(),
            hora_texto(t.vence.time()),
            t.hecha,
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
    ui.set_form_error("".into());
    ui.set_campo_mal(0);
    ui.set_form_confirma(false);
    ui.set_formulario(cual);
    actualizar(ui);
}

fn cancelar(ui: &AppWindow) {
    let volver_ajustes = con_estado(|estado| {
        estado.volver_ajustes && matches!(estado.edicion, Edicion::Recordatorio(_))
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
        Edicion::Ninguna => {}
    }
}

fn guardar_tarea(ui: &AppWindow) {
    let titulo = ui.get_f_titulo().trim().to_string();
    if titulo.is_empty() {
        falta(ui, 1, "El título no puede quedar vacío.");
        return;
    }
    let Some(fecha) = interpretar_fecha(&ui.get_f_fecha()) else {
        falta(ui, 2, "Elige la fecha de entrega.");
        return;
    };
    let Some(hora) = interpretar_hora(&ui.get_f_hora()) else {
        falta(ui, 3, "Elige la hora de entrega.");
        return;
    };

    let materias = materias_actuales();
    let nueva = NuevaTarea {
        titulo,
        materia: opcional(&canonica(&materias, &ui.get_f_materia())),
        vence: fecha.and_time(hora),
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
    let materia = ui.get_f_materia().trim().to_string();
    if materia.is_empty() {
        falta(ui, 1, "La materia no puede quedar vacía.");
        return;
    }
    let dias = dias_de_clase(ui);
    if dias.is_empty() {
        falta(ui, 4, "Marca al menos un día para la clase.");
        return;
    }
    let Some(inicio) = interpretar_hora(&ui.get_f_hora()) else {
        falta(ui, 2, "Elige la hora de inicio.");
        return;
    };
    let Some(fin) = interpretar_hora(&ui.get_f_fin()) else {
        falta(ui, 3, "Elige la hora de fin.");
        return;
    };
    if fin <= inicio {
        falta(ui, 3, "La hora de fin debe ser posterior a la de inicio.");
        return;
    }

    let id = match con_estado(|estado| estado.edicion.clone()) {
        Edicion::Clase(id) => id,
        _ => String::new(),
    };
    let materia = canonica(&materias_actuales(), &materia);
    let clase = Clase {
        id,
        materia,
        dias,
        inicio,
        fin,
        lugar: opcional(&ui.get_f_lugar()),
    };

    let resultado = almacen().modificar(|doc| ops::guardar_clase(doc, &clase));
    terminar(ui, resultado, false);
}

fn guardar_recordatorio(ui: &AppWindow) {
    let texto = ui.get_r_texto().trim().to_string();
    if texto.is_empty() {
        falta(ui, 1, "El texto del recordatorio no puede quedar vacío.");
        return;
    }
    let Some(hora) = interpretar_hora(&ui.get_r_hora()) else {
        falta(ui, 2, "Elige la hora del recordatorio.");
        return;
    };
    let dias = dias_del_recordatorio(ui);
    if dias.is_empty() {
        falta(ui, 4, "Marca al menos un día para el recordatorio.");
        return;
    }

    let id = match con_estado(|estado| estado.edicion.clone()) {
        Edicion::Recordatorio(id) => id,
        _ => String::new(),
    };
    let recordatorio = Recordatorio {
        id,
        texto,
        dias,
        hora,
    };

    let resultado = almacen().modificar(|doc| ops::guardar_recordatorio(doc, &recordatorio));
    // Si se abrió desde Ajustes, al guardar se vuelve a su lista.
    let volver = con_estado(|estado| estado.volver_ajustes);
    terminar(ui, resultado, volver);
}

fn guardar_ajustes(ui: &AppWindow) {
    let mut semestre = None;
    if !ui.get_a_sin_semestre() {
        let Some(inicio) = interpretar_fecha(&ui.get_a_inicio()) else {
            falta(ui, 1, "Elige el inicio del semestre.");
            return;
        };
        let Some(fin) = interpretar_fecha(&ui.get_a_fin()) else {
            falta(ui, 2, "Elige el fin del semestre.");
            return;
        };
        if fin < inicio {
            falta(ui, 2, "El fin del semestre debe ser posterior al inicio.");
            return;
        }
        semestre = Some(Semestre { inicio, fin });
    }

    let Some(aviso) = interpretar_hora(&ui.get_a_hora()) else {
        falta(ui, 3, "Elige la hora del aviso.");
        return;
    };
    let horas = ui
        .get_a_horas()
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|horas| (1..=168).contains(horas));
    let Some(aviso_horas_antes) = horas else {
        falta(
            ui,
            4,
            "Las horas de aviso deben ser un número entre 1 y 168.",
        );
        return;
    };

    let ajustes = Ajustes {
        aviso_dia_antes: aviso,
        aviso_horas_antes,
    };
    let resultado = almacen().modificar(|doc| {
        ops::guardar_ajustes(doc, &ajustes)?;
        ops::guardar_semestre(doc, semestre.as_ref())
    });
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
            });
            ui.set_form_confirma(false);
            ui.set_form_error("".into());
            ui.set_campo_mal(0);
            ui.set_formulario(0);
            actualizar(ui);
            if volver_ajustes {
                abrir_ajustes(ui);
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
            });
            ui.set_formulario(0);
            actualizar(ui);
            if volver_ajustes {
                abrir_ajustes(ui);
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
            resumen: resumen(&tareas, estado.filtro.activo()),
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
        }
    })
}

/// "7:00–9:00 a. m.".
fn rango_hora(bloque: &semana::Bloque) -> String {
    reloj::rango_texto(bloque.inicio, bloque.fin)
}

/// Una clase para las vistas de resumen. Si no es de hoy, la hora lleva delante
/// el día ("jue 26 nov · 09:00–11:00").
fn clase_ui(bloque: &semana::Bloque, fecha: NaiveDate, hoy: NaiveDate) -> ClaseInicioUI {
    let hora = if fecha == hoy {
        rango_hora(bloque)
    } else {
        format!("{} {} · {}", nombre_dia(fecha).to_lowercase(), fecha_corta(fecha), rango_hora(bloque))
    };
    ClaseInicioUI {
        id: bloque.clase_id.clone().into(),
        materia: bloque.materia.clone().into(),
        lugar: bloque.lugar.clone().unwrap_or_default().into(),
        hora: hora.into(),
        tono: tono_materia(&bloque.materia),
    }
}

/// Una tarea tal como la dibujan las tres listas (Pendientes, Inicio y Día).
fn tarea_a_ui(tarea: &Tarea, ahora: NaiveDateTime) -> TareaUI {
    TareaUI {
        id: tarea.id.clone().into(),
        titulo: tarea.titulo.clone().into(),
        materia: tarea.materia.clone().unwrap_or_default().into(),
        fecha: format!(
            "{} {} · {}",
            nombre_dia(tarea.vence.date()),
            fecha_corta(tarea.vence.date()),
            hora_texto(tarea.vence.time())
        )
        .into(),
        vencida: semana::vencida(tarea, ahora),
        hecha: tarea.hecha,
        tono: tono_materia(tarea.materia.as_deref().unwrap_or("")),
    }
}

/// Un recordatorio con su cuándo escrito: "hoy · 6:00 p. m.", "mañana · 9:00
/// a. m." o "lun 30 nov · 9:00 a. m.".
fn recordatorio_ui(recordatorio: &semana::RecordatorioVisto, hoy: NaiveDate) -> RecordatorioInicioUI {
    let hora = hora_texto(recordatorio.hora);
    let cuando = if recordatorio.fecha == hoy {
        format!("hoy · {hora}")
    } else if recordatorio.fecha == hoy + TimeDelta::days(1) {
        format!("mañana · {hora}")
    } else {
        format!(
            "{} {} · {hora}",
            nombre_dia(recordatorio.fecha).to_lowercase(),
            fecha_corta(recordatorio.fecha)
        )
    };
    RecordatorioInicioUI {
        id: recordatorio.id.clone().into(),
        texto: recordatorio.texto.clone().into(),
        cuando: cuando.into(),
    }
}

/// "miércoles 25 de noviembre", para la cabecera de Inicio y de Día.
fn fecha_larga(fecha: NaiveDate) -> String {
    format!(
        "{} {} de {}",
        DIAS_LARGOS[fecha.weekday().num_days_from_monday() as usize],
        fecha.day(),
        MESES_LARGOS[fecha.month0() as usize]
    )
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
        nombre: nombre_dia(dia.fecha).to_string().into(),
        fecha: fecha_corta(dia.fecha).into(),
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
    ui.set_sel_celdas(ModelRc::new(VecModel::from(celdas_mes(
        seleccion.anio,
        seleccion.mes,
        seleccion.dia,
    ))));
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

/// Las doce horas de la rueda, de la 1 a las 12.
fn horas_texto() -> Vec<String> {
    (1..=12).map(|hora| hora.to_string()).collect()
}

fn minutos_texto() -> Vec<String> {
    (0..60).map(|minuto| format!("{minuto:02}")).collect()
}

fn etiqueta_mes(anio: i32, mes: u32) -> String {
    let nombre = MESES_LARGOS
        .get(mes.saturating_sub(1) as usize)
        .copied()
        .unwrap_or("mes");
    format!("{nombre} de {anio}")
}

/// El mes que está `delta` meses antes o después, cruzando años.
fn mes_vecino(anio: i32, mes: u32, delta: i32) -> (i32, u32) {
    let total = anio * 12 + mes as i32 - 1 + delta;
    (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32)
}

fn dias_del_mes(anio: i32, mes: u32) -> i32 {
    let (siguiente_anio, siguiente_mes) = mes_vecino(anio, mes, 1);
    NaiveDate::from_ymd_opt(siguiente_anio, siguiente_mes, 1)
        .and_then(|primero| primero.pred_opt())
        .map(|ultimo| ultimo.day() as i32)
        .unwrap_or(30)
}

/// Seis semanas de siete casillas, con el día 1 en su columna real.
fn celdas_mes(anio: i32, mes: u32, elegido: u32) -> Vec<CeldaDia> {
    let Some(primero) = NaiveDate::from_ymd_opt(anio, mes, 1) else {
        return Vec::new();
    };
    let desfase = primero.weekday().num_days_from_monday() as i32;
    let dias = dias_del_mes(anio, mes);
    let hoy = Local::now().date_naive();

    (0..42)
        .map(|i| {
            let dia = i - desfase + 1;
            let dentro = (1..=dias).contains(&dia);
            CeldaDia {
                dia: if dentro { dia } else { 0 },
                fila: i / 7,
                columna: i % 7,
                hoy: dentro && hoy.year() == anio && hoy.month() == mes && hoy.day() as i32 == dia,
                elegida: dentro && dia == elegido as i32,
            }
        })
        .collect()
}

/// Las materias del archivo, en el orden en que aparecen (primero las clases),
/// sin repetir aunque cambien mayúsculas o acentos.
fn materias_de(datos: &Datos) -> Vec<String> {
    let mut lista: Vec<String> = Vec::new();
    let mut claves: Vec<String> = Vec::new();
    for materia in datos
        .clases
        .iter()
        .map(|clase| &clase.materia)
        .chain(datos.tareas.iter().filter_map(|tarea| tarea.materia.as_ref()))
    {
        let materia = materia.trim();
        if materia.is_empty() {
            continue;
        }
        let clave = clave_materia(materia);
        if claves.contains(&clave) {
            continue;
        }
        claves.push(clave);
        lista.push(materia.to_string());
    }
    lista
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
    materias
        .iter()
        .map(|materia| MateriaUI {
            nombre: materia.as_str().into(),
            tono: tono_materia(materia),
        })
        .collect()
}

/// La grafía que ya existe para esa materia, o el texto tal cual si es nueva.
fn canonica(materias: &[String], texto: &str) -> String {
    let texto = texto.trim();
    if texto.is_empty() {
        return String::new();
    }
    let clave = clave_materia(texto);
    materias
        .iter()
        .find(|materia| clave_materia(materia) == clave)
        .cloned()
        .unwrap_or_else(|| texto.to_string())
}

/// Clave de comparación: minúsculas, sin acentos y con un solo espacio entre
/// palabras. "Robótica  " y "robotica" dan la misma clave.
fn clave_materia(texto: &str) -> String {
    let mut clave = String::new();
    for caracter in texto.trim().chars() {
        let plano = match caracter {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'Á' | 'À' | 'Ä' | 'Â' | 'Ã' => 'a',
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
            'ñ' | 'Ñ' => 'n',
            'ç' | 'Ç' => 'c',
            otro => otro,
        };
        clave.extend(plano.to_lowercase());
    }
    clave.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Color de las materias
// ---------------------------------------------------------------------------
// Cada materia tiene un color suave y estable, sacado solo de su nombre: el
// mismo nombre da siempre el mismo tono, sin importar mayúsculas, acentos ni
// el orden del archivo. La paleta vive en `tema.slint` (`Tema.materias` y
// `Tema.materias-fondo`); aquí solo se elige el índice.

/// Cuántos tonos tiene la paleta de materias de `tema.slint`.
pub const TONOS_MATERIA: i32 = 9;

/// El tono gris de los elementos sin materia: el último de la paleta.
pub const TONO_SIN_MATERIA: i32 = TONOS_MATERIA - 1;

/// El tono que le toca a una materia. Vacía (o "Sin materia") va en gris.
pub fn tono_materia(materia: &str) -> i32 {
    let clave = clave_materia(materia);
    if clave.is_empty() || clave == "sin materia" {
        return TONO_SIN_MATERIA;
    }
    // djb2 (h * 33 ^ byte): barato y reparte bien los nombres cortos.
    let mut hash: u32 = 5381;
    for byte in clave.as_bytes() {
        hash = hash.wrapping_mul(33) ^ u32::from(*byte);
    }
    (hash % TONOS_MATERIA as u32) as i32
}

// ---------------------------------------------------------------------------
// Textos y cálculos pequeños
// ---------------------------------------------------------------------------

fn minutos(hora: NaiveTime) -> i32 {
    hora.hour() as i32 * 60 + hora.minute() as i32
}

fn hora(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap_or_default()
}

fn nombre_dia(fecha: NaiveDate) -> &'static str {
    texto_dia(dia_de(fecha.weekday()))
}

fn texto_dia(dia: Dia) -> &'static str {
    match dia {
        Dia::Lun => "Lun",
        Dia::Mar => "Mar",
        Dia::Mie => "Mié",
        Dia::Jue => "Jue",
        Dia::Vie => "Vie",
        Dia::Sab => "Sáb",
        Dia::Dom => "Dom",
    }
}

fn dias_texto(dias: &[Dia]) -> String {
    dias.iter()
        .map(|dia| texto_dia(*dia))
        .collect::<Vec<_>>()
        .join(", ")
}

/// La opción de fecha de la barra de filtros: 0 Todas · 1 Hoy · 2 Esta semana
/// · 3 Rango. El orden es el de las fichas que dibuja pendientes.slint.
fn codigo_fecha(tipo: TipoFecha) -> i32 {
    match tipo {
        TipoFecha::Todas => 0,
        TipoFecha::Hoy => 1,
        TipoFecha::EstaSemana => 2,
        TipoFecha::Rango => 3,
    }
}

fn fecha_del_codigo(codigo: i32) -> TipoFecha {
    match codigo {
        1 => TipoFecha::Hoy,
        2 => TipoFecha::EstaSemana,
        3 => TipoFecha::Rango,
        _ => TipoFecha::Todas,
    }
}

fn fecha_o_vacio(fecha: Option<NaiveDate>) -> String {
    fecha
        .map(|fecha| fecha.format(FORMATO_FECHA).to_string())
        .unwrap_or_default()
}

fn hora_o_vacio(hora: Option<NaiveTime>) -> String {
    hora.map(hora_texto).unwrap_or_default()
}

fn fecha_corta(fecha: NaiveDate) -> String {
    format!("{} {}", fecha.day(), MESES[fecha.month0() as usize])
}

/// "17 – 21 de noviembre de 2026", o "30 de noviembre – 4 de diciembre de 2026"
/// cuando la semana cruza de mes.
fn etiqueta_semana(lunes: NaiveDate, dias: &[semana::DiaVista]) -> String {
    let Some(ultimo) = dias.last() else {
        return String::new();
    };
    let fin = ultimo.fecha;
    if lunes.month() == fin.month() {
        format!(
            "{} – {} de {} de {}",
            lunes.day(),
            fin.day(),
            MESES_LARGOS[lunes.month0() as usize],
            lunes.year()
        )
    } else {
        format!(
            "{} {} – {} {} de {}",
            lunes.day(),
            MESES_LARGOS[lunes.month0() as usize],
            fin.day(),
            MESES_LARGOS[fin.month0() as usize],
            fin.year()
        )
    }
}

/// Lo que dice la cabecera de Pendientes: cuántas quedan y cuántas hechas se
/// están viendo. Con filtros puestos, se dice que la lista está filtrada.
fn resumen(tareas: &[TareaUI], filtrado: bool) -> String {
    let hechas = tareas.iter().filter(|tarea| tarea.hecha).count();
    let pendientes = tareas.len() - hechas;
    let cuerpo = match (pendientes, hechas) {
        (0, 0) if filtrado => "sin resultados para estos filtros".to_string(),
        (0, 0) => "nada por entregar".to_string(),
        (1, 0) => "1 tarea pendiente".to_string(),
        (n, 0) => format!("{n} tareas pendientes"),
        (0, 1) => "1 tarea hecha".to_string(),
        (0, h) => format!("{h} tareas hechas"),
        (1, 1) => "1 pendiente · 1 hecha".to_string(),
        (1, h) => format!("1 pendiente · {h} hechas"),
        (n, 1) => format!("{n} pendientes · 1 hecha"),
        (n, h) => format!("{n} pendientes · {h} hechas"),
    };
    // "sin resultados para estos filtros" ya lo dice todo: no se repite.
    if filtrado && !cuerpo.starts_with("sin resultados") {
        format!("{cuerpo} (filtrado)")
    } else {
        cuerpo
    }
}

fn pie(datos: &Datos, pendientes: usize, ahora: NaiveDateTime) -> String {
    let mut texto = match pendientes {
        0 => "Hoy: sin tareas pendientes".to_string(),
        1 => "Hoy: 1 tarea pendiente".to_string(),
        n => format!("Hoy: {n} tareas pendientes"),
    };
    if let Some((recordatorio, momento)) = proximo_recordatorio(datos, ahora) {
        let hora = hora_texto(momento.time());
        let cuando = if momento.date() == ahora.date() {
            hora
        } else {
            format!("{} {}", nombre_dia(momento.date()).to_lowercase(), hora)
        };
        texto.push_str(&format!(" · {recordatorio} {cuando}"));
    }
    texto
}

/// El próximo recordatorio que toca, buscando en los próximos ocho días. Sale
/// de la misma lista sin repetidos que la pestaña Inicio, así que el pie dice
/// siempre el que viene antes, y una sola vez aunque el recordatorio sea diario.
fn proximo_recordatorio(datos: &Datos, ahora: NaiveDateTime) -> Option<(String, NaiveDateTime)> {
    let proximo = semana::proximos_recordatorios(datos, ahora, 1)
        .into_iter()
        .next()?;
    Some((proximo.texto, proximo.fecha.and_time(proximo.hora)))
}

fn dia_de(dia: Weekday) -> Dia {
    match dia {
        Weekday::Mon => Dia::Lun,
        Weekday::Tue => Dia::Mar,
        Weekday::Wed => Dia::Mie,
        Weekday::Thu => Dia::Jue,
        Weekday::Fri => Dia::Vie,
        Weekday::Sat => Dia::Sab,
        Weekday::Sun => Dia::Dom,
    }
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

/// Texto del campo, o `None` si quedó vacío (para los campos opcionales).
fn opcional(texto: &str) -> Option<String> {
    let texto = texto.trim();
    if texto.is_empty() {
        None
    } else {
        Some(texto.to_string())
    }
}

fn interpretar_fecha(texto: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(texto.trim(), FORMATO_FECHA).ok()
}

/// Acepta lo que escriben los campos: "23:59" (24 h) y también "11:59 p. m.",
/// "7:00 a. m." o "7:00am", que es como se leen las horas en la ventana.
fn interpretar_hora(texto: &str) -> Option<NaiveTime> {
    let limpio = texto.trim();
    if let Ok(hora) = NaiveTime::parse_from_str(limpio, FORMATO_HORA) {
        return Some(hora);
    }
    let compacto: String = limpio
        .to_lowercase()
        .chars()
        .filter(|caracter| !caracter.is_whitespace() && *caracter != '.')
        .collect();
    let (reloj, tarde) = compacto
        .strip_suffix("am")
        .map(|reloj| (reloj, false))
        .or_else(|| compacto.strip_suffix("pm").map(|reloj| (reloj, true)))?;
    let (horas, minutos) = reloj.split_once(':')?;
    let horas: u32 = horas.parse().ok()?;
    let minutos: u32 = minutos.parse().ok()?;
    if !(1..=12).contains(&horas) || minutos > 59 {
        return None;
    }
    NaiveTime::from_hms_opt(a_24h(horas, tarde), minutos, 0)
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

#[cfg(test)]
mod pruebas {
    use super::*;

    fn fecha(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).unwrap()
    }

    fn momento(anio: i32, mes: u32, dia: u32, h: u32, m: u32) -> NaiveDateTime {
        fecha(anio, mes, dia).and_hms_opt(h, m, 0).unwrap()
    }

    fn datos() -> Datos {
        datos_vacios()
    }

    #[test]
    fn etiqueta_dentro_del_mes() {
        let dias = crate::semana::semana(&datos(), fecha(2026, 11, 16));
        assert_eq!(
            etiqueta_semana(fecha(2026, 11, 16), &dias),
            "16 – 20 de noviembre de 2026"
        );
    }

    #[test]
    fn etiqueta_cruzando_mes() {
        let dias = crate::semana::semana(&datos(), fecha(2026, 11, 30));
        assert_eq!(
            etiqueta_semana(fecha(2026, 11, 30), &dias),
            "30 noviembre – 4 diciembre de 2026"
        );
    }

    #[test]
    fn pie_resume_pendientes_y_recordatorio() {
        let mut d = datos();
        d.recordatorios.push(Recordatorio {
            id: "r-uno".into(),
            texto: "Revisar Moodle".into(),
            dias: vec![Dia::Vie],
            hora: NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
        });
        // Viernes 20-nov-2026, 17:00.
        let texto = pie(&d, 2, momento(2026, 11, 20, 17, 0));
        assert_eq!(
            texto,
            "Hoy: 2 tareas pendientes · Revisar Moodle 6:00 p. m."
        );
    }

    #[test]
    fn proximo_recordatorio_salta_al_siguiente_dia() {
        let mut d = datos();
        d.recordatorios.push(Recordatorio {
            id: "r-uno".into(),
            texto: "Revisar Classroom".into(),
            dias: vec![Dia::Lun],
            hora: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
        });
        // Viernes 20-nov-2026: el próximo lunes es el 23.
        let (texto, cuando) = proximo_recordatorio(&d, momento(2026, 11, 20, 10, 0)).unwrap();
        assert_eq!(texto, "Revisar Classroom");
        assert_eq!(cuando, momento(2026, 11, 23, 9, 0));
    }

    #[test]
    fn sin_recordatorios_el_pie_es_solo_el_conteo() {
        assert_eq!(
            pie(&datos(), 1, momento(2026, 11, 20, 10, 0)),
            "Hoy: 1 tarea pendiente"
        );
    }

    /// El pie enseña el próximo recordatorio una sola vez, aunque sea diario.
    #[test]
    fn el_pie_no_repite_recordatorios() {
        let mut d = datos();
        d.recordatorios.push(Recordatorio {
            id: "r-moodle".into(),
            texto: "Revisar Moodle".into(),
            dias: vec![Dia::Lun, Dia::Mar, Dia::Mie, Dia::Jue, Dia::Vie],
            hora: NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
        });
        // Viernes 20-nov-2026, 17:00: el de hoy a las 18:00.
        assert_eq!(
            pie(&d, 0, momento(2026, 11, 20, 17, 0)),
            "Hoy: sin tareas pendientes · Revisar Moodle 6:00 p. m."
        );
        // Pasadas las 18:00, el lunes a la misma hora: una sola vez.
        assert_eq!(
            pie(&d, 0, momento(2026, 11, 20, 19, 0)),
            "Hoy: sin tareas pendientes · Revisar Moodle lun 6:00 p. m."
        );
    }

    /// Los formularios aceptan los formatos del archivo y las horas tal como
    /// se leen en la ventana (12 h con a. m. / p. m.).
    #[test]
    fn solo_acepta_los_formatos_del_archivo() {
        assert_eq!(interpretar_fecha("2026-11-28"), Some(fecha(2026, 11, 28)));
        assert_eq!(interpretar_fecha(" 2026-11-28 "), Some(fecha(2026, 11, 28)));
        assert_eq!(interpretar_fecha("28/11/2026"), None);
        assert_eq!(interpretar_fecha(""), None);

        assert_eq!(
            interpretar_hora("23:59"),
            Some(NaiveTime::from_hms_opt(23, 59, 0).unwrap())
        );
        assert_eq!(interpretar_hora("25:00"), None);
        assert_eq!(interpretar_hora("7"), None);
    }

    /// El campo de hora entiende lo que la ventana muestra (12 h) y lo que
    /// hay escrito en el archivo (24 h).
    #[test]
    fn el_campo_de_hora_entiende_las_dos_formas() {
        for (texto, esperada) in [
            ("7:00 a. m.", hora(7, 0)),
            ("07:00 a.m.", hora(7, 0)),
            ("7:00am", hora(7, 0)),
            ("12:00 a. m.", hora(0, 0)),
            ("12:00 p. m.", hora(12, 0)),
            ("11:59 p. m.", hora(23, 59)),
            ("2:30 p. m.", hora(14, 30)),
            ("23:59", hora(23, 59)),
        ] {
            assert_eq!(interpretar_hora(texto), Some(esperada), "{texto}");
        }
        assert_eq!(interpretar_hora("13:00 p. m."), None, "13 no existe en 12 h");
        assert_eq!(interpretar_hora("7:60 a. m."), None);
        assert_eq!(interpretar_hora("7:00 x. m."), None);
        assert_eq!(hora_texto(hora(14, 30)), "2:30 p. m.");
    }

    #[test]
    fn dias_del_recordatorio_en_orden() {
        assert_eq!(dias_texto(&[Dia::Lun, Dia::Mie, Dia::Dom]), "Lun, Mié, Dom");
        assert_eq!(DIAS_SEMANA[0], Dia::Lun, "el formulario empieza en lunes");
        assert_eq!(DIAS_SEMANA[6], Dia::Dom);
    }

    /// El calendario pone el día 1 en su columna y marca el elegido.
    #[test]
    fn calendario_del_mes() {
        // Noviembre de 2026 empieza en domingo (columna 6) y tiene 30 días.
        let celdas = celdas_mes(2026, 11, 21);
        assert_eq!(celdas.len(), 42, "siempre seis semanas");
        assert!(celdas[..6].iter().all(|celda| celda.dia == 0), "huecos");
        assert_eq!(celdas[6].dia, 1, "el 1 va en la columna del domingo");
        assert_eq!(celdas[6].columna, 6);
        assert_eq!(celdas[6].fila, 0);
        let veintiuno = celdas.iter().find(|celda| celda.dia == 21).unwrap();
        assert!(veintiuno.elegida, "el día elegido va marcado");
        assert_eq!(
            celdas.iter().filter(|celda| celda.elegida).count(),
            1,
            "solo uno elegido"
        );
    }

    #[test]
    fn meses_vecinos_cruzan_el_anio() {
        assert_eq!(mes_vecino(2026, 12, 1), (2027, 1));
        assert_eq!(mes_vecino(2026, 1, -1), (2025, 12));
        assert_eq!(mes_vecino(2026, 11, -1), (2026, 10));
        assert_eq!(dias_del_mes(2028, 2), 29, "año bisiesto");
        assert_eq!(dias_del_mes(2026, 11), 30);
        assert_eq!(etiqueta_mes(2026, 11), "noviembre de 2026");
    }

    /// La lista de materias no repite y manda la grafía del archivo.
    #[test]
    fn materias_sin_duplicados_ni_acentos() {
        let mut d = datos();
        d.clases.push(Clase {
            id: "c-uno".into(),
            materia: "Robótica".into(),
            dias: vec![Dia::Lun],
            inicio: hora(7, 0),
            fin: hora(9, 0),
            lugar: None,
        });
        d.clases.push(Clase {
            id: "c-dos".into(),
            materia: "CONTROL".into(),
            dias: vec![Dia::Mar],
            inicio: hora(7, 0),
            fin: hora(9, 0),
            lugar: None,
        });
        d.tareas.push(Tarea {
            id: "t-uno".into(),
            titulo: "Taller".into(),
            materia: Some("robotica".into()),
            vence: momento(2026, 11, 28, 23, 59),
            hecha: false,
            hecha_en: None,
        });

        let lista = materias_de(&d);
        assert_eq!(lista, vec!["Robótica".to_string(), "CONTROL".to_string()]);
        assert_eq!(canonica(&lista, " robotica "), "Robótica");
        assert_eq!(canonica(&lista, "control"), "CONTROL");
        assert_eq!(canonica(&lista, "Cálculo"), "Cálculo");
        assert_eq!(canonica(&lista, "   "), "");
        assert_eq!(clave_materia("  Análisis   Numérico "), "analisis numerico");
    }

    /// El color de una materia depende solo de su nombre normalizado y no
    /// cambia entre arranques: si cambiara, las materias cambiarían de color
    /// solas y la interfaz perdería su sentido.
    #[test]
    fn tono_estable_por_materia() {
        assert_eq!(tono_materia("Control"), tono_materia("control"));
        assert_eq!(tono_materia("Robótica"), tono_materia("  robotica "));
        assert_eq!(tono_materia("Álgebra"), tono_materia("algebra"));

        // Valores fijos: si cambia el cálculo, estos números lo delatan.
        assert_eq!(tono_materia("Control"), 3);
        assert_eq!(tono_materia("Robótica"), 7);
        assert_eq!(tono_materia("Cálculo"), 5);
        assert_eq!(tono_materia("Física"), 1);
        assert_eq!(TONOS_MATERIA, 9, "la paleta de tema.slint tiene nueve tonos");

        assert_eq!(tono_materia(""), TONO_SIN_MATERIA);
        assert_eq!(tono_materia("   "), TONO_SIN_MATERIA);
        assert_eq!(tono_materia("Sin materia"), TONO_SIN_MATERIA);

        for materia in ["Control", "Robótica", "Cálculo", "Física", "Análisis Numérico"] {
            let tono = tono_materia(materia);
            assert!((0..TONOS_MATERIA).contains(&tono), "{materia} → {tono}");
        }

        // La paleta se reparte: no todas las materias caen en el mismo tono.
        let distintos: std::collections::HashSet<i32> = [
            "Control",
            "Robótica",
            "Cálculo",
            "Física",
            "Álgebra",
            "Probabilidad",
        ]
        .iter()
        .map(|materia| tono_materia(materia))
        .collect();
        assert!(distintos.len() >= 4, "tonos distintos: {distintos:?}");

        // El selector recibe el nombre y su tono.
        let lista = materias_ui(&["Robótica".to_string(), "Control".to_string()]);
        assert_eq!(lista[0].nombre, "Robótica");
        assert_eq!(lista[0].tono, tono_materia("Robótica"));
        assert_eq!(lista[1].tono, tono_materia("CONTROL"));
    }

    /// Las tareas y las clases llegan a la interfaz con el tono de su materia.
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
        };
        assert_eq!(tarea_a_ui(&tarea, ahora).tono, tono_materia("Robótica"));

        let sin_materia = Tarea {
            materia: None,
            ..tarea
        };
        assert_eq!(tarea_a_ui(&sin_materia, ahora).tono, TONO_SIN_MATERIA);

        let bloque = crate::semana::Bloque {
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
    }

    /// Un campo vacío no se guarda como cadena vacía en el TOML.
    #[test]
    fn campos_opcionales_vacios_son_none() {
        assert_eq!(opcional("   "), None);
        assert_eq!(opcional(" Control "), Some("Control".to_string()));
    }

    fn tarea_ui(hecha: bool) -> TareaUI {
        TareaUI {
            id: "t-uno".into(),
            titulo: "Taller".into(),
            materia: "Control".into(),
            fecha: "lun 30 nov · 23:59".into(),
            vencida: false,
            hecha,
            tono: 3,
        }
    }

    /// La cabecera de Pendientes cuenta lo pendiente y lo hecho por separado.
    #[test]
    fn resumen_cuenta_pendientes_y_hechas() {
        let lista = vec![tarea_ui(false), tarea_ui(false), tarea_ui(true)];
        assert_eq!(resumen(&lista, false), "2 pendientes · 1 hecha");
        assert_eq!(resumen(&lista, true), "2 pendientes · 1 hecha (filtrado)");
        assert_eq!(resumen(&[], false), "nada por entregar");
        assert_eq!(resumen(&[], true), "sin resultados para estos filtros");
        assert_eq!(resumen(&[tarea_ui(false)], false), "1 tarea pendiente");
        assert_eq!(resumen(&[tarea_ui(true), tarea_ui(true)], false), "2 tareas hechas");
    }

    /// Las fichas de fecha de la barra de filtros y su código van de la mano.
    #[test]
    fn codigos_de_fecha_del_filtro() {
        for (codigo, tipo) in [
            (0, TipoFecha::Todas),
            (1, TipoFecha::Hoy),
            (2, TipoFecha::EstaSemana),
            (3, TipoFecha::Rango),
        ] {
            assert_eq!(codigo_fecha(tipo), codigo);
            assert_eq!(fecha_del_codigo(codigo), tipo);
        }
        assert_eq!(fecha_del_codigo(99), TipoFecha::Todas, "código raro = todas");
    }

    /// Los campos del filtro se muestran vacíos cuando no hay valor.
    #[test]
    fn filtros_vacios_no_escriben_texto() {
        assert_eq!(fecha_o_vacio(None), "");
        assert_eq!(fecha_o_vacio(Some(fecha(2026, 12, 1))), "2026-12-01");
        assert_eq!(hora_o_vacio(None), "");
        assert_eq!(
            hora_o_vacio(Some(NaiveTime::from_hms_opt(8, 5, 0).unwrap())),
            "8:05 a. m."
        );
    }

    #[test]
    fn fecha_larga_en_espanol() {
        assert_eq!(fecha_larga(fecha(2026, 11, 25)), "miércoles 25 de noviembre");
        assert_eq!(fecha_larga(fecha(2026, 8, 3)), "lunes 3 de agosto");
    }

    /// Una clase de otro día lleva el día delante; la de hoy no.
    #[test]
    fn clase_ui_lleva_el_dia_solo_si_no_es_hoy() {
        let bloque = crate::semana::Bloque {
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
        let recordatorio = |dia: NaiveDate| crate::semana::RecordatorioVisto {
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
}
