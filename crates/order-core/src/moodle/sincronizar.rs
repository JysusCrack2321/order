//! Reglas puras de sincronización de eventos de Moodle contra `Datos`.

use crate::modelo::{Datos, FORMATO_FECHA, FORMATO_FECHA_HORA, Tarea};
use crate::moodle::{EventoMoodle, ResumenSync, emparejar_materia};
use crate::ops::{self, NuevaTarea};
use crate::presentacion::materias::materias_de;
use chrono::{NaiveDate, NaiveDateTime};
use std::collections::HashSet;
use toml_edit::DocumentMut;

/// Tarea nueva que se debe agregar al documento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TareaNuevaSync {
    pub titulo: String,
    pub materia: Option<String>,
    pub vence: NaiveDateTime,
    pub origen: String,
    pub enlace: Option<String>,
    pub hecha: bool,
    pub hecha_en: Option<NaiveDate>,
}

/// Actualización sobre una tarea existente identificada por `id` (o `origen`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActualizacionTareaSync {
    pub id: String,
    pub origen: String,
    pub titulo: Option<String>,
    pub vence: Option<NaiveDateTime>,
    pub enlace: Option<Option<String>>,
    pub marcar_hecha_en: Option<NaiveDate>,
}

/// Diferencias calculadas al sincronizar contra `Datos`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cambios {
    pub creadas: Vec<TareaNuevaSync>,
    pub actualizadas: Vec<ActualizacionTareaSync>,
    pub num_creadas: usize,
    pub num_actualizadas: usize,
    pub num_hechas: usize,
}

impl Cambios {
    pub fn hay_cambios(&self) -> bool {
        !self.creadas.is_empty() || !self.actualizadas.is_empty()
    }

    pub fn resumen(&self) -> ResumenSync {
        ResumenSync {
            creadas: self.num_creadas as u32,
            actualizadas: self.num_actualizadas as u32,
            hechas: self.num_hechas as u32,
        }
    }

    /// Aplica los cambios calculados sobre el documento TOML (`Almacen::modificar`).
    pub fn aplicar_en_doc(&self, doc: &mut DocumentMut) -> Result<(), String> {
        for nueva in &self.creadas {
            let t = NuevaTarea {
                titulo: nueva.titulo.clone(),
                materia: nueva.materia.clone(),
                vence: nueva.vence,
            };
            ops::agregar_tarea_sincronizada(
                doc,
                &t,
                &nueva.origen,
                nueva.enlace.as_deref(),
                nueva.hecha,
                nueva.hecha_en,
            )?;
        }

        for act in &self.actualizadas {
            ops::actualizar_tarea_sincronizada(
                doc,
                &act.id,
                &act.origen,
                act.titulo.as_deref(),
                act.vence,
                act.enlace.as_ref().map(|e| e.as_deref()),
                act.marcar_hecha_en,
            )?;
        }

        Ok(())
    }

    /// Aplica los cambios sobre una copia en memoria de `Datos` (útil en pruebas).
    pub fn aplicar_en_datos(&self, datos: &mut Datos) {
        for nueva in &self.creadas {
            datos.tareas.push(Tarea {
                id: String::new(),
                titulo: nueva.titulo.clone(),
                materia: nueva.materia.clone(),
                vence: nueva.vence,
                hecha: nueva.hecha,
                hecha_en: nueva.hecha_en,
                origen: Some(nueva.origen.clone()),
                enlace: nueva.enlace.clone(),
            });
        }
        for act in &self.actualizadas {
            if let Some(t) = datos.tareas.iter_mut().find(|t| {
                (!act.id.is_empty() && t.id == act.id)
                    || t.origen.as_deref() == Some(act.origen.as_str())
            }) {
                if let Some(titulo) = &act.titulo {
                    t.titulo = titulo.clone();
                }
                if let Some(vence) = act.vence {
                    t.vence = vence;
                }
                if let Some(enlace) = &act.enlace {
                    t.enlace = enlace.clone();
                }
                if let Some(fecha) = act.marcar_hecha_en {
                    t.hecha = true;
                    t.hecha_en = Some(fecha);
                }
            }
        }
    }
}

