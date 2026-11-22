//! Qué avisar y cuándo, y el registro de lo ya enviado.
//!
//! Un aviso es un par título/cuerpo con una **clave** que lo identifica. La
//! clave siempre termina en una fecha (`YYYY-MM-DD`) para que `Registro::podar`
//! pueda tirar las viejas sin entender qué significan. Una tarea avisa dos
//! veces: la tarde anterior (a `ajustes.aviso_dia_antes`) y unas horas antes de
//! vencer (a `ajustes.aviso_horas_antes`), y como la fecha de vencimiento va en
//! la clave, cambiar el vencimiento hace que se vuelva a avisar.
//!
//! Un momento es "debido" solo dentro de una ventana corta alrededor de la hora
//! exacta: si el equipo estuvo apagado, los avisos perdidos **no** llegan en
//! ráfaga; para eso está `resumen_arranque`, que da una sola línea al abrir.
//! Aquí no se envía nada: solo se calcula la lista y se anota en el registro.

use crate::modelo::{Datos, Dia, FORMATO_FECHA, Tarea};
use crate::reloj;
use chrono::{Datelike, Days, Duration, NaiveDate, NaiveDateTime, Weekday};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Minutos que dura la ventana de un aviso. El reloj revisa cada 30 s, así que
/// 15 min sobran; una ventana corta es lo que evita la ráfaga de avisos
/// atrasados cuando el equipo estuvo apagado.
const VENTANA_MINUTOS: i64 = 15;

/// Cómo se dice la fecha dentro del cuerpo de un aviso; la hora va aparte,
/// en 12 h con a. m. / p. m., igual que en la ventana.
const FORMATO_DIA_MES: &str = "%d/%m";

/// Lo que hay que mostrar: un título corto, el cuerpo y la clave que lo marca
/// como enviado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aviso {
    pub clave: String,
    pub titulo: String,
    pub cuerpo: String,
}

/// Todo lo que toca avisar en este instante, sin contar lo que ya está en
/// `enviados`.
pub fn avisos_debidos(
    datos: &Datos,
    ahora: NaiveDateTime,
    enviados: &HashSet<String>,
) -> Vec<Aviso> {
    let mut lista = Vec::new();

    for t in &datos.tareas {
        if t.hecha {
            continue;
        }
        let vence = t.vence.date();
        let fecha = vence.format(FORMATO_FECHA).to_string();

        // La tarde anterior al vencimiento.
        let momento = vence
            .checked_sub_days(Days::new(1))
            .map(|dia| dia.and_time(datos.ajustes.aviso_dia_antes));
        if let Some(momento) = momento {
            añadir(
                &mut lista,
                enviados,
                format!("t:{}:dia:{}", t.id, fecha),
                momento,
                ahora,
                format!("Vence mañana: {}", t.titulo),
                cuerpo_tarea(t),
            );
        }

        // Unas horas antes de la hora exacta de vencimiento.
        let momento = t
            .vence
            .checked_sub_signed(Duration::hours(i64::from(datos.ajustes.aviso_horas_antes)));
        if let Some(momento) = momento {
            añadir(
                &mut lista,
                enviados,
                format!("t:{}:horas:{}", t.id, fecha),
                momento,
                ahora,
                format!("Vence pronto: {}", t.titulo),
                cuerpo_tarea(t),
            );
        }
    }

    let hoy = ahora.date();
    for r in &datos.recordatorios {
        if !r.dias.contains(&dia_de(hoy)) {
            continue;
        }
        añadir(
            &mut lista,
            enviados,
            format!("r:{}:{}", r.id, hoy.format(FORMATO_FECHA)),
            hoy.and_time(r.hora),
            ahora,
            r.texto.clone(),
            format!("Recordatorio de las {}", reloj::hora_texto(r.hora)),
        );
    }

    lista
}

/// Un solo aviso al arrancar, para no disparar en ráfaga lo que se perdió con
/// el equipo apagado: cuántas tareas hay vencidas o por vencer hoy.
pub fn resumen_arranque(datos: &Datos, ahora: NaiveDateTime) -> Option<Aviso> {
    let hoy = ahora.date();
    let n = datos
        .tareas
        .iter()
        .filter(|t| !t.hecha && t.vence.date() <= hoy)
        .count();
    if n == 0 {
        return None;
    }

    let cuerpo = if n == 1 {
        "Tienes 1 tarea vencida o por vencer hoy".to_string()
    } else {
        format!("Tienes {n} tareas vencidas o por vencer hoy")
    };
    Some(Aviso {
        clave: format!("resumen:{}", hoy.format(FORMATO_FECHA)),
        titulo: "Tareas vencidas o por vencer".to_string(),
        cuerpo,
    })
}

/// Registro de claves ya enviadas: una por línea en
/// `~/.local/state/order/enviados.txt`. Se le puede dar cualquier ruta (las
/// pruebas usan una temporal) y, si falta el directorio padre, lo crea.
pub struct Registro {
    pub ruta: PathBuf,
}

