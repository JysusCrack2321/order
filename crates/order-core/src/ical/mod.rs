//! Calendarios iCal externos (Google Calendar u otros `.ics` / `webcal://`).

pub mod config;
pub mod parser;
pub mod sincronizar;

use crate::almacen::Almacen;
use crate::moodle::ResumenSync;
use chrono::{Local, NaiveDate};
use config::{CalendarioConfig, CalendarioVista};
use std::path::Path;
use std::time::Duration;

const ESPERA_ICAL: Duration = Duration::from_secs(20);

/// Detecta si un texto del portapapeles parece una dirección iCal.
pub fn parece_url_ical(texto: &str) -> bool {
    let limpio = texto.trim();
    if limpio.is_empty() || limpio.chars().any(char::is_whitespace) {
        return false;
    }
    let lower = limpio.to_lowercase();
    let esquema_ok = lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("webcal://");
    if !esquema_ok {
        return false;
    }
    lower.starts_with("webcal://")
        || lower.contains(".ics")
        || lower.contains("calendar.google.com/calendar/ical/")
}

/// Convierte `webcal://` en `https://`.
pub fn normalizar_url_ical(url: &str) -> String {
    let limpio = url.trim();
    if limpio.len() >= 9 && limpio[..9].eq_ignore_ascii_case("webcal://") {
        format!("https://{}", &limpio[9..])
    } else {
        limpio.to_string()
    }
}

/// Hash FNV-1a corto (12 hex) de la URL normalizada, estable entre dispositivos.
pub fn hash_corto_url(url: &str) -> String {
    let normal = normalizar_url_ical(url);
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in normal.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:012x}", hash & 0xffff_ffff_ffff)
}

fn descargar_ics(url: &str) -> Result<String, String> {
    let url_http = normalizar_url_ical(url);
    let agente = ureq::Agent::config_builder()
        .timeout_global(Some(ESPERA_ICAL))
        .http_status_as_error(false)
        .build()
        .new_agent();

    let mut resp = agente
        .get(&url_http)
        .header("accept", "text/calendar, text/plain, */*")
        .call()
        .map_err(|e| format!("No se pudo descargar el calendario: {e}"))?;

    let estado = resp.status().as_u16();
    let cuerpo = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("No se pudo leer el calendario: {e}"))?;

    if !(200..=299).contains(&estado) {
        return Err(format!("El calendario respondió HTTP {estado}."));
    }
    if !cuerpo.contains("BEGIN:VCALENDAR") {
        return Err("La dirección no devolvió un archivo iCal (.ics) válido.".to_string());
    }
    Ok(cuerpo)
}

/// Descarga y agrega un calendario a `calendarios.toml`.
pub fn agregar_calendario(
    ruta_cal: &Path,
    url: &str,
    nombre: Option<&str>,
    materia: Option<&str>,
) -> Result<CalendarioVista, String> {
    let url_normal = normalizar_url_ical(url);
    if !(url_normal.starts_with("https://") || url_normal.starts_with("http://"))
        || url_normal.chars().any(char::is_whitespace)
    {
        return Err("Escribe una dirección iCal válida (https:// o webcal://).".to_string());
    }

    let id = hash_corto_url(&url_normal);
    let mut cfg = config::cargar_de(ruta_cal);
    if cfg.calendarios.iter().any(|c| c.id == id) {
        return Err("Ese calendario ya está agregado.".to_string());
    }

    let texto = descargar_ics(&url_normal)?;
    let ics = parser::parsear_ics(&texto, Local::now().date_naive());

    let nombre_final = nombre
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or(ics.nombre)
        .unwrap_or_else(|| "Calendario".to_string());

    let materia_final = materia
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let nuevo = CalendarioConfig {
        id,
        nombre: nombre_final,
        url: url_normal,
        materia: materia_final,
        ultima_actualizacion: None,
    };
    let vista = nuevo.a_vista(Local::now().naive_local());
    cfg.calendarios.push(nuevo);
    config::guardar_en(ruta_cal, &cfg)?;
    Ok(vista)
}

pub fn quitar_calendario(ruta_cal: &Path, id: &str) -> Result<(), String> {
    let mut cfg = config::cargar_de(ruta_cal);
    let antes = cfg.calendarios.len();
    cfg.calendarios.retain(|c| c.id != id);
    if cfg.calendarios.len() != antes {
        config::guardar_en(ruta_cal, &cfg)?;
    }
    Ok(())
}

