//! Cálculo de la vista **Semana** y de la lista de **Pendientes**.
//!
//! Aquí no hay UI: solo se convierte el contenido del archivo (`Datos`) en lo
//! que la ventana necesita dibujar. La semana se calcula a partir de su lunes
//! y las clases se repiten cada siete días mientras la fecha caiga dentro de
//! `[semestre]`; sin `[semestre]`, aparecen todas las semanas.

use crate::modelo::{Clase, Datos, Dia, Tarea};
use chrono::{Datelike, Days, NaiveDate, NaiveDateTime, NaiveTime, Weekday};

/// Una clase ya colocada en un día concreto de la semana mostrada.
#[derive(Debug, Clone, PartialEq)]
pub struct Bloque {
    pub clase_id: String,
    pub materia: String,
    pub inicio: NaiveTime,
    pub fin: NaiveTime,
    pub lugar: Option<String>,
}

/// Un día de la semana: sus clases y las tareas que vencen ese día.
#[derive(Debug, Clone, PartialEq)]
pub struct DiaVista {
    pub fecha: NaiveDate,
    pub bloques: Vec<Bloque>,
    /// Tareas (hechas o no) cuyo `vence` cae en `fecha`, por hora.
    pub tareas: Vec<Tarea>,
}

/// El lunes de la semana que contiene `fecha` (la semana empieza en lunes).
pub fn lunes_de(fecha: NaiveDate) -> NaiveDate {
    fecha - Days::new(u64::from(fecha.weekday().num_days_from_monday()))
}

/// Los días de la semana que empieza en `lunes`: de lunes a viernes siempre, y
/// sábado o domingo solo si ese día tiene clases dentro del semestre.
pub fn semana(datos: &Datos, lunes: NaiveDate) -> Vec<DiaVista> {
    let mut dias = Vec::with_capacity(7);

    for atraso in 0..7u64 {
        let Some(fecha) = lunes.checked_add_days(Days::new(atraso)) else {
            break;
        };
        let dia = dia_de(fecha);

        let del_dia: Vec<&Clase> = datos
            .clases
            .iter()
            .filter(|c| c.dia == dia && dentro_del_semestre(datos, fecha))
            .collect();

        // El fin de semana solo ocupa columna si de verdad hay clase.
        if matches!(dia, Dia::Sab | Dia::Dom) && del_dia.is_empty() {
            continue;
        }

        let mut bloques: Vec<Bloque> = del_dia
            .into_iter()
            .map(|c| Bloque {
                clase_id: c.id.clone(),
                materia: c.materia.clone(),
                inicio: c.inicio,
                fin: c.fin,
                lugar: c.lugar.clone(),
            })
            .collect();
        bloques.sort_by(|a, b| {
            a.inicio
                .cmp(&b.inicio)
                .then(a.fin.cmp(&b.fin))
                .then(a.materia.cmp(&b.materia))
        });

        let mut tareas: Vec<Tarea> = datos
            .tareas
            .iter()
            .filter(|t| t.vence.date() == fecha)
            .cloned()
            .collect();
        tareas.sort_by_key(|t| t.vence);

        dias.push(DiaVista {
            fecha,
            bloques,
            tareas,
        });
    }

    dias
}

/// Tareas sin hacer, de la que vence antes a la que vence después.
pub fn pendientes(datos: &Datos) -> Vec<Tarea> {
    let mut lista: Vec<Tarea> = datos.tareas.iter().filter(|t| !t.hecha).cloned().collect();
    lista.sort_by_key(|t| t.vence);
    lista
}

/// ¿Pasó su vencimiento sin estar hecha?
pub fn vencida(t: &Tarea, ahora: NaiveDateTime) -> bool {
    !t.hecha && t.vence < ahora
}

/// Día del archivo (`lun` … `dom`) que corresponde a una fecha.
fn dia_de(fecha: NaiveDate) -> Dia {
    match fecha.weekday() {
        Weekday::Mon => Dia::Lun,
        Weekday::Tue => Dia::Mar,
        Weekday::Wed => Dia::Mie,
        Weekday::Thu => Dia::Jue,
        Weekday::Fri => Dia::Vie,
        Weekday::Sat => Dia::Sab,
        Weekday::Sun => Dia::Dom,
    }
}