impl Registro {
    /// `~/.local/state/order/enviados.txt`.
    pub fn por_defecto() -> Registro {
        let base = dirs::state_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("state")))
            .unwrap_or_else(|| PathBuf::from("."));
        Registro {
            ruta: base.join("order").join("enviados.txt"),
        }
    }

    /// Las claves apuntadas. Si el archivo no existe o no se puede leer, la
    /// lista va vacía: lo peor que pasa es repetir un aviso.
    pub fn leer(&self) -> HashSet<String> {
        std::fs::read_to_string(&self.ruta)
            .map(|texto| {
                texto
                    .lines()
                    .map(str::trim)
                    .filter(|linea| !linea.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Apunta una clave al final del archivo. Si no se puede escribir se
    /// calla: un registro perdido solo significa un aviso repetido.
    pub fn anotar(&self, clave: &str) {
        crear_padre(&self.ruta);
        let Ok(mut archivo) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.ruta)
        else {
            return;
        };
        let _ = writeln!(archivo, "{clave}");
    }

    /// Tira las claves cuya fecha (el último campo, tras el último `:`) es
    /// anterior a `antes_de`. Las líneas que no terminan en fecha se quedan:
    /// no se sabe de cuándo son, así que no se tocan.
    pub fn podar(&self, antes_de: NaiveDate) {
        let Ok(texto) = std::fs::read_to_string(&self.ruta) else {
            return;
        };

        let mut conservadas = Vec::new();
        let mut cambio = false;
        for linea in texto.lines() {
            let clave = linea.trim();
            if clave.is_empty() {
                cambio = true;
                continue;
            }
            match fecha_de_clave(clave) {
                Some(fecha) if fecha < antes_de => cambio = true,
                _ => conservadas.push(clave),
            }
        }
        if !cambio {
            return;
        }

        let mut nuevo = conservadas.join("\n");
        if !nuevo.is_empty() {
            nuevo.push('\n');
        }
        crear_padre(&self.ruta);
        let _ = std::fs::write(&self.ruta, nuevo);
    }
}

/// Añade el aviso si su momento cae dentro de la ventana y no se envió ya.
fn añadir(
    lista: &mut Vec<Aviso>,
    enviados: &HashSet<String>,
    clave: String,
    momento: NaiveDateTime,
    ahora: NaiveDateTime,
    titulo: String,
    cuerpo: String,
) {
    if en_ventana(momento, ahora) && !enviados.contains(&clave) {
        lista.push(Aviso {
            clave,
            titulo,
            cuerpo,
        });
    }
}

/// `momento <= ahora < momento + VENTANA_MINUTOS`.
fn en_ventana(momento: NaiveDateTime, ahora: NaiveDateTime) -> bool {
    momento <= ahora && ahora < momento + Duration::minutes(VENTANA_MINUTOS)
}

fn cuerpo_tarea(t: &Tarea) -> String {
    let cuando = format!(
        "{} a las {}",
        t.vence.format(FORMATO_DIA_MES),
        reloj::hora_texto(t.vence.time())
    );
    match t.materia.as_deref().map(str::trim) {
        Some(materia) if !materia.is_empty() => format!("{materia} · vence el {cuando}"),
        _ => format!("Vence el {cuando}"),
    }
}

/// Día del archivo (`lun` … `dom`) que corresponde a una fecha.
fn dia_de(fecha: NaiveDate) -> Dia {
    match fecha.weekday() {
        Weekday::Mon => Dia::Lun,
        Weekday::Tue => Dia::Mar,
        Weekday::Wed => Dia::Mie,
        Weekday::Thu => Dia::Jue,
        Weekday::Fri => Dia::Vie,
        Weekday::Sat => Dia::Sab,
        Weekday::Sun => Dia::Dom,
    }
}

/// La fecha con la que termina una clave, si termina en una.
fn fecha_de_clave(clave: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(clave.rsplit(':').next()?, FORMATO_FECHA).ok()
}

fn crear_padre(ruta: &Path) {
    if let Some(dir) = ruta.parent().filter(|d| !d.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(dir);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::modelo::{Ajustes, Recordatorio};

    fn fecha(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).unwrap()
    }

    fn momento(anio: i32, mes: u32, dia: u32, h: u32, m: u32) -> NaiveDateTime {
        fecha(anio, mes, dia).and_hms_opt(h, m, 0).unwrap()
    }

    /// Una tarea que vence el sábado 28-nov-2026 a las 23:59.
    fn tarea(hecha: bool) -> Tarea {
        Tarea {
            id: "t-uno".to_string(),
            titulo: "Taller 2".to_string(),
            materia: Some("Control".to_string()),
            vence: momento(2026, 11, 28, 23, 59),
            hecha,
            hecha_en: hecha.then(|| fecha(2026, 11, 28)),
        }
    }

    fn datos(tareas: Vec<Tarea>, recordatorios: Vec<Recordatorio>) -> Datos {
        Datos {
            ajustes: Ajustes::default(),
            semestre: None,
            clases: Vec::new(),
            tareas,
            recordatorios,
        }
    }

    /// Un registro sobre un directorio temporal que todavía no existe, para
    /// comprobar que se crea el padre.
    fn registro(dir: &tempfile::TempDir) -> Registro {
        Registro {
            ruta: dir.path().join("state").join("order").join("enviados.txt"),
        }
    }

    #[test]
    fn aviso_dia_antes() {
        let d = datos(vec![tarea(false)], Vec::new());
        let lista = avisos_debidos(&d, momento(2026, 11, 27, 20, 5), &HashSet::new());

        assert_eq!(lista.len(), 1, "solo toca el aviso de la tarde anterior");
        assert_eq!(lista[0].clave, "t:t-uno:dia:2026-11-28");
        assert!(
            lista[0].titulo.contains("Taller 2"),
            "título: {}",
            lista[0].titulo
        );
        assert!(
            lista[0].cuerpo.contains("Control"),
            "cuerpo: {}",
            lista[0].cuerpo
        );
    }

    #[test]
    fn aviso_horas_antes() {
        let d = datos(vec![tarea(false)], Vec::new());
        let lista = avisos_debidos(&d, momento(2026, 11, 28, 20, 59), &HashSet::new());

        assert_eq!(lista.len(), 1, "solo toca el aviso de las 3 horas");
        assert_eq!(lista[0].clave, "t:t-uno:horas:2026-11-28");
    }

    #[test]
    fn no_repite() {
        let dir = tempfile::tempdir().expect("directorio temporal");
        let registro = registro(&dir);
        assert!(
            !registro.ruta.exists(),
            "la ruta empieza sin archivo ni directorio"
        );

        registro.anotar("t:t-uno:dia:2026-11-28");
        assert!(
            registro.ruta.exists(),
            "anotar debe crear el directorio padre y el archivo"
        );
        let enviados = registro.leer();
        assert!(enviados.contains("t:t-uno:dia:2026-11-28"));

        let d = datos(vec![tarea(false)], Vec::new());
        let lista = avisos_debidos(&d, momento(2026, 11, 27, 20, 5), &enviados);
        assert!(lista.is_empty(), "una clave ya enviada no se repite");
    }

    #[test]
    fn no_avisa_momentos_pasados() {
        let d = datos(vec![tarea(false)], Vec::new());

        // Pasada la ventana de 15 min de ambos momentos: ni ráfaga ni nada.
        assert!(
            avisos_debidos(&d, momento(2026, 11, 28, 22, 0), &HashSet::new()).is_empty(),
            "un aviso perdido no se dispara tarde"
        );
    }

    #[test]
    fn recordatorio_por_dia() {
        let r = Recordatorio {
            id: "r-uno".to_string(),
            texto: "Revisar Moodle".to_string(),
            dias: vec![Dia::Lun],
            hora: chrono::NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
        };
        let d = datos(Vec::new(), vec![r]);

        // Lunes 30-nov-2026 a las 18:03.
        let lista = avisos_debidos(&d, momento(2026, 11, 30, 18, 3), &HashSet::new());
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].clave, "r:r-uno:2026-11-30");
        assert_eq!(lista[0].titulo, "Revisar Moodle");

        // Martes 1-dic-2026: ese recordatorio no es de hoy.
        assert!(
            avisos_debidos(&d, momento(2026, 12, 1, 18, 3), &HashSet::new()).is_empty(),
            "solo avisa los días indicados"
        );
    }

    /// Los avisos dicen la hora igual que la ventana: en 12 h.
    #[test]
    fn el_aviso_dice_la_hora_en_doce_horas() {
        let d = datos(vec![tarea(false)], Vec::new());
        let lista = avisos_debidos(&d, momento(2026, 11, 27, 20, 5), &HashSet::new());
        assert!(
            lista[0].cuerpo.contains("a las 11:59 p. m."),
            "cuerpo: {}",
            lista[0].cuerpo
        );

        let r = Recordatorio {
            id: "r-uno".to_string(),
            texto: "Revisar Moodle".to_string(),
            dias: vec![Dia::Lun],
            hora: chrono::NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
        };
        let d = datos(Vec::new(), vec![r]);
        let lista = avisos_debidos(&d, momento(2026, 11, 30, 18, 3), &HashSet::new());
        assert_eq!(lista[0].cuerpo, "Recordatorio de las 6:00 p. m.");
    }

    #[test]
    fn ignora_hechas() {
        let d = datos(vec![tarea(true)], Vec::new());

        assert!(
            avisos_debidos(&d, momento(2026, 11, 27, 20, 5), &HashSet::new()).is_empty(),
            "una tarea hecha no avisa el día antes"
        );
        assert!(
            avisos_debidos(&d, momento(2026, 11, 28, 20, 59), &HashSet::new()).is_empty(),
            "una tarea hecha no avisa las horas antes"
        );
        assert!(
            resumen_arranque(&d, momento(2026, 11, 29, 9, 0)).is_none(),
            "una tarea hecha no cuenta en el resumen"
        );
    }
}
