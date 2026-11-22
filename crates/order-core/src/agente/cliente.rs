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
use crate::agente::adjuntos::{self, Adjunto, ContenidoAdjunto, ErrorAdjunto};
use crate::agente::config::{AgenteConfig, Proveedor};
use crate::agente::conversacion::{Conversacion, PROMPT_COMPACTACION};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
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
    /// Problema con los adjuntos (límites, PDF dañado, sin texto).
    Adjunto(ErrorAdjunto),
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
            ErrorAgente::Adjunto(err) => write!(f, "{err}"),
        }
    }
}

impl From<ErrorAdjunto> for ErrorAgente {
    fn from(err: ErrorAdjunto) -> Self {
        ErrorAgente::Adjunto(err)
    }
}

impl std::error::Error for ErrorAgente {}

/// Pregunta al modelo y devuelve su texto (el JSON crudo de `respuesta` y
/// `acciones`). Bloquea hasta 60 s: la ventana la llama desde un hilo.
pub fn consultar(
    config: &AgenteConfig,
    contexto: &str,
    pregunta: &str,
    adjuntos: &[Adjunto],
    conversacion: &Conversacion,
) -> Result<String, ErrorAgente> {
    validar_config_basica(config)?;

    let agente = agente_http();
    match config.proveedor {
        Proveedor::Claude => claude(&agente, config, contexto, pregunta, adjuntos, conversacion),
        _ => openai(&agente, config, contexto, pregunta, adjuntos, conversacion),
    }
}

