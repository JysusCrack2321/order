//! Parser ICS (RFC 5545) en Rust puro.
//!
//! Soporta líneas plegadas, `TZID`, UTC (`Z`), fechas de todo el día
//! (`VALUE=DATE`, vencen a las 23:59), `STATUS:CANCELLED` (ignorado) y `RRULE`
//! sencilla (`DAILY`, `WEEKLY`, `MONTHLY`, `YEARLY`), tomando solo la primera
//! ocurrencia futura si una regla es más compleja.

use chrono::{
    DateTime, Datelike, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta,
    TimeZone, Utc, Weekday,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventoIcal {
    pub uid: String,
    pub titulo: String,
    pub vence: NaiveDateTime,
    pub enlace: Option<String>,
    pub fecha_recurrencia: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarioIcs {
    pub nombre: Option<String>,
    pub eventos: Vec<EventoIcal>,
}

struct EventoCrudo {
    uid: String,
    summary: String,
    dtstart: Option<FechaIcs>,
    rrule: Option<String>,
    status: String,
    url: Option<String>,
}

#[derive(Debug, Clone)]
enum FechaIcs {
    TodoElDia(NaiveDate),
    Local(NaiveDateTime),
    Utc(NaiveDateTime),
    ConOffset(NaiveDateTime, i32),
}

impl FechaIcs {
    fn a_naive_local(&self) -> NaiveDateTime {
        match self {
            FechaIcs::TodoElDia(fecha) => {
                fecha.and_time(NaiveTime::from_hms_opt(23, 59, 0).expect("23:59"))
            }
            FechaIcs::Local(dt) => *dt,
            FechaIcs::Utc(dt) => DateTime::<Utc>::from_naive_utc_and_offset(*dt, Utc)
                .with_timezone(&Local)
                .naive_local(),
            FechaIcs::ConOffset(dt, segs) => {
                let Some(offset) = FixedOffset::east_opt(*segs) else {
                    return *dt;
                };
                match offset.from_local_datetime(dt).single() {
                    Some(fijo) => fijo.with_timezone(&Local).naive_local(),
                    None => *dt,
                }
            }
        }
    }

    fn es_todo_el_dia(&self) -> bool {
        matches!(self, FechaIcs::TodoElDia(_))
    }
}

/// Parsea el texto ICS y devuelve los eventos comprendidos entre `hoy - 1 día`
/// y `hoy + 120 días`.
pub fn parsear_ics(texto: &str, hoy: NaiveDate) -> CalendarioIcs {
    let desde = hoy - TimeDelta::days(1);
    let hasta = hoy + TimeDelta::days(120);
    let lineas = desplegar_lineas(texto);

    let mut nombre_cal: Option<String> = None;
    let mut eventos = Vec::new();
    let mut actual: Option<EventoCrudo> = None;
    let mut profundidad_ajena = 0usize; // p. ej. VALARM / VTIMEZONE dentro o fuera

    for linea in lineas {
        let trim = linea.trim();
        if trim.is_empty() {
            continue;
        }
        if trim.eq_ignore_ascii_case("BEGIN:VEVENT") {
            actual = Some(EventoCrudo {
                uid: String::new(),
                summary: String::new(),
                dtstart: None,
                rrule: None,
                status: String::new(),
                url: None,
            });
            profundidad_ajena = 0;
            continue;
        }
        if trim.eq_ignore_ascii_case("END:VEVENT") {
            if let Some(ev) = actual.take() {
                expandir_evento(ev, desde, hasta, &mut eventos);
            }
            profundidad_ajena = 0;
            continue;
        }
        if trim.len() >= 6 && trim[..6].eq_ignore_ascii_case("BEGIN:") {
            if actual.is_some() || !trim[6..].eq_ignore_ascii_case("VCALENDAR") {
                profundidad_ajena += 1;
            }
            continue;
        }
        if trim.len() >= 4 && trim[..4].eq_ignore_ascii_case("END:") {
            if profundidad_ajena > 0 {
                profundidad_ajena -= 1;
            }
            continue;
        }
        if profundidad_ajena > 0 {
            continue;
        }

        let Some((clave_y_params, valor)) = trim.split_once(':') else {
            continue;
        };
        let (nombre_prop, params) = match clave_y_params.split_once(';') {
            Some((n, p)) => (n.trim(), p),
            None => (clave_y_params.trim(), ""),
        };

        if let Some(ref mut ev) = actual {
            if nombre_prop.eq_ignore_ascii_case("UID") {
                ev.uid = valor.trim().to_string();
            } else if nombre_prop.eq_ignore_ascii_case("SUMMARY") {
                ev.summary = desescapar(valor.trim());
            } else if nombre_prop.eq_ignore_ascii_case("STATUS") {
                ev.status = valor.trim().to_string();
            } else if nombre_prop.eq_ignore_ascii_case("URL") {
                let u = valor.trim();
                if !u.is_empty() {
                    ev.url = Some(u.to_string());
                }
            } else if nombre_prop.eq_ignore_ascii_case("DTSTART") {
                ev.dtstart = parsear_fecha_ics(valor.trim(), params);
            } else if nombre_prop.eq_ignore_ascii_case("RRULE") {
                ev.rrule = Some(valor.trim().to_string());
            }
        } else if nombre_prop.eq_ignore_ascii_case("X-WR-CALNAME") && nombre_cal.is_none() {
            let n = desescapar(valor.trim());
            if !n.is_empty() {
                nombre_cal = Some(n);
            }
        }
    }

    CalendarioIcs {
        nombre: nombre_cal,
        eventos,
    }
}

/// Une líneas plegadas según RFC 5545 (salto de línea seguido de espacio o tabulador).
fn desplegar_lineas(texto: &str) -> Vec<String> {
    let mut salida: Vec<String> = Vec::new();
    for cruda in texto.split('\n') {
        let linea = cruda.strip_suffix('\r').unwrap_or(cruda);
        if let Some(resto) = linea.strip_prefix(' ').or_else(|| linea.strip_prefix('\t')) {
            if let Some(ultima) = salida.last_mut() {
                ultima.push_str(resto);
            }
        } else {
            salida.push(linea.to_string());
        }
    }
    salida
}

fn desescapar(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len());
    let mut chars = texto.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') | Some('N') => salida.push(' '),
                Some(',') => salida.push(','),
                Some(';') => salida.push(';'),
                Some('\\') => salida.push('\\'),
                Some(otro) => salida.push(otro),
                None => break,
            }
        } else {
            salida.push(ch);
        }
    }
    salida.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parsear_fecha_ics(valor: &str, params: &str) -> Option<FechaIcs> {
    let mut es_date = false;
    let mut tzid: Option<&str> = None;

    for param in params.split(';') {
        let param = param.trim();
        if let Some((k, v)) = param.split_once('=') {
            if k.eq_ignore_ascii_case("VALUE") && v.trim().eq_ignore_ascii_case("DATE") {
                es_date = true;
            } else if k.eq_ignore_ascii_case("TZID") {
                tzid = Some(v.trim().trim_matches('"'));
            }
        }
    }

    if es_date || (valor.len() == 8 && valor.bytes().all(|b| b.is_ascii_digit())) {
        let fecha = NaiveDate::parse_from_str(valor, "%Y%m%d").ok()?;
        return Some(FechaIcs::TodoElDia(fecha));
    }

    if let Some(sin_z) = valor.strip_suffix('Z').or_else(|| valor.strip_suffix('z')) {
        let dt = NaiveDateTime::parse_from_str(sin_z, "%Y%m%dT%H%M%S")
            .or_else(|_| NaiveDateTime::parse_from_str(sin_z, "%Y%m%dT%H%M"))
            .ok()?;
        return Some(FechaIcs::Utc(dt));
    }

    let dt = NaiveDateTime::parse_from_str(valor, "%Y%m%dT%H%M%S")
        .or_else(|_| NaiveDateTime::parse_from_str(valor, "%Y%m%dT%H%M"))
        .ok()?;

    if let Some(tz) = tzid {
        if let Some(segs) = offset_tzid_segundos(tz) {
            return Some(FechaIcs::ConOffset(dt, segs));
        }
    }

    Some(FechaIcs::Local(dt))
}

