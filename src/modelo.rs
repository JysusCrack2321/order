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

impl Dia {
    /// Los siete días en el orden en que se escriben y se muestran (lun → dom).
    pub const TODOS: [Dia; 7] = [
        Dia::Lun,
        Dia::Mar,
        Dia::Mie,
        Dia::Jue,
        Dia::Vie,
        Dia::Sab,
        Dia::Dom,
    ];

    /// Cómo se escribe en el archivo: `"lun"` … `"dom"`.
    pub fn texto(self) -> &'static str {
        match self {
            Dia::Lun => "lun",
            Dia::Mar => "mar",
            Dia::Mie => "mie",
            Dia::Jue => "jue",
            Dia::Vie => "vie",
            Dia::Sab => "sab",
            Dia::Dom => "dom",
        }
    }
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
#[serde(from = "ClaseCruda")]
pub struct Clase {
    pub id: String,
    pub materia: String,
    /// Días en los que se repite la clase, sin repetir y en orden lun → dom.
    pub dias: Vec<Dia>,
    #[serde(with = "hora")]
    pub inicio: NaiveTime,
    #[serde(with = "hora")]
    pub fin: NaiveTime,
    pub lugar: Option<String>,
}

/// Una clase tal como viene del archivo: acepta `dias = ["lun", "mie"]` y
/// también el `dia = "lun"` de los archivos anteriores.
#[derive(Deserialize)]
struct ClaseCruda {
    #[serde(default)]
    id: String,
    materia: String,
    #[serde(default)]
    dia: Option<Dia>,
    #[serde(default)]
    dias: Option<Vec<Dia>>,
    #[serde(with = "hora")]
    inicio: NaiveTime,
    #[serde(with = "hora")]
    fin: NaiveTime,
    #[serde(default)]
    lugar: Option<String>,
}

impl From<ClaseCruda> for Clase {
    fn from(cruda: ClaseCruda) -> Clase {
        Clase {
            id: cruda.id,
            materia: cruda.materia,
            dias: unir_dias(cruda.dia, cruda.dias),
            inicio: cruda.inicio,
            fin: cruda.fin,
            lugar: cruda.lugar,
        }
    }
}

/// Junta el `dia` antiguo con `dias`, sin repetir y en orden lun → dom.
fn unir_dias(dia: Option<Dia>, dias: Option<Vec<Dia>>) -> Vec<Dia> {
    let mut lista = dias.unwrap_or_default();
    if let Some(dia) = dia {
        lista.push(dia);
    }
    Dia::TODOS
        .iter()
        .copied()
        .filter(|dia| lista.contains(dia))
        .collect()
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

// ---------------------------------------------------------------------------
// Advertencias por claves desconocidas
// ---------------------------------------------------------------------------
// `parsear` acepta el archivo aunque lleve claves que el modelo no conoce
// (serde las ignora sin quejarse). Eso es bueno para no bloquear la app, pero
// una tecla de más —`titlo` por `titulo`— haría desaparecer el dato en
// silencio. Esta función las señala con su línea; la ventana las muestra en
// amarillo, aparte del rojo de los errores que sí bloquean.

const CLAVES_RAIZ: &[&str] = &["ajustes", "semestre", "clase", "tarea", "recordatorio"];
const CLAVES_AJUSTES: &[&str] = &["aviso_dia_antes", "aviso_horas_antes"];
const CLAVES_SEMESTRE: &[&str] = &["inicio", "fin"];
const CLAVES_CLASE: &[&str] = &["id", "materia", "dia", "dias", "inicio", "fin", "lugar"];
const CLAVES_TAREA: &[&str] = &["id", "titulo", "materia", "vence", "hecha", "hecha_en"];
const CLAVES_RECORDATORIO: &[&str] = &["id", "texto", "dias", "hora"];

/// Sección en la que va el escáner y las claves que admite.
enum Seccion {
    Raiz,
    Tabla {
        etiqueta: String,
        claves: &'static [&'static str],
    },
    /// Sección que el modelo no conoce: se avisa una vez y no se miran sus
    /// claves (avisar de cada una sería ruido).
    Ignorada,
}

