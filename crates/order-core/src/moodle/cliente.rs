//! Cliente HTTP bloqueante para la API móvil/REST de Moodle.

use crate::moodle::config::MoodleConfig;
use crate::moodle::{EventoMoodle, normalizar_sitio, url_sitio};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeDelta};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

const ESPERA_MOODLE: Duration = Duration::from_secs(25);
const LIMITE_EVENTOS_PAGINA: usize = 50;
const MAX_PAGINAS: usize = 20;

/// Errores traducidos a mensajes claros para el usuario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorMoodle {
    NoConfigurado,
    CredencialesInvalidas,
    TokenInvalido,
    NoEsMoodle,
    SinServicioMovil,
    SinRed(String),
    Servidor(String),
}

impl fmt::Display for ErrorMoodle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorMoodle::NoConfigurado => write!(f, "Moodle no está conectado."),
            ErrorMoodle::CredencialesInvalidas => {
                write!(f, "Usuario o contraseña incorrectos.")
            }
            ErrorMoodle::TokenInvalido => {
                write!(f, "Vuelve a iniciar sesión en Moodle.")
            }
            ErrorMoodle::NoEsMoodle => {
                write!(f, "Ese sitio no parece ser un Moodle compatible.")
            }
            ErrorMoodle::SinServicioMovil => {
                write!(f, "Este Moodle no permite iniciar sesión desde otras apps.")
            }
            ErrorMoodle::SinRed(detalle) => {
                write!(f, "Sin conexión con Moodle: {detalle}")
            }
            ErrorMoodle::Servidor(detalle) => write!(f, "{detalle}"),
        }
    }
}

impl std::error::Error for ErrorMoodle {}

fn agente_http() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(ESPERA_MOODLE))
        .http_status_as_error(false)
        .build()
        .new_agent()
}

/// Comprueba `tool_mobile_get_public_config` antes de pedir token.
pub fn verificar_config_publica(sitio: &str) -> Result<(), ErrorMoodle> {
    let base = url_sitio(sitio);
    let url = format!("{base}/lib/ajax/service-nologin.php?info=tool_mobile_get_public_config");
    let agente = agente_http();
    let cuerpo = json!([{
        "index": 0,
        "methodname": "tool_mobile_get_public_config",
        "args": {}
    }]);

    let mut resp = agente
        .post(&url)
        .header("accept", "application/json")
        .send_json(&cuerpo)
        .map_err(|e| ErrorMoodle::SinRed(e.to_string()))?;

    let estado = resp.status().as_u16();
    let texto = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| ErrorMoodle::SinRed(e.to_string()))?;

    if estado >= 400 {
        return Err(ErrorMoodle::NoEsMoodle);
    }

    let valor: Value = serde_json::from_str(&texto).map_err(|_| ErrorMoodle::NoEsMoodle)?;
    let item = valor
        .as_array()
        .and_then(|arr| arr.first())
        .ok_or(ErrorMoodle::NoEsMoodle)?;

    if item.get("error").and_then(Value::as_bool) == Some(true) {
        return Err(ErrorMoodle::SinServicioMovil);
    }

    let data = item.get("data").ok_or(ErrorMoodle::NoEsMoodle)?;
    let enable_mobile = match data.get("enablemobilewebservice") {
        Some(Value::Number(n)) => n.as_i64() == Some(1),
        Some(Value::Bool(b)) => *b,
        _ => return Err(ErrorMoodle::NoEsMoodle),
    };
    let typeoflogin = data.get("typeoflogin").and_then(Value::as_i64);

    if !enable_mobile || typeoflogin != Some(1) {
        return Err(ErrorMoodle::SinServicioMovil);
    }

    Ok(())
}

