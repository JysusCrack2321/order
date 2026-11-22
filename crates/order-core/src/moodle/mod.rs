//! Integración con Moodle: tipos, emparejado de materias y sincronización.

pub mod sincronizar;

use crate::modelo;
use chrono::NaiveDateTime;

/// Evento de entrega o actividad obtenido de Moodle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventoMoodle {
    pub sitio: String,
    pub modulename: String,
    pub instance: u64,
    pub titulo: String,
    pub curso: String,
    pub vence: NaiveDateTime,
    pub enlace: Option<String>,
    pub completada: bool,
}

impl EventoMoodle {
    /// Identificador estable para evitar duplicados entre sincronizaciones y dispositivos.
    pub fn origen(&self) -> String {
        format!(
            "moodle:{}:{}:{}",
            normalizar_sitio(&self.sitio),
            self.modulename.trim().to_lowercase(),
            self.instance
        )
    }
}

/// Resumen de una sincronización aplicada.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResumenSync {
    pub creadas: u32,
    pub actualizadas: u32,
    pub hechas: u32,
}

/// Quita el esquema (`https://` / `http://`) y barras finales del sitio de Moodle.
pub fn normalizar_sitio(sitio: &str) -> String {
    let s = sitio.trim();
    let sin_esquema = if s.len() >= 8 && s[..8].eq_ignore_ascii_case("https://") {
        &s[8..]
    } else if s.len() >= 7 && s[..7].eq_ignore_ascii_case("http://") {
        &s[7..]
    } else {
        s
    };
    sin_esquema.trim_end_matches('/').to_lowercase()
}

/// Construye la URL base del sitio (agregando `https://` si no trae esquema).
pub fn url_sitio(sitio: &str) -> String {
    let s = sitio.trim().trim_end_matches('/');
    if s.len() >= 8 && s[..8].eq_ignore_ascii_case("https://")
        || s.len() >= 7 && s[..7].eq_ignore_ascii_case("http://")
    {
        s.to_string()
    } else {
        format!("https://{s}")
    }
}

/// Empareja el nombre de un curso de Moodle con las materias existentes en ORDER.
///
/// Compara sin mayúsculas ni acentos, por código numérico (5+ dígitos, por ej.
/// `[2022888]` o `2022888`) o por contención de nombres. Si no coincide con
/// ninguna materia de ORDER, devuelve el nombre del curso limpio sin códigos ni grupo.
pub fn emparejar_materia(curso: &str, materias_order: &[String]) -> Option<String> {
    let crudo = curso.trim();
    if crudo.is_empty() {
        return None;
    }

    let codigos_curso = extraer_codigos_numericos(crudo);
    if !codigos_curso.is_empty() {
        for materia in materias_order {
            let codigos_materia = extraer_codigos_numericos(materia);
            if codigos_curso
                .iter()
                .any(|c| codigos_materia.iter().any(|m| m == c))
            {
                return Some(materia.clone());
            }
        }
    }

    let limpio = limpiar_nombre_curso(crudo);
    let clave_limpia = modelo::clave_materia(&limpio);
    let clave_cruda = modelo::clave_materia(crudo);

    if !clave_limpia.is_empty() {
        if let Some(exacta) = materias_order
            .iter()
            .find(|m| modelo::clave_materia(m) == clave_limpia)
        {
            return Some(exacta.clone());
        }
    }

    // Coincidencia por contención: se prefiere la materia de ORDER con clave más
    // larga que coincida para evitar falsos aciertos con nombres cortos.
    let mut mejor: Option<(usize, &String)> = None;
    for materia in materias_order {
        let clave_m = modelo::clave_materia(materia);
        if clave_m.chars().count() < 3 {
            continue;
        }
        let coincide = (!clave_limpia.is_empty()
            && (clave_limpia.contains(&clave_m)
                || (clave_limpia.chars().count() >= 3 && clave_m.contains(&clave_limpia))))
            || clave_cruda.contains(&clave_m);
        if coincide {
            let len = clave_m.len();
            if mejor.is_none_or(|(mejor_len, _)| len > mejor_len) {
                mejor = Some((len, materia));
            }
        }
    }
    if let Some((_, materia)) = mejor {
        return Some(materia.clone());
    }

    if limpio.is_empty() {
        Some(crudo.to_string())
    } else {
        Some(limpio)
    }
}

/// Secuencias de 5 o más dígitos (códigos de asignatura, evitando años de 4 dígitos).
fn extraer_codigos_numericos(texto: &str) -> Vec<String> {
    let mut codigos = Vec::new();
    let mut actual = String::new();
    for ch in texto.chars() {
        if ch.is_ascii_digit() {
            actual.push(ch);
        } else {
            if actual.len() >= 5 {
                codigos.push(std::mem::take(&mut actual));
            } else {
                actual.clear();
            }
        }
    }
    if actual.len() >= 5 {
        codigos.push(actual);
    }
    codigos
}