impl Seccion {
    fn etiqueta(&self) -> &str {
        match self {
            Seccion::Raiz => "la raíz",
            Seccion::Tabla { etiqueta, .. } => etiqueta,
            Seccion::Ignorada => "",
        }
    }

    fn claves(&self) -> Option<&'static [&'static str]> {
        match self {
            Seccion::Raiz => Some(CLAVES_RAIZ),
            Seccion::Tabla { claves, .. } => Some(claves),
            Seccion::Ignorada => None,
        }
    }
}

/// Devuelve una entrada por cada clave que el modelo no conoce, con su línea.
/// Las advertencias **no bloquean**: `parsear` sigue devolviendo los datos. Si
/// el archivo no parsea, la lista va vacía (de ese archivo ya se encarga el
/// error rojo de `parsear`).
pub fn advertencias(texto: &str) -> Vec<ErrorDatos> {
    if toml::from_str::<toml::Value>(texto).is_err() {
        return Vec::new();
    }

    let mut avisos = Vec::new();
    let mut seccion = Seccion::Raiz;
    let mut texto_largo: Option<&'static str> = None; // dentro de """ o '''
    let mut abiertos = 0i32; // un valor partido en varias líneas

    for (indice, linea) in texto.lines().enumerate() {
        let numero = indice + 1;
        let limpia = linea.trim();

        // Continuación de un valor de varias líneas: aquí no hay claves.
        if let Some(marca) = texto_largo {
            if cuenta(limpia, marca) % 2 == 1 {
                texto_largo = None;
            }
            continue;
        }
        if abiertos > 0 {
            abiertos = (abiertos + balance(limpia)).max(0);
            continue;
        }
        if limpia.is_empty() || limpia.starts_with('#') {
            continue;
        }

        if limpia.starts_with('[') {
            seccion = nueva_seccion(limpia, numero, &mut avisos);
            continue;
        }

        let Some((crudo, valor)) = limpia.split_once('=') else {
            continue;
        };
        let clave = crudo.trim();
        if clave.is_empty() {
            continue;
        }

        if let Some(claves) = seccion.claves() {
            if !claves.contains(&clave) {
                avisos.push(ErrorDatos {
                    linea: Some(numero),
                    mensaje: format!("clave desconocida \"{clave}\" en {}", seccion.etiqueta()),
                });
            }
        }

        // ¿El valor sigue en las líneas siguientes?
        let valor = sin_comentario(valor.trim());
        for marca in ["\"\"\"", "'''"] {
            if cuenta(&valor, marca) % 2 == 1 {
                texto_largo = Some(marca);
            }
        }
        if texto_largo.is_none() {
            abiertos = balance(&valor).max(0);
        }
    }

    avisos
}

/// Interpreta la cabecera `[ajustes]` / `[[tarea]]` y, si el modelo no la
/// conoce, deja la advertencia.
fn nueva_seccion(limpia: &str, numero: usize, avisos: &mut Vec<ErrorDatos>) -> Seccion {
    let cabecera = limpia
        .split('#')
        .next()
        .unwrap_or(limpia)
        .trim()
        .to_string();
    let nombre = cabecera
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split('.')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let claves = match nombre.as_str() {
        "ajustes" => CLAVES_AJUSTES,
        "semestre" => CLAVES_SEMESTRE,
        "clase" => CLAVES_CLASE,
        "tarea" => CLAVES_TAREA,
        "recordatorio" => CLAVES_RECORDATORIO,
        _ => {
            avisos.push(ErrorDatos {
                linea: Some(numero),
                mensaje: format!("sección desconocida \"{cabecera}\""),
            });
            return Seccion::Ignorada;
        }
    };
    Seccion::Tabla {
        etiqueta: cabecera,
        claves,
    }
}

/// Cuántas veces aparece `marca` en el texto.
fn cuenta(texto: &str, marca: &str) -> usize {
    texto.matches(marca).count()
}

/// Corchetes y llaves sin cerrar en una línea (`[` y `{` suman, `]` y `}`
/// restan). Sirve para saber si un valor continúa abajo.
fn balance(texto: &str) -> i32 {
    texto
        .chars()
        .map(|c| match c {
            '[' | '{' => 1,
            ']' | '}' => -1,
            _ => 0,
        })
        .sum()
}

