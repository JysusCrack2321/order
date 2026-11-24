//! Textos en español y cálculos puros de fecha, hora y calendario.

use crate::modelo::{Datos, Dia, FORMATO_FECHA};
use crate::reloj::{self, hora_texto};
use crate::semana;
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike, Weekday};

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
pub const DIAS_SEMANA: [Dia; 7] = [
    Dia::Lun,
    Dia::Mar,
    Dia::Mie,
    Dia::Jue,
    Dia::Vie,
    Dia::Sab,
    Dia::Dom,
];

/// Una casilla del selector de calendario mensual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeldaCalendario {
    pub dia: i32,
    pub fila: i32,
    pub columna: i32,
    pub hoy: bool,
    pub elegida: bool,
}

/// "0:07" para el indicador de grabación.
pub fn formato_tiempo(segundos: u64) -> String {
    format!("{}:{:02}", segundos / 60, segundos % 60)
}

pub fn minutos(hora: NaiveTime) -> i32 {
    hora.hour() as i32 * 60 + hora.minute() as i32
}

pub fn hora(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap_or_default()
}

pub fn nombre_dia(fecha: NaiveDate) -> &'static str {
    texto_dia(dia_de(fecha.weekday()))
}

pub fn texto_dia(dia: Dia) -> &'static str {
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

