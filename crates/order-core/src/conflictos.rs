//! Detección y resolución de copias en conflicto de Syncthing.
//!
//! Si Syncthing crea `order.sync-conflict-*.toml` junto a `order.toml`, aquí
//! se detecta y se ofrece resolver sin borrar nunca nada: cualquier versión
//! reemplazada o descartada se mueve a `respaldos/` con marca de tiempo.

use crate::presentacion::textos::fecha_corta;
use crate::reloj::hora_texto;
use chrono::{Local, NaiveDateTime};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Una copia en conflicto encontrada junto al archivo de datos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflicto {
    pub ruta: PathBuf,
    pub fecha: Option<NaiveDateTime>,
}

/// Busca archivos `<nombre>.sync-conflict-*.<ext>` en la misma carpeta que
/// `ruta_order`.
pub fn detectar(ruta_order: &Path) -> Vec<Conflicto> {
    let dir = carpeta_de(ruta_order);
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let (prefijo, sufijo) = patron_conflicto(ruta_order);

    let mut encontrados = Vec::new();
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if !ruta.is_file() {
            continue;
        }
        let Some(nombre) = ruta.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if let Some(fecha) = fecha_desde_nombre(nombre, &prefijo, &sufijo) {
            encontrados.push(Conflicto { ruta, fecha });
        }
    }

    // Más reciente primero; si empatan o no tienen fecha, por ruta.
    encontrados.sort_by(|a, b| b.fecha.cmp(&a.fecha).then_with(|| a.ruta.cmp(&b.ruta)));
    encontrados
}

/// Indica si `candidato` corresponde a una copia en conflicto de `ruta_order`.
pub fn es_archivo_conflicto(ruta_order: &Path, candidato: &Path) -> bool {
    let Some(nombre) = candidato.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let (prefijo, sufijo) = patron_conflicto(ruta_order);
    fecha_desde_nombre(nombre, &prefijo, &sufijo).is_some()
}

/// Mueve la copia en conflicto a `respaldos/`, conservando `ruta_order` tal
/// como está.
pub fn quedarme_con_actual(ruta_order: &Path, c: &Conflicto) -> Result<(), String> {
    if !c.ruta.exists() {
        return Err(format!("no existe la copia en conflicto {}", c.ruta.display()));
    }
    let dir = carpeta_de(ruta_order);
    let respaldos = asegurar_respaldos(dir)?;
    let destino = ruta_respaldo(&respaldos, ruta_order, "conflicto");
    mover_archivo(&c.ruta, &destino)
}

/// Mueve el `order.toml` actual a `respaldos/` y deja la copia en conflicto en
/// su lugar. Nunca borra ninguna versión.
pub fn usar_la_otra(ruta_order: &Path, c: &Conflicto) -> Result<(), String> {
    if !c.ruta.exists() {
        return Err(format!("no existe la copia en conflicto {}", c.ruta.display()));
    }
    let dir = carpeta_de(ruta_order);
    let respaldos = asegurar_respaldos(dir)?;
    let respaldo_actual = if ruta_order.exists() {
        let destino = ruta_respaldo(&respaldos, ruta_order, "actual");
        mover_archivo(ruta_order, &destino)?;
        Some(destino)
    } else {
        None
    };

    if let Err(error) = mover_archivo(&c.ruta, ruta_order) {
        if let Some(respaldo) = respaldo_actual {
            let _ = mover_archivo(&respaldo, ruta_order);
        }
        return Err(error);
    }
    Ok(())
}

/// Texto del banner para la interfaz: "Hay una copia en conflicto (fecha)".
pub fn mensaje_banner(c: &Conflicto) -> String {
    match c.fecha {
        Some(dt) => format!(
            "Hay una copia en conflicto ({} · {})",
            fecha_corta(dt.date()),
            hora_texto(dt.time())
        ),
        None => "Hay una copia en conflicto (sin fecha)".to_string(),
    }
}