/// Inicia sesión en Moodle y devuelve la configuración lista para guardar (sin contraseña).
pub fn iniciar_sesion(sitio: &str, usuario: &str, contrasena: &str) -> Result<MoodleConfig, ErrorMoodle> {
    let sitio_limpio = sitio.trim();
    if sitio_limpio.is_empty() {
        return Err(ErrorMoodle::Servidor("Escribe la dirección de tu Moodle.".into()));
    }
    if usuario.trim().is_empty() || contrasena.is_empty() {
        return Err(ErrorMoodle::Servidor("Escribe usuario y contraseña.".into()));
    }

    verificar_config_publica(sitio_limpio)?;

    let base = url_sitio(sitio_limpio);
    let url = format!("{base}/login/token.php");
    let agente = agente_http();
    let mut resp = agente
        .post(&url)
        .header("accept", "application/json")
        .send_form([
            ("username", usuario.trim()),
            ("password", contrasena),
            ("service", "moodle_mobile_app"),
        ])
        .map_err(|e| ErrorMoodle::SinRed(e.to_string()))?;

    let estado = resp.status().as_u16();
    let texto = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| ErrorMoodle::SinRed(e.to_string()))?;

    if estado >= 400 {
        return Err(ErrorMoodle::NoEsMoodle);
    }

    let valor: Value = serde_json::from_str(&texto).map_err(|_| ErrorMoodle::NoEsMoodle)?;
    if let Some(errorcode) = valor.get("errorcode").and_then(Value::as_str) {
        return Err(match errorcode {
            "invalidlogin" => ErrorMoodle::CredencialesInvalidas,
            "servicenotavailable" | "enablewsdescription" => ErrorMoodle::SinServicioMovil,
            _ => {
                let msg = valor
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("No se pudo iniciar sesión en Moodle.");
                ErrorMoodle::Servidor(msg.to_string())
            }
        });
    }

    let token = valor
        .get("token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or(ErrorMoodle::NoEsMoodle)?
        .to_string();

    let (nombre_usuario, userid) = obtener_info_sitio(&agente, &base, &token)?;

    Ok(MoodleConfig {
        sitio: preservar_sitio_para_base(sitio_limpio),
        token,
        usuario: nombre_usuario,
        userid,
        ultima_sincronizacion: None,
        token_invalido: false,
    })
}

fn preservar_sitio_para_base(sitio: &str) -> String {
    let s = sitio.trim().trim_end_matches('/');
    if s.len() >= 7 && s[..7].eq_ignore_ascii_case("http://") {
        // Conserva http:// explícito (p. ej. servidores de prueba locales).
        s.to_string()
    } else {
        normalizar_sitio(s)
    }
}

fn llamar_ws(
    agente: &ureq::Agent,
    base: &str,
    token: &str,
    funcion: &str,
    params: &[(&str, &str)],
) -> Result<Value, ErrorMoodle> {
    let url = format!("{base}/webservice/rest/server.php");
    let mut form: Vec<(&str, &str)> = Vec::with_capacity(params.len() + 3);
    form.push(("wstoken", token));
    form.push(("wsfunction", funcion));
    form.push(("moodlewsrestformat", "json"));
    form.extend_from_slice(params);

    let mut resp = agente
        .post(&url)
        .header("accept", "application/json")
        .send_form(form)
        .map_err(|e| ErrorMoodle::SinRed(e.to_string()))?;

    let estado = resp.status().as_u16();
    let texto = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| ErrorMoodle::SinRed(e.to_string()))?;

    if estado >= 400 {
        return Err(ErrorMoodle::Servidor(format!(
            "Moodle respondió HTTP {estado}."
        )));
    }

    let valor: Value = serde_json::from_str(&texto).map_err(|_| ErrorMoodle::NoEsMoodle)?;
    if let Some(errorcode) = valor.get("errorcode").and_then(Value::as_str) {
        if matches!(errorcode, "invalidtoken" | "expiredtoken") {
            return Err(ErrorMoodle::TokenInvalido);
        }
        let msg = valor
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(errorcode);
        return Err(ErrorMoodle::Servidor(msg.to_string()));
    }
    if valor.get("exception").is_some() {
        let msg = valor
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Error al consultar Moodle.");
        return Err(ErrorMoodle::Servidor(msg.to_string()));
    }

    Ok(valor)
}

