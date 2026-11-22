//! Tipos de datos de ORDER y lectura del archivo TOML.
//!
//! El archivo (`~/.config/order/order.toml`) está pensado para leerse a mano:
//! las horas se escriben `"HH:MM"`, las fechas `"YYYY-MM-DD"` y los
//! vencimientos `"YYYY-MM-DDTHH:MM"` (hora local), siempre entre comillas.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const FORMATO_HORA: &str = "%H:%M";
pub const FORMATO_FECHA: &str = "%Y-%m-%d";
pub const FORMATO_FECHA_HORA: &str = "%Y-%m-%dT%H:%M";

// ---------------------------------------------------------------------------
// Formatos de fecha y hora
// ---------------------------------------------------------------------------
// Se usan como `#[serde(with = "...")]` para que el TOML lleve exactamente los
// formatos del diseño y no los de chrono (que pide segundos).

mod hora {
    use super::*;

    pub fn serialize<S: serde::Serializer>(v: &NaiveTime, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.format(FORMATO_HORA).to_string())
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<NaiveTime, D::Error> {
        let texto = a_texto(d)?;
        NaiveTime::parse_from_str(&texto, FORMATO_HORA).map_err(|_| {
            serde::de::Error::custom(format!("hora «{texto}» no válida (se espera HH:MM)"))
        })
    }
}

mod fecha {
    use super::*;

    pub fn serialize<S: serde::Serializer>(v: &NaiveDate, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.format(FORMATO_FECHA).to_string())
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<NaiveDate, D::Error> {
        let texto = a_texto(d)?;
        NaiveDate::parse_from_str(&texto, FORMATO_FECHA).map_err(|_| {
            serde::de::Error::custom(format!("fecha «{texto}» no válida (se espera YYYY-MM-DD)"))
        })
    }
}

mod fecha_hora {
    use super::*;

    pub fn serialize<S: serde::Serializer>(v: &NaiveDateTime, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.format(FORMATO_FECHA_HORA).to_string())
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<NaiveDateTime, D::Error> {
        let texto = a_texto(d)?;
        NaiveDateTime::parse_from_str(&texto, FORMATO_FECHA_HORA).map_err(|_| {
            serde::de::Error::custom(format!(
                "vencimiento «{texto}» no válido (se espera YYYY-MM-DDTHH:MM)"
            ))
        })
    }
}

mod fecha_opcional {
    use super::*;

    pub fn serialize<S: serde::Serializer>(v: &Option<NaiveDate>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(f) => s.serialize_str(&f.format(FORMATO_FECHA).to_string()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Option<NaiveDate>, D::Error> {
        let texto = a_texto(d)?;
        NaiveDate::parse_from_str(&texto, FORMATO_FECHA)
            .map(Some)
            .map_err(|_| {
                serde::de::Error::custom(format!(
                    "fecha «{texto}» no válida (se espera YYYY-MM-DD)"
                ))
            })
    }
}

/// Los valores de fecha y hora se escriben entre comillas en el archivo. Este
/// ayudante lo exige y, si no lo están, lo dice (en vez del error críptico de
/// serde al encontrarse con un datetime de TOML).
fn a_texto<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    match toml::Value::deserialize(d)? {
        toml::Value::String(texto) => Ok(texto),
        otro => Err(serde::de::Error::custom(format!(
            "se esperaba el valor entre comillas y llegó uno de tipo {}",
            otro.type_str()
        ))),
    }
}

// ---------------------------------------------------------------------------
// Tipos
// ---------------------------------------------------------------------------

/// Día de la semana tal como se escribe en el TOML: `"lun"` … `"dom"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dia {
    Lun,
    Mar,
    Mie,
    Jue,
    Vie,
    Sab,
    Dom,
}

/// Cuándo avisar de una tarea por vencer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ajustes {
    #[serde(with = "hora", default = "hora_aviso")]
    pub aviso_dia_antes: NaiveTime,
    #[serde(default = "horas_aviso")]
    pub aviso_horas_antes: u32,
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes {
            aviso_dia_antes: hora_aviso(),
            aviso_horas_antes: horas_aviso(),
        }
    }
}

fn hora_aviso() -> NaiveTime {
    NaiveTime::from_hms_opt(20, 0, 0).expect("20:00 es una hora válida")
}