/// Offset estándar (en segundos) para zonas horarias habituales en ICS.
fn offset_tzid_segundos(tzid: &str) -> Option<i32> {
    let tz = tzid.trim();
    if tz.eq_ignore_ascii_case("UTC")
        || tz.eq_ignore_ascii_case("Etc/UTC")
        || tz.eq_ignore_ascii_case("GMT")
    {
        return Some(0);
    }
    let hora = match tz {
        "America/Bogota"
        | "America/Lima"
        | "America/Guayaquil"
        | "America/Panama"
        | "SA Pacific Standard Time"
        | "America/New_York"
        | "US/Eastern"
        | "Eastern Standard Time" => -5,
        "America/Mexico_City"
        | "America/Guatemala"
        | "Central Standard Time" => -6,
        "America/Caracas" | "America/La_Paz" => -4,
        "America/Santiago"
        | "America/Argentina/Buenos_Aires"
        | "America/Sao_Paulo" => -3,
        "America/Los_Angeles" | "US/Pacific" | "Pacific Standard Time" => -8,
        "Europe/London" => 0,
        "Europe/Madrid" | "Europe/Paris" | "Europe/Berlin" => 1,
        _ => return None,
    };
    Some(hora * 3600)
}

fn expandir_evento(
    ev: EventoCrudo,
    desde: NaiveDate,
    hasta: NaiveDate,
    salida: &mut Vec<EventoIcal>,
) {
    if ev.status.eq_ignore_ascii_case("CANCELLED") {
        return;
    }
    let titulo = ev.summary.trim().to_string();
    if titulo.is_empty() {
        return;
    }
    let Some(dtstart) = ev.dtstart else {
        return;
    };
    let inicio_local = dtstart.a_naive_local();
    let hora_local = if dtstart.es_todo_el_dia() {
        NaiveTime::from_hms_opt(23, 59, 0).expect("23:59")
    } else {
        inicio_local.time()
    };

    let uid = if ev.uid.trim().is_empty() {
        format!("{}-{}", titulo, inicio_local.date())
    } else {
        ev.uid.trim().to_string()
    };

    match ev.rrule {
        None => {
            let fecha = inicio_local.date();
            if fecha >= desde && fecha <= hasta {
                salida.push(EventoIcal {
                    uid,
                    titulo,
                    vence: inicio_local,
                    enlace: ev.url,
                    fecha_recurrencia: None,
                });
            }
        }
        Some(ref rrule) => {
            let fechas = ocurrencias_rrule(inicio_local.date(), rrule, desde, hasta);
            for fecha in fechas {
                salida.push(EventoIcal {
                    uid: uid.clone(),
                    titulo: titulo.clone(),
                    vence: fecha.and_time(hora_local),
                    enlace: ev.url.clone(),
                    fecha_recurrencia: Some(fecha),
                });
            }
        }
    }
}

