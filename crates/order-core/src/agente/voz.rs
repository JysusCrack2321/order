//! Voz: transcribir un WAV con Whisper API y gestionar el modelo local.
//!
//! El cuerpo `multipart/form-data` se arma en memoria y la llamada HTTP bloquea
//! igual que `cliente`: quien la llame (escritorio o Android) lo hace en un
//! hilo de fondo.

use crate::agente::cliente::{self, ErrorAgente};
use crate::agente::config::Voz;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Frecuencia estándar que espera Whisper (16 kHz mono PCM16).
pub const TASA_MONO_16K: u32 = 16_000;

/// Nombre del archivo del modelo local de whisper.cpp.
pub const NOMBRE_MODELO_LOCAL: &str = "ggml-small.bin";

/// URL oficial de descarga de `ggml-small.bin`.
pub const URL_MODELO_LOCAL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin";

/// Mensaje cuando se intenta grabar con API remota sin clave.
pub const MSG_FALTA_CLAVE_VOZ: &str =
    "Falta la clave de voz: ponla en Ajustes → Agente o usa «Este PC (gratis)»";

/// Mensaje cuando no se capturó audio útil.
pub const MSG_GRABACION_VACIA: &str = "No se oyó nada";

/// Ruta por defecto del modelo local: `~/.local/share/order/modelos/ggml-small.bin`.
pub fn ruta_modelo_local() -> PathBuf {
    let base = dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from("."));
    ruta_modelo_local_en(&base)
}

/// Ruta del modelo local dentro de una carpeta base de datos locales.
pub fn ruta_modelo_local_en(base_datos_local: &Path) -> PathBuf {
    base_datos_local
        .join("order")
        .join("modelos")
        .join(NOMBRE_MODELO_LOCAL)
}

/// Comprueba si el modelo local ya existe en disco y no está vacío.
pub fn modelo_local_disponible(ruta: &Path) -> bool {
    std::fs::metadata(ruta)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

/// Descarga un archivo grande (el modelo `ggml-small.bin`) a un temporal y lo
/// renombra atómicamente al terminar, reportando `(descargados, total)`.
pub fn descargar_modelo<F>(
    url: &str,
    destino: &Path,
    cancelar: &AtomicBool,
    mut al_progreso: F,
) -> Result<(), String>
where
    F: FnMut(u64, Option<u64>),
{
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("La dirección del modelo debe empezar por https:// o http://.".to_string());
    }
    let dir = destino
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;

    let agente = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(Duration::from_secs(120)))
        .http_status_as_error(false)
        .build()
        .new_agent();

    let mut respuesta = agente
        .get(url)
        .call()
        .map_err(|e| format!("Sin conexión al descargar el modelo: {e}"))?;
    let estado = respuesta.status().as_u16();
    if !(200..=299).contains(&estado) {
        return Err(format!("No se pudo descargar el modelo (HTTP {estado})."));
    }

    let total = respuesta
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|&n| n > 0);

    let tmp = destino.with_extension("bin.tmp");
    let resultado = (|| -> Result<(), String> {
        let archivo = std::fs::File::create(&tmp)
            .map_err(|e| format!("No se pudo crear {}: {e}", tmp.display()))?;
        let mut escritor = BufWriter::new(archivo);
        let mut lector = respuesta.body_mut().as_reader();
        let mut buf = [0u8; 64 * 1024];
        let mut descargados: u64 = 0;

        al_progreso(0, total);
        loop {
            if cancelar.load(Ordering::Relaxed) {
                return Err("Descarga cancelada.".to_string());
            }
            let leidos = lector
                .read(&mut buf)
                .map_err(|e| format!("Se interrumpió la descarga: {e}"))?;
            if leidos == 0 {
                break;
            }
            escritor
                .write_all(&buf[..leidos])
                .map_err(|e| format!("No se pudo guardar el modelo: {e}"))?;
            descargados = descargados.saturating_add(leidos as u64);
            al_progreso(descargados, total);
        }

        if descargados == 0 {
            return Err("El archivo descargado está vacío.".to_string());
        }
        if let Some(esperado) = total {
            if descargados != esperado {
                return Err(format!(
                    "Descarga incompleta ({descargados} de {esperado} bytes)."
                ));
            }
        }
        escritor
            .flush()
            .map_err(|e| format!("No se pudo vaciar el archivo temporal: {e}"))?;
        let archivo = escritor
            .into_inner()
            .map_err(|e| format!("No se pudo cerrar el archivo temporal: {e}"))?;
        archivo
            .sync_all()
            .map_err(|e| format!("No se pudo sincronizar el archivo: {e}"))?;
        Ok(())
    })();

    if let Err(err) = resultado {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }

    std::fs::rename(&tmp, destino).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("No se pudo mover el modelo a {}: {e}", destino.display())
    })
}