/// Resume los mensajes antiguos (todos menos los últimos 10) junto con el
/// resumen anterior usando el mismo proveedor y modelo en texto libre, sin
/// esquema JSON de acciones.
pub fn compactar(
    config: &AgenteConfig,
    conversacion: &Conversacion,
) -> Result<String, ErrorAgente> {
    validar_config_basica(config)?;
    let Some(contenido) = conversacion.texto_para_compactar() else {
        return Ok(conversacion.resumen().to_string());
    };

    let agente = agente_http();
    match config.proveedor {
        Proveedor::Claude => {
            let url = endpoint_claude(&config.url_base);
            let cuerpo = cuerpo_compactacion_claude(config, &contenido);
            let (estado, texto) = enviar(
                &agente,
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
        _ => {
            let url = endpoint_chat(&config.url_base);
            let cuerpo = cuerpo_compactacion_openai(config, &contenido);
            let (estado, texto) = enviar(
                &agente,
                &url,
                &cuerpo,
                &[("Authorization", format!("Bearer {}", config.api_key.trim()))],
            )?;
            comprobar(estado, &texto)?;
            let valor: Value = serde_json::from_str(&texto)
                .map_err(|e| ErrorAgente::Respuesta(format!("no es JSON: {e}")))?;
            texto_openai(&valor)
        }
    }
}

/// Indica si el error del proveedor corresponde a un contexto demasiado largo.
pub fn es_error_contexto_largo(err: &ErrorAgente) -> bool {
    let ErrorAgente::Servidor(codigo, detalle) = err else {
        return false;
    };
    if !matches!(*codigo, 400 | 413) {
        return false;
    }
    let lower = detalle.to_lowercase();
    [
        "context_length",
        "context length",
        "maximum context",
        "too many tokens",
        "token limit",
        "too long",
        "prompt is too long",
        "exceeds the maximum",
        "context window",
    ]
    .iter()
    .any(|patron| lower.contains(patron))
}

fn validar_config_basica(config: &AgenteConfig) -> Result<(), ErrorAgente> {
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
    Ok(())
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

fn prompt_usuario(contexto: &str, pregunta: &str, conversacion: &Conversacion) -> String {
    let bloque = conversacion.bloque_contexto();
    if bloque.is_empty() {
        format!("Datos actuales de ORDER:\n{contexto}\nPetición del usuario:\n{pregunta}\n")
    } else {
        format!(
            "Datos actuales de ORDER:\n{contexto}\n{bloque}\n\nPetición del usuario:\n{pregunta}\n"
        )
    }
}

fn prompt_usuario_con_adjuntos(
    contexto: &str,
    pregunta: &str,
    conversacion: &Conversacion,
    texto_adjuntos: Option<&str>,
) -> String {
    match texto_adjuntos {
        Some(adj) if !adj.trim().is_empty() => {
            let bloque = conversacion.bloque_contexto();
            if bloque.is_empty() {
                format!(
                    "Datos actuales de ORDER:\n{contexto}\nArchivos adjuntos:\n{adj}\n\nPetición del usuario:\n{pregunta}\n"
                )
            } else {
                format!(
                    "Datos actuales de ORDER:\n{contexto}\n{bloque}\n\nArchivos adjuntos:\n{adj}\n\nPetición del usuario:\n{pregunta}\n"
                )
            }
        }
        _ => prompt_usuario(contexto, pregunta, conversacion),
    }
}

// ---------------------------------------------------------------------------
// OpenAI-compatible
// ---------------------------------------------------------------------------

fn openai(
    agente: &ureq::Agent,
    config: &AgenteConfig,
    contexto: &str,
    pregunta: &str,
    adjuntos: &[Adjunto],
    conversacion: &Conversacion,
) -> Result<String, ErrorAgente> {
    let url = endpoint_chat(&config.url_base);
    let mut cuerpo = cuerpo_openai(config, contexto, pregunta, adjuntos, conversacion, true)?;
    let (mut estado, mut texto) = enviar(
        agente,
        &url,
        &cuerpo,
        &[("Authorization", format!("Bearer {}", config.api_key.trim()))],
    )?;

    // Gemini rechaza `response_format`; si el error viene por ese campo, se
    // reintenta sin él.
    if reintentar_sin_formato(estado, &texto) {
        cuerpo = cuerpo_openai(config, contexto, pregunta, adjuntos, conversacion, false)?;
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
    adjuntos: &[Adjunto],
    conversacion: &Conversacion,
    formato: bool,
) -> Result<Value, ErrorAgente> {
    let contenido_usuario = if adjuntos.is_empty() {
        Value::String(prompt_usuario(contexto, pregunta, conversacion))
    } else {
        let ve_imagenes = config.ve_imagenes();
        adjuntos::validar(adjuntos, config.proveedor, ve_imagenes)?;
        if ve_imagenes {
            let mut partes = Vec::new();
            for adjunto in adjuntos {
                match &adjunto.contenido {
                    ContenidoAdjunto::Pdf { paginas_jpeg, .. } => {
                        if paginas_jpeg.is_empty() {
                            return Err(ErrorAgente::Adjunto(ErrorAdjunto::PdfDanado(
                                "el PDF no tiene páginas renderizadas".to_string(),
                            )));
                        }
                        for pag in paginas_jpeg {
                            partes.push(json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:image/jpeg;base64,{}", BASE64.encode(pag))
                                }
                            }));
                        }
                    }
                    ContenidoAdjunto::Imagen { jpeg, .. } => {
                        partes.push(json!({
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:image/jpeg;base64,{}", BASE64.encode(jpeg))
                            }
                        }));
                    }
                }
            }
            partes.push(json!({
                "type": "text",
                "text": prompt_usuario(contexto, pregunta, conversacion)
            }));
            Value::Array(partes)
        } else {
            let texto_adjuntos = adjuntos::a_texto(adjuntos)?;
            Value::String(prompt_usuario_con_adjuntos(
                contexto,
                pregunta,
                conversacion,
                Some(&texto_adjuntos),
            ))
        }
    };

    let mut cuerpo = json!({
        "model": config.modelo.trim(),
        "messages": [
            {"role": "system", "content": sistema()},
            {"role": "user", "content": contenido_usuario}
        ]
    });
    if formato {
        cuerpo["response_format"] = json!({"type": "json_object"});
    }
    Ok(cuerpo)
}

