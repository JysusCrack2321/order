//! Llamada al modelo: HTTP bloqueante, pensada para correr en un hilo.
//!
//! Dos familias de API:
//! - **OpenAI-compatible** (DeepSeek, Gemini, «compatible con OpenAI»):
//!   `POST {url_base}/chat/completions` con `Authorization: Bearer`.
//! - **Claude** (`/v1/messages`): headers `x-api-key` y `anthropic-version`,
//!   y `output_config` con `effort: low` y un esquema JSON. Sin `thinking`,
//!   `temperature` ni `top_p`.
//!
//! Aquí no se aplica nada: se devuelve el texto del modelo para que
//! `acciones::parsear` lo lea y la ventana muestre las tarjetas.

use crate::agente::acciones;
use crate::agente::config::{AgenteConfig, Proveedor};
use serde_json::{Value, json};
use std::fmt;
use std::time::Duration;

/// Tiempo máximo de una petición (el diseño pide 60 s).
pub const ESPERA: Duration = Duration::from_secs(60);
/// Versión de la API de Claude que usa ORDER.
pub const VERSION_ANTHROPIC: &str = "2026-10-21";
/// Tope de tokens de la respuesta de Claude.
pub const MAX_TOKENS: u32 = 4096;

/// Fallo al hablar con el modelo, ya con un mensaje que se le puede enseñar
/// al usuario.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorAgente {
    /// Falta configuración (clave, URL o modelo vacíos).
    Configuracion(String),
    /// 401/403: la clave no sirve.
    Clave,
    /// 429: demasiadas peticiones.
    Limite,
    /// No hubo red o el servidor no contestó.
    Red(String),
    /// Otro código HTTP.
    Servidor(u16, String),
    /// Contestó algo que no se entiende.
    Respuesta(String),
    /// `stop_reason: "refusal"` de Claude.
    Rechazo(String),
}

impl fmt::Display for ErrorAgente {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorAgente::Configuracion(detalle) => write!(f, "falta configurar el agente: {detalle}"),
            ErrorAgente::Clave => write!(
                f,
                "la clave del agente no es válida (401). Revísala en Ajustes → Agente."
            ),
            ErrorAgente::Limite => write!(
                f,
                "el proveedor alcanzó su límite de peticiones (429); espera un momento."
            ),
            ErrorAgente::Red(detalle) => write!(f, "sin conexión con el proveedor: {detalle}"),
            ErrorAgente::Servidor(codigo, detalle) => {
                write!(f, "el proveedor respondió {codigo}: {detalle}")
            }
            ErrorAgente::Respuesta(detalle) => {
                write!(f, "respuesta inesperada del proveedor: {detalle}")
            }
            ErrorAgente::Rechazo(detalle) => write!(f, "el modelo rechazó la petición: {detalle}"),
        }
    }
}

impl std::error::Error for ErrorAgente {}

/// Pregunta al modelo y devuelve su texto (el JSON crudo de `respuesta` y
/// `acciones`). Bloquea hasta 60 s: la ventana la llama desde un hilo.
pub fn consultar(
    config: &AgenteConfig,
    contexto: &str,
    pregunta: &str,
) -> Result<String, ErrorAgente> {
    if config.url_base.trim().is_empty() {
        return Err(ErrorAgente::Configuracion(
            "la URL base está vacía".to_string(),
        ));
    }
    if config.modelo.trim().is_empty() {
        return Err(ErrorAgente::Configuracion("el modelo está vacío".to_string()));
    }
    if config.api_key.trim().is_empty() {
        return Err(ErrorAgente::Configuracion(
            "no hay clave (Ajustes → Agente)".to_string(),
        ));
    }

    let agente = agente_http();
    match config.proveedor {
        Proveedor::Claude => claude(&agente, config, contexto, pregunta),
        _ => openai(&agente, config, contexto, pregunta),
    }
}

fn agente_http() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(ESPERA))
        // Los códigos se miran a mano para poder leer el cuerpo del error.
        .http_status_as_error(false)
        .build()
        .new_agent()
}