fn ocurrencias_rrule(
    inicio: NaiveDate,
    rrule: &str,
    desde: NaiveDate,
    hasta: NaiveDate,
) -> Vec<NaiveDate> {
    let mut freq: Option<&str> = None;
    let mut interval: u32 = 1;
    let mut count: Option<usize> = None;
    let mut until: Option<NaiveDate> = None;
    let mut byday: Vec<Weekday> = Vec::new();
    let mut compleja = false;

    for parte in rrule.split(';') {
        let Some((k, v)) = parte.trim().split_once('=') else {
            continue;
        };
        if k.eq_ignore_ascii_case("FREQ") {
            freq = Some(v.trim());
        } else if k.eq_ignore_ascii_case("INTERVAL") {
            interval = v.trim().parse::<u32>().unwrap_or(1).max(1);
        } else if k.eq_ignore_ascii_case("COUNT") {
            count = v.trim().parse::<usize>().ok();
        } else if k.eq_ignore_ascii_case("UNTIL") {
            until = parsear_until(v.trim());
        } else if k.eq_ignore_ascii_case("BYDAY") {
            for tok in v.split(',') {
                if let Some(wd) = parsear_weekday(tok.trim()) {
                    byday.push(wd);
                } else {
                    compleja = true;
                }
            }
        } else if !k.eq_ignore_ascii_case("WKST") {
            compleja = true;
        }
    }

    let Some(freq) = freq else {
        return Vec::new();
    };
    let limite_fin = match until {
        Some(u) => hasta.min(u),
        None => hasta,
    };

    if compleja || ( !byday.is_empty() && !freq.eq_ignore_ascii_case("WEEKLY") ) {
        // Regla compleja: solo la primera ocurrencia futura dentro de la ventana.
        return primera_ocurrencia_simple(inicio, freq, interval, count, limite_fin, desde)
            .into_iter()
            .collect();
    }

    let mut fechas = Vec::new();
    let max_total = count.unwrap_or(200).min(400);

    if freq.eq_ignore_ascii_case("WEEKLY") && !byday.is_empty() {
        let mut semana_idx = 0u32;
        let mut generadas = 0usize;
        let lunes_base = inicio - TimeDelta::days(i64::from(inicio.weekday().num_days_from_monday()));
        while generadas < max_total {
            let inicio_sem = lunes_base + TimeDelta::weeks(i64::from(semana_idx * interval));
            if inicio_sem > limite_fin {
                break;
            }
            for offset in 0..7i64 {
                let d = inicio_sem + TimeDelta::days(offset);
                if d < inicio {
                    continue;
                }
                if !byday.contains(&d.weekday()) {
                    continue;
                }
                if d > limite_fin || generadas >= max_total {
                    return fechas;
                }
                generadas += 1;
                if d >= desde {
                    fechas.push(d);
                }
            }
            semana_idx += 1;
            if semana_idx > 200 {
                break;
            }
        }
        return fechas;
    }

    let mut actual = inicio;
    let mut n = 0usize;
    while actual <= limite_fin && n < max_total {
        n += 1;
        if actual >= desde {
            fechas.push(actual);
        }
        let Some(siguiente) = avanzar_fecha(actual, freq, interval) else {
            break;
        };
        if siguiente <= actual {
            break;
        }
        actual = siguiente;
    }
    fechas
}