fn obtener_info_sitio(
    agente: &ureq::Agent,
    base: &str,
    token: &str,
) -> Result<(String, u64), ErrorMoodle> {
    let valor = llamar_ws(agente, base, token, "core_webservice_get_site_info", &[])?;
    let nombre = valor
        .get("fullname")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| valor.get("username").and_then(Value::as_str))
        .unwrap_or("Moodle")
        .trim()
        .to_string();
    let userid = valor.get("userid").and_then(Value::as_u64).unwrap_or(0);
    Ok((nombre, userid))
}

/// Descarga las actividades pendientes/próximas (desde `hoy - 14 días`) y su estado de entrega.
pub fn obtener_eventos(config: &MoodleConfig, hoy: NaiveDate) -> Result<Vec<EventoMoodle>, ErrorMoodle> {
    if !config.conectado() {
        return Err(ErrorMoodle::NoConfigurado);
    }

    let base = url_sitio(&config.sitio);
    let agente = agente_http();

    let userid = if config.userid > 0 {
        config.userid
    } else {
        let (_, uid) = obtener_info_sitio(&agente, &base, &config.token)?;
        uid
    };

    let inicio = (hoy - TimeDelta::days(14))
        .and_hms_opt(0, 0, 0)
        .unwrap_or_default()
        .and_utc()
        .timestamp();
    let timesortfrom = inicio.to_string();
    let limitnum = LIMITE_EVENTOS_PAGINA.to_string();

    let mut crudos = Vec::new();
    let mut aftereventid: Option<u64> = None;

    for _ in 0..MAX_PAGINAS {
        let after_str = aftereventid.map(|id| id.to_string());
        let mut params = vec![
            ("timesortfrom", timesortfrom.as_str()),
            ("limitnum", limitnum.as_str()),
        ];
        if let Some(ref after) = after_str {
            params.push(("aftereventid", after.as_str()));
        }

        let valor = llamar_ws(
            &agente,
            &base,
            &config.token,
            "core_calendar_get_action_events_by_timesort",
            &params,
        )?;

        let Some(lista) = valor.get("events").and_then(Value::as_array) else {
            break;
        };
        let cantidad = lista.len();
        let ultimo_id = lista.last().and_then(|e| e.get("id")).and_then(Value::as_u64);
        crudos.extend(lista.iter().cloned());

        if cantidad < LIMITE_EVENTOS_PAGINA {
            break;
        }
        match ultimo_id {
            Some(id) if Some(id) != aftereventid => aftereventid = Some(id),
            _ => break,
        }
    }

    let mut cache_complecion: HashMap<u64, Result<Vec<EstadoComplecionItem>, ErrorMoodle>> =
        HashMap::new();
    let mut eventos = Vec::with_capacity(crudos.len());

    for ev in crudos {
        let Some(modulename) = ev
            .get("modulename")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(instance) = ev.get("instance").and_then(Value::as_u64) else {
            continue;
        };
        let Some(titulo) = ev
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(timesort) = ev.get("timesort").and_then(Value::as_i64) else {
            continue;
        };
        let Some(vence) = timestamp_a_local(timesort) else {
            continue;
        };

        let course = ev.get("course");
        let course_id = course.and_then(|c| c.get("id")).and_then(Value::as_u64).unwrap_or(0);
        let curso = course
            .and_then(|c| c.get("fullname").and_then(Value::as_str))
            .filter(|s| !s.trim().is_empty())
            .or_else(|| course.and_then(|c| c.get("shortname").and_then(Value::as_str)))
            .unwrap_or_default()
            .to_string();

        let enlace = ev
            .get("url")
            .and_then(Value::as_str)
            .or_else(|| ev.get("viewurl").and_then(Value::as_str))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let cmid = ev.get("cmid").and_then(Value::as_u64);

        let completada = if modulename.eq_ignore_ascii_case("assign") {
            match entrega_assign(&agente, &base, &config.token, instance) {
                Ok(v) => v,
                Err(ErrorMoodle::TokenInvalido) => return Err(ErrorMoodle::TokenInvalido),
                Err(_) => false,
            }
        } else if course_id > 0 && userid > 0 {
            let entry = cache_complecion.entry(course_id).or_insert_with(|| {
                complecion_curso(&agente, &base, &config.token, course_id, userid)
            });
            match entry {
                Ok(items) => actividad_completa(items, cmid, modulename, instance),
                Err(ErrorMoodle::TokenInvalido) => return Err(ErrorMoodle::TokenInvalido),
                Err(_) => false,
            }
        } else {
            false
        };

        eventos.push(EventoMoodle {
            sitio: config.sitio.clone(),
            modulename: modulename.to_string(),
            instance,
            titulo: titulo.to_string(),
            curso,
            vence,
            enlace,
            completada,
        });
    }

    Ok(eventos)
}

