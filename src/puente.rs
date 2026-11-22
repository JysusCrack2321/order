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
use crate::modelo::{
    self, Ajustes, Clase, Datos, Dia, ErrorDatos, FORMATO_FECHA, FORMATO_HORA, Recordatorio,
    Semestre, Tarea,
};
use crate::ops::{self, NuevaTarea};
use crate::semana;
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike, Weekday};
use slint::{ModelRc, SharedString, VecModel, Weak};
use std::cell::RefCell;

slint::include_modules!();

const MESES: [&str; 12] = [
    "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
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
    error: Option<String>,
    advertencias: Vec<String>,
    edicion: Edicion,
    seleccion: Option<Seleccion>,
}

impl Estado {
    fn nuevo() -> Estado {
        Estado {
            datos: None,
            lunes: semana::lunes_de(Local::now().date_naive()),
            error: None,
            advertencias: Vec::new(),
            edicion: Edicion::Ninguna,
            seleccion: None,
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
    ui.on_marcar_hecha(move |id: SharedString| {
        let Some(ui) = debil.upgrade() else { return };
        marcar(&ui, &id);
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
    ui.on_selector_hora(move |hora: i32| {
        let Some(ui) = debil.upgrade() else { return };
        elegir_hora(&ui, hora, None);
    });

    let debil = ui.as_weak();
    ui.on_selector_minuto(move |minuto: i32| {
        let Some(ui) = debil.upgrade() else { return };
        elegir_hora(&ui, -1, Some(minuto));
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
            t.vence.format(FORMATO_HORA).to_string(),
            t.hecha,
        ),
        None => (
            String::new(),
            String::new(),
            String::new(),
            (Local::now().date_naive() + TimeDelta::days(7))
                .format(FORMATO_FECHA)
                .to_string(),
            "23:59".to_string(),
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
    ui.set_f_hora(inicio.format(FORMATO_HORA).to_string().into());
    ui.set_f_fin(fin.format(FORMATO_HORA).to_string().into());
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
    ui.set_a_hora(
        datos
            .ajustes
            .aviso_dia_antes
            .format(FORMATO_HORA)
            .to_string()
            .into(),
    );
    ui.set_a_horas(datos.ajustes.aviso_horas_antes.to_string().into());
    abrir_panel(ui, 3);
}

fn abrir_recordatorio(ui: &AppWindow, recordatorio: Option<&Recordatorio>) {
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
    ui.set_r_hora(hora.format(FORMATO_HORA).to_string().into());
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
    let edicion = con_estado(|estado| estado.edicion.clone());
    ui.set_form_confirma(false);
    ui.set_form_error("".into());
    ui.set_campo_mal(0);
    if matches!(edicion, Edicion::Recordatorio(_)) {
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
    // Al guardar se vuelve a los ajustes, que es de donde se abrió.
    terminar(ui, resultado, true);
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
    let resultado = almacen().modificar(|doc| match &edicion {
        Edicion::Tarea(id) => ops::borrar_tarea(doc, id),
        Edicion::Clase(id) => ops::borrar_clase(doc, id),
        Edicion::Recordatorio(id) => ops::borrar_recordatorio(doc, id),
        _ => Err("no hay nada que borrar".to_string()),
    });

    ui.set_form_confirma(false);
    match resultado {
        Ok(datos) => {
            let volver_ajustes = matches!(edicion, Edicion::Recordatorio(_));
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

/// La casilla de Pendientes: marca la tarea como hecha hoy.
fn marcar(ui: &AppWindow, id: &str) {
    let hoy = Local::now().date_naive();
    let resultado = almacen().modificar(|doc| ops::marcar_tarea(doc, id, true, hoy));
    match resultado {
        Ok(datos) => {
            crate::sistema::publicar(&datos);
            con_estado(|estado| {
                estado.datos = Some(datos);
                estado.error = None;
            });
        }
        Err(error) => {
            con_estado(|estado| estado.error = Some(format!("No se pudo marcar — {error}")));
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
    ui.set_resumen_pendientes(vista.resumen.into());
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

        let ahora_min = ahora.hour() as i32 * 60 + ahora.minute() as i32;
        let es_actual = estado.lunes == semana::lunes_de(ahora.date());
        let mostrar_ahora = es_actual && ahora_min >= base * 60 && ahora_min <= fin * 60;

        let dias: Vec<DiaUI> = dias_vista.iter().map(|dia| dia_ui(dia)).collect();
        let hay_tareas = dias_vista.iter().any(|dia| !dia.tareas.is_empty());

        let pendientes = semana::pendientes(datos);
        let tareas: Vec<TareaUI> = pendientes
            .iter()
            .map(|tarea| TareaUI {
                id: tarea.id.clone().into(),
                titulo: tarea.titulo.clone().into(),
                materia: tarea.materia.clone().unwrap_or_default().into(),
                fecha: format!(
                    "{} {} · {}",
                    nombre_dia(tarea.vence.date()),
                    fecha_corta(tarea.vence.date()),
                    tarea.vence.format("%H:%M")
                )
                .into(),
                vencida: semana::vencida(tarea, ahora),
            })
            .collect();

        Vista {
            dias,
            etiqueta: etiqueta_semana(estado.lunes, &dias_vista),
            es_actual,
            base,
            fin,
            etiquetas: (base..=fin)
                .map(|hora| SharedString::from(format!("{hora:02}:00")))
                .collect(),
            mostrar_ahora,
            ahora_min,
            hay_tareas,
            resumen: resumen(&tareas),
            pie: pie(datos, tareas.len(), ahora),
            tareas,
            recordatorios: recordatorios_ui(datos),
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

fn dia_ui(dia: &semana::DiaVista) -> DiaUI {
    let mut tareas: Vec<TareaDiaUI> = dia
        .tareas
        .iter()
        .map(|tarea| TareaDiaUI {
            titulo: tarea.titulo.clone().into(),
            hora: tarea.vence.format("%H:%M").to_string().into(),
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
                    hora: format!(
                        "{}–{}",
                        bloque.inicio.format("%H:%M"),
                        bloque.fin.format("%H:%M")
                    )
                    .into(),
                    inicio: minutos(bloque.inicio),
                    fin: minutos(bloque.fin),
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
            hora: recordatorio.hora.format("%H:%M").to_string().into(),
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
    ui.set_sel_materias(modelo_textos(&materias));
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

/// `hora` negativa significa "solo cambió el minuto" (los dos botones de la
/// lista de horas y minutos llaman aquí).
fn elegir_hora(ui: &AppWindow, hora_elegida: i32, minuto_elegido: Option<i32>) {
    let Some(mut seleccion) = con_estado(|estado| estado.seleccion.clone()) else {
        return;
    };
    if seleccion.tipo != Selector::Hora {
        return;
    }
    if (0..24).contains(&hora_elegida) {
        seleccion.hora = hora_elegida as u32;
    }
    if let Some(minuto) = minuto_elegido.filter(|m| (0..60).contains(m)) {
        seleccion.minuto = minuto as u32;
    }
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
}

/// Elige una materia de la lista (o vacía con "Sin materia"). Se guarda con la
/// grafía que ya existe en el archivo aunque se haya escrito distinto.
fn aplicar_materia(ui: &AppWindow, texto: &str) {
    let materia = canonica(&materias_actuales(), texto);
    let campo = con_estado(|estado| estado.seleccion.as_ref().map(|s| s.campo));
    if matches!(campo, Some(1)) {
        ui.set_f_materia(materia.into());
    }
    cerrar_selector(ui);
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
    ui.set_sel_hora(seleccion.hora as i32);
    ui.set_sel_minuto(seleccion.minuto as i32);
    ui.set_sel_hora_texto(format!("{:02}:{:02}", seleccion.hora, seleccion.minuto).into());
}

/// La fecha que muestra el campo que se va a llenar, si ya tiene una válida.
fn fecha_del_campo(ui: &AppWindow, campo: i32) -> Option<NaiveDate> {
    match con_estado(|estado| estado.edicion.clone()) {
        Edicion::Tarea(_) if campo == 2 => interpretar_fecha(&ui.get_f_fecha()),
        Edicion::Ajustes if campo == 1 => interpretar_fecha(&ui.get_a_inicio()),
        Edicion::Ajustes if campo == 2 => interpretar_fecha(&ui.get_a_fin()),
        _ => None,
    }
}

/// La hora que muestra el campo que se va a llenar, si ya tiene una válida.
fn hora_del_campo(ui: &AppWindow, campo: i32) -> Option<NaiveTime> {
    match con_estado(|estado| estado.edicion.clone()) {
        Edicion::Tarea(_) if campo == 3 => interpretar_hora(&ui.get_f_hora()),
        Edicion::Clase(_) if campo == 2 => interpretar_hora(&ui.get_f_hora()),
        Edicion::Clase(_) if campo == 3 => interpretar_hora(&ui.get_f_fin()),
        Edicion::Recordatorio(_) if campo == 2 => interpretar_hora(&ui.get_r_hora()),
        Edicion::Ajustes if campo == 3 => interpretar_hora(&ui.get_a_hora()),
        _ => None,
    }
}

fn poner_fecha_en_campo(ui: &AppWindow, campo: i32, fecha: NaiveDate) {
    let texto = SharedString::from(fecha.format(FORMATO_FECHA).to_string());
    match con_estado(|estado| estado.edicion.clone()) {
        Edicion::Tarea(_) if campo == 2 => ui.set_f_fecha(texto),
        Edicion::Ajustes if campo == 1 => ui.set_a_inicio(texto),
        Edicion::Ajustes if campo == 2 => ui.set_a_fin(texto),
        _ => {}
    }
}

fn poner_hora_en_campo(ui: &AppWindow, campo: i32, hora: NaiveTime) {
    let texto = SharedString::from(hora.format(FORMATO_HORA).to_string());
    match con_estado(|estado| estado.edicion.clone()) {
        Edicion::Tarea(_) if campo == 3 => ui.set_f_hora(texto),
        Edicion::Clase(_) if campo == 2 => ui.set_f_hora(texto),
        Edicion::Clase(_) if campo == 3 => ui.set_f_fin(texto),
        Edicion::Recordatorio(_) if campo == 2 => ui.set_r_hora(texto),
        Edicion::Ajustes if campo == 3 => ui.set_a_hora(texto),
        _ => {}
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

fn horas_texto() -> Vec<String> {
    (0..24).map(|hora| format!("{hora:02}")).collect()
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

fn resumen(tareas: &[TareaUI]) -> String {
    match tareas.len() {
        0 => "nada por entregar".to_string(),
        1 => "1 tarea por entregar".to_string(),
        n => format!("{n} tareas por entregar"),
    }
}

fn pie(datos: &Datos, pendientes: usize, ahora: NaiveDateTime) -> String {
    let mut texto = match pendientes {
        0 => "Hoy: sin tareas pendientes".to_string(),
        1 => "Hoy: 1 tarea pendiente".to_string(),
        n => format!("Hoy: {n} tareas pendientes"),
    };
    if let Some((recordatorio, momento)) = proximo_recordatorio(datos, ahora) {
        let cuando = if momento.date() == ahora.date() {
            momento.format("%H:%M").to_string()
        } else {
            format!(
                "{} {}",
                nombre_dia(momento.date()).to_lowercase(),
                momento.format("%H:%M")
            )
        };
        texto.push_str(&format!(" · {recordatorio} {cuando}"));
    }
    texto
}

/// El próximo recordatorio que toca, buscando en los próximos ocho días.
fn proximo_recordatorio(datos: &Datos, ahora: NaiveDateTime) -> Option<(String, NaiveDateTime)> {
    let mut mejor: Option<(&Recordatorio, NaiveDateTime)> = None;
    for adelanto in 0..8 {
        let fecha = ahora.date() + TimeDelta::days(adelanto);
        let dia = dia_de(fecha.weekday());
        for recordatorio in &datos.recordatorios {
            if !recordatorio.dias.contains(&dia) {
                continue;
            }
            let Some(candidato) =
                fecha.and_hms_opt(recordatorio.hora.hour(), recordatorio.hora.minute(), 0)
            else {
                continue;
            };
            if candidato <= ahora {
                continue;
            }
            if mejor
                .as_ref()
                .is_none_or(|(_, momento)| candidato < *momento)
            {
                mejor = Some((recordatorio, candidato));
            }
        }
    }
    mejor.map(|(recordatorio, momento)| (recordatorio.texto.clone(), momento))
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

fn interpretar_hora(texto: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(texto.trim(), FORMATO_HORA).ok()
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
        assert_eq!(texto, "Hoy: 2 tareas pendientes · Revisar Moodle 18:00");
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

    /// Los formularios solo aceptan los formatos del archivo.
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

    /// Un campo vacío no se guarda como cadena vacía en el TOML.
    #[test]
    fn campos_opcionales_vacios_son_none() {
        assert_eq!(opcional("   "), None);
        assert_eq!(opcional(" Control "), Some("Control".to_string()));
    }
}
