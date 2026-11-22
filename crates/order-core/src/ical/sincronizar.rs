//! Reglas puras de sincronización de eventos iCal contra `Datos`.

use crate::ical::config::CalendarioConfig;
use crate::ical::parser::EventoIcal;
use crate::modelo::Datos;
use crate::moodle;
use crate::moodle::sincronizar::{ActualizacionTareaSync, Cambios, TareaNuevaSync};
use crate::presentacion::materias::materias_de;
use std::collections::HashSet;

/// Calcula el identificador único `origen` de un evento iCal.
pub fn origen_ical(hash_url: &str, evento: &EventoIcal) -> String {
    match evento.fecha_recurrencia {
        Some(fecha) => format!("ical:{hash_url}:{}:{fecha}", evento.uid),
        None => format!("ical:{hash_url}:{}", evento.uid),
    }
}

/// Calcula los cambios de un calendario iCal contra `datos`.
///
/// Reglas:
/// - Evento sin tarea con ese `origen` → crear tarea (`hecha = false`).
/// - Tarea existente con ese `origen` → actualizar `titulo`, `vence` y `enlace` si cambiaron.
/// - Nunca marca o desmarca `hecha` sola.
/// - Nunca borra tareas ni toca tareas manuales.
pub fn aplicar(datos: &Datos, cal: &CalendarioConfig, eventos: &[EventoIcal]) -> Cambios {
    let materias = materias_de(datos);
    let mut cambios = Cambios::default();
    let mut vistos = HashSet::new();

    let materia_por_defecto = resolver_materia_calendario(cal, &materias);

    for evento in eventos {
        let origen = origen_ical(&cal.id, evento);
        if !vistos.insert(origen.clone()) {
            continue;
        }
        let titulo = evento.titulo.split_whitespace().collect::<Vec<_>>().join(" ");
        if titulo.is_empty() {
            continue;
        }
        let enlace = evento
            .enlace
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        match datos
            .tareas
            .iter()
            .find(|t| t.origen.as_deref() == Some(origen.as_str()))
        {
            Some(existente) => {
                let mut act = ActualizacionTareaSync {
                    id: existente.id.clone(),
                    origen: origen.clone(),
                    titulo: None,
                    vence: None,
                    enlace: None,
                    marcar_hecha_en: None,
                };
                let mut cambio = false;

                if existente.titulo != titulo {
                    act.titulo = Some(titulo);
                    cambio = true;
                }
                if existente.vence != evento.vence {
                    act.vence = Some(evento.vence);
                    cambio = true;
                }
                if existente.enlace != enlace {
                    act.enlace = Some(enlace);
                    cambio = true;
                }

                if cambio {
                    cambios.num_actualizadas += 1;
                    cambios.actualizadas.push(act);
                }
            }
            None => {
                cambios.creadas.push(TareaNuevaSync {
                    titulo,
                    materia: materia_por_defecto.clone(),
                    vence: evento.vence,
                    origen,
                    enlace,
                    hecha: false,
                    hecha_en: None,
                });
                cambios.num_creadas += 1;
            }
        }
    }

    cambios
}

fn resolver_materia_calendario(cal: &CalendarioConfig, materias_order: &[String]) -> Option<String> {
    if let Some(m) = cal.materia.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        return Some(crate::presentacion::materias::canonica(materias_order, m));
    }
    if cal.nombre.trim().is_empty() || materias_order.is_empty() {
        return None;
    }
    // Solo asigna materia automática si el nombre del calendario coincide con alguna materia de ORDER.
    let candidata = moodle::emparejar_materia(&cal.nombre, materias_order)?;
    if materias_order
        .iter()
        .any(|m| crate::modelo::clave_materia(m) == crate::modelo::clave_materia(&candidata))
    {
        Some(candidata)
    } else {
        None
    }
}