fn primera_ocurrencia_simple(
    inicio: NaiveDate,
    freq: &str,
    interval: u32,
    count: Option<usize>,
    limite_fin: NaiveDate,
    desde: NaiveDate,
) -> Option<NaiveDate> {
    let max_total = count.unwrap_or(200).min(400);
    let mut actual = inicio;
    for _ in 0..max_total {
        if actual > limite_fin {
            return None;
        }
        if actual >= desde {
            return Some(actual);
        }
        actual = avanzar_fecha(actual, freq, interval)?;
    }
    None
}

fn avanzar_fecha(fecha: NaiveDate, freq: &str, interval: u32) -> Option<NaiveDate> {
    if freq.eq_ignore_ascii_case("DAILY") {
        Some(fecha + TimeDelta::days(i64::from(interval)))
    } else if freq.eq_ignore_ascii_case("WEEKLY") {
        Some(fecha + TimeDelta::weeks(i64::from(interval)))
    } else if freq.eq_ignore_ascii_case("MONTHLY") {
        let total_meses = fecha.month0() as i32 + interval as i32;
        let anio = fecha.year() + total_meses.div_euclid(12);
        let mes = (total_meses.rem_euclid(12) + 1) as u32;
        let dia = fecha.day().min(dias_mes(anio, mes));
        NaiveDate::from_ymd_opt(anio, mes, dia)
    } else if freq.eq_ignore_ascii_case("YEARLY") {
        let anio = fecha.year() + interval as i32;
        let dia = fecha.day().min(dias_mes(anio, fecha.month()));
        NaiveDate::from_ymd_opt(anio, fecha.month(), dia)
    } else {
        None
    }
}