/// Convierte un `ErrorAgente` de transcripción de voz en un mensaje claro para el usuario.
pub fn mensaje_error_voz(err: &ErrorAgente) -> String {
    match err {
        ErrorAgente::Configuracion(detalle) => {
            if detalle.contains("clave") {
                MSG_FALTA_CLAVE_VOZ.to_string()
            } else if detalle.contains("no se grabó") {
                MSG_GRABACION_VACIA.to_string()
            } else {
                format!("Falta configurar la voz: {detalle}")
            }
        }
        ErrorAgente::Clave => {
            "La clave de voz no es válida (401). Revísala en Ajustes → Agente o usa «Este PC (gratis)»."
                .to_string()
        }
        ErrorAgente::Limite => {
            "El proveedor de voz alcanzó su límite de peticiones (429); espera un momento o usa «Este PC (gratis)»."
                .to_string()
        }
        ErrorAgente::Red(detalle) => format!("Sin conexión con el servicio de voz: {detalle}"),
        ErrorAgente::Servidor(codigo, detalle) => {
            format!("El servicio de voz respondió {codigo}: {detalle}")
        }
        ErrorAgente::Respuesta(detalle) => {
            if detalle.contains("vacía") {
                MSG_GRABACION_VACIA.to_string()
            } else {
                format!("Respuesta inesperada del servicio de voz: {detalle}")
            }
        }
        otro => otro.to_string(),
    }
}

