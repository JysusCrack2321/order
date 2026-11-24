//! Resumen compacto de los datos para el prompt del agente.
//!
//! El modelo no ve el archivo: ve este texto. Lleva las fechas en el formato
//! del TOML (`YYYY-MM-DDTHH:MM`) y los `id` de todo lo que se puede editar o
//! borrar, para que las acciones apunten a elementos que existen.

use crate::modelo::{Datos, FORMATO_FECHA, Tarea};
use chrono::{Datelike, NaiveDateTime, Timelike};

const DIAS: [&str; 7] = [
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
    "domingo",
];
const MESES: [&str; 12] = [
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

/// Texto que se le manda al modelo junto con la pregunta.
///
/// `ahora` va en hora local y `zona` es su desplazamiento legible (p. ej.
/// `UTC-05:00`); se pasan aparte para poder probar el resumen con fechas fijas.
pub fn resumen(datos: &Datos, ahora: NaiveDateTime, zona: &str) -> String {
    let mut texto = String::new();

    let fecha = ahora.date();
    texto.push_str(&format!(
        "Fecha y hora local: {}, {} de {} de {}, {:02}:{:02} ({zona}).\n",
        DIAS[fecha.weekday().num_days_from_monday() as usize],
        fecha.day(),
        MESES[fecha.month0() as usize],
        fecha.year(),
        ahora.hour(),
        ahora.minute()
    ));

    let materias = materias(datos);
    texto.push_str(&format!(
        "Materias: {}.\n",
        if materias.is_empty() {
            "ninguna".to_string()
        } else {
            materias.join(", ")
        }
    ));

    texto.push_str("\nClases:\n");
    if datos.clases.is_empty() {
        texto.push_str("- (ninguna)\n");
    }
    for clase in &datos.clases {
        let dias: Vec<&str> = clase.dias.iter().map(|dia| dia.texto()).collect();
        texto.push_str(&format!(
            "- id {}: {} · {} · {}–{}{}\n",
            clase.id,
            clase.materia,
            dias.join(", "),
            hora(clase.inicio),
            hora(clase.fin),
            clase
                .lugar
                .as_ref()
                .map(|lugar| format!(" · {lugar}"))
                .unwrap_or_default()
        ));
    }

    texto.push_str("\nTareas sin hacer:\n");
    let mut pendientes = 0;
    for tarea in datos.tareas.iter().filter(|tarea| !tarea.hecha) {
        texto.push_str(&fila_tarea(tarea, ahora, true));
        pendientes += 1;
    }
    if pendientes == 0 {
        texto.push_str("- (ninguna)\n");
    }

    // Las hechas de los últimos 30 días: sirven para deshacer o corregir sin
    // tener que abrir el archivo.
    let recientes: Vec<&Tarea> = datos
        .tareas
        .iter()
        .filter(|tarea| {
            tarea.hecha
                && tarea
                    .hecha_en
                    .is_some_and(|fecha| (ahora.date() - fecha).num_days() <= 30)
        })
        .collect();
    if !recientes.is_empty() {
        texto.push_str("\nTareas hechas hace poco:\n");
        for tarea in recientes {
            texto.push_str(&fila_tarea(tarea, ahora, false));
        }
    }

    texto.push_str("\nRecordatorios:\n");
    if datos.recordatorios.is_empty() {
        texto.push_str("- (ninguno)\n");
    }
    for recordatorio in &datos.recordatorios {
        let dias: Vec<&str> = recordatorio.dias.iter().map(|dia| dia.texto()).collect();
        texto.push_str(&format!(
            "- id {}: «{}» · {} · {}\n",
            recordatorio.id,
            recordatorio.texto,
            dias.join(", "),
            hora(recordatorio.hora)
        ));
    }

    texto.push_str("\nEventos:\n");
    if datos.eventos.is_empty() {
        texto.push_str("- (ninguno)\n");
    } else {
        let mut eventos: Vec<_> = datos.eventos.iter().collect();
        eventos.sort_by(|a, b| {
            a.fecha
                .cmp(&b.fecha)
                .then(a.inicio.cmp(&b.inicio))
                .then(a.fin.cmp(&b.fin))
                .then(a.id.cmp(&b.id))
        });
        for evento in eventos {
            let tarea = evento
                .tarea
                .as_deref()
                .and_then(|tid| datos.tareas.iter().find(|t| t.id == tid))
                .map(|t| format!(" · tarea {} («{}»)", t.id, t.titulo))
                .unwrap_or_default();
            let nota = evento
                .nota
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(|n| format!(" · nota: {n}"))
                .unwrap_or_default();
            texto.push_str(&format!(
                "- id {}: «{}» · {} · {}–{}{tarea}{nota}\n",
                evento.id,
                evento.titulo,
                evento.fecha.format(FORMATO_FECHA),
                hora(evento.inicio),
                hora(evento.fin),
            ));
        }
    }

    texto
}

fn fila_tarea(tarea: &Tarea, ahora: NaiveDateTime, pendiente: bool) -> String {
    let materia = tarea
        .materia
        .as_ref()
        .map(|materia| format!(" · {materia}"))
        .unwrap_or_default();
    if pendiente {
        format!(
            "- id {}: «{}»{materia} · vence {} ({})\n",
            tarea.id,
            tarea.titulo,
            tarea.vence.format("%Y-%m-%dT%H:%M"),
            cuando(tarea.vence, ahora)
        )
    } else {
        let hecha = tarea
            .hecha_en
            .map(|fecha| fecha.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "sin fecha".to_string());
        format!(
            "- id {}: «{}»{materia} · vencía {} · hecha el {hecha}\n",
            tarea.id,
            tarea.titulo,
            tarea.vence.format("%Y-%m-%dT%H:%M")
        )
    }
}

/// "hoy", "mañana", "en 3 días", "hace 2 días"… para que el modelo entienda
/// de un vistazo lo urgente que es.
fn cuando(vence: NaiveDateTime, ahora: NaiveDateTime) -> String {
    let dias = (vence.date() - ahora.date()).num_days();
    match dias {
        0 => "hoy".to_string(),
        1 => "mañana".to_string(),
        -1 => "venció ayer".to_string(),
        dias if dias > 1 => format!("en {dias} días"),
        dias => format!("venció hace {} días", -dias),
    }
}

fn materias(datos: &Datos) -> Vec<String> {
    let mut lista: Vec<String> = Vec::new();
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
        let clave = crate::modelo::clave_materia(materia);
        if lista
            .iter()
            .any(|vista| crate::modelo::clave_materia(vista) == clave)
        {
            continue;
        }
        lista.push(materia.to_string());
    }
    lista
}