fn dias_mes(anio: i32, mes: u32) -> u32 {
    let sig = if mes == 12 {
        NaiveDate::from_ymd_opt(anio + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(anio, mes + 1, 1)
    };
    sig.and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(28)
}

fn parsear_until(texto: &str) -> Option<NaiveDate> {
    if texto.len() >= 8 {
        NaiveDate::parse_from_str(&texto[..8], "%Y%m%d").ok()
    } else {
        None
    }
}

fn parsear_weekday(tok: &str) -> Option<Weekday> {
    match tok {
        "MO" => Some(Weekday::Mon),
        "TU" => Some(Weekday::Tue),
        "WE" => Some(Weekday::Wed),
        "TH" => Some(Weekday::Thu),
        "FR" => Some(Weekday::Fri),
        "SA" => Some(Weekday::Sat),
        "SU" => Some(Weekday::Sun),
        _ => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn parsea_utc_tzid_todo_el_dia_plegado_cancelado_y_rrule() {
        let ics = "\
BEGIN:VCALENDAR\r
VERSION:2.0\r
X-WR-CALNAME:Entregas Control\r
BEGIN:VEVENT\r
UID:ev-todo-dia\r
SUMMARY:Entrega taller\r
  final\r
DTSTART;VALUE=DATE:20261125\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:ev-utc\r
SUMMARY:Parcial\\, Parte 1\r
DTSTART:20261128T150000Z\r
URL:https://ejemplo.edu/evento\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:ev-tzid\r
SUMMARY:Laboratorio\r
DTSTART;TZID=America/Bogota:20261126T140000\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:ev-cancelado\r
SUMMARY:Cancelado\r
DTSTART;VALUE=DATE:20261127\r
STATUS:CANCELLED\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:ev-viejo\r
SUMMARY:Muy viejo\r
DTSTART;VALUE=DATE:20261001\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:ev-lejos\r
SUMMARY:Muy lejano\r
DTSTART;VALUE=DATE:20270801\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:ev-semanal\r
SUMMARY:Monitoría\r
DTSTART;VALUE=DATE:20261123\r
RRULE:FREQ=WEEKLY;COUNT=3\r
END:VEVENT\r
END:VCALENDAR\r
";

        let hoy = NaiveDate::from_ymd_opt(2026, 11, 22).unwrap();
        let cal = parsear_ics(ics, hoy);
        assert_eq!(cal.nombre.as_deref(), Some("Entregas Control"));

        // Cancelado, viejo y lejano fuera.
        assert!(!cal.eventos.iter().any(|e| e.uid == "ev-cancelado"));
        assert!(!cal.eventos.iter().any(|e| e.uid == "ev-viejo"));
        assert!(!cal.eventos.iter().any(|e| e.uid == "ev-lejos"));

        // Línea plegada unida y todo el día vence a las 23:59.
        let todo_dia = cal.eventos.iter().find(|e| e.uid == "ev-todo-dia").unwrap();
        assert_eq!(todo_dia.titulo, "Entrega taller final");
        assert_eq!(
            todo_dia.vence,
            NaiveDate::from_ymd_opt(2026, 11, 25)
                .unwrap()
                .and_hms_opt(23, 59, 0)
                .unwrap()
        );

        // Desescape de coma y enlace.
        let utc_ev = cal.eventos.iter().find(|e| e.uid == "ev-utc").unwrap();
        assert_eq!(utc_ev.titulo, "Parcial, Parte 1");
        assert_eq!(utc_ev.enlace.as_deref(), Some("https://ejemplo.edu/evento"));

        // TZID presente y parseado.
        assert!(cal.eventos.iter().any(|e| e.uid == "ev-tzid"));

        // Recurrencia semanal de 3 ocurrencias con fecha_recurrencia distinta.
        let rec: Vec<_> = cal
            .eventos
            .iter()
            .filter(|e| e.uid == "ev-semanal")
            .collect();
        assert_eq!(rec.len(), 3);
        assert_eq!(
            rec[0].fecha_recurrencia,
            Some(NaiveDate::from_ymd_opt(2026, 11, 23).unwrap())
        );
        assert_eq!(
            rec[2].fecha_recurrencia,
            Some(NaiveDate::from_ymd_opt(2026, 12, 7).unwrap())
        );
    }
}