pub fn dias_texto(dias: &[Dia]) -> String {
    dias.iter()
        .map(|dia| texto_dia(*dia))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn dia_de(dia: Weekday) -> Dia {
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

pub fn fecha_corta(fecha: NaiveDate) -> String {
    format!("{} {}", fecha.day(), MESES[fecha.month0() as usize])
}

/// "miércoles 25 de noviembre", para la cabecera de Inicio y de Día.
pub fn fecha_larga(fecha: NaiveDate) -> String {
    format!(
        "{} {} de {}",
        DIAS_LARGOS[fecha.weekday().num_days_from_monday() as usize],
        fecha.day(),
        MESES_LARGOS[fecha.month0() as usize]
    )
}

pub fn fecha_o_vacio(fecha: Option<NaiveDate>) -> String {
    fecha
        .map(|fecha| fecha.format(FORMATO_FECHA).to_string())
        .unwrap_or_default()
}

pub fn hora_o_vacio(hora: Option<NaiveTime>) -> String {
    hora.map(hora_texto).unwrap_or_default()
}

/// "7:00–9:00 a. m.".
pub fn rango_hora(bloque: &semana::Bloque) -> String {
    reloj::rango_texto(bloque.inicio, bloque.fin)
}

/// "3:00–4:00 p. m.".
pub fn rango_evento(inicio: NaiveTime, fin: NaiveTime) -> String {
    reloj::rango_texto(inicio, fin)
}

/// Cuándo es un evento: "hoy · 3:00–4:00 p. m.", "mañana · ..." o "jue 26 nov · ...".
pub fn cuando_evento(
    fecha: NaiveDate,
    inicio: NaiveTime,
    fin: NaiveTime,
    hoy: NaiveDate,
) -> String {
    let rango = rango_evento(inicio, fin);
    if fecha == hoy {
        format!("hoy · {rango}")
    } else if fecha == hoy + TimeDelta::days(1) {
        format!("mañana · {rango}")
    } else {
        format!(
            "{} {} · {rango}",
            nombre_dia(fecha).to_lowercase(),
            fecha_corta(fecha)
        )
    }
}

/// Hora de una clase en las vistas de resumen. Si no es de hoy, lleva delante
/// el día ("jue 26 nov · 7:00–9:00 a. m.").
pub fn hora_clase(bloque: &semana::Bloque, fecha: NaiveDate, hoy: NaiveDate) -> String {
    if fecha == hoy {
        rango_hora(bloque)
    } else {
        format!(
            "{} {} · {}",
            nombre_dia(fecha).to_lowercase(),
            fecha_corta(fecha),
            rango_hora(bloque)
        )
    }
}

/// Fecha y hora corta de una tarea: "lun 30 nov · 11:59 p. m.".
pub fn fecha_tarea(vence: NaiveDateTime) -> String {
    format!(
        "{} {} · {}",
        nombre_dia(vence.date()),
        fecha_corta(vence.date()),
        hora_texto(vence.time())
    )
}

/// Cuándo toca un recordatorio: "hoy · 6:00 p. m.", "mañana · 9:00 a. m." o
/// "lun 30 nov · 9:00 a. m.".
pub fn cuando_recordatorio(recordatorio: &semana::RecordatorioVisto, hoy: NaiveDate) -> String {
    let hora = hora_texto(recordatorio.hora);
    if recordatorio.fecha == hoy {
        format!("hoy · {hora}")
    } else if recordatorio.fecha == hoy + TimeDelta::days(1) {
        format!("mañana · {hora}")
    } else {
        format!(
            "{} {} · {hora}",
            nombre_dia(recordatorio.fecha).to_lowercase(),
            fecha_corta(recordatorio.fecha)
        )
    }
}

/// "17 – 21 de noviembre de 2026", o "30 noviembre – 4 diciembre de 2026"
/// cuando la semana cruza de mes.
pub fn etiqueta_semana(lunes: NaiveDate, dias: &[semana::DiaVista]) -> String {
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
pub fn resumen(hechas_por_tarea: &[bool], filtrado: bool) -> String {
    let hechas = hechas_por_tarea.iter().filter(|hecha| **hecha).count();
    let pendientes = hechas_por_tarea.len() - hechas;
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

pub fn pie(datos: &Datos, pendientes: usize, ahora: NaiveDateTime) -> String {
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
pub fn proximo_recordatorio(datos: &Datos, ahora: NaiveDateTime) -> Option<(String, NaiveDateTime)> {
    let proximo = semana::proximos_recordatorios(datos, ahora, 1)
        .into_iter()
        .next()?;
    Some((proximo.texto, proximo.fecha.and_time(proximo.hora)))
}

/// Las doce horas de la rueda, de la 1 a las 12.
pub fn horas_texto() -> Vec<String> {
    (1..=12).map(|hora| hora.to_string()).collect()
}

pub fn minutos_texto() -> Vec<String> {
    (0..60).map(|minuto| format!("{minuto:02}")).collect()
}

pub fn etiqueta_mes(anio: i32, mes: u32) -> String {
    let nombre = MESES_LARGOS
        .get(mes.saturating_sub(1) as usize)
        .copied()
        .unwrap_or("mes");
    format!("{nombre} de {anio}")
}

/// El mes que está `delta` meses antes o después, cruzando años.
pub fn mes_vecino(anio: i32, mes: u32, delta: i32) -> (i32, u32) {
    let total = anio * 12 + mes as i32 - 1 + delta;
    (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32)
}

pub fn dias_del_mes(anio: i32, mes: u32) -> i32 {
    let (siguiente_anio, siguiente_mes) = mes_vecino(anio, mes, 1);
    NaiveDate::from_ymd_opt(siguiente_anio, siguiente_mes, 1)
        .and_then(|primero| primero.pred_opt())
        .map(|ultimo| ultimo.day() as i32)
        .unwrap_or(30)
}

/// Seis semanas de siete casillas, con el día 1 en su columna real.
pub fn celdas_mes(anio: i32, mes: u32, elegido: u32, hoy: NaiveDate) -> Vec<CeldaCalendario> {
    let Some(primero) = NaiveDate::from_ymd_opt(anio, mes, 1) else {
        return Vec::new();
    };
    let desfase = primero.weekday().num_days_from_monday() as i32;
    let dias = dias_del_mes(anio, mes);

    (0..42)
        .map(|i| {
            let dia = i - desfase + 1;
            let dentro = (1..=dias).contains(&dia);
            CeldaCalendario {
                dia: if dentro { dia } else { 0 },
                fila: i / 7,
                columna: i % 7,
                hoy: dentro && hoy.year() == anio && hoy.month() == mes && hoy.day() as i32 == dia,
                elegida: dentro && dia == elegido as i32,
            }
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::modelo::{Ajustes, Recordatorio};

    fn fecha(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).unwrap()
    }

    fn momento(anio: i32, mes: u32, dia: u32, h: u32, m: u32) -> NaiveDateTime {
        fecha(anio, mes, dia).and_hms_opt(h, m, 0).unwrap()
    }

    fn datos() -> Datos {
        Datos {
            ajustes: Ajustes::default(),
            semestre: None,
            clases: Vec::new(),
            tareas: Vec::new(),
            recordatorios: Vec::new(),
            eventos: Vec::new(),
        }
    }

    #[test]
    fn etiqueta_dentro_del_mes() {
        let dias = semana::semana(&datos(), fecha(2026, 11, 16));
        assert_eq!(
            etiqueta_semana(fecha(2026, 11, 16), &dias),
            "16 – 20 de noviembre de 2026"
        );
    }

    #[test]
    fn etiqueta_cruzando_mes() {
        let dias = semana::semana(&datos(), fecha(2026, 11, 30));
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
        let celdas = celdas_mes(2026, 11, 21, fecha(2026, 11, 21));
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

    /// La cabecera de Pendientes cuenta lo pendiente y lo hecho por separado.
    #[test]
    fn resumen_cuenta_pendientes_y_hechas() {
        let lista = [false, false, true];
        assert_eq!(resumen(&lista, false), "2 pendientes · 1 hecha");
        assert_eq!(resumen(&lista, true), "2 pendientes · 1 hecha (filtrado)");
        assert_eq!(resumen(&[], false), "nada por entregar");
        assert_eq!(resumen(&[], true), "sin resultados para estos filtros");
        assert_eq!(resumen(&[false], false), "1 tarea pendiente");
        assert_eq!(resumen(&[true, true], false), "2 tareas hechas");
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

    #[test]
    fn texto_de_evento() {
        let hoy = fecha(2026, 11, 25);
        assert_eq!(
            cuando_evento(hoy, hora(15, 0), hora(16, 0), hoy),
            "hoy · 3:00–4:00 p. m."
        );
        assert_eq!(
            cuando_evento(fecha(2026, 11, 26), hora(15, 0), hora(16, 0), hoy),
            "mañana · 3:00–4:00 p. m."
        );
        assert_eq!(
            cuando_evento(fecha(2026, 11, 28), hora(9, 0), hora(10, 30), hoy),
            "sáb 28 nov · 9:00–10:30 a. m."
        );
    }
}
