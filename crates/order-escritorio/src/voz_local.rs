//! Transcripción local en escritorio con whisper.cpp (`whisper-rs`).
//!
//! El modelo (`ggml-small.bin`) se carga bajo demanda en el primer uso y se
//! descarga automáticamente de memoria tras 10 minutos sin utilizarse.

use crate::grabacion;
use order_core::agente::voz;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Tiempo máximo sin uso antes de liberar el modelo de memoria (10 min).
pub const INACTIVIDAD_DESCARGA: Duration = Duration::from_secs(10 * 60);

struct ModeloCargado {
    ruta: PathBuf,
    ctx: Arc<WhisperContext>,
    ultimo_uso: Instant,
}

static CACHE: Mutex<Option<ModeloCargado>> = Mutex::new(None);
static VIGILANTE: OnceLock<()> = OnceLock::new();

/// Transcribe muestras PCM f32 (tasa y canales de `cpal`) usando el modelo local
/// en español.
pub fn transcribir_local(
    ruta_modelo: &Path,
    muestras: &[f32],
    tasa: u32,
    canales: u16,
) -> Result<String, String> {
    if !hay_sonido(muestras) {
        return Err(voz::MSG_GRABACION_VACIA.to_string());
    }
    let mono = grabacion::mono_16k(muestras, tasa, canales);
    if mono.is_empty() {
        return Err(voz::MSG_GRABACION_VACIA.to_string());
    }
    if !voz::modelo_local_disponible(ruta_modelo) {
        return Err("Falta el modelo de voz local (ggml-small.bin).".to_string());
    }

    let ctx = obtener_contexto(ruta_modelo)?;
    let mut state = ctx
        .create_state()
        .map_err(|e| format!("No se pudo preparar el transcriptor local: {e}"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 5 });
    params.set_language(Some("es"));
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    let hilos = std::thread::available_parallelism()
        .map(|n| n.get().clamp(1, 4) as i32)
        .unwrap_or(4);
    params.set_n_threads(hilos);

    state
        .full(params, &mono)
        .map_err(|e| format!("No se pudo transcribir en local: {e}"))?;

    let mut salida = String::new();
    for segmento in state.as_iter() {
        if !salida.is_empty() {
            salida.push(' ');
        }
        salida.push_str(&segmento.to_string());
    }
    marcar_uso(ruta_modelo);

    limpiar_transcripcion(&salida).ok_or_else(|| voz::MSG_GRABACION_VACIA.to_string())
}

fn hay_sonido(muestras: &[f32]) -> bool {
    !muestras.is_empty() && muestras.iter().any(|&s| s.abs() >= 1e-6)
}

fn obtener_contexto(ruta_modelo: &Path) -> Result<Arc<WhisperContext>, String> {
    arrancar_vigilante();
    let mut cache = CACHE
        .lock()
        .map_err(|_| "El transcriptor local quedó bloqueado.".to_string())?;
    if let Some(cargado) = cache.as_mut() {
        if cargado.ruta == ruta_modelo {
            cargado.ultimo_uso = Instant::now();
            return Ok(Arc::clone(&cargado.ctx));
        }
    }
    let ctx = WhisperContext::new_with_params(ruta_modelo, WhisperContextParameters::default())
        .map_err(|e| format!("No se pudo cargar el modelo de voz: {e}"))?;
    let ctx = Arc::new(ctx);
    *cache = Some(ModeloCargado {
        ruta: ruta_modelo.to_path_buf(),
        ctx: Arc::clone(&ctx),
        ultimo_uso: Instant::now(),
    });
    Ok(ctx)
}

fn marcar_uso(ruta_modelo: &Path) {
    if let Ok(mut cache) = CACHE.lock() {
        if let Some(cargado) = cache.as_mut() {
            if cargado.ruta == ruta_modelo {
                cargado.ultimo_uso = Instant::now();
            }
        }
    }
}

fn arrancar_vigilante() {
    VIGILANTE.get_or_init(|| {
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_secs(30));
            descargar_si_expiro(&CACHE, Instant::now(), INACTIVIDAD_DESCARGA);
        });
    });
}