fn horas_aviso() -> u32 {
    3
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Semestre {
    #[serde(with = "fecha")]
    pub inicio: NaiveDate,
    #[serde(with = "fecha")]
    pub fin: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clase {
    #[serde(default)]
    pub id: String,
    pub materia: String,
    pub dia: Dia,
    #[serde(with = "hora")]
    pub inicio: NaiveTime,
    #[serde(with = "hora")]
    pub fin: NaiveTime,
    #[serde(default)]
    pub lugar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tarea {
    #[serde(default)]
    pub id: String,
    pub titulo: String,
    #[serde(default)]
    pub materia: Option<String>,
    #[serde(with = "fecha_hora")]
    pub vence: NaiveDateTime,
    #[serde(default)]
    pub hecha: bool,
    #[serde(with = "fecha_opcional", default)]
    pub hecha_en: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recordatorio {
    #[serde(default)]
    pub id: String,
    pub texto: String,
    pub dias: Vec<Dia>,
    #[serde(with = "hora")]
    pub hora: NaiveTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Datos {
    #[serde(default)]
    pub ajustes: Ajustes,
    #[serde(default)]
    pub semestre: Option<Semestre>,
    #[serde(default, rename = "clase")]
    pub clases: Vec<Clase>,
    #[serde(default, rename = "tarea")]
    pub tareas: Vec<Tarea>,
    #[serde(default, rename = "recordatorio")]
    pub recordatorios: Vec<Recordatorio>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorDatos {
    pub linea: Option<usize>,
    pub mensaje: String,
}

impl fmt::Display for ErrorDatos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.linea {
            Some(linea) => write!(f, "línea {linea}: {}", self.mensaje),
            None => write!(f, "{}", self.mensaje),
        }
    }
}

impl std::error::Error for ErrorDatos {}

/// Lee el TOML completo. Si hay un error, informa la línea donde está.
pub fn parsear(texto: &str) -> Result<Datos, ErrorDatos> {
    toml::from_str::<Datos>(texto).map_err(|e| ErrorDatos {
        linea: e.span().map(|rango| numero_de_linea(texto, rango.start)),
        mensaje: con_ayuda(mensaje_corto(&e)),
    })
}

/// Recuerda el formato cuando el error parece ser de hora o fecha.
fn con_ayuda(mensaje: String) -> String {
    let pista = "las horas, fechas y vencimientos van entre comillas: \
                 \"07:00\", \"2026-11-28\" o \"2026-11-28T23:59\"";
    if mensaje.contains("time") || mensaje.contains("date") {
        format!("{mensaje} — {pista}")
    } else {
        mensaje
    }
}

/// Número de línea (1-based) del byte indicado.
fn numero_de_linea(texto: &str, byte: usize) -> usize {
    let corte = byte.min(texto.len());
    texto.as_bytes()[..corte]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        + 1
}

/// El mensaje del error de `toml` sin el dibujo del fragmento ni la posición
/// (la app ya muestra la línea por su lado).
fn mensaje_corto(e: &toml::de::Error) -> String {
    e.to_string()
        .lines()
        .map(str::trim)
        .find(|l| {
            !l.is_empty()
                && !l.starts_with("TOML parse error")
                && !l.starts_with('^')
                && !l.contains('|')
        })
        .map(str::to_string)
        .unwrap_or_else(|| "error de lectura del archivo".to_string())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El archivo de ejemplo que se copia en el primer arranque debe ser válido.
    #[test]
    fn parsea_ejemplo() {
        let datos = parsear(include_str!("../ejemplo.toml")).expect("ejemplo.toml debe parsear");
        assert!(
            !datos.clases.is_empty(),
            "el ejemplo trae al menos una clase"
        );
        assert!(
            !datos.tareas.is_empty(),
            "el ejemplo trae al menos una tarea"
        );
        assert!(
            !datos.recordatorios.is_empty(),
            "el ejemplo trae al menos un recordatorio"
        );
        assert_eq!(datos.ajustes.aviso_horas_antes, 3);
        assert_eq!(
            datos.ajustes.aviso_dia_antes,
            NaiveTime::from_hms_opt(20, 0, 0).unwrap()
        );
        let semestre = datos.semestre.expect("el ejemplo trae semestre");
        assert_eq!(
            semestre.inicio,
            NaiveDate::from_ymd_opt(2026, 8, 3).unwrap()
        );
        assert_eq!(semestre.fin, NaiveDate::from_ymd_opt(2026, 12, 5).unwrap());
        assert_eq!(datos.clases[0].dia, Dia::Lun);
        assert_eq!(datos.clases[0].lugar.as_deref(), Some("453-201"));
        assert_eq!(
            datos.tareas[0].vence,
            NaiveDateTime::parse_from_str("2026-11-28T23:59", FORMATO_FECHA_HORA).unwrap()
        );
        assert_eq!(datos.recordatorios[0].dias.len(), 5);
    }

    /// Un valor inválido debe reportar la línea donde está.
    #[test]
    fn error_con_linea() {
        let texto =
            "[[recordatorio]]\ntexto = \"Revisar Moodle\"\ndias = [\"lun\"]\nhora = 25:99\n";
        let error = parsear(texto).expect_err("25:99 no es una hora válida");
        assert_eq!(error.linea, Some(4), "mensaje: {}", error.mensaje);
    }

    /// Sin [ajustes] se usan los valores por defecto (20:00 y 3 horas).
    #[test]
    fn ajustes_por_defecto() {
        let texto = "[[tarea]]\ntitulo = \"Taller\"\nvence = \"2026-11-28T23:59\"\nhecha = false\n";
        let datos = parsear(texto).expect("un archivo sin [ajustes] es válido");
        assert_eq!(datos.ajustes.aviso_horas_antes, 3);
        assert_eq!(
            datos.ajustes.aviso_dia_antes,
            NaiveTime::from_hms_opt(20, 0, 0).unwrap()
        );
    }

    /// El `id` es opcional al leer: queda vacío y la app lo asigna después.
    #[test]
    fn id_faltante_es_vacio() {
        let texto = "[[tarea]]\ntitulo = \"Taller\"\nvence = \"2026-11-28T23:59\"\nhecha = false\n";
        let datos = parsear(texto).expect("una tarea sin id es válida");
        assert_eq!(datos.tareas[0].id, "");
    }
}