fn cuerpo_compactacion_openai(config: &AgenteConfig, contenido: &str) -> Value {
    json!({
        "model": config.modelo.trim(),
        "messages": [
            {"role": "system", "content": PROMPT_COMPACTACION},
            {"role": "user", "content": contenido}
        ]
    })
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
    adjuntos: &[Adjunto],
    conversacion: &Conversacion,
) -> Result<String, ErrorAgente> {
    let url = endpoint_claude(&config.url_base);
    let cuerpo = cuerpo_claude(config, contexto, pregunta, adjuntos, conversacion)?;
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

fn cuerpo_claude(
    config: &AgenteConfig,
    contexto: &str,
    pregunta: &str,
    adjuntos: &[Adjunto],
    conversacion: &Conversacion,
) -> Result<Value, ErrorAgente> {
    let contenido = if adjuntos.is_empty() {
        Value::String(prompt_usuario(contexto, pregunta, conversacion))
    } else {
        adjuntos::validar(adjuntos, Proveedor::Claude, true)?;
        let mut bloques = Vec::new();
        for adjunto in adjuntos {
            match &adjunto.contenido {
                ContenidoAdjunto::Pdf {
                    bytes,
                    paginas_jpeg,
                    ..
                } => {
                    let usar_pdf_crudo = !bytes.is_empty()
                        && (paginas_jpeg.is_empty()
                            || adjuntos::contar_paginas_pdf(bytes)
                                .is_ok_and(|n| n <= adjuntos::MAX_PAGINAS_PDF));
                    if usar_pdf_crudo {
                        bloques.push(json!({
                            "type": "document",
                            "source": {
                                "type": "base64",
                                "media_type": "application/pdf",
                                "data": BASE64.encode(bytes)
                            }
                        }));
                    } else {
                        for pag in paginas_jpeg {
                            bloques.push(json!({
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/jpeg",
                                    "data": BASE64.encode(pag)
                                }
                            }));
                        }
                    }
                }
                ContenidoAdjunto::Imagen { jpeg, .. } => {
                    bloques.push(json!({
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/jpeg",
                            "data": BASE64.encode(jpeg)
                        }
                    }));
                }
            }
        }
        bloques.push(json!({
            "type": "text",
            "text": prompt_usuario(contexto, pregunta, conversacion)
        }));
        Value::Array(bloques)
    };

    Ok(json!({
        "model": config.modelo.trim(),
        "max_tokens": MAX_TOKENS,
        "system": sistema(),
        "messages": [{"role": "user", "content": contenido}],
        "output_config": {
            "effort": "low",
            "format": {"type": "json_schema", "schema": acciones::esquema()}
        }
    }))
}