fn descargar_si_expiro<T>(
    celda: &Mutex<Option<T>>,
    ahora: Instant,
    max_inactivo: Duration,
) -> bool
where
    T: ConUltimoUso,
{
    let Ok(mut guard) = celda.lock() else {
        return false;
    };
    let expiro = guard
        .as_ref()
        .is_some_and(|item| ahora.saturating_duration_since(item.ultimo_uso()) >= max_inactivo);
    if expiro {
        *guard = None;
        true
    } else {
        false
    }
}

trait ConUltimoUso {
    fn ultimo_uso(&self) -> Instant;
}

impl ConUltimoUso for ModeloCargado {
    fn ultimo_uso(&self) -> Instant {
        self.ultimo_uso
    }
}

/// Limpia espacios y marcadores de silencio que emite whisper.cpp.
pub fn limpiar_transcripcion(texto: &str) -> Option<String> {
    let partes: Vec<&str> = texto
        .split_whitespace()
        .filter(|tok| {
            !matches!(
                *tok,
                "[BLANK_AUDIO]" | "[SILENCE]" | "(silence)" | "(silencio)" | "[Música]" | "[Music]"
            )
        })
        .collect();
    let unido = partes.join(" ");
    if unido.is_empty() {
        None
    } else {
        Some(unido)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    struct ItemPrueba {
        ultimo: Instant,
    }

    impl ConUltimoUso for ItemPrueba {
        fn ultimo_uso(&self) -> Instant {
            self.ultimo
        }
    }

    #[test]
    fn limpia_marcadores_de_silencio() {
        assert_eq!(limpiar_transcripcion("   [BLANK_AUDIO]  "), None);
        assert_eq!(limpiar_transcripcion("(silence)"), None);
        assert_eq!(
            limpiar_transcripcion("  taller de control   para el viernes "),
            Some("taller de control para el viernes".to_string())
        );
    }

    #[test]
    fn silencio_o_vacio_devuelve_no_se_oyo_nada() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ruta = dir.path().join("ggml-small.bin");
        let err_vacio = transcribir_local(&ruta, &[], 16_000, 1).expect_err("vacío");
        assert_eq!(err_vacio, voz::MSG_GRABACION_VACIA);

        let silencio = vec![0.0_f32; 16_000];
        let err_sil = transcribir_local(&ruta, &silencio, 16_000, 1).expect_err("silencio");
        assert_eq!(err_sil, voz::MSG_GRABACION_VACIA);
    }

    #[test]
    fn modelo_se_descarga_de_memoria_tras_inactividad() {
        let inicio = Instant::now();
        let celda = Mutex::new(Some(ItemPrueba { ultimo: inicio }));

        assert!(!descargar_si_expiro(
            &celda,
            inicio + Duration::from_secs(9 * 60),
            INACTIVIDAD_DESCARGA,
        ));
        assert!(celda.lock().unwrap().is_some());

        assert!(descargar_si_expiro(
            &celda,
            inicio + INACTIVIDAD_DESCARGA,
            INACTIVIDAD_DESCARGA,
        ));
        assert!(celda.lock().unwrap().is_none());
    }

    /// Transcripción real con `ggml-small.bin` y un WAV corto (requiere modelo descargado).
    #[test]
    #[ignore]
    fn transcripcion_real_con_wav_corto() {
        let ruta_modelo = voz::ruta_modelo_local();
        assert!(
            voz::modelo_local_disponible(&ruta_modelo),
            "falta {}",
            ruta_modelo.display()
        );
        let wav_path = std::env::var("ORDER_WAV_PRUEBA").expect("definir ORDER_WAV_PRUEBA");
        let mut lector = hound::WavReader::open(wav_path).expect("abrir wav");
        let spec = lector.spec();
        let muestras: Vec<f32> = lector
            .samples::<i16>()
            .map(|s| s.expect("muestra") as f32 / i16::MAX as f32)
            .collect();
        let texto = transcribir_local(&ruta_modelo, &muestras, spec.sample_rate, spec.channels)
            .expect("transcribir");
        assert!(!texto.is_empty());
    }
}