/// System prompt: el contrato que el modelo debe cumplir.
pub fn sistema() -> String {
    format!(
        r#"Eres el asistente de ORDER, una app personal de horario, tareas y recordatorios.

Respondes SIEMPRE con un único objeto JSON, sin texto fuera del JSON:
{{"respuesta": "texto corto en español", "acciones": [ ... ]}}

Formatos exactos (los mismos del archivo):
- vencimiento de tarea: "YYYY-MM-DDTHH:MM" (hora local, 24 h)
- hora de clase o recordatorio: "HH:MM"
- días: "lun", "mar", "mie", "jue", "vie", "sab", "dom"

Tipos de acción admitidos (campo "tipo"):
- crear_tarea: {{"tipo":"crear_tarea","titulo":"…","materia":"…"|null,"vence":"YYYY-MM-DDTHH:MM"}}
- editar_tarea: {{"tipo":"editar_tarea","id":"t-…","titulo":"…","materia":"…","vence":"…"}} (solo los campos que cambian)
- marcar_tarea: {{"tipo":"marcar_tarea","id":"t-…","hecha":true|false}}
- borrar_tarea: {{"tipo":"borrar_tarea","id":"t-…"}}
- guardar_clase: {{"tipo":"guardar_clase","id":"c-…"|null,"materia":"…","dias":["lun","mie"],"inicio":"HH:MM","fin":"HH:MM","lugar":"…"|null}}
- borrar_clase: {{"tipo":"borrar_clase","id":"c-…"}}
- guardar_recordatorio: {{"tipo":"guardar_recordatorio","id":"r-…"|null,"texto":"…","dias":["lun"],"hora":"HH:MM"}}
- borrar_recordatorio: {{"tipo":"borrar_recordatorio","id":"r-…"}}

Reglas:
1. Solo puedes editar, marcar o borrar con los id que aparecen en los datos. Nunca inventes un id.
2. Usa la grafía exacta de las materias que ya existen en los datos; no traduzcas ni cambies mayúsculas.
3. Si la petición es una pregunta o una consulta, contéstala breve en "respuesta" y deja "acciones" vacío.
4. Propón una acción por cada cosa que se pida; no apliques nada tú: el usuario confirma.
5. Si falta un dato imprescindible (por ejemplo, el vencimiento de una tarea nueva), pregunta en "respuesta" sin acciones.
6. "respuesta" va en español, corta (una o dos frases), sin markdown ni bloques de código.
"#
    )
}

fn prompt_usuario(contexto: &str, pregunta: &str) -> String {
    format!("Datos actuales de ORDER:\n{contexto}\nPetición del usuario:\n{pregunta}\n")
}

// ---------------------------------------------------------------------------
// OpenAI-compatible
// ---------------------------------------------------------------------------

fn openai(
    agente: &ureq::Agent,
    config: &AgenteConfig,
    contexto: &str,
    pregunta: &str,
) -> Result<String, ErrorAgente> {
    let url = endpoint_chat(&config.url_base);
    let mut cuerpo = cuerpo_openai(config, contexto, pregunta, true);
    let (mut estado, mut texto) = enviar(
        agente,
        &url,
        &cuerpo,
        &[("Authorization", format!("Bearer {}", config.api_key.trim()))],
    )?;

    // Gemini rechaza `response_format`; si el error viene por ese campo, se
    // reintenta sin él.
    if reintentar_sin_formato(estado, &texto) {
        cuerpo = cuerpo_openai(config, contexto, pregunta, false);
        (estado, texto) = enviar(
            agente,
            &url,
            &cuerpo,
            &[("Authorization", format!("Bearer {}", config.api_key.trim()))],
        )?;
    }

    comprobar(estado, &texto)?;
    let valor: Value = serde_json::from_str(&texto)
        .map_err(|e| ErrorAgente::Respuesta(format!("no es JSON: {e}")))?;
    texto_openai(&valor)
}

