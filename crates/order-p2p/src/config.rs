//! Configuración privada P2P (`sync.toml`) con clave secreta del nodo y
//! lista de dispositivos emparejados. Siempre con permisos `0600`.

use chrono::{Local, NaiveDateTime};
use iroh::SecretKey;
use order_core::moodle::config::texto_hace;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

const FORMATO_FECHA_HORA: &str = "%Y-%m-%dT%H:%M:%S";

/// Dispositivo remoto emparejado en `sync.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispositivoEmparejado {
    pub id: String,
    pub nombre: String,
    pub emparejado_en: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultima_sincronizacion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultimo_error: Option<String>,
}

/// Estado resumido de un dispositivo emparejado listo para mostrar en la UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoDispositivo {
    pub id: String,
    pub nombre: String,
    pub texto_estado: String,
    pub en_linea: bool,
}

impl DispositivoEmparejado {
    pub fn a_info(&self, ahora: NaiveDateTime) -> InfoDispositivo {
        let (texto_estado, en_linea) = if self.ultimo_error.is_some() {
            ("sin conexión".to_string(), false)
        } else if let Some(ultima) = self
            .ultima_sincronizacion
            .as_deref()
            .and_then(|s| NaiveDateTime::parse_from_str(s.trim(), FORMATO_FECHA_HORA).ok())
        {
            (format!("sincronizado {}", texto_hace(ultima, ahora)), true)
        } else {
            ("sin sincronizar aún".to_string(), true)
        };
        InfoDispositivo {
            id: self.id.clone(),
            nombre: self.nombre.clone(),
            texto_estado,
            en_linea,
        }
    }
}

/// Contenido completo de `sync.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncConfig {
    pub clave_secreta: String,
    pub nombre_dispositivo: String,
    #[serde(default)]
    pub emparejados: Vec<DispositivoEmparejado>,
}

impl SyncConfig {
    pub fn nueva(nombre_inicial: Option<&str>) -> Self {
        let clave = SecretKey::generate();
        SyncConfig {
            clave_secreta: a_hex(&clave.to_bytes()),
            nombre_dispositivo: nombre_limpio_o_defecto(nombre_inicial),
            emparejados: Vec::new(),
        }
    }

    pub fn secret_key(&self) -> Result<SecretKey, String> {
        let bytes = desde_hex_32(&self.clave_secreta)?;
        Ok(SecretKey::from_bytes(&bytes))
    }

    pub fn esta_emparejado(&self, id: &str) -> bool {
        self.emparejados.iter().any(|d| d.id == id)
    }

    pub fn registrar_emparejado(&mut self, id: String, nombre: String, ahora: NaiveDateTime) {
        let nombre_final = if nombre.trim().is_empty() {
            "Dispositivo".to_string()
        } else {
            nombre.trim().to_string()
        };
        let fecha = ahora.format(FORMATO_FECHA_HORA).to_string();
        if let Some(existente) = self.emparejados.iter_mut().find(|d| d.id == id) {
            existente.nombre = nombre_final;
            existente.ultimo_error = None;
        } else {
            self.emparejados.push(DispositivoEmparejado {
                id,
                nombre: nombre_final,
                emparejado_en: fecha,
                ultima_sincronizacion: None,
                ultimo_error: None,
            });
        }
    }

    pub fn anotar_exito(&mut self, id: &str, nombre: Option<&str>, ahora: NaiveDateTime) {
        if let Some(d) = self.emparejados.iter_mut().find(|d| d.id == id) {
            if let Some(n) = nombre {
                let n = n.trim();
                if !n.is_empty() {
                    d.nombre = n.to_string();
                }
            }
            d.ultima_sincronizacion = Some(ahora.format(FORMATO_FECHA_HORA).to_string());
            d.ultimo_error = None;
        }
    }