/// Sincroniza todos los calendarios configurados contra `order.toml`.
/// Escribe en disco solo si hubo cambios.
pub fn sincronizar_calendarios(
    ruta_datos: &Path,
    ruta_cal: &Path,
    hoy: NaiveDate,
) -> Result<ResumenSync, String> {
    let mut cfg = config::cargar_de(ruta_cal);
    if cfg.calendarios.is_empty() {
        return Ok(ResumenSync::default());
    }

    let almacen = Almacen {
        ruta: ruta_datos.to_path_buf(),
    };
    let mut datos = almacen.cargar().map_err(|e| e.to_string())?;
    let mut total = ResumenSync::default();
    let mut errores = Vec::new();
    let mut algun_exito = false;

    for cal in &mut cfg.calendarios {
        match descargar_ics(&cal.url) {
            Ok(texto) => {
                algun_exito = true;
                let ics = parser::parsear_ics(&texto, hoy);
                let cambios = sincronizar::aplicar(&datos, cal, &ics.eventos);
                if cambios.hay_cambios() {
                    datos = almacen
                        .modificar(|doc| cambios.aplicar_en_doc(doc))
                        .map_err(|e| e.to_string())?;
                    total.creadas += cambios.num_creadas as u32;
                    total.actualizadas += cambios.num_actualizadas as u32;
                }
                cal.anotar_actualizacion(Local::now().naive_local());
            }
            Err(err) => errores.push(err),
        }
    }

    if algun_exito {
        config::guardar_en(ruta_cal, &cfg)?;
        Ok(total)
    } else {
        Err(errores
            .into_iter()
            .next()
            .unwrap_or_else(|| "No se pudieron actualizar los calendarios.".to_string()))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::thread;

    fn servidor_ics(cuerpo: &str) -> (String, Arc<AtomicBool>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let parar = Arc::new(AtomicBool::new(false));
        let parar_h = Arc::clone(&parar);
        let cuerpo = cuerpo.to_string();

        let handle = thread::spawn(move || {
            while !parar_h.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 1024];
                        let _ = stream.read(&mut buf);
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/calendar\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
                            cuerpo.len()
                        );
                        let _ = stream.write_all(resp.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });

        (format!("http://{addr}/basic.ics"), parar, handle)
    }

    #[test]
    fn detecta_urls_ical_y_sincroniza_sin_duplicar_ni_tocar_hechas() {
        assert!(parece_url_ical("https://calendar.google.com/calendar/ical/abc/public/basic.ics"));
        assert!(parece_url_ical("webcal://ejemplo.edu/calendario"));
        assert!(parece_url_ical("http://localhost:8080/agenda.ics"));
        assert!(!parece_url_ical("https://ejemplo.edu"));
        assert!(!parece_url_ical("hola mundo.ics"));
        assert_eq!(
            normalizar_url_ical("webcal://ejemplo.edu/cal.ics"),
            "https://ejemplo.edu/cal.ics"
        );

        let ics = "\
BEGIN:VCALENDAR\r
X-WR-CALNAME:Control\r
BEGIN:VEVENT\r
UID:uid-1\r
SUMMARY:Entrega proyecto\r
DTSTART;VALUE=DATE:20261125\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:uid-2\r
SUMMARY:Cancelado\r
DTSTART;VALUE=DATE:20261126\r
STATUS:CANCELLED\r
END:VEVENT\r
END:VCALENDAR\r
";
        let (url, parar, handle) = servidor_ics(ics);
        let dir = tempfile::tempdir().unwrap();
        let ruta_datos = dir.path().join("order.toml");
        let ruta_cal = dir.path().join("calendarios.toml");
        std::fs::write(
            &ruta_datos,
            "[[clase]]\nid = \"c-1\"\nmateria = \"Control\"\ndias = [\"lun\"]\ninicio = \"07:00\"\nfin = \"09:00\"\n",
        )
        .unwrap();

        let vista = agregar_calendario(&ruta_cal, &url, None, None).unwrap();
        assert_eq!(vista.nombre, "Control");

        let hoy = NaiveDate::from_ymd_opt(2026, 11, 22).unwrap();
        let r1 = sincronizar_calendarios(&ruta_datos, &ruta_cal, hoy).unwrap();
        assert_eq!(r1.creadas, 1);

        let datos = Almacen {
            ruta: ruta_datos.clone(),
        }
        .cargar()
        .unwrap();
        assert_eq!(datos.tareas.len(), 1);
        assert_eq!(datos.tareas[0].materia.as_deref(), Some("Control"));
        assert!(!datos.tareas[0].hecha);

        // Segunda sincronización sin cambios no reescribe order.toml.
        let antes = std::fs::read_to_string(&ruta_datos).unwrap();
        let r2 = sincronizar_calendarios(&ruta_datos, &ruta_cal, hoy).unwrap();
        assert_eq!(r2, ResumenSync::default());
        assert_eq!(std::fs::read_to_string(&ruta_datos).unwrap(), antes);

        // Quitar el calendario no borra las tareas ya creadas.
        quitar_calendario(&ruta_cal, &vista.id).unwrap();
        assert!(config::listar_de(&ruta_cal).is_empty());
        assert_eq!(Almacen { ruta: ruta_datos }.cargar().unwrap().tareas.len(), 1);

        parar.store(true, Ordering::Relaxed);
        let _ = handle.join();
    }
}