/// Calcula los cambios necesarios para reflejar `eventos` de Moodle sobre `datos`.
///
/// Reglas:
/// - Evento sin tarea con ese `origen` → crea tarea nueva.
/// - Tarea existente con ese `origen` → actualiza `titulo`, `vence` y `enlace` si cambiaron.
/// - Si Moodle dice completada/entregada → `hecha = true`, `hecha_en = hoy`. Nunca desmarca.
/// - Tareas de Moodle que ya no llegan no se borran.
/// - Tareas manuales (sin `origen`) nunca se tocan.
pub fn aplicar(datos: &Datos, eventos: &[EventoMoodle], hoy: NaiveDate) -> Cambios {
    let materias = materias_de(datos);
    let mut cambios = Cambios::default();
    let mut vistos = HashSet::new();

    for evento in eventos {
        let origen = evento.origen();
        if !vistos.insert(origen.clone()) {
            continue;
        }
        let titulo = limpiar_texto(&evento.titulo);
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
                let mut cambio = ActualizacionTareaSync {
                    id: existente.id.clone(),
                    origen: origen.clone(),
                    titulo: None,
                    vence: None,
                    enlace: None,
                    marcar_hecha_en: None,
                };
                let mut toco_meta = false;

                if existente.titulo != titulo {
                    cambio.titulo = Some(titulo);
                    toco_meta = true;
                }
                if existente.vence != evento.vence {
                    cambio.vence = Some(evento.vence);
                    toco_meta = true;
                }
                if existente.enlace != enlace {
                    cambio.enlace = Some(enlace);
                    toco_meta = true;
                }
                let marco_hecha = if evento.completada && !existente.hecha {
                    cambio.marcar_hecha_en = Some(hoy);
                    true
                } else {
                    false
                };

                if toco_meta || marco_hecha {
                    if toco_meta {
                        cambios.num_actualizadas += 1;
                    }
                    if marco_hecha {
                        cambios.num_hechas += 1;
                    }
                    cambios.actualizadas.push(cambio);
                }
            }
            None => {
                let materia = emparejar_materia(&evento.curso, &materias);
                let hecha = evento.completada;
                let hecha_en = hecha.then_some(hoy);
                cambios.creadas.push(TareaNuevaSync {
                    titulo,
                    materia,
                    vence: evento.vence,
                    origen,
                    enlace,
                    hecha,
                    hecha_en,
                });
                cambios.num_creadas += 1;
            }
        }
    }

    let _ = (FORMATO_FECHA, FORMATO_FECHA_HORA);
    cambios
}