fn carpeta_de(ruta_order: &Path) -> &Path {
    ruta_order
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn patron_conflicto(ruta_order: &Path) -> (String, String) {
    let stem = ruta_order
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("order");
    let ext = ruta_order
        .extension()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("toml");
    (format!("{stem}.sync-conflict-"), format!(".{ext}"))
}

/// Si el nombre coincide con `{prefijo}*{sufijo}`, devuelve `Some(Option<fecha>)`.
fn fecha_desde_nombre(
    nombre: &str,
    prefijo: &str,
    sufijo: &str,
) -> Option<Option<NaiveDateTime>> {
    let resto = nombre.strip_prefix(prefijo)?.strip_suffix(sufijo)?;
    if resto.is_empty() {
        return None;
    }
    let fecha = resto
        .get(..15)
        .and_then(|trozo| NaiveDateTime::parse_from_str(trozo, "%Y%m%d-%H%M%S").ok());
    Some(fecha)
}

fn asegurar_respaldos(dir: &Path) -> Result<PathBuf, String> {
    let respaldos = dir.join("respaldos");
    std::fs::create_dir_all(&respaldos)
        .map_err(|e| format!("no se pudo crear {}: {e}", respaldos.display()))?;
    Ok(respaldos)
}

fn ruta_respaldo(respaldos: &Path, ruta_order: &Path, etiqueta: &str) -> PathBuf {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let stem = ruta_order
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("order");
    let ext = ruta_order
        .extension()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("toml");
    let marca = Local::now().format("%Y%m%d-%H%M%S");
    let candidato = respaldos.join(format!("{stem}.{etiqueta}-{marca}.{ext}"));
    if !candidato.exists() {
        return candidato;
    }
    loop {
        let n = CONTADOR.fetch_add(1, Ordering::Relaxed) + 1;
        let otro = respaldos.join(format!("{stem}.{etiqueta}-{marca}-{n}.{ext}"));
        if !otro.exists() {
            return otro;
        }
    }
}

fn mover_archivo(origen: &Path, destino: &Path) -> Result<(), String> {
    std::fs::rename(origen, destino).map_err(|e| {
        format!(
            "no se pudo mover {} a {}: {e}",
            origen.display(),
            destino.display()
        )
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use chrono::NaiveDate;
    use std::fs;

    #[test]
    fn detecta_copias_en_conflicto() {
        let dir = tempfile::tempdir().expect("directorio temporal");
        let order = dir.path().join("order.toml");
        fs::write(&order, "actual").expect("escribir order.toml");

        let c1 = dir
            .path()
            .join("order.sync-conflict-20261122-014500-ABCDEFG.toml");
        let c2 = dir
            .path()
            .join("order.sync-conflict-20261121-193012-1234567.toml");
        let c_raro = dir.path().join("order.sync-conflict-manual.toml");
        let otro = dir.path().join("historial.sync-conflict-20261122-010000-AA.toml");

        fs::write(&c1, "copia 1").expect("escribir c1");
        fs::write(&c2, "copia 2").expect("escribir c2");
        fs::write(&c_raro, "raro").expect("escribir c_raro");
        fs::write(&otro, "otro").expect("escribir otro");

        let encontrados = detectar(&order);
        assert_eq!(encontrados.len(), 3, "solo los de order.toml");
        assert_eq!(encontrados[0].ruta, c1);
        assert_eq!(
            encontrados[0].fecha,
            Some(
                NaiveDate::from_ymd_opt(2026, 11, 22)
                    .unwrap()
                    .and_hms_opt(1, 45, 0)
                    .unwrap()
            )
        );
        assert_eq!(encontrados[1].ruta, c2);
        assert_eq!(encontrados[2].ruta, c_raro);
        assert_eq!(encontrados[2].fecha, None);
        assert!(es_archivo_conflicto(&order, &c1));
        assert!(!es_archivo_conflicto(&order, &otro));
    }

    #[test]
    fn quedarme_mueve_copia_a_respaldos() {
        let dir = tempfile::tempdir().expect("directorio temporal");
        let order = dir.path().join("order.toml");
        let copia = dir
            .path()
            .join("order.sync-conflict-20261122-014500-ABCDEFG.toml");
        fs::write(&order, "version actual").expect("escribir actual");
        fs::write(&copia, "version conflicto").expect("escribir copia");

        let conflictos = detectar(&order);
        assert_eq!(conflictos.len(), 1);
        quedarme_con_actual(&order, &conflictos[0]).expect("resolver");

        assert_eq!(fs::read_to_string(&order).unwrap(), "version actual");
        assert!(!copia.exists(), "la copia salió de la raíz");
        assert!(detectar(&order).is_empty(), "ya no hay conflicto");

        let respaldos: Vec<_> = fs::read_dir(dir.path().join("respaldos"))
            .expect("respaldos")
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(respaldos.len(), 1);
        assert_eq!(
            fs::read_to_string(&respaldos[0]).unwrap(),
            "version conflicto"
        );
    }

    #[test]
    fn usar_la_otra_conserva_ambas() {
        let dir = tempfile::tempdir().expect("directorio temporal");
        let order = dir.path().join("order.toml");
        let copia = dir
            .path()
            .join("order.sync-conflict-20261122-014500-ABCDEFG.toml");
        fs::write(&order, "version actual").expect("escribir actual");
        fs::write(&copia, "version otra").expect("escribir copia");

        let conflictos = detectar(&order);
        assert_eq!(conflictos.len(), 1);
        usar_la_otra(&order, &conflictos[0]).expect("usar la otra");

        assert_eq!(fs::read_to_string(&order).unwrap(), "version otra");
        assert!(!copia.exists(), "la copia pasó a ser order.toml");
        assert!(detectar(&order).is_empty(), "ya no hay conflicto");

        let respaldos: Vec<_> = fs::read_dir(dir.path().join("respaldos"))
            .expect("respaldos")
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(respaldos.len(), 1);
        assert_eq!(
            fs::read_to_string(&respaldos[0]).unwrap(),
            "version actual"
        );
    }
}
