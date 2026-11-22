//! Configuración privada de calendarios iCal (`calendarios.toml`).
//!
//! Vive junto a `agente.toml` y `moodle.toml` (permisos `600`). Las URLs
//! privadas de iCal pueden contener tokens secretos: nunca van en `order.toml`
//! ni en logs.

use crate::moodle::config::texto_hace;
use crate::presentacion::materias::{TONO_SIN_MATERIA, tono_materia};
use chrono::{Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

const FORMATO_SYNC: &str = "%Y-%m-%dT%H:%M:%S";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarioConfig {
    pub id: String,
    pub nombre: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materia: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultima_actualizacion: Option<String>,
}

impl CalendarioConfig {
    pub fn anotar_actualizacion(&mut self, ahora: NaiveDateTime) {
        self.ultima_actualizacion = Some(ahora.format(FORMATO_SYNC).to_string());
    }

    pub fn ultima_fecha_hora(&self) -> Option<NaiveDateTime> {
        self.ultima_actualizacion
            .as_deref()
            .and_then(|s| NaiveDateTime::parse_from_str(s.trim(), FORMATO_SYNC).ok())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CalendariosConfig {
    #[serde(default, rename = "calendario")]
    pub calendarios: Vec<CalendarioConfig>,
}

/// Vista segura de un calendario para mostrar en UI (sin exponer la URL secreta).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarioVista {
    pub id: String,
    pub nombre: String,
    pub materia: String,
    pub tono: i32,
    pub ultima_texto: String,
}

impl CalendarioConfig {
    pub fn a_vista(&self, ahora: NaiveDateTime) -> CalendarioVista {
        let materia = self.materia.clone().unwrap_or_default();
        let tono = if materia.trim().is_empty() {
            TONO_SIN_MATERIA
        } else {
            tono_materia(&materia)
        };
        let ultima_texto = match self.ultima_fecha_hora() {
            Some(dt) => format!("actualizado {}", texto_hace(dt, ahora)),
            None => "sin actualizar".to_string(),
        };
        CalendarioVista {
            id: self.id.clone(),
            nombre: self.nombre.clone(),
            materia,
            tono,
            ultima_texto,
        }
    }
}

/// Ruta por defecto (`~/.config/order/calendarios.toml` o junto a `ORDER_ARCHIVO`).
pub fn ruta() -> PathBuf {
    if let Some(archivo) = std::env::var_os("ORDER_ARCHIVO") {
        if !archivo.is_empty() {
            return junto_a(Path::new(&archivo));
        }
    }
    let base = dirs::config_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("order").join("calendarios.toml")
}

pub fn junto_a(archivo: &Path) -> PathBuf {
    archivo.with_file_name("calendarios.toml")
}

pub fn cargar_de(ruta: &Path) -> CalendariosConfig {
    std::fs::read_to_string(ruta)
        .ok()
        .and_then(|texto| toml::from_str::<CalendariosConfig>(&texto).ok())
        .unwrap_or_default()
}

pub fn guardar_en(ruta: &Path, config: &CalendariosConfig) -> Result<(), String> {
    let dir = ruta
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;

    let cuerpo = toml::to_string_pretty(config)
        .map_err(|e| format!("no se pudo preparar calendarios.toml: {e}"))?;
    let texto = format!(
        "# Calendarios iCal de ORDER.\n\
         # Este archivo NO se sincroniza ni se comparte. Permisos 600.\n\n{cuerpo}"
    );

    let tmp = ruta.with_extension("toml.tmp");
    let escritura = (|| -> std::io::Result<()> {
        let mut opciones = std::fs::OpenOptions::new();
        opciones.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opciones.mode(0o600);
        }
        let mut archivo = opciones.open(&tmp)?;
        archivo.write_all(texto.as_bytes())?;
        archivo.sync_all()
    })();
    if let Err(e) = escritura {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("no se pudo escribir {}: {e}", tmp.display()));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }

    std::fs::rename(&tmp, ruta).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("no se pudo guardar {}: {e}", ruta.display())
    })
}

pub fn listar_de(ruta: &Path) -> Vec<CalendarioVista> {
    let ahora = Local::now().naive_local();
    cargar_de(ruta)
        .calendarios
        .iter()
        .map(|c| c.a_vista(ahora))
        .collect()
}