/// Quita el comentario final (`# …`) sin tocar los `#` que van entre comillas.
fn sin_comentario(valor: &str) -> String {
    let mut salida = String::with_capacity(valor.len());
    let mut comilla: Option<char> = None;
    let mut previo = '\0';
    for c in valor.chars() {
        match comilla {
            Some(m) if c == m && previo != '\\' => comilla = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => comilla = Some(c),
            None if c == '#' => break,
            None => {}
        }
        salida.push(c);
        previo = c;
    }
    salida
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
        assert_eq!(datos.clases[0].dias, vec![Dia::Lun, Dia::Mie]);
        assert_eq!(datos.clases[1].dias, vec![Dia::Mar, Dia::Jue]);
        assert_eq!(datos.clases[0].lugar.as_deref(), Some("453-201"));
        assert_eq!(
            datos.tareas[0].vence,
            NaiveDateTime::parse_from_str("2026-11-28T23:59", FORMATO_FECHA_HORA).unwrap()
        );
        assert_eq!(datos.recordatorios[0].dias.len(), 5);
    }

    /// Los archivos anteriores traen `dia = "lun"`; se sigue aceptando, y
    /// `dia` + `dias` juntos se juntan sin repetir y en orden.
    #[test]
    fn acepta_dia_antiguo_y_dias() {
        let texto = "\
[[clase]]
materia = \"Control\"
dia = \"lun\"
inicio = \"07:00\"
fin = \"09:00\"

[[clase]]
materia = \"Robótica\"
dias = [\"jue\", \"mar\", \"jue\"]
inicio = \"09:00\"
fin = \"11:00\"

[[clase]]
materia = \"Cálculo\"
dia = \"vie\"
dias = [\"lun\", \"vie\"]
inicio = \"11:00\"
fin = \"13:00\"
";
        let datos = parsear(texto).expect("válido");
        assert_eq!(datos.clases[0].dias, vec![Dia::Lun]);
        assert_eq!(
            datos.clases[1].dias,
            vec![Dia::Mar, Dia::Jue],
            "sin repetir y en orden lun → dom"
        );
        assert_eq!(
            datos.clases[2].dias,
            vec![Dia::Lun, Dia::Vie],
            "el `dia` antiguo se suma a `dias`"
        );
    }

    /// Al escribir una clase sale `dias = [...]` y nunca el `dia` antiguo.
    #[test]
    fn escribe_dias_y_no_dia() {
        let clase = Clase {
            id: "c-uno".to_string(),
            materia: "Control".to_string(),
            dias: vec![Dia::Lun, Dia::Vie],
            inicio: NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            fin: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            lugar: None,
        };
        let texto = toml::to_string(&clase).expect("la clase se serializa");
        assert!(texto.contains("dias = [\"lun\", \"vie\"]"), "{texto}");
        assert!(!texto.contains("\ndia = "), "{texto}");

        let vuelta: Clase = toml::from_str(&texto).expect("y se vuelve a leer");
        assert_eq!(vuelta, clase);
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

    /// Una clave mal escrita no bloquea la lectura, pero se advierte con su
    /// línea y el nombre de la clave.
    #[test]
    fn advierte_clave_mal_escrita() {
        let texto = "[[tarea]]
titulo = \"Taller\"
titlo = \"x\"
vence = \"2026-11-28T23:59\"
";
        let avisos = advertencias(texto);
        assert_eq!(avisos.len(), 1, "advertencias: {avisos:?}");
        assert_eq!(avisos[0].linea, Some(3), "mensaje: {}", avisos[0].mensaje);
        assert!(
            avisos[0].mensaje.contains("titlo"),
            "mensaje: {}",
            avisos[0].mensaje
        );
        assert!(
            parsear(texto).is_ok(),
            "las advertencias no deben bloquear la lectura"
        );
    }

    /// El archivo de ejemplo no debe producir advertencias.
    #[test]
    fn ejemplo_sin_advertencias() {
        let avisos = advertencias(include_str!("../ejemplo.toml"));
        assert!(avisos.is_empty(), "advertencias: {avisos:?}");
    }
}