fn timestamp_a_local(ts: i64) -> Option<NaiveDateTime> {
    DateTime::from_timestamp(ts, 0).map(|dt| dt.with_timezone(&Local).naive_local())
}

fn entrega_assign(
    agente: &ureq::Agent,
    base: &str,
    token: &str,
    assignid: u64,
) -> Result<bool, ErrorMoodle> {
    let id_str = assignid.to_string();
    let valor = llamar_ws(
        agente,
        base,
        token,
        "mod_assign_get_submission_status",
        &[("assignid", id_str.as_str())],
    )?;
    let estado_propio = valor
        .pointer("/lastattempt/submission/status")
        .and_then(Value::as_str);
    let estado_grupo = valor
        .pointer("/lastattempt/teamsubmission/status")
        .and_then(Value::as_str);
    Ok(estado_propio == Some("submitted") || estado_grupo == Some("submitted"))
}

struct EstadoComplecionItem {
    cmid: u64,
    modname: String,
    instance: u64,
    completa: bool,
}

fn complecion_curso(
    agente: &ureq::Agent,
    base: &str,
    token: &str,
    courseid: u64,
    userid: u64,
) -> Result<Vec<EstadoComplecionItem>, ErrorMoodle> {
    let cid = courseid.to_string();
    let uid = userid.to_string();
    let valor = llamar_ws(
        agente,
        base,
        token,
        "core_completion_get_activities_completion_status",
        &[("courseid", cid.as_str()), ("userid", uid.as_str())],
    )?;

    let mut items = Vec::new();
    if let Some(statuses) = valor.get("statuses").and_then(Value::as_array) {
        for s in statuses {
            let state = s.get("state").and_then(Value::as_i64).unwrap_or(0);
            // En Moodle: 1 = COMPLETION_COMPLETE, 2 = COMPLETION_COMPLETE_PASS.
            let completa = state == 1 || state == 2;
            items.push(EstadoComplecionItem {
                cmid: s.get("cmid").and_then(Value::as_u64).unwrap_or(0),
                modname: s
                    .get("modname")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                instance: s.get("instance").and_then(Value::as_u64).unwrap_or(0),
                completa,
            });
        }
    }
    Ok(items)
}