/// Empaqueta muestras PCM16 mono en un WAV mono a 16 kHz en memoria.
/// Si `frecuencia` difiere de 16 kHz, remuestrea con interpolación lineal.
/// Si `muestras` está vacía (o `frecuencia == 0`), devuelve un vector vacío.
pub fn wav_de_pcm16(muestras: &[i16], frecuencia: u32) -> Vec<u8> {
    if muestras.is_empty() || frecuencia == 0 {
        return Vec::new();
    }

    let remuestreadas: Vec<i16> = if frecuencia == TASA_MONO_16K {
        muestras.to_vec()
    } else {
        let paso = frecuencia as f64 / TASA_MONO_16K as f64;
        let cuantas = (muestras.len() as f64 / paso).floor() as usize;
        let mut salida = Vec::with_capacity(cuantas);
        for i in 0..cuantas {
            let pos = i as f64 * paso;
            let base = pos.floor() as usize;
            let resto = pos - base as f64;
            let actual = muestras[base] as f64;
            let siguiente = muestras.get(base + 1).copied().unwrap_or(muestras[base]) as f64;
            let valor = (actual + (siguiente - actual) * resto)
                .round()
                .clamp(i16::MIN as f64, i16::MAX as f64);
            salida.push(valor as i16);
        }
        salida
    };

    if remuestreadas.is_empty() {
        return Vec::new();
    }

    let data_len = (remuestreadas.len() * 2) as u32;
    let riff_len = 36u32.saturating_add(data_len);
    let mut wav = Vec::with_capacity(44 + data_len as usize);

    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_len.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (PCM)
    wav.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat (PCM = 1)
    wav.extend_from_slice(&1u16.to_le_bytes()); // NumChannels (mono = 1)
    wav.extend_from_slice(&TASA_MONO_16K.to_le_bytes()); // SampleRate
    wav.extend_from_slice(&(TASA_MONO_16K * 2).to_le_bytes()); // ByteRate
    wav.extend_from_slice(&2u16.to_le_bytes()); // BlockAlign
    wav.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for muestra in remuestreadas {
        wav.extend_from_slice(&muestra.to_le_bytes());
    }
    wav
}

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
            MSG_FALTA_CLAVE_VOZ.to_string(),
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

    #[test]
    fn wav_de_pcm16_genera_mono_16k() {
        assert!(wav_de_pcm16(&[], 16_000).is_empty());
        assert!(wav_de_pcm16(&[100, 200], 0).is_empty());

        // 1 segundo a 16 kHz = 44 bytes de cabecera + 32000 bytes de datos.
        let muestras = vec![1000i16; 16_000];
        let wav = wav_de_pcm16(&muestras, 16_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 1);
        assert_eq!(u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]), 16_000);
        assert_eq!(u16::from_le_bytes([wav[34], wav[35]]), 16);
        assert_eq!(wav.len(), 44 + 32_000);

        // Remuestreo 32 kHz -> 16 kHz reduce a la mitad.
        let muestras_32k = vec![500i16; 32_000];
        let wav_32k = wav_de_pcm16(&muestras_32k, 32_000);
        assert_eq!(wav_32k.len(), 44 + 32_000);
    }

    #[test]
    fn ruta_y_mensajes_de_error_de_voz() {
        let base = Path::new("/home/usuario/.local/share");
        assert_eq!(
            ruta_modelo_local_en(base),
            PathBuf::from("/home/usuario/.local/share/order/modelos/ggml-small.bin")
        );
        assert!(!modelo_local_disponible(&ruta_modelo_local_en(base)));

        assert_eq!(
            mensaje_error_voz(&ErrorAgente::Configuracion("sin clave".into())),
            MSG_FALTA_CLAVE_VOZ
        );
        assert!(mensaje_error_voz(&ErrorAgente::Clave).contains("401"));
        assert!(mensaje_error_voz(&ErrorAgente::Limite).contains("429"));
        assert!(mensaje_error_voz(&ErrorAgente::Red("timeout".into())).contains("Sin conexión"));
        assert_eq!(
            mensaje_error_voz(&ErrorAgente::Respuesta("la transcripción vino vacía".into())),
            MSG_GRABACION_VACIA
        );
    }

    #[test]
    fn descargar_modelo_escribe_en_temporal_y_renombra() {
        use std::net::TcpListener;

        let dir = tempfile::tempdir().expect("tempdir");
        let destino = dir.path().join("modelos").join(NOMBRE_MODELO_LOCAL);
        let payload = b"ggml-modelo-simulado-1234567890";

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len()
            );
            stream.write_all(resp.as_bytes()).expect("headers");
            stream.write_all(payload).expect("body");
        });

        let cancelar = AtomicBool::new(false);
        let mut progreso = Vec::new();
        descargar_modelo(
            &format!("http://{addr}/ggml-small.bin"),
            &destino,
            &cancelar,
            |hecho, total| progreso.push((hecho, total)),
        )
        .expect("descarga ok");
        let _ = handle.join();

        assert!(modelo_local_disponible(&destino));
        assert_eq!(std::fs::read(&destino).expect("leer"), payload);
        assert!(!destino.with_extension("bin.tmp").exists());
        assert_eq!(progreso.last().copied(), Some((payload.len() as u64, Some(payload.len() as u64))));
    }

    #[test]
    fn descargar_modelo_incompleto_o_cancelado_no_deja_archivo_final() {
        use std::net::TcpListener;

        let dir = tempfile::tempdir().expect("tempdir");
        let destino = dir.path().join(NOMBRE_MODELO_LOCAL);

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let resp = "HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nhola";
            stream.write_all(resp.as_bytes()).expect("headers");
        });

        let cancelar = AtomicBool::new(false);
        let err = descargar_modelo(
            &format!("http://{addr}/ggml-small.bin"),
            &destino,
            &cancelar,
            |_, _| {},
        )
        .expect_err("debe fallar por descarga incompleta");
        let _ = handle.join();

        assert!(
            err.contains("incompleta") || err.contains("interrumpió"),
            "{err}"
        );
        assert!(!destino.exists());
        assert!(!destino.with_extension("bin.tmp").exists());
    }
}