/// Sin `[semestre]` todo vale; con él, solo las fechas entre `inicio` y `fin`.
fn dentro_del_semestre(datos: &Datos, fecha: NaiveDate) -> bool {
    datos
        .semestre
        .as_ref()
        .is_none_or(|s| fecha >= s.inicio && fecha <= s.fin)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::modelo::{Ajustes, Semestre};

    fn fecha(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).unwrap()
    }

    fn hora(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn momento(anio: i32, mes: u32, dia: u32, h: u32, m: u32) -> NaiveDateTime {
        fecha(anio, mes, dia).and_hms_opt(h, m, 0).unwrap()
    }

    fn datos(clases: Vec<Clase>, tareas: Vec<Tarea>, semestre: Option<Semestre>) -> Datos {
        Datos {
            ajustes: Ajustes::default(),
            semestre,
            clases,
            tareas,
            recordatorios: Vec::new(),
        }
    }

    fn clase(materia: &str, dia: Dia) -> Clase {
        Clase {
            id: String::new(),
            materia: materia.to_string(),
            dia,
            inicio: hora(7, 0),
            fin: hora(9, 0),
            lugar: None,
        }
    }

    fn tarea(titulo: &str, vence: NaiveDateTime, hecha: bool) -> Tarea {
        Tarea {
            id: String::new(),
            titulo: titulo.to_string(),
            materia: None,
            vence,
            hecha,
            hecha_en: hecha.then(|| vence.date()),
        }
    }

    fn fechas(dias: &[DiaVista]) -> Vec<NaiveDate> {
        dias.iter().map(|d| d.fecha).collect()
    }

    #[test]
    fn semana_respeta_semestre() {
        let d = datos(
            vec![clase("Control", Dia::Lun)],
            Vec::new(),
            Some(Semestre {
                inicio: fecha(2026, 8, 3),
                fin: fecha(2026, 12, 4),
            }),
        );

        let dentro = semana(&d, fecha(2026, 11, 30));
        let lunes = &dentro[0];
        assert_eq!(lunes.fecha, fecha(2026, 11, 30));
        assert_eq!(
            lunes.bloques.len(),
            1,
            "el lunes 30-nov aún cae en semestre"
        );
        assert_eq!(lunes.bloques[0].materia, "Control");

        let despues = semana(&d, fecha(2026, 12, 7));
        assert_eq!(
            despues.len(),
            5,
            "la semana se dibuja aunque no haya clases"
        );
        assert!(
            despues.iter().all(|dia| dia.bloques.is_empty()),
            "pasado el fin del semestre no se dibuja ninguna clase"
        );
    }

    #[test]
    fn semana_cruza_anio() {
        assert_eq!(lunes_de(fecha(2027, 1, 1)), fecha(2026, 12, 28));

        let d = datos(vec![clase("Control", Dia::Vie)], Vec::new(), None);
        let dias = semana(&d, lunes_de(fecha(2027, 1, 1)));
        assert_eq!(
            fechas(&dias),
            vec![
                fecha(2026, 12, 28),
                fecha(2026, 12, 29),
                fecha(2026, 12, 30),
                fecha(2026, 12, 31),
                fecha(2027, 1, 1),
            ]
        );
        assert_eq!(dias[4].bloques.len(), 1, "la clase del viernes es el 1-ene");
    }

    #[test]
    fn fin_de_semana_solo_si_hay_clases() {
        let entre_semana = datos(vec![clase("Control", Dia::Lun)], Vec::new(), None);
        assert_eq!(semana(&entre_semana, fecha(2026, 11, 30)).len(), 5);

        let con_sabado = datos(
            vec![clase("Control", Dia::Lun), clase("Taller", Dia::Sab)],
            Vec::new(),
            None,
        );
        let dias = semana(&con_sabado, fecha(2026, 11, 30));
        assert_eq!(dias.len(), 6);
        assert_eq!(dias[5].fecha, fecha(2026, 12, 5));
        assert_eq!(dias[5].bloques[0].materia, "Taller");
    }

    #[test]
    fn pendientes_ordenados_sin_hechas() {
        let d = datos(
            Vec::new(),
            vec![
                tarea("Informe", momento(2026, 12, 2, 18, 0), false),
                tarea("Taller 2", momento(2026, 11, 28, 23, 59), false),
                tarea("Quiz viejo", momento(2026, 11, 25, 10, 0), true),
            ],
            None,
        );

        let lista = pendientes(&d);
        let titulos: Vec<&str> = lista.iter().map(|t| t.titulo.as_str()).collect();
        assert_eq!(titulos, vec!["Taller 2", "Informe"]);
        assert!(
            lista.windows(2).all(|par| par[0].vence <= par[1].vence),
            "van de la fecha más próxima a la más lejana"
        );
    }
}
