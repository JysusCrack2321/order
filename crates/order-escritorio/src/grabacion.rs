//! Grabación de micrófono y conversión a WAV mono 16 kHz en memoria.
//!
//! `cpal` entrega las muestras tal como las da el micrófono (44.1 o 48 kHz,
//! uno o dos canales) y aquí se mezclan a mono y se remuestrean a 16 kHz, que
//! es lo que espera Whisper. El WAV (`hound`) se arma sin tocar el disco.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{SampleFormat as FormatoWav, WavSpec, WavWriter};
use std::io::{Cursor, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Tasa que se manda a Whisper.
pub const TASA: u32 = 16_000;

/// Una grabación en curso. Al soltarla sin `detener` no se transcribe nada.
pub struct Grabacion {
    muestras: Arc<Mutex<Vec<f32>>>,
    tasa: u32,
    canales: u16,
    flujo: Option<cpal::Stream>,
    inicio: Instant,
}

impl Grabacion {
    pub fn segundos(&self) -> u64 {
        self.inicio.elapsed().as_secs()
    }

    /// Para el micrófono y devuelve las muestras tal como llegaron.
    pub fn detener(mut self) -> (Vec<f32>, u32, u16) {
        if let Some(flujo) = self.flujo.take() {
            let _ = flujo.pause();
            // PipeWire/PulseAudio puede truncar los últimos milisegundos si el
            // flujo se suelta de inmediato: se le da un momento para entregar.
            std::thread::sleep(Duration::from_millis(200));
            drop(flujo);
        }
        let muestras = self
            .muestras
            .lock()
            .map(|buffer| buffer.clone())
            .unwrap_or_default();
        (muestras, self.tasa, self.canales)
    }
}

/// Abre el micrófono predeterminado y empieza a acumular muestras.
pub fn empezar() -> Result<Grabacion, String> {
    let host = cpal::default_host();
    let dispositivo = host
        .default_input_device()
        .ok_or_else(|| "no hay ningún micrófono disponible".to_string())?;
    let soportada = dispositivo
        .default_input_config()
        .map_err(|e| format!("no se pudo leer el micrófono: {e}"))?;
    let config: cpal::StreamConfig = soportada.clone().into();
    let muestras = Arc::new(Mutex::new(Vec::<f32>::new()));

    let flujo = match soportada.sample_format() {
        cpal::SampleFormat::F32 => arrancar::<f32>(&dispositivo, &config, &muestras, |v| v),
        cpal::SampleFormat::I16 => {
            arrancar::<i16>(&dispositivo, &config, &muestras, |v| v as f32 / 32_768.0)
        }
        cpal::SampleFormat::U16 => arrancar::<u16>(&dispositivo, &config, &muestras, |v| {
            (v as f32 - 32_768.0) / 32_768.0
        }),
        cpal::SampleFormat::I32 => arrancar::<i32>(&dispositivo, &config, &muestras, |v| {
            v as f32 / 2_147_483_648.0
        }),
        otro => return Err(format!("el micrófono usa un formato no soportado ({otro})")),
    }?;

    flujo
        .play()
        .map_err(|e| format!("no se pudo empezar a grabar: {e}"))?;

    Ok(Grabacion {
        muestras,
        tasa: config.sample_rate,
        canales: config.channels,
        flujo: Some(flujo),
        inicio: Instant::now(),
    })
}

fn arrancar<T>(
    dispositivo: &cpal::Device,
    config: &cpal::StreamConfig,
    muestras: &Arc<Mutex<Vec<f32>>>,
    convertir: fn(T) -> f32,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + 'static,
{
    let buffer = Arc::clone(muestras);
    dispositivo
        .build_input_stream::<T, _, _>(
            config.clone(),
            move |datos: &[T], _| {
                if let Ok(mut buffer) = buffer.lock() {
                    buffer.extend(datos.iter().map(|muestra| convertir(*muestra)));
                }
            },
            |error| eprintln!("ORDER: micrófono: {error}"),
            None,
        )
        .map_err(|e| format!("no se pudo abrir el micrófono: {e}"))
}

// ---------------------------------------------------------------------------
// Audio en memoria
// ---------------------------------------------------------------------------

/// Mezcla a mono y remuestrea a 16 kHz (interpolación lineal).
pub fn mono_16k(muestras: &[f32], tasa: u32, canales: u16) -> Vec<f32> {
    let canales = canales.max(1) as usize;
    let mono: Vec<f32> = muestras
        .chunks(canales)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect();
    if tasa == TASA || mono.is_empty() {
        return mono;
    }
    let paso = tasa as f64 / TASA as f64;
    let cuantas = (mono.len() as f64 / paso).floor() as usize;
    let mut salida = Vec::with_capacity(cuantas);
    for i in 0..cuantas {
        let posicion = i as f64 * paso;
        let base = posicion.floor() as usize;
        let resto = posicion - base as f64;
        let actual = mono[base];
        let siguiente = mono.get(base + 1).copied().unwrap_or(actual);
        salida.push(actual + (siguiente - actual) * resto as f32);
    }
    salida
}

/// WAV mono 16 kHz de 16 bits, en memoria.
pub fn a_wav(muestras: &[f32], tasa: u32, canales: u16) -> Result<Vec<u8>, String> {
    let mono = mono_16k(muestras, tasa, canales);
    let memoria = Memoria::default();
    let spec = WavSpec {
        channels: 1,
        sample_rate: TASA,
        bits_per_sample: 16,
        sample_format: FormatoWav::Int,
    };
    let mut escritor =
        WavWriter::new(memoria.clone(), spec).map_err(|e| format!("no se pudo armar el WAV: {e}"))?;
    for muestra in &mono {
        let valor = (muestra.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        escritor
            .write_sample(valor)
            .map_err(|e| format!("no se pudo escribir el WAV: {e}"))?;
    }
    escritor
        .finalize()
        .map_err(|e| format!("no se pudo cerrar el WAV: {e}"))?;
    memoria.bytes()
}

/// Escritura y lectura sobre el mismo `Vec<u8>`, que hound necesita para
/// parchear la cabecera al finalizar.
#[derive(Clone, Default)]
struct Memoria(Arc<Mutex<Cursor<Vec<u8>>>>);

impl Memoria {
    fn bytes(&self) -> Result<Vec<u8>, String> {
        self.0
            .lock()
            .map(|cursor| cursor.get_ref().clone())
            .map_err(|_| "no se pudo leer el audio grabado".to_string())
    }
}

impl Write for Memoria {
    fn write(&mut self, datos: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("audio bloqueado"))?
            .write(datos)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Seek for Memoria {
    fn seek(&mut self, posicion: SeekFrom) -> std::io::Result<u64> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("audio bloqueado"))?
            .seek(posicion)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn remuestrea_y_mezcla_canales() {
        // 32 kHz estéreo: el canal izquierdo vale 1 y el derecho 0 → mono 0.5.
        let mut muestras = Vec::new();
        for _ in 0..3_200 {
            muestras.push(1.0);
            muestras.push(0.0);
        }
        let mono = mono_16k(&muestras, 32_000, 2);
        assert_eq!(mono.len(), 1_600, "la mitad de muestras a 16 kHz");
        assert!((mono[0] - 0.5).abs() < 1e-6, "mezcla de canales: {}", mono[0]);

        // A la misma tasa solo se mezcla.
        let igual = mono_16k(&[0.25, 0.25], 16_000, 2);
        assert_eq!(igual, vec![0.25]);
    }

    #[test]
    fn wav_mono_16k() {
        // Un segundo a 48 kHz se convierte en un segundo a 16 kHz.
        let muestras = vec![0.0_f32; 48_000];
        let wav = a_wav(&muestras, 48_000, 1).expect("WAV");

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        let canales = u16::from_le_bytes([wav[22], wav[23]]);
        let tasa = u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]);
        let bits = u16::from_le_bytes([wav[34], wav[35]]);
        assert_eq!(canales, 1);
        assert_eq!(tasa, TASA);
        assert_eq!(bits, 16);
        // 1 s a 16 kHz, 16 bits: 32000 bytes de datos + 44 de cabecera.
        assert_eq!(wav.len(), 44 + 32_000);
    }
}