    pub fn anotar_error(&mut self, id: &str, error: &str) {
        if let Some(d) = self.emparejados.iter_mut().find(|d| d.id == id) {
            d.ultimo_error = Some(error.to_string());
        }
    }
}

/// Calcula la ruta de `sync.toml` en la carpeta de configuración dada (que puede
/// ser un directorio o una ruta a otro archivo como `agente.toml`).
pub fn ruta_sync_toml(ruta_config: &Path) -> PathBuf {
    if ruta_config.is_dir() || ruta_config.extension().is_none() {
        ruta_config.join("sync.toml")
    } else {
        ruta_config.with_file_name("sync.toml")
    }
}

/// Calcula la ruta de `sync-estado.json` en la misma carpeta que `sync.toml`.
pub fn ruta_sync_estado(ruta_config: &Path) -> PathBuf {
    ruta_sync_toml(ruta_config).with_file_name("sync-estado.json")
}

/// Ruta por defecto en escritorio (`~/.config/order/sync.toml` o junto a `ORDER_ARCHIVO`).
pub fn ruta_por_defecto() -> PathBuf {
    if let Some(archivo) = std::env::var_os("ORDER_ARCHIVO") {
        if !archivo.is_empty() {
            return Path::new(&archivo).with_file_name("sync.toml");
        }
    }
    let base = dirs_config().unwrap_or_else(|| PathBuf::from("."));
    base.join("order").join("sync.toml")
}

fn dirs_config() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg));
        }
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))
}

/// Lee `sync.toml` o lo crea con una clave nueva si no existe (o si la clave es inválida).
pub fn cargar_o_crear(ruta: &Path, nombre_inicial: Option<&str>) -> Result<SyncConfig, String> {
    if let Ok(texto) = std::fs::read_to_string(ruta) {
        if let Ok(mut cfg) = toml::from_str::<SyncConfig>(&texto) {
            if cfg.secret_key().is_ok() {
                if cfg.nombre_dispositivo.trim().is_empty() {
                    cfg.nombre_dispositivo = nombre_limpio_o_defecto(nombre_inicial);
                    let _ = guardar(ruta, &cfg);
                }
                return Ok(cfg);
            }
        }
    }
    let cfg = SyncConfig::nueva(nombre_inicial);
    guardar(ruta, &cfg)?;
    Ok(cfg)
}

/// Guarda `sync.toml` de forma atómica con permisos `0600`.
pub fn guardar(ruta: &Path, cfg: &SyncConfig) -> Result<(), String> {
    let dir = ruta
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;

    let cuerpo =
        toml::to_string_pretty(cfg).map_err(|e| format!("no se pudo preparar sync.toml: {e}"))?;
    let texto = format!(
        "# Configuración P2P privada de ORDER (clave del nodo y dispositivos emparejados).\n\
         # Este archivo NO se sincroniza ni se comparte. Permisos 600.\n\n{cuerpo}"
    );

    let nombre = ruta
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sync.toml");
    let tmp = dir.join(format!(".{nombre}.{}.tmp", std::process::id()));

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

pub fn ahora_local() -> NaiveDateTime {
    Local::now().naive_local()
}

fn nombre_limpio_o_defecto(nombre: Option<&str>) -> String {
    if let Some(n) = nombre {
        let t = n.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    if let Ok(host) = std::fs::read_to_string("/etc/hostname") {
        let t = host.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    if let Ok(h) = std::env::var("HOSTNAME") {
        let t = h.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    "Este dispositivo".to_string()
}

fn a_hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn desde_hex_32(texto: &str) -> Result<[u8; 32], String> {
    let limpio = texto.trim();
    if limpio.len() != 64 {
        return Err("clave secreta inválida en sync.toml".to_string());
    }
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        let byte = u8::from_str_radix(&limpio[i * 2..i * 2 + 2], 16)
            .map_err(|_| "clave secreta hexadecimal inválida en sync.toml".to_string())?;
        *slot = byte;
    }
    Ok(out)
}
