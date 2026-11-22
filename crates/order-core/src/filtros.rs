//! Filtros combinables de la vista **Pendientes**.
//!
//! La lista muestra las tareas que pasan **todos** los filtros activos:
//! materia, fecha de entrega y hora de entrega. Sin filtros se ven todas.
//! Primero van las pendientes (de la que vence antes a la que vence después) y
//! después las hechas, para que lo que falta quede siempre a la vista.
//!
//! Aquí no hay UI: `puente.rs` traduce los controles a un `Filtro` y este
//! módulo decide qué se ve. Todo se prueba sin ventana.

use crate::modelo::Tarea;
use chrono::{Days, NaiveDate, NaiveTime};

/// Qué fechas de entrega se quieren.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoFecha {
    Todas,
    Hoy,
    /// De lunes a domingo de la semana que contiene hoy.
    EstaSemana,
    /// Entre `desde` y `hasta` (los dos inclusive; `None` deja el extremo abierto).
    Rango,
}

/// Lo que el usuario dejó puesto en la barra de filtros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filtro {
    /// `None` = todas las materias.
    pub materia: Option<String>,
    pub tipo_fecha: TipoFecha,
    pub desde: Option<NaiveDate>,
    pub hasta: Option<NaiveDate>,
    /// Hora de entrega mínima y máxima (las dos inclusive).
    pub hora_desde: Option<NaiveTime>,
    pub hora_hasta: Option<NaiveTime>,
    /// Si es `false`, las hechas no se dibujan.
    pub mostrar_hechas: bool,
}

impl Default for Filtro {
    fn default() -> Self {
        Filtro {
            materia: None,
            tipo_fecha: TipoFecha::Todas,
            desde: None,
            hasta: None,
            hora_desde: None,
            hora_hasta: None,
            mostrar_hechas: true,
        }
    }
}

impl Filtro {
    /// ¿Hay algún filtro puesto (aparte del interruptor de hechas)?
    pub fn activo(&self) -> bool {
        self.materia.is_some()
            || self.tipo_fecha != TipoFecha::Todas
            || self.hora_desde.is_some()
            || self.hora_hasta.is_some()
    }

    /// Las tareas que se ven, ya ordenadas: pendientes primero por
    /// vencimiento y las hechas al final, también por vencimiento.
    pub fn aplicar(&self, tareas: &[Tarea], hoy: NaiveDate) -> Vec<Tarea> {
        let mut lista: Vec<Tarea> = tareas
            .iter()
            .filter(|tarea| self.pasa(tarea, hoy))
            .cloned()
            .collect();
        lista.sort_by_key(|tarea| (tarea.hecha, tarea.vence));
        lista
    }

    /// ¿Esta tarea pasa todos los filtros?
    pub fn pasa(&self, tarea: &Tarea, hoy: NaiveDate) -> bool {
        if tarea.hecha && !self.mostrar_hechas {
            return false;
        }
        self.pasa_materia(tarea) && self.pasa_fecha(tarea, hoy) && self.pasa_hora(tarea)
    }

    fn pasa_materia(&self, tarea: &Tarea) -> bool {
        let Some(filtro) = self.materia.as_deref().map(str::trim) else {
            return true;
        };
        if filtro.is_empty() {
            return true;
        }
        tarea
            .materia
            .as_deref()
            .is_some_and(|materia| materia.trim() == filtro)
    }

    fn pasa_fecha(&self, tarea: &Tarea, hoy: NaiveDate) -> bool {
        let fecha = tarea.vence.date();
        match self.tipo_fecha {
            TipoFecha::Todas => true,
            TipoFecha::Hoy => fecha == hoy,
            TipoFecha::EstaSemana => {
                let lunes = crate::semana::lunes_de(hoy);
                let domingo = lunes.checked_add_days(Days::new(6)).unwrap_or(lunes);
                fecha >= lunes && fecha <= domingo
            }
            TipoFecha::Rango => {
                self.desde.is_none_or(|desde| fecha >= desde)
                    && self.hasta.is_none_or(|hasta| fecha <= hasta)
            }
        }
    }