fn actividad_completa(
    items: &[EstadoComplecionItem],
    cmid: Option<u64>,
    modulename: &str,
    instance: u64,
) -> bool {
    items.iter().any(|item| {
        if !item.completa {
            return false;
        }
        if let Some(c) = cmid {
            if item.cmid == c && c > 0 {
                return true;
            }
        }
        item.instance == instance && item.modname.eq_ignore_ascii_case(modulename)
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::almacen::Almacen;
    use crate::moodle::{ResumenSync, config, conectar, sincronizar_ahora};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use std::thread;

    struct RespuestaMock {
        estado: u16,
        cuerpo: String,
    }

    fn servidor_moodle<F>(handler: F) -> (String, Arc<AtomicBool>, thread::JoinHandle<()>)
    where
        F: Fn(&str, &str) -> RespuestaMock + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let addr = listener.local_addr().expect("addr");
        let parar = Arc::new(AtomicBool::new(false));
        let parar_hilo = Arc::clone(&parar);
        let handler = Arc::new(handler);

        let handle = thread::spawn(move || {
            while !parar_hilo.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let mut datos = Vec::new();
                        let mut buf = [0u8; 2048];
                        loop {
                            match stream.read(&mut buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    datos.extend_from_slice(&buf[..n]);
                                    if let Some(fin_cab) = encontrar_fin_cabeceras(&datos) {
                                        let cab = String::from_utf8_lossy(&datos[..fin_cab]);
                                        let len = content_length(&cab);
                                        if datos.len() >= fin_cab + 4 + len {
                                            break;
                                        }
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        let req = String::from_utf8_lossy(&datos);
                        let (cab, cuerpo) = req.split_once("\r\n\r\n").unwrap_or((&req, ""));
                        let ruta = cab
                            .lines()
                            .next()
                            .and_then(|l| l.split_whitespace().nth(1))
                            .unwrap_or("/");
                        let r = handler(ruta, cuerpo);
                        let resp = format!(
                            "HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            r.estado,
                            r.cuerpo.len(),
                            r.cuerpo
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

        (format!("http://{addr}"), parar, handle)
    }

    fn encontrar_fin_cabeceras(datos: &[u8]) -> Option<usize> {
        datos.windows(4).position(|w| w == b"\r\n\r\n")
    }

    fn content_length(cabeceras: &str) -> usize {
        cabeceras
            .lines()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                if k.trim().eq_ignore_ascii_case("content-length") {
                    v.trim().parse().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0)
    }

    #[test]
    fn rechaza_sso_no_moodle_y_credenciales_malas() {
        // 1. SSO (typeoflogin != 1) -> mensaje claro.
        let (url_sso, parar_sso, h_sso) = servidor_moodle(|_, _| RespuestaMock {
            estado: 200,
            cuerpo: r#"[{"error":false,"data":{"typeoflogin":2,"enablemobilewebservice":1}}]"#
                .into(),
        });
        let err = iniciar_sesion(&url_sso, "ana", "123").unwrap_err();
        parar_sso.store(true, Ordering::Relaxed);
        let _ = h_sso.join();
        assert_eq!(
            err.to_string(),
            "Este Moodle no permite iniciar sesión desde otras apps."
        );

        // 2. Sitio que no es Moodle (404 HTML).
        let (url_404, parar_404, h_404) = servidor_moodle(|_, _| RespuestaMock {
            estado: 404,
            cuerpo: "<html>Not found</html>".into(),
        });
        let err = iniciar_sesion(&url_404, "ana", "123").unwrap_err();
        parar_404.store(true, Ordering::Relaxed);
        let _ = h_404.join();
        assert_eq!(
            err.to_string(),
            "Ese sitio no parece ser un Moodle compatible."
        );

        // 3. Contraseña errada -> invalidlogin.
        let (url_bad, parar_bad, h_bad) = servidor_moodle(|ruta, _| {
            if ruta.contains("service-nologin.php") {
                RespuestaMock {
                    estado: 200,
                    cuerpo: r#"[{"error":false,"data":{"typeoflogin":1,"enablemobilewebservice":1}}]"#
                        .into(),
                }
            } else {
                RespuestaMock {
                    estado: 200,
                    cuerpo: r#"{"error":"Invalid login","errorcode":"invalidlogin"}"#.into(),
                }
            }
        });
        let err = iniciar_sesion(&url_bad, "ana", "mala").unwrap_err();
        parar_bad.store(true, Ordering::Relaxed);
        let _ = h_bad.join();
        assert_eq!(err.to_string(), "Usuario o contraseña incorrectos.");

        // 4. Sin red (puerto cerrado).
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        let err = iniciar_sesion(&format!("http://127.0.0.1:{p}"), "ana", "123").unwrap_err();
        assert!(matches!(err, ErrorMoodle::SinRed(_)), "{err:?}");
    }

    #[test]
    fn login_paginacion_estado_y_token_invalido() {
        let llamadas_ws = Arc::new(Mutex::new(Vec::<String>::new()));
        let token_vencido = Arc::new(AtomicBool::new(false));
        let llamadas_c = Arc::clone(&llamadas_ws);
        let token_c = Arc::clone(&token_vencido);

        let (url, parar, handle) = servidor_moodle(move |ruta, cuerpo| {
            if ruta.contains("service-nologin.php") {
                return RespuestaMock {
                    estado: 200,
                    cuerpo: r#"[{"error":false,"data":{"typeoflogin":1,"enablemobilewebservice":1}}]"#
                        .into(),
                };
            }
            if ruta.contains("login/token.php") {
                return RespuestaMock {
                    estado: 200,
                    cuerpo: r#"{"token":"tok-abc-123"}"#.into(),
                };
            }
            if token_c.load(Ordering::Relaxed) {
                return RespuestaMock {
                    estado: 200,
                    cuerpo: r#"{"exception":"moodle_exception","errorcode":"invalidtoken","message":"Invalid token"}"#
                        .into(),
                };
            }

            if let Ok(mut guard) = llamadas_c.lock() {
                guard.push(cuerpo.to_string());
            }

            if cuerpo.contains("core_webservice_get_site_info") {
                return RespuestaMock {
                    estado: 200,
                    cuerpo: r#"{"fullname":"Ana Estudiante","userid":99}"#.into(),
                };
            }
            if cuerpo.contains("core_calendar_get_action_events_by_timesort") {
                // Primera página de 50 eventos (49 assign + 1 quiz), segunda página de 1 evento forum.
                if cuerpo.contains("aftereventid=50") {
                    return RespuestaMock {
                        estado: 200,
                        cuerpo: json!({
                            "events": [{
                                "id": 51,
                                "name": "Foro 1",
                                "timesort": 1795823940,
                                "modulename": "forum",
                                "instance": 700,
                                "cmid": 701,
                                "url": "https://moodle.test/mod/forum/view.php?id=701",
                                "course": {"id": 12, "fullname": "[3000123] Robótica"}
                            }]
                        })
                        .to_string(),
                    };
                }
                let mut evs = Vec::new();
                for i in 1..=49u64 {
                    evs.push(json!({
                        "id": i,
                        "name": format!("Taller {i}"),
                        "timesort": 1795823940,
                        "modulename": "assign",
                        "instance": 100 + i,
                        "cmid": 200 + i,
                        "url": format!("https://moodle.test/mod/assign/view.php?id={}", 200 + i),
                        "course": {"id": 10, "fullname": "[2022888] Control Automático - Grupo 1"}
                    }));
                }
                evs.push(json!({
                    "id": 50,
                    "name": "Quiz 1",
                    "timesort": 1795823940,
                    "modulename": "quiz",
                    "instance": 500,
                    "cmid": 501,
                    "url": "https://moodle.test/mod/quiz/view.php?id=501",
                    "course": {"id": 10, "fullname": "[2022888] Control Automático"}
                }));
                return RespuestaMock {
                    estado: 200,
                    cuerpo: json!({ "events": evs }).to_string(),
                };
            }
            if cuerpo.contains("mod_assign_get_submission_status") {
                if cuerpo.contains("assignid=101") {
                    return RespuestaMock {
                        estado: 200,
                        cuerpo: r#"{"lastattempt":{"submission":{"status":"submitted"}}}"#.into(),
                    };
                }
                return RespuestaMock {
                    estado: 200,
                    cuerpo: r#"{"lastattempt":{"submission":{"status":"new"}}}"#.into(),
                };
            }
            if cuerpo.contains("core_completion_get_activities_completion_status") {
                if cuerpo.contains("courseid=12") {
                    // Permiso denegado en un curso no rompe la sincronización.
                    return RespuestaMock {
                        estado: 200,
                        cuerpo: r#"{"exception":"required_capability_exception","errorcode":"nopermissions","message":"Sin permiso"}"#.into(),
                    };
                }
                return RespuestaMock {
                    estado: 200,
                    cuerpo: json!({
                        "statuses": [
                            {"cmid": 501, "modname": "quiz", "instance": 500, "state": 1}
                        ]
                    })
                    .to_string(),
                };
            }
            RespuestaMock {
                estado: 404,
                cuerpo: "{}".into(),
            }
        });

        let dir = tempfile::tempdir().expect("tempdir");
        let ruta_datos = dir.path().join("order.toml");
        let ruta_moodle = dir.path().join("moodle.toml");
        std::fs::write(
            &ruta_datos,
            "[[clase]]\nid = \"c-1\"\nmateria = \"Control\"\ndias = [\"lun\"]\ninicio = \"07:00\"\nfin = \"09:00\"\n",
        )
        .unwrap();

        let estado = conectar(&ruta_moodle, &url, "ana", "secreta").expect("conectar");
        assert!(estado.conectado);
        assert_eq!(estado.usuario, "Ana Estudiante");
        assert!(!std::fs::read_to_string(&ruta_moodle).unwrap().contains("secreta"));

        let hoy = NaiveDate::from_ymd_opt(2026, 11, 22).unwrap();
        let resumen1 = sincronizar_ahora(&ruta_datos, &ruta_moodle, hoy).expect("sync 1");
        assert_eq!(resumen1.creadas, 51);
        assert_eq!(resumen1.actualizadas, 0);

        let datos = Almacen {
            ruta: ruta_datos.clone(),
        }
        .cargar()
        .unwrap();
        assert_eq!(datos.tareas.len(), 51);
        let t1 = datos
            .tareas
            .iter()
            .find(|t| t.titulo == "Taller 1")
            .unwrap();
        assert!(t1.hecha, "assign 101 estaba submitted");
        assert_eq!(t1.materia.as_deref(), Some("Control"));
        let quiz = datos.tareas.iter().find(|t| t.titulo == "Quiz 1").unwrap();
        assert!(quiz.hecha, "quiz 500 estaba completo");
        let foro = datos.tareas.iter().find(|t| t.titulo == "Foro 1").unwrap();
        assert!(!foro.hecha, "error de permisos no rompe sync");
        assert_eq!(foro.materia.as_deref(), Some("Robótica"));

        // Segunda sincronización: 0 cambios y el archivo en disco no se reescribe.
        let antes = std::fs::read_to_string(&ruta_datos).unwrap();
        let resumen2 = sincronizar_ahora(&ruta_datos, &ruta_moodle, hoy).expect("sync 2");
        assert_eq!(resumen2, ResumenSync::default());
        assert_eq!(std::fs::read_to_string(&ruta_datos).unwrap(), antes);

        // Token vencido: devuelve TokenInvalido y marca requiere_reingreso sin borrar tareas.
        token_vencido.store(true, Ordering::Relaxed);
        let err = sincronizar_ahora(&ruta_datos, &ruta_moodle, hoy).unwrap_err();
        assert_eq!(err, ErrorMoodle::TokenInvalido);
        let est = config::estado_de(&ruta_moodle);
        assert!(est.conectado);
        assert!(est.requiere_reingreso);
        assert_eq!(est.texto_estado, "Vuelve a iniciar sesión en Moodle.");
        assert_eq!(
            Almacen { ruta: ruta_datos }
                .cargar()
                .unwrap()
                .tareas
                .len(),
            51
        );

        parar.store(true, Ordering::Relaxed);
        let _ = handle.join();
    }
}