/// Quita códigos entre corchetes/paréntesis, códigos numéricos largos y sufijos
/// de grupo o periodo del nombre de curso de Moodle.
pub fn limpiar_nombre_curso(curso: &str) -> String {
    let mut salida = quitar_bloques_codigo(curso);

    // Quita sufijos de grupo tras separador (ej. "- Grupo 1", "- Gr. 02", "- G1").
    salida = quitar_sufijo_grupo(&salida);

    // Quita tokens numéricos largos (ej. "2022888", "2022888-1", "2022888-01").
    let palabras: Vec<&str> = salida
        .split_whitespace()
        .filter(|p| !es_token_codigo(p))
        .collect();
    let unido = palabras.join(" ");
    let limpio = unido
        .trim_matches(|c: char| {
            c.is_whitespace() || matches!(c, '-' | '–' | '—' | ':' | '·' | ',' | '/')
        })
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if limpio.is_empty() {
        curso.trim().to_string()
    } else {
        limpio
    }
}

fn quitar_bloques_codigo(texto: &str) -> String {
    let mut resultado = String::with_capacity(texto.len());
    let chars: Vec<char> = texto.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '[' || ch == '(' {
            let cierre = if ch == '[' { ']' } else { ')' };
            if let Some(rel) = chars[i + 1..].iter().position(|&c| c == cierre) {
                let fin = i + 1 + rel;
                let dentro: String = chars[i + 1..fin].iter().collect();
                if es_bloque_metadato(ch == '[', &dentro) {
                    resultado.push(' ');
                    i = fin + 1;
                    continue;
                }
            }
        }
        resultado.push(ch);
        i += 1;
    }
    resultado
}

fn es_bloque_metadato(es_corchete: bool, contenido: &str) -> bool {
    let limpio = contenido.trim();
    if limpio.is_empty() {
        return true;
    }
    if es_corchete {
        // En Moodle los corchetes suelen traer el código o grupo: `[2022888]`, `[G1]`.
        return limpio.chars().any(|c| c.is_ascii_digit()) || limpio.len() <= 8;
    }
    let lower = limpio.to_lowercase();
    if lower.starts_with("grupo")
        || lower.starts_with("gr.")
        || lower.starts_with("gr ")
        || lower.starts_with("g.")
        || (lower.starts_with('g') && lower[1..].trim().chars().all(|c| c.is_ascii_digit()))
    {
        return true;
    }
    // Solo dígitos, códigos con guion o periodo tipo `2026-2` / `01`.
    limpio
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '-' | '_' | ' ' | '.'))
}

fn quitar_sufijo_grupo(texto: &str) -> String {
    let lower = texto.to_lowercase();
    for marca in [
        " - grupo",
        " – grupo",
        " — grupo",
        " - gr.",
        " - gr ",
        " – gr.",
        " – gr ",
        " - g.",
        " - g",
        " grupo ",
    ] {
        if let Some(pos) = lower.rfind(marca) {
            let resto = lower[pos + marca.len()..].trim();
            if resto.is_empty()
                || resto
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, '-' | '.' | ' '))
            {
                return texto[..pos].to_string();
            }
        }
    }
    texto.to_string()
}

fn es_token_codigo(token: &str) -> bool {
    let t = token.trim_matches(|c: char| matches!(c, '-' | ':' | ',' | '.'));
    if t.is_empty() {
        return false;
    }
    let digitos = t.chars().filter(|c| c.is_ascii_digit()).count();
    digitos >= 5
        && t.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn normaliza_sitios_y_origen() {
        assert_eq!(
            normalizar_sitio("https://micampus.unal.edu.co/"),
            "micampus.unal.edu.co"
        );
        assert_eq!(
            normalizar_sitio("  HTTP://MiCampus.UNAL.edu.co "),
            "micampus.unal.edu.co"
        );
        assert_eq!(
            url_sitio("micampus.unal.edu.co/"),
            "https://micampus.unal.edu.co"
        );
        assert_eq!(
            url_sitio("http://127.0.0.1:8080/"),
            "http://127.0.0.1:8080"
        );
    }

    #[test]
    fn empareja_materias_por_codigo_nombre_y_limpia_nuevas() {
        let materias = vec![
            "Control".to_string(),
            "Robótica II".to_string(),
            "2016377 Señales".to_string(),
        ];

        // Sin mayúsculas ni acentos y por contención.
        assert_eq!(
            emparejar_materia("[2022888] CONTROL AUTOMÁTICO - Grupo 1", &materias).as_deref(),
            Some("Control")
        );
        assert_eq!(
            emparejar_materia("Robotica II (2026-2) (02)", &materias).as_deref(),
            Some("Robótica II")
        );
        // Por código numérico.
        assert_eq!(
            emparejar_materia("2016377-1 Teoria de Senales", &materias).as_deref(),
            Some("2016377 Señales")
        );
        // Curso nuevo sin coincidencia: limpia código y grupo, conserva "II".
        assert_eq!(
            emparejar_materia("Álgebra Lineal II [1000003] - Grupo 04", &materias).as_deref(),
            Some("Álgebra Lineal II")
        );
        assert_eq!(emparejar_materia("   ", &materias), None);
    }
}