    fn pasa_hora(&self, tarea: &Tarea) -> bool {
        let hora = tarea.vence.time();
        // Un rango al revés (desde 20:00 hasta 08:00) no selecciona nada: el
        // formulario de filtros no deja guardarlo así, pero el archivo sí.
        if let (Some(desde), Some(hasta)) = (self.hora_desde, self.hora_hasta) {
            if desde > hasta {
                return false;
            }
        }
        self.hora_desde.is_none_or(|desde| hora >= desde)
            && self.hora_hasta.is_none_or(|hasta| hora <= hasta)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use chrono::NaiveDateTime;

    fn fecha(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).unwrap()
    }

    fn hora(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn momento(anio: i32, mes: u32, dia: u32, h: u32, m: u32) -> NaiveDateTime {
        fecha(anio, mes, dia).and_hms_opt(h, m, 0).unwrap()
    }

    fn tarea(titulo: &str, materia: Option<&str>, vence: NaiveDateTime, hecha: bool) -> Tarea {
        Tarea {
            id: titulo.to_lowercase().replace(' ', "-"),
            titulo: titulo.to_string(),
            materia: materia.map(str::to_string),
            vence,
            hecha,
            hecha_en: hecha.then(|| vence.date()),
            origen: None,
            enlace: None,
        }
    }

    /// Tres tareas de la misma semana, una ya hecha y una de otra materia.
    fn ejemplo() -> Vec<Tarea> {
        vec![
            tarea(
                "Informe",
                Some("Control"),
                momento(2026, 11, 27, 23, 0),
                false,
            ),
            tarea(
                "Taller 2",
                Some("Control"),
                momento(2026, 11, 25, 12, 0),
                false,
            ),
            tarea(
                "Quiz viejo",
                Some("Robótica"),
                momento(2026, 11, 24, 8, 0),
                true,
            ),
            tarea(
                "Examen",
                Some("Robótica"),
                momento(2026, 12, 3, 9, 0),
                false,
            ),
        ]
    }

    fn titulos(tareas: &[Tarea]) -> Vec<&str> {
        tareas.iter().map(|tarea| tarea.titulo.as_str()).collect()
    }

    /// Sin filtros se ve todo y las pendientes van antes que las hechas, cada
    /// grupo por vencimiento.
    #[test]
    fn sin_filtros_pendientes_primero() {
        let lista = Filtro::default().aplicar(&ejemplo(), fecha(2026, 11, 25));
        assert_eq!(
            titulos(&lista),
            vec!["Taller 2", "Informe", "Examen", "Quiz viejo"]
        );
        assert!(!Filtro::default().activo());
    }

    /// El interruptor de hechas las esconde sin tocar el resto.
    #[test]
    fn oculta_las_hechas() {
        let filtro = Filtro {
            mostrar_hechas: false,
            ..Filtro::default()
        };
        let lista = filtro.aplicar(&ejemplo(), fecha(2026, 11, 25));
        assert_eq!(titulos(&lista), vec!["Taller 2", "Informe", "Examen"]);
        assert!(!filtro.activo(), "el interruptor no es un filtro");
    }

    #[test]
    fn filtra_por_materia() {
        let filtro = Filtro {
            materia: Some("Robótica".to_string()),
            ..Filtro::default()
        };
        let lista = filtro.aplicar(&ejemplo(), fecha(2026, 11, 25));
        assert_eq!(titulos(&lista), vec!["Examen", "Quiz viejo"]);

        // Una materia que no está deja la lista vacía.
        let filtro = Filtro {
            materia: Some("Cálculo".to_string()),
            ..Filtro::default()
        };
        assert!(filtro.aplicar(&ejemplo(), fecha(2026, 11, 25)).is_empty());
    }

    /// "Hoy" es solo ese día; "Esta semana" va de lunes a domingo.
    #[test]
    fn filtra_por_hoy_y_por_semana() {
        let hoy = fecha(2026, 11, 25); // miércoles
        let solo_hoy = Filtro {
            tipo_fecha: TipoFecha::Hoy,
            ..Filtro::default()
        };
        assert_eq!(
            titulos(&solo_hoy.aplicar(&ejemplo(), hoy)),
            vec!["Taller 2"]
        );

        let semana = Filtro {
            tipo_fecha: TipoFecha::EstaSemana,
            ..Filtro::default()
        };
        assert_eq!(
            titulos(&semana.aplicar(&ejemplo(), hoy)),
            vec!["Taller 2", "Informe", "Quiz viejo"],
            "el examen del 3-dic cae en la semana siguiente"
        );

        // El domingo sigue siendo la misma semana que su lunes.
        let domingo = fecha(2026, 11, 29);
        assert_eq!(
            semana.aplicar(&ejemplo(), domingo).len(),
            3,
            "domingo 29-nov: misma semana"
        );
    }

    /// El rango incluye los dos extremos y admite un solo lado.
    #[test]
    fn rango_inclusivo() {
        let filtro = Filtro {
            tipo_fecha: TipoFecha::Rango,
            desde: Some(fecha(2026, 11, 25)),
            hasta: Some(fecha(2026, 11, 27)),
            ..Filtro::default()
        };
        assert_eq!(
            titulos(&filtro.aplicar(&ejemplo(), fecha(2026, 11, 25))),
            vec!["Taller 2", "Informe"]
        );

        let desde = Filtro {
            tipo_fecha: TipoFecha::Rango,
            desde: Some(fecha(2026, 12, 1)),
            ..Filtro::default()
        };
        assert_eq!(
            titulos(&desde.aplicar(&ejemplo(), fecha(2026, 11, 25))),
            vec!["Examen"],
            "sin tope superior"
        );
    }

    #[test]
    fn filtra_por_hora_de_entrega() {
        let manana = Filtro {
            hora_hasta: Some(hora(12, 0)),
            ..Filtro::default()
        };
        assert_eq!(
            titulos(&manana.aplicar(&ejemplo(), fecha(2026, 11, 25))),
            vec!["Taller 2", "Examen", "Quiz viejo"],
            "las de mediodía o antes"
        );

        let tarde = Filtro {
            hora_desde: Some(hora(13, 0)),
            ..Filtro::default()
        };
        assert_eq!(
            titulos(&tarde.aplicar(&ejemplo(), fecha(2026, 11, 25))),
            vec!["Informe"],
            "después del mediodía"
        );

        let ventana = Filtro {
            hora_desde: Some(hora(9, 0)),
            hora_hasta: Some(hora(23, 0)),
            ..Filtro::default()
        };
        assert_eq!(
            titulos(&ventana.aplicar(&ejemplo(), fecha(2026, 11, 25))),
            vec!["Taller 2", "Informe", "Examen"]
        );
    }

    /// Un rango de horas al revés no selecciona nada (no se interpreta como
    /// "toda la noche").
    #[test]
    fn rango_de_horas_invertido_no_pasa() {
        let filtro = Filtro {
            hora_desde: Some(hora(20, 0)),
            hora_hasta: Some(hora(8, 0)),
            ..Filtro::default()
        };
        assert!(filtro.aplicar(&ejemplo(), fecha(2026, 11, 25)).is_empty());
    }

    /// Los filtros se combinan: materia + fecha + hora.
    #[test]
    fn filtros_combinados() {
        let filtro = Filtro {
            materia: Some("Control".to_string()),
            tipo_fecha: TipoFecha::EstaSemana,
            hora_desde: Some(hora(13, 0)),
            ..Filtro::default()
        };
        assert_eq!(
            titulos(&filtro.aplicar(&ejemplo(), fecha(2026, 11, 25))),
            vec!["Informe"],
            "Control, esta semana y después del mediodía"
        );
        assert!(filtro.activo());
    }

    /// Una tarea sin materia no aparece al filtrar por una materia.
    #[test]
    fn sin_materia_no_pasa_el_filtro_de_materia() {
        let suelta = tarea("Suelta", None, momento(2026, 11, 25, 10, 0), false);
        let filtro = Filtro {
            materia: Some("Control".to_string()),
            ..Filtro::default()
        };
        assert!(!filtro.pasa(&suelta, fecha(2026, 11, 25)));
        assert!(Filtro::default().pasa(&suelta, fecha(2026, 11, 25)));
    }
}
