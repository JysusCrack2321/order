//! Normalización y paleta de colores de las materias.
//!
//! Cada materia tiene un color suave y estable, sacado solo de su nombre: el
//! mismo nombre da siempre el mismo tono, sin importar mayúsculas, acentos ni
//! el orden del archivo. La paleta vive en la interfaz; aquí se elige el índice.

use crate::modelo::{self, Datos};

/// Cuántos tonos tiene la paleta de materias de la interfaz.
pub const TONOS_MATERIA: i32 = 9;

/// El tono gris de los elementos sin materia: el último de la paleta.
pub const TONO_SIN_MATERIA: i32 = TONOS_MATERIA - 1;

/// Una materia lista para mostrar en el selector con su tono asignado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MateriaConTono {
    pub nombre: String,
    pub tono: i32,
}

/// Las materias del archivo, en el orden en que aparecen (primero las clases),
/// sin repetir aunque cambien mayúsculas o acentos.
pub fn materias_de(datos: &Datos) -> Vec<String> {
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

/// Las materias con el tono que les toca, para el selector.
pub fn materias_con_tono(materias: &[String]) -> Vec<MateriaConTono> {
    materias
        .iter()
        .map(|materia| MateriaConTono {
            nombre: materia.clone(),
            tono: tono_materia(materia),
        })
        .collect()
}

/// La grafía que ya existe para esa materia, o el texto tal cual si es nueva.
pub fn canonica(materias: &[String], texto: &str) -> String {
    modelo::canonica_materia(materias, texto)
}

/// Clave de comparación: minúsculas, sin acentos y con un solo espacio entre
/// palabras. "Robótica  " y "robotica" dan la misma clave.
pub fn clave_materia(texto: &str) -> String {
    modelo::clave_materia(texto)
}

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

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::modelo::{Ajustes, Clase, Dia, Tarea};
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

    fn hora(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn momento(anio: i32, mes: u32, dia: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(anio, mes, dia)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    fn datos_vacios() -> Datos {
        Datos {
            ajustes: Ajustes::default(),
            semestre: None,
            clases: Vec::new(),
            tareas: Vec::new(),
            recordatorios: Vec::new(),
        }
    }

    /// La lista de materias no repite y manda la grafía del archivo.
    #[test]
    fn materias_sin_duplicados_ni_acentos() {
        let mut d = datos_vacios();
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
            origen: None,
            enlace: None,
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
        let lista = materias_con_tono(&["Robótica".to_string(), "Control".to_string()]);
        assert_eq!(lista[0].nombre, "Robótica");
        assert_eq!(lista[0].tono, tono_materia("Robótica"));
        assert_eq!(lista[1].tono, tono_materia("CONTROL"));
    }
}
