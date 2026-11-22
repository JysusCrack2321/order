//! Voz: transcribir un WAV con Whisper API.
//!
//! El cuerpo `multipart/form-data` se arma en memoria y la llamada HTTP bloquea
//! igual que `cliente`: quien la llame (escritorio o Android) lo hace en un
//! hilo de fondo.

use crate::agente::cliente::{self, ErrorAgente};
use crate::agente::config::Voz;

// ---------------------------------------------------------------------------
// Multipart y transcripción
// ---------------------------------------------------------------------------

/// Cuerpo `multipart/form-data` con `file`, `model` y `language`.
pub fn cuerpo_multipart(frontera: &str, modelo: &str, audio: &[u8]) -> Vec<u8> {
    let mut cuerpo = Vec::with_capacity(audio.len() + 512);
    campo(&mut cuerpo, frontera, "model", modelo);
    campo(&mut cuerpo, frontera, "language", "es");
    cuerpo.extend(
        format!(
            "--{frontera}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"orden.wav\"\r\n\
             Content-Type: audio/wav\r\n\r\n"
        )
        .as_bytes(),
    );
    cuerpo.extend_from_slice(audio);
    cuerpo.extend(format!("\r\n--{frontera}--\r\n").as_bytes());
    cuerpo
}

fn campo(cuerpo: &mut Vec<u8>, frontera: &str, nombre: &str, valor: &str) {
    cuerpo.extend(
        format!(
            "--{frontera}\r\n\
             Content-Disposition: form-data; name=\"{nombre}\"\r\n\r\n\
             {valor}\r\n"
        )
        .as_bytes(),
    );
}

fn frontera() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duracion| duracion.as_nanos())
        .unwrap_or_default();
    format!("order-{nanos:x}")
}

/// Manda el WAV a `{voz.url_base}/audio/transcriptions` y devuelve el texto.
pub fn transcribir(voz: &Voz, audio: &[u8]) -> Result<String, ErrorAgente> {
    if voz.url_base.trim().is_empty() {
        return Err(ErrorAgente::Configuracion(
            "falta la URL base de la voz".to_string(),
        ));
    }
    if voz.modelo.trim().is_empty() {
        return Err(ErrorAgente::Configuracion(
            "falta el modelo de la voz".to_string(),
        ));
    }
    if voz.api_key.trim().is_empty() {
        return Err(ErrorAgente::Configuracion(
            "no hay clave de voz (Ajustes → Agente)".to_string(),
        ));
    }
    if audio.is_empty() {
        return Err(ErrorAgente::Configuracion(
            "no se grabó ningún audio".to_string(),
        ));
    }

    let url = format!(
        "{}/audio/transcriptions",
        voz.url_base.trim().trim_end_matches('/')
    );
    let frontera = frontera();
    let cuerpo = cuerpo_multipart(&frontera, voz.modelo.trim(), audio);

    let agente = ureq::Agent::config_builder()
        .timeout_global(Some(cliente::ESPERA))
        .http_status_as_error(false)
        .build()
        .new_agent();
    let respuesta = agente
        .post(&url)
        .header("accept", "application/json")
        .header(
            "authorization",
            format!("Bearer {}", voz.api_key.trim()),
        )
        .header(
            "content-type",
            format!("multipart/form-data; boundary={frontera}"),
        )
        .send(cuerpo.as_slice())
        .map_err(|e| ErrorAgente::Red(e.to_string()))?;

    let estado = respuesta.status().as_u16();
    let mut respuesta = respuesta;
    let texto = respuesta
        .body_mut()
        .read_to_string()
        .map_err(|e| ErrorAgente::Red(format!("no se pudo leer la respuesta: {e}")))?;
    cliente::comprobar(estado, &texto)?;

    let valor: serde_json::Value = serde_json::from_str(&texto)
        .map_err(|e| ErrorAgente::Respuesta(format!("no es JSON: {e}")))?;
    let transcripcion = valor
        .get("text")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if transcripcion.is_empty() {
        return Err(ErrorAgente::Respuesta(
            "la transcripción vino vacía".to_string(),
        ));
    }
    Ok(transcripcion)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El multipart lleva los tres campos y el audio íntegro.
    #[test]
    fn multipart_bien_armado() {
        let audio = vec![0xABu8; 128];
        let cuerpo = cuerpo_multipart("frontera-prueba", "whisper-1", &audio);
        let texto = String::from_utf8_lossy(&cuerpo);

        assert!(texto.starts_with("--frontera-prueba\r\n"));
        assert!(texto.contains("name=\"model\"\r\n\r\nwhisper-1\r\n"));
        assert!(texto.contains("name=\"language\"\r\n\r\nes\r\n"));
        assert!(texto.contains("name=\"file\"; filename=\"orden.wav\""));
        assert!(texto.contains("Content-Type: audio/wav"));
        assert!(texto.trim_end().ends_with("--frontera-prueba--"));
        assert_eq!(
            cuerpo.iter().filter(|byte| **byte == 0xAB).count(),
            audio.len(),
            "el audio viaja completo"
        );
    }

    /// Sin clave no se toca la red.
    #[test]
    fn sin_clave_no_llama() {
        let voz = Voz::default();
        let error = transcribir(&voz, &[1, 2, 3]).expect_err("sin clave");
        assert!(error.to_string().contains("clave"), "{error}");
        assert!(matches!(error, ErrorAgente::Configuracion(_)));
    }

    #[test]
    fn sin_audio_es_error_legible() {
        let voz = Voz {
            api_key: "sk-prueba".into(),
            ..Voz::default()
        };
        let error = transcribir(&voz, &[]).expect_err("sin audio");
        assert!(error.to_string().contains("no se grabó"), "{error}");
    }
}