fn cuerpo_compactacion_claude(config: &AgenteConfig, contenido: &str) -> Value {
    json!({
        "model": config.modelo.trim(),
        "max_tokens": MAX_TOKENS,
        "system": PROMPT_COMPACTACION,
        "messages": [{"role": "user", "content": contenido}]
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

pub fn comprobar(estado: u16, texto: &str) -> Result<(), ErrorAgente> {
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
        let vacia = Conversacion::nueva();
        let cuerpo =
            cuerpo_openai(&config, "datos", "crea una tarea", &[], &vacia, true).expect("cuerpo");
        assert_eq!(cuerpo["model"], "deepseek-v4-flash");
        assert_eq!(cuerpo["response_format"]["type"], "json_object");
        let mensajes = cuerpo["messages"].as_array().expect("mensajes");
        assert_eq!(mensajes.len(), 2);
        assert_eq!(mensajes[0]["role"], "system");
        assert!(mensajes[0]["content"].as_str().unwrap().contains("ORDER"));
        assert_eq!(mensajes[1]["role"], "user");
        assert!(mensajes[1]["content"].as_str().unwrap().contains("crea una tarea"));

        // El reintento para Gemini va sin `response_format`.
        let sin =
            cuerpo_openai(&config, "datos", "pregunta", &[], &vacia, false).expect("sin formato");
        assert!(sin.get("response_format").is_none());
    }

    #[test]
    fn cuerpo_claude_sin_thinking_ni_temperatura() {
        let config = config(Proveedor::Claude);
        let vacia = Conversacion::nueva();
        let cuerpo = cuerpo_claude(&config, "datos", "hazlo", &[], &vacia).expect("cuerpo");
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
    fn cuerpo_claude_manda_document_e_imagen_antes_del_texto() {
        let config = config(Proveedor::Claude);
        let vacia = Conversacion::nueva();
        let pdf = crate::agente::adjuntos::pruebas_ayuda::pdf_con_paginas(&["Horario de Control"]);
        let adjuntos = vec![
            Adjunto {
                nombre: "horario.pdf".into(),
                contenido: ContenidoAdjunto::Pdf {
                    bytes: pdf.clone(),
                    paginas_jpeg: vec![],
                    texto_ocr: None,
                },
            },
            Adjunto {
                nombre: "foto.jpg".into(),
                contenido: ContenidoAdjunto::Imagen {
                    jpeg: vec![0xFF, 0xD8, 0xFF, 0xD9],
                    texto_ocr: None,
                },
            },
        ];

        let cuerpo =
            cuerpo_claude(&config, "datos", "agrega las clases", &adjuntos, &vacia).expect("cuerpo");
        let content = cuerpo["messages"][0]["content"].as_array().expect("bloques");
        assert_eq!(content.len(), 3);
        assert_eq!(content[0]["type"], "document");
        assert_eq!(content[0]["source"]["type"], "base64");
        assert_eq!(content[0]["source"]["media_type"], "application/pdf");
        assert_eq!(content[0]["source"]["data"], BASE64.encode(&pdf));
        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["source"]["media_type"], "image/jpeg");
        assert_eq!(content[2]["type"], "text");
        assert!(content[2]["text"].as_str().unwrap().contains("agrega las clases"));
    }

    #[test]
    fn cuerpo_gemini_manda_image_url_data_url() {
        let config = config(Proveedor::Gemini);
        let vacia = Conversacion::nueva();
        let adjuntos = vec![
            Adjunto {
                nombre: "enunciado.pdf".into(),
                contenido: ContenidoAdjunto::Pdf {
                    bytes: vec![],
                    paginas_jpeg: vec![vec![1, 2, 3], vec![4, 5, 6]],
                    texto_ocr: None,
                },
            },
            Adjunto {
                nombre: "tablero.jpg".into(),
                contenido: ContenidoAdjunto::Imagen {
                    jpeg: vec![7, 8, 9],
                    texto_ocr: None,
                },
            },
        ];

        let cuerpo =
            cuerpo_openai(&config, "datos", "lee el enunciado", &adjuntos, &vacia, false)
                .expect("cuerpo");
        let content = cuerpo["messages"][1]["content"].as_array().expect("bloques");
        assert_eq!(content.len(), 4);
        assert_eq!(content[0]["type"], "image_url");
        assert_eq!(
            content[0]["image_url"]["url"],
            format!("data:image/jpeg;base64,{}", BASE64.encode([1, 2, 3]))
        );
        assert_eq!(content[2]["type"], "image_url");
        assert_eq!(
            content[2]["image_url"]["url"],
            format!("data:image/jpeg;base64,{}", BASE64.encode([7, 8, 9]))
        );
        assert_eq!(content[3]["type"], "text");
    }

    #[test]
    fn cuerpo_deepseek_y_otro_solo_texto() {
        let vacia = Conversacion::nueva();
        let pdf = crate::agente::adjuntos::pruebas_ayuda::pdf_con_paginas(&["Taller 3 para el viernes"]);
        let adjuntos = vec![
            Adjunto {
                nombre: "taller.pdf".into(),
                contenido: ContenidoAdjunto::Pdf {
                    bytes: pdf,
                    paginas_jpeg: vec![vec![1, 2, 3]],
                    texto_ocr: None,
                },
            },
            Adjunto {
                nombre: "nota.jpg".into(),
                contenido: ContenidoAdjunto::Imagen {
                    jpeg: vec![9, 9],
                    texto_ocr: Some("Parcial el lunes 8am".into()),
                },
            },
        ];

        let deepseek = config(Proveedor::DeepSeek);
        let cuerpo =
            cuerpo_openai(&deepseek, "datos", "resume", &adjuntos, &vacia, true).expect("deepseek");
        let texto = cuerpo["messages"][1]["content"].as_str().expect("solo texto");
        assert!(texto.contains("--- archivo: taller.pdf (página 1) ---"), "{texto}");
        assert!(texto.contains("Taller 3 para el viernes"), "{texto}");
        assert!(texto.contains("--- archivo: nota.jpg ---"), "{texto}");
        assert!(texto.contains("Parcial el lunes 8am"), "{texto}");

        // «Otro» sin ve_imagenes se comporta como DeepSeek; con ve_imagenes como Gemini.
        let mut otro = config(Proveedor::OpenAICompatible);
        let cuerpo_sin =
            cuerpo_openai(&otro, "datos", "resume", &adjuntos, &vacia, false).expect("otro sin");
        assert!(cuerpo_sin["messages"][1]["content"].is_string());

        otro.ve_imagenes = true;
        let cuerpo_con =
            cuerpo_openai(&otro, "datos", "resume", &adjuntos, &vacia, false).expect("otro con");
        assert!(cuerpo_con["messages"][1]["content"].is_array());
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
        let error = consultar(&config, "datos", "hola", &[], &Conversacion::nueva())
            .expect_err("sin clave");
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
        assert!(!sistema.contains("\"resumen\""));
    }

    #[test]
    fn cuerpos_claude_y_openai_incluyen_conversacion_en_orden() {
        let mut conv = Conversacion::nueva();
        conv.agregar_documento("horario.pdf", "LUNES  Control  11:00 - 13:00");
        conv.registrar_turno(
            "pon taller el jueves",
            &["horario.pdf".into()],
            "Propongo el jueves.",
        );
        conv.aplicar_compactacion("Hablan de taller de Control.");
        conv.anotar_resultado("Crear la tarea «Taller» (jueves)", true);

        let openai_cfg = config(Proveedor::DeepSeek);
        let cuerpo_oai = cuerpo_openai(
            &openai_cfg,
            "Agenda de hoy\n",
            "no, mejor el viernes",
            &[],
            &conv,
            true,
        )
        .expect("openai");
        let texto_oai = cuerpo_oai["messages"][1]["content"].as_str().expect("texto");

        let claude_cfg = config(Proveedor::Claude);
        let cuerpo_cld = cuerpo_claude(
            &claude_cfg,
            "Agenda de hoy\n",
            "no, mejor el viernes",
            &[],
            &conv,
        )
        .expect("claude");
        let texto_cld = cuerpo_cld["messages"][0]["content"].as_str().expect("texto");

        for texto in [texto_oai, texto_cld] {
            let p1 = texto.find("Datos actuales de ORDER:").expect("1. agenda");
            let p2 = texto
                .find("## Documentos de la conversación")
                .expect("2. docs");
            let p3 = texto
                .find("## Resumen de lo hablado antes")
                .expect("3. resumen");
            let p4 = texto.find("## Conversación").expect("4. conversacion");
            let p5 = texto
                .find("## Qué pasó con tus propuestas")
                .expect("5. resultados");
            let p6 = texto.find("Petición del usuario:").expect("6. peticion");
            assert!(p1 < p2 && p2 < p3 && p3 < p4 && p4 < p5 && p5 < p6, "{texto}");
            assert!(texto.contains("[adjuntó: horario.pdf]"), "{texto}");
            assert!(texto.contains("ORDER: Propongo el jueves."), "{texto}");
            assert!(texto.contains("no, mejor el viernes"), "{texto}");
        }
    }

    #[test]
    fn cuerpo_compactacion_usa_prompt_sin_esquema() {
        let mut conv = Conversacion::nueva();
        for i in 1..=6 {
            conv.registrar_turno(&format!("pregunta {i}"), &[], &format!("respuesta {i}"));
        }
        let contenido = conv.texto_para_compactar().expect("para compactar");

        let openai_cfg = config(Proveedor::DeepSeek);
        let oai = cuerpo_compactacion_openai(&openai_cfg, &contenido);
        assert_eq!(oai["messages"][0]["content"], PROMPT_COMPACTACION);
        assert!(oai["messages"][1]["content"].as_str().unwrap().contains("pregunta 1"));
        assert!(oai.get("response_format").is_none());

        let claude_cfg = config(Proveedor::Claude);
        let cld = cuerpo_compactacion_claude(&claude_cfg, &contenido);
        assert_eq!(cld["system"], PROMPT_COMPACTACION);
        assert!(cld["messages"][0]["content"].as_str().unwrap().contains("pregunta 1"));
        assert!(cld.get("output_config").is_none());
    }

    #[test]
    fn detecta_error_contexto_demasiado_largo() {
        assert!(es_error_contexto_largo(&ErrorAgente::Servidor(
            400,
            "This model's maximum context length is 65536 tokens".into()
        )));
        assert!(es_error_contexto_largo(&ErrorAgente::Servidor(
            413,
            "prompt is too long".into()
        )));
        assert!(!es_error_contexto_largo(&ErrorAgente::Servidor(
            500,
            "context length exceeded".into()
        )));
        assert!(!es_error_contexto_largo(&ErrorAgente::Limite));
    }
}