fn hora(hora: chrono::NaiveTime) -> String {
    hora.format("%H:%M").to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::modelo::{Dia, Recordatorio};
    use chrono::NaiveDate;

    fn momento(texto: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(texto, "%Y-%m-%dT%H:%M").expect("fecha de prueba")
    }

    fn fecha(texto: &str) -> NaiveDate {
        NaiveDate::parse_from_str(texto, "%Y-%m-%d").expect("fecha de prueba")
    }

    fn datos() -> Datos {
        let mut datos = Datos {
            ajustes: Default::default(),
            semestre: None,
            clases: Vec::new(),
            tareas: Vec::new(),
            recordatorios: Vec::new(),
            eventos: Vec::new(),
        };
        datos.clases.push(crate::modelo::Clase {
            id: "c-uno".into(),
            materia: "Control".into(),
            dias: vec![Dia::Lun, Dia::Mie],
            inicio: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            fin: chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            lugar: Some("453-201".into()),
        });
        datos.tareas.push(Tarea {
            id: "t-uno".into(),
            titulo: "Taller 2".into(),
            materia: Some("Control".into()),
            vence: momento("2026-11-28T23:59"),
            hecha: false,
            hecha_en: None,
            origen: None,
            enlace: None,
        });
        datos.tareas.push(Tarea {
            id: "t-dos".into(),
            titulo: "Laboratorio".into(),
            materia: None,
            vence: momento("2026-11-20T10:00"),
            hecha: true,
            hecha_en: Some(fecha("2026-11-19")),
            origen: None,
            enlace: None,
        });
        datos.recordatorios.push(Recordatorio {
            id: "r-uno".into(),
            texto: "Revisar Moodle".into(),
            dias: vec![Dia::Lun, Dia::Vie],
            hora: chrono::NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
        });
        datos
    }

    #[test]
    fn resumen_lleva_lo_que_el_modelo_necesita() {
        // Domingo 22 de noviembre de 2026, 01:12 (como el arranque real).
        let texto = resumen(&datos(), momento("2026-11-22T01:12"), "UTC-05:00");

        assert!(
            texto.contains("domingo, 22 de noviembre de 2026, 01:12 (UTC-05:00)"),
            "{texto}"
        );
        assert!(texto.contains("Materias: Control."), "{texto}");
        assert!(
            texto.contains("- id c-uno: Control · lun, mie · 07:00–09:00 · 453-201"),
            "{texto}"
        );
        assert!(
            texto.contains("- id t-uno: «Taller 2» · Control · vence 2026-11-28T23:59 (en 6 días)"),
            "{texto}"
        );
        assert!(
            texto.contains("- id t-dos: «Laboratorio» · vencía 2026-11-20T10:00 · hecha el 2026-11-19"),
            "{texto}"
        );
        assert!(
            texto.contains("- id r-uno: «Revisar Moodle» · lun, vie · 18:00"),
            "{texto}"
        );
        assert!(texto.contains("Eventos:\n- (ninguno)"), "{texto}");

        let mut con_evento = datos();
        con_evento.eventos.push(crate::modelo::Evento {
            id: "e-uno".into(),
            titulo: "Estudiar Taller 2".into(),
            fecha: fecha("2026-11-26"),
            inicio: chrono::NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
            fin: chrono::NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
            tarea: Some("t-uno".into()),
            nota: None,
        });
        let texto_ev = resumen(&con_evento, momento("2026-11-22T01:12"), "UTC-05:00");
        assert!(
            texto_ev.contains("- id e-uno: «Estudiar Taller 2» · 2026-11-26 · 15:00–16:00 · tarea t-uno («Taller 2»)"),
            "{texto_ev}"
        );
    }

    #[test]
    fn resumen_vacio_no_finge_datos() {
        let vacio = Datos {
            ajustes: Default::default(),
            semestre: None,
            clases: Vec::new(),
            tareas: Vec::new(),
            recordatorios: Vec::new(),
            eventos: Vec::new(),
        };
        let texto = resumen(&vacio, momento("2026-11-22T01:12"), "UTC-05:00");
        assert!(texto.contains("Materias: ninguna."), "{texto}");
        assert!(texto.contains("Clases:\n- (ninguna)"), "{texto}");
        assert!(texto.contains("Recordatorios:\n- (ninguno)"), "{texto}");
        assert!(!texto.contains("Tareas hechas hace poco"), "{texto}");
    }

    /// Lo hecho hace más de 30 días no ensucia el prompt.
    #[test]
    fn hechas_viejas_fuera() {
        let mut datos = datos();
        datos.tareas[1].hecha_en = Some(fecha("2026-09-01"));
        let texto = resumen(&datos, momento("2026-11-22T01:12"), "UTC-05:00");
        assert!(!texto.contains("t-dos"), "{texto}");
    }

    #[test]
    fn cuando_habla_en_dias() {
        let ahora = momento("2026-11-22T08:00");
        assert_eq!(cuando(momento("2026-11-22T23:59"), ahora), "hoy");
        assert_eq!(cuando(momento("2026-11-23T00:10"), ahora), "mañana");
        assert_eq!(cuando(momento("2026-11-21T10:00"), ahora), "venció ayer");
        assert_eq!(cuando(momento("2026-11-19T10:00"), ahora), "venció hace 3 días");
    }
}
