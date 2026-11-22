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

/// Lo que la ventana necesita en cada refresco.
struct Estado {
    datos: Option<Datos>,
    /// Lunes de la semana que se está viendo.
    lunes: NaiveDate,
    error: Option<String>,
    advertencias: Vec<String>,
    edicion: Edicion,
}

impl Estado {
    fn nuevo() -> Estado {
        Estado {
            datos: None,
            lunes: semana::lunes_de(Local::now().date_naive()),
            error: None,
            advertencias: Vec::new(),
            edicion: Edicion::Ninguna,
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
    let (id, materia, dia, inicio, fin, lugar) = match clase {
        Some(c) => (
            c.id.clone(),
            c.materia.clone(),
            c.dia,
            c.inicio,
            c.fin,
            c.lugar.clone().unwrap_or_default(),
        ),
        None => (
            String::new(),
            String::new(),
            Dia::Lun,
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
    ui.set_f_dia(indice_dia(dia));
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
    poner_dias(ui, &dias);
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
        falta(ui, 2, "La fecha de entrega debe ir como 2026-11-28.");
        return;
    };
    let Some(hora) = interpretar_hora(&ui.get_f_hora()) else {
        falta(ui, 3, "La hora de entrega debe ir como 23:59.");
        return;
    };

    let nueva = NuevaTarea {
        titulo,
        materia: opcional(&ui.get_f_materia()),
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
    let Some(inicio) = interpretar_hora(&ui.get_f_hora()) else {
        falta(ui, 2, "La hora de inicio debe ir como 07:00.");
        return;
    };
    let Some(fin) = interpretar_hora(&ui.get_f_fin()) else {
        falta(ui, 3, "La hora de fin debe ir como 09:00.");
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
    let clase = Clase {
        id,
        materia,
        dia: DIAS_SEMANA[(ui.get_f_dia().clamp(0, 6)) as usize],
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
        falta(ui, 2, "La hora debe ir como 18:00.");
        return;
    };
    let dias = dias_elegidos(ui);
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
            falta(ui, 1, "El inicio del semestre debe ir como 2026-08-03.");
            return;
        };
        let Some(fin) = interpretar_fecha(&ui.get_a_fin()) else {
            falta(ui, 2, "El fin del semestre debe ir como 2026-12-05.");
            return;
        };
        if fin < inicio {
            falta(ui, 2, "El fin del semestre debe ser posterior al inicio.");
            return;
        }
        semestre = Some(Semestre { inicio, fin });
    }

    let Some(aviso) = interpretar_hora(&ui.get_a_hora()) else {
        falta(ui, 3, "La hora del aviso debe ir como 20:00.");
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

fn indice_dia(dia: Dia) -> i32 {
    match dia {
        Dia::Lun => 0,
        Dia::Mar => 1,
        Dia::Mie => 2,
        Dia::Jue => 3,
        Dia::Vie => 4,
        Dia::Sab => 5,
        Dia::Dom => 6,
    }
}

/// Los siete días marcados en el formulario de recordatorio.
fn dias_elegidos(ui: &AppWindow) -> Vec<Dia> {
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

fn poner_dias(ui: &AppWindow, dias: &[Dia]) {
    ui.set_r_lun(dias.contains(&Dia::Lun));
    ui.set_r_mar(dias.contains(&Dia::Mar));
    ui.set_r_mie(dias.contains(&Dia::Mie));
    ui.set_r_jue(dias.contains(&Dia::Jue));
    ui.set_r_vie(dias.contains(&Dia::Vie));
    ui.set_r_sab(dias.contains(&Dia::Sab));
    ui.set_r_dom(dias.contains(&Dia::Dom));
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
        assert_eq!(indice_dia(Dia::Lun), 0);
        assert_eq!(indice_dia(Dia::Dom), 6);
    }

    /// Un campo vacío no se guarda como cadena vacía en el TOML.
    #[test]
    fn campos_opcionales_vacios_son_none() {
        assert_eq!(opcional("   "), None);
        assert_eq!(opcional(" Control "), Some("Control".to_string()));
    }
}