fn limpiar_texto(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::almacen::Almacen;
    use crate::modelo::{Ajustes, Clase, Dia};
    use chrono::NaiveTime;
    use tempfile::TempDir;

    fn fecha(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).unwrap()
    }

    fn momento(anio: i32, mes: u32, dia: u32, h: u32, m: u32) -> NaiveDateTime {
        fecha(anio, mes, dia).and_hms_opt(h, m, 0).unwrap()
    }

    fn datos_base() -> Datos {
        Datos {
            ajustes: Ajustes::default(),
            semestre: None,
            clases: vec![Clase {
                id: "c-1".into(),
                materia: "Control".into(),
                dias: vec![Dia::Lun],
                inicio: NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
                fin: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                lugar: None,
            }],
            tareas: vec![Tarea {
                id: "t-manual".into(),
                titulo: "Comprar cuaderno".into(),
                materia: None,
                vence: momento(2026, 11, 25, 18, 0),
                hecha: false,
                hecha_en: None,
                origen: None,
                enlace: None,
            }],
            recordatorios: Vec::new(),
            eventos: Vec::new(),
        }
    }

    fn evento(instance: u64, titulo: &str, curso: &str, vence: NaiveDateTime, completada: bool) -> EventoMoodle {
        EventoMoodle {
            sitio: "https://micampus.unal.edu.co/".into(),
            modulename: "assign".into(),
            instance,
            titulo: titulo.into(),
            curso: curso.into(),
            vence,
            enlace: Some(format!("https://micampus.unal.edu.co/mod/assign/view.php?id={instance}")),
            completada,
        }
    }

    #[test]
    fn primera_sincronizacion_y_segunda_sin_cambios() {
        let mut datos = datos_base();
        let hoy = fecha(2026, 11, 22);
        let eventos = vec![
            evento(1, "Taller 1", "[2022888] CONTROL AUTOMÁTICO (01)", momento(2026, 11, 28, 23, 59), false),
            evento(2, "Quiz 1", "Robótica [3000123] - Grupo 2", momento(2026, 11, 29, 20, 0), false),
            evento(3, "Foro 1", "[2022888] Control", momento(2026, 11, 30, 12, 0), true),
        ];

        let c1 = aplicar(&datos, &eventos, hoy);
        assert!(c1.hay_cambios());
        assert_eq!(c1.num_creadas, 3);
        assert_eq!(c1.num_actualizadas, 0);
        assert_eq!(c1.num_hechas, 0);
        assert_eq!(c1.creadas[0].materia.as_deref(), Some("Control"));
        assert_eq!(c1.creadas[1].materia.as_deref(), Some("Robótica"));
        assert!(c1.creadas[2].hecha);
        assert_eq!(c1.creadas[2].hecha_en, Some(hoy));

        c1.aplicar_en_datos(&mut datos);
        assert_eq!(datos.tareas.len(), 4);

        // Segunda sincronización con los mismos eventos: cero cambios.
        let c2 = aplicar(&datos, &eventos, hoy);
        assert!(!c2.hay_cambios());
        assert_eq!(c2.resumen(), ResumenSync::default());
    }

    #[test]
    fn actualiza_titulo_fecha_enlace_y_marca_entregada_sin_desmarcar_manual() {
        let mut datos = datos_base();
        let hoy = fecha(2026, 11, 22);
        datos.tareas.push(Tarea {
            id: "t-m1".into(),
            titulo: "Taller editado a mano".into(),
            materia: Some("Control".into()),
            vence: momento(2026, 11, 28, 23, 59),
            hecha: false,
            hecha_en: None,
            origen: Some("moodle:micampus.unal.edu.co:assign:10".into()),
            enlace: Some("https://micampus.unal.edu.co/mod/assign/view.php?id=10".into()),
        });
        // Esta ya la marcó el usuario hecha a mano aunque Moodle diga false.
        datos.tareas.push(Tarea {
            id: "t-m2".into(),
            titulo: "Quiz".into(),
            materia: Some("Control".into()),
            vence: momento(2026, 11, 24, 10, 0),
            hecha: true,
            hecha_en: Some(fecha(2026, 11, 21)),
            origen: Some("moodle:micampus.unal.edu.co:quiz:20".into()),
            enlace: None,
        });
        // Esta ya no viene de Moodle: no debe borrarse.
        datos.tareas.push(Tarea {
            id: "t-m3".into(),
            titulo: "Entrega pasada".into(),
            materia: Some("Control".into()),
            vence: momento(2026, 10, 10, 23, 59),
            hecha: false,
            hecha_en: None,
            origen: Some("moodle:micampus.unal.edu.co:assign:99".into()),
            enlace: None,
        });

        let eventos = vec![
            EventoMoodle {
                sitio: "micampus.unal.edu.co".into(),
                modulename: "assign".into(),
                instance: 10,
                titulo: "Taller 1 (plazo ampliado)".into(),
                curso: "Control".into(),
                vence: momento(2026, 12, 1, 23, 59),
                enlace: Some("https://micampus.unal.edu.co/mod/assign/view.php?id=10&n=1".into()),
                completada: true,
            },
            EventoMoodle {
                sitio: "micampus.unal.edu.co".into(),
                modulename: "quiz".into(),
                instance: 20,
                titulo: "Quiz".into(),
                curso: "Control".into(),
                vence: momento(2026, 11, 24, 10, 0),
                enlace: None,
                completada: false,
            },
        ];

        let c = aplicar(&datos, &eventos, hoy);
        assert_eq!(c.num_creadas, 0);
        assert_eq!(c.num_actualizadas, 1);
        assert_eq!(c.num_hechas, 1);
        assert_eq!(c.actualizadas.len(), 1);

        c.aplicar_en_datos(&mut datos);
        let t1 = datos.tareas.iter().find(|t| t.id == "t-m1").unwrap();
        assert_eq!(t1.titulo, "Taller 1 (plazo ampliado)");
        assert_eq!(t1.vence, momento(2026, 12, 1, 23, 59));
        assert!(t1.hecha);
        assert_eq!(t1.hecha_en, Some(hoy));

        let t2 = datos.tareas.iter().find(|t| t.id == "t-m2").unwrap();
        assert!(t2.hecha, "nunca desmarca una tarea ya hecha");
        assert_eq!(t2.hecha_en, Some(fecha(2026, 11, 21)));

        assert!(datos.tareas.iter().any(|t| t.id == "t-m3"), "no borra tareas viejas");
        assert!(datos.tareas.iter().any(|t| t.id == "t-manual" && !t.hecha), "no toca manuales");
    }

    #[test]
    fn aplica_sobre_almacen_y_no_reescribe_si_no_hay_cambios() {
        let dir = TempDir::new().expect("tempdir");
        let almacen = Almacen {
            ruta: dir.path().join("order.toml"),
        };
        std::fs::write(
            &almacen.ruta,
            "# Nota personal\n[[clase]]\nid = \"c-1\"\nmateria = \"Control\"\ndias = [\"lun\"]\ninicio = \"07:00\"\nfin = \"09:00\"\n",
        )
        .unwrap();

        let hoy = fecha(2026, 11, 22);
        let eventos = vec![evento(
            5,
            "Proyecto",
            "[2022888] Control",
            momento(2026, 12, 2, 23, 59),
            false,
        )];

        let datos = almacen.cargar().unwrap();
        let c1 = aplicar(&datos, &eventos, hoy);
        assert!(c1.hay_cambios());
        let datos2 = almacen.modificar(|doc| c1.aplicar_en_doc(doc)).unwrap();
        assert_eq!(datos2.tareas.len(), 1);
        assert!(!datos2.tareas[0].id.is_empty());
        assert_eq!(
            datos2.tareas[0].origen.as_deref(),
            Some("moodle:micampus.unal.edu.co:assign:5")
        );

        let texto_en_disco = std::fs::read_to_string(&almacen.ruta).unwrap();
        assert!(texto_en_disco.contains("# Nota personal"));

        // Segunda corrida: no hay cambios, así que no reescribiríamos.
        let c2 = aplicar(&datos2, &eventos, hoy);
        assert!(!c2.hay_cambios());
    }
}