/// `{url_base}/chat/completions`, sin duplicar la barra final.
fn endpoint_chat(url_base: &str) -> String {
    let base = url_base.trim().trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

fn cuerpo_openai(
    config: &AgenteConfig,
    contexto: &str,
    pregunta: &str,
    formato: bool,
) -> Value {
    let mut cuerpo = json!({
        "model": config.modelo.trim(),
        "messages": [
            {"role": "system", "content": sistema()},
            {"role": "user", "content": prompt_usuario(contexto, pregunta)}
        ]
    });
    if formato {
        cuerpo["response_format"] = json!({"type": "json_object"});
    }
    cuerpo
}

/// ¿El 400 fue por `response_format`? Entonces se reintenta sin el campo.
fn reintentar_sin_formato(estado: u16, cuerpo: &str) -> bool {
    estado == 400 && cuerpo.contains("response_format")
}

fn texto_openai(valor: &Value) -> Result<String, ErrorAgente> {
    let mensaje = valor
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|opciones| opciones.first())
        .and_then(|opcion| opcion.get("message"));
    let Some(mensaje) = mensaje else {
        return Err(ErrorAgente::Respuesta(
            "la respuesta no trae «choices»".to_string(),
        ));
    };
    if let Some(texto) = mensaje.get("content").and_then(Value::as_str) {
        return Ok(texto.to_string());
    }
    // Algunos modelos devuelven la negativa en `refusal`.
    if let Some(negativa) = mensaje.get("refusal").and_then(Value::as_str) {
        return Err(ErrorAgente::Rechazo(negativa.to_string()));
    }
    Err(ErrorAgente::Respuesta(
        "el mensaje no trae texto".to_string(),
    ))
}

// ---------------------------------------------------------------------------
// Claude (Messages API)
// ---------------------------------------------------------------------------

fn claude(
    agente: &ureq::Agent,
    config: &AgenteConfig,
    contexto: &str,
    pregunta: &str,
) -> Result<String, ErrorAgente> {
    let url = endpoint_claude(&config.url_base);
    let cuerpo = cuerpo_claude(config, contexto, pregunta);
    let (estado, texto) = enviar(
        agente,
        &url,
        &cuerpo,
        &[
            ("x-api-key", config.api_key.trim().to_string()),
            ("anthropic-version", VERSION_ANTHROPIC.to_string()),
            ("content-type", "application/json".to_string()),
        ],
    )?;
    comprobar(estado, &texto)?;
    let valor: Value = serde_json::from_str(&texto)
        .map_err(|e| ErrorAgente::Respuesta(format!("no es JSON: {e}")))?;
    respuesta_claude(&valor)
}

/// `…/v1/messages` a partir de la URL base (con o sin `/v1`).
fn endpoint_claude(url_base: &str) -> String {
    let base = url_base.trim().trim_end_matches('/');
    if base.ends_with("/messages") {
        base.to_string()
    } else if base.ends_with("/v1") {
        format!("{base}/messages")
    } else {
        format!("{base}/v1/messages")
    }
}

fn cuerpo_claude(config: &AgenteConfig, contexto: &str, pregunta: &str) -> Value {
    json!({
        "model": config.modelo.trim(),
        "max_tokens": MAX_TOKENS,
        "system": sistema(),
        "messages": [{"role": "user", "content": prompt_usuario(contexto, pregunta)}],
        "output_config": {
            "effort": "low",
            "format": {"type": "json_schema", "schema": acciones::esquema()}
        }
    })
}

fn respuesta_claude(valor: &Value) -> Result<String, ErrorAgente> {
    if valor.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
        let detalle = valor
            .get("stop_details")
            .and_then(|detalles| detalles.get("reason"))
            .and_then(Value::as_str)
            .unwrap_or("el modelo no quiso responder")
            .to_string();
        return Err(ErrorAgente::Rechazo(detalle));
    }
    let bloques = valor
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| ErrorAgente::Respuesta("la respuesta no trae «content»".to_string()))?;
    let texto = bloques
        .iter()
        .find(|bloque| bloque.get("type").and_then(Value::as_str) == Some("text"))
        .and_then(|bloque| bloque.get("text"))
        .and_then(Value::as_str);
    texto
        .map(str::to_string)
        .ok_or_else(|| ErrorAgente::Respuesta("no hay ningún bloque de texto".to_string()))
}

// ---------------------------------------------------------------------------
// HTTP en común
// ---------------------------------------------------------------------------

