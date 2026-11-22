//! Puente entre los datos de ORDER y la ventana Slint.
//!
//! La lógica (semana, pendientes, vencidas) vive en `semana.rs`; aquí solo se
//! traduce a los modelos que dibuja la interfaz y se conectan los botones.
//! El estado de la ventana es de un solo hilo: Slint corre el bucle de eventos
//! en el hilo principal y estas funciones se llaman desde ahí.

use crate::modelo::{Ajustes, Datos, Dia, ErrorDatos, Recordatorio};
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
const DIAS: [&str; 7] = ["Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom"];

/// Lo que la ventana necesita en cada refresco.
struct Estado {
    datos: Option<Datos>,
    /// Lunes de la semana que se está viendo.
    lunes: NaiveDate,
    error: Option<String>,
    advertencias: Vec<String>,
}

impl Estado {
    fn nuevo() -> Estado {
        Estado {
            datos: None,
            lunes: semana::lunes_de(Local::now().date_naive()),
            error: None,
            advertencias: Vec::new(),
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

// ---------------------------------------------------------------------------
// Textos y cálculos pequeños
// ---------------------------------------------------------------------------

fn minutos(hora: NaiveTime) -> i32 {
    hora.hour() as i32 * 60 + hora.minute() as i32
}

fn nombre_dia(fecha: NaiveDate) -> &'static str {
    DIAS[fecha.weekday().num_days_from_monday() as usize]
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
}