/// Envía el JSON y devuelve código y cuerpo. Un fallo de red no es un código
/// HTTP: se convierte en `ErrorAgente::Red`.
fn enviar(
    agente: &ureq::Agent,
    url: &str,
    cuerpo: &Value,
    cabeceras: &[(&str, String)],
) -> Result<(u16, String), ErrorAgente> {
    let mut peticion = agente.post(url).header("accept", "application/json");
    for (nombre, valor) in cabeceras {
        peticion = peticion.header(*nombre, valor.as_str());
    }
    let respuesta = peticion
        .send_json(cuerpo)
        .map_err(|e| ErrorAgente::Red(e.to_string()))?;
    let estado = respuesta.status().as_u16();
    let mut respuesta = respuesta;
    let texto = respuesta
        .body_mut()
        .read_to_string()
        .map_err(|e| ErrorAgente::Red(format!("no se pudo leer la respuesta: {e}")))?;
    Ok((estado, texto))
}

fn comprobar(estado: u16, texto: &str) -> Result<(), ErrorAgente> {
    match estado {
        200..=299 => Ok(()),
        401 | 403 => Err(ErrorAgente::Clave),
        429 => Err(ErrorAgente::Limite),
        codigo => Err(ErrorAgente::Servidor(codigo, mensaje_error(texto))),
    }
}

/// Saca el mensaje del cuerpo de error sin volcar todo el JSON.
fn mensaje_error(texto: &str) -> String {
    let recortado = texto.trim();
    if let Ok(valor) = serde_json::from_str::<Value>(recortado) {
        for ruta in [&["error", "message"][..], &["message"][..], &["error"][..]] {
            let mut actual = &valor;
            for clave in ruta {
                actual = &actual[*clave];
            }
            if let Some(mensaje) = actual.as_str() {
                return mensaje.to_string();
            }
        }
    }
    let corto: String = recortado.chars().take(200).collect();
    if corto.is_empty() {
        "sin detalle".to_string()
    } else {
        corto
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn config(proveedor: Proveedor) -> AgenteConfig {
        let mut config = AgenteConfig::default().con_proveedor(proveedor);
        config.api_key = "sk-prueba".into();
        config
    }

    #[test]
    fn endpoints() {
        assert_eq!(
            endpoint_chat("https://api.deepseek.com"),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(
            endpoint_chat("https://generativelanguage.googleapis.com/v1beta/openai/"),
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        );
        assert_eq!(
            endpoint_chat("http://localhost:1234/v1/chat/completions"),
            "http://localhost:1234/v1/chat/completions"
        );

        assert_eq!(
            endpoint_claude("https://api.anthropic.com"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            endpoint_claude("https://api.anthropic.com/v1/"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            endpoint_claude("https://api.anthropic.com/v1/messages"),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn cuerpo_openai_lleva_lo_que_pide_la_api() {
        let config = config(Proveedor::DeepSeek);
        let cuerpo = cuerpo_openai(&config, "datos", "crea una tarea", true);
        assert_eq!(cuerpo["model"], "deepseek-v4-flash");
        assert_eq!(cuerpo["response_format"]["type"], "json_object");
        let mensajes = cuerpo["messages"].as_array().expect("mensajes");
        assert_eq!(mensajes.len(), 2);
        assert_eq!(mensajes[0]["role"], "system");
        assert!(mensajes[0]["content"].as_str().unwrap().contains("ORDER"));
        assert_eq!(mensajes[1]["role"], "user");
        assert!(mensajes[1]["content"].as_str().unwrap().contains("crea una tarea"));

        // El reintento para Gemini va sin `response_format`.
        let sin = cuerpo_openai(&config, "datos", "pregunta", false);
        assert!(sin.get("response_format").is_none());
    }

    #[test]
    fn cuerpo_claude_sin_thinking_ni_temperatura() {
        let config = config(Proveedor::Claude);
        let cuerpo = cuerpo_claude(&config, "datos", "hazlo");
        assert_eq!(cuerpo["model"], "claude-opus-5-5");
        assert_eq!(cuerpo["max_tokens"], MAX_TOKENS);
        assert_eq!(cuerpo["output_config"]["effort"], "low");
        assert_eq!(cuerpo["output_config"]["format"]["type"], "json_schema");
        assert_eq!(
            cuerpo["output_config"]["format"]["schema"]["properties"]["acciones"]["type"],
            "array"
        );
        for prohibido in ["thinking", "temperature", "top_p"] {
            assert!(cuerpo.get(prohibido).is_none(), "no debe llevar {prohibido}");
        }
        assert_eq!(cuerpo["messages"].as_array().expect("mensajes").len(), 1);
        assert_eq!(cuerpo["messages"][0]["role"], "user");
    }

    #[test]
    fn lee_respuesta_openai() {
        let ejemplo = json!({
            "choices": [{"message": {"role": "assistant",
                "content": "{\"respuesta\":\"Hecho\",\"acciones\":[]}"}}]
        });
        let texto = texto_openai(&ejemplo).expect("texto");
        assert_eq!(texto, "{\"respuesta\":\"Hecho\",\"acciones\":[]}");

        let negativa = json!({"choices": [{"message": {"content": null, "refusal": "no"}}]});
        assert!(matches!(
            texto_openai(&negativa),
            Err(ErrorAgente::Rechazo(_))
        ));
        assert!(texto_openai(&json!({"choices": []})).is_err());
    }

    #[test]
    fn lee_respuesta_claude() {
        let ejemplo = json!({
            "stop_reason": "end_turn",
            "content": [
                {"type": "thinking", "thinking": "…"},
                {"type": "text", "text": "{\"respuesta\":\"Listo\",\"acciones\":[]}"}
            ]
        });
        assert_eq!(
            respuesta_claude(&ejemplo).expect("texto"),
            "{\"respuesta\":\"Listo\",\"acciones\":[]}"
        );

        let rechazo = json!({
            "stop_reason": "refusal",
            "content": [],
            "stop_details": {"reason": "no puedo ayudar con eso"}
        });
        let error = respuesta_claude(&rechazo).expect_err("rechazo");
        assert!(error.to_string().contains("no puedo ayudar con eso"), "{error}");

        assert!(respuesta_claude(&json!({"content": []})).is_err());
    }

    #[test]
    fn errores_http_legibles() {
        assert_eq!(comprobar(401, "{}"), Err(ErrorAgente::Clave));
        assert_eq!(comprobar(429, "{}"), Err(ErrorAgente::Limite));
        let error = comprobar(
            500,
            r#"{"error":{"message":"modelo saturado","type":"server_error"}}"#,
        )
        .expect_err("500");
        assert_eq!(
            error.to_string(),
            "el proveedor respondió 500: modelo saturado"
        );

        let error = comprobar(400, r#"{"error":{"message":"clave inválida"}}"#).expect_err("400");
        assert!(error.to_string().contains("clave inválida"));
        assert!(comprobar(200, "{}").is_ok());
    }

    #[test]
    fn reintento_solo_por_response_format() {
        assert!(reintentar_sin_formato(
            400,
            r#"{"error":{"message":"response_format is not supported"}}"#
        ));
        assert!(!reintentar_sin_formato(400, r#"{"error":{"message":"bad model"}}"#));
        assert!(!reintentar_sin_formato(429, "response_format"));
    }

    /// Sin clave no se sale a la red: el error lo dice antes.
    #[test]
    fn sin_clave_no_llama() {
        let mut config = AgenteConfig::default();
        config.api_key.clear();
        let error = consultar(&config, "datos", "hola").expect_err("sin clave");
        assert!(matches!(error, ErrorAgente::Configuracion(_)));
        assert!(error.to_string().contains("clave"), "{error}");
    }

    /// El system prompt fija los formatos y las reglas que pide el diseño.
    #[test]
    fn system_prompt_en_espanol() {
        let sistema = sistema();
        assert!(sistema.contains("YYYY-MM-DDTHH:MM"));
        assert!(sistema.contains("\"HH:MM\""));
        assert!(sistema.contains("no inventes un id") || sistema.contains("Nunca inventes un id"));
        assert!(sistema.contains("grafía exacta de las materias"));
        assert!(sistema.contains("pregunta"));
    }
}
