//! Configuración privada de Moodle (`moodle.toml`).
//!
//! Vive junto a `agente.toml` (escritorio: `~/.config/order/moodle.toml` o
//! junto a `ORDER_ARCHIVO`; Android: `filesDir/moodle.toml`). Se escribe con
//! permisos `600`. **La contraseña nunca se guarda**; solo el token.

use chrono::{Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

const FORMATO_SYNC: &str = "%Y-%m-%dT%H:%M:%S";

/// Datos guardados en `moodle.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MoodleConfig {
    pub sitio: String,
    pub token: String,
    pub usuario: String,
    #[serde(default)]
    pub userid: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultima_sincronizacion: Option<String>,
    #[serde(default)]
    pub token_invalido: bool,
}

impl MoodleConfig {
    /// ¿Hay un sitio y un token guardados?
    pub fn conectado(&self) -> bool {
        !self.sitio.trim().is_empty() && !self.token.trim().is_empty()
    }

    /// Marca el instante actual como última sincronización exitosa.
    pub fn anotar_sincronizacion(&mut self, ahora: NaiveDateTime) {
        self.ultima_sincronizacion = Some(ahora.format(FORMATO_SYNC).to_string());
        self.token_invalido = false;
    }

    pub fn ultima_fecha_hora(&self) -> Option<NaiveDateTime> {
        self.ultima_sincronizacion
            .as_deref()
            .and_then(|s| NaiveDateTime::parse_from_str(s.trim(), FORMATO_SYNC).ok())
    }
}

/// Estado listo para mostrar en la interfaz (sin exponer nunca el token).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstadoMoodle {
    pub conectado: bool,
    pub sitio: String,
    pub usuario: String,
    pub texto_estado: String,
    pub requiere_reingreso: bool,
}

/// Ruta por defecto (`~/.config/order/moodle.toml` o junto a `ORDER_ARCHIVO`).
pub fn ruta() -> PathBuf {
    if let Some(archivo) = std::env::var_os("ORDER_ARCHIVO") {
        if !archivo.is_empty() {
            return junto_a(Path::new(&archivo));
        }
    }
    let base = dirs::config_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("order").join("moodle.toml")
}

/// `moodle.toml` en la misma carpeta que otro archivo de configuración o datos.
pub fn junto_a(archivo: &Path) -> PathBuf {
    archivo.with_file_name("moodle.toml")
}

/// Lee `moodle.toml`; si no existe o está dañado devuelve el estado vacío.
pub fn cargar_de(ruta: &Path) -> MoodleConfig {
    std::fs::read_to_string(ruta)
        .ok()
        .and_then(|texto| toml::from_str::<MoodleConfig>(&texto).ok())
        .unwrap_or_default()
}

/// Escribe `moodle.toml` de forma atómica y con permisos `600`.
pub fn guardar_en(ruta: &Path, config: &MoodleConfig) -> Result<(), String> {
    let dir = ruta
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;

    let cuerpo = toml::to_string_pretty(config)
        .map_err(|e| format!("no se pudo preparar moodle.toml: {e}"))?;
    let texto = format!(
        "# Conexión de ORDER con Moodle (solo token, nunca contraseña).\n\
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

/// Borra `moodle.toml` al desconectar (no toca las tareas de `order.toml`).
pub fn borrar_en(ruta: &Path) -> Result<(), String> {
    match std::fs::remove_file(ruta) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("no se pudo desconectar Moodle: {e}")),
    }
}

/// Construye el resumen visible del estado actual de Moodle.
pub fn estado_de(ruta: &Path) -> EstadoMoodle {
    estado_en(ruta, Local::now().naive_local())
}

pub fn estado_en(ruta: &Path, ahora: NaiveDateTime) -> EstadoMoodle {
    let cfg = cargar_de(ruta);
    if !cfg.conectado() {
        return EstadoMoodle {
            conectado: false,
            sitio: if cfg.sitio.trim().is_empty() {
                "micampus.unal.edu.co".to_string()
            } else {
                cfg.sitio
            },
            usuario: String::new(),
            texto_estado: String::new(),
            requiere_reingreso: false,
        };
    }

    let nombre = if cfg.usuario.trim().is_empty() {
        "Moodle".to_string()
    } else {
        cfg.usuario.trim().to_string()
    };

    let texto_estado = if cfg.token_invalido {
        "Vuelve a iniciar sesión en Moodle.".to_string()
    } else {
        match cfg.ultima_fecha_hora() {
            Some(ultima) => format!(
                "Conectado como {nombre} · última sincronización {}",
                texto_hace(ultima, ahora)
            ),
            None => format!("Conectado como {nombre} · sin sincronizar aún"),
        }
    };

    EstadoMoodle {
        conectado: true,
        sitio: cfg.sitio,
        usuario: nombre,
        texto_estado,
        requiere_reingreso: cfg.token_invalido,
    }
}

/// Texto corto en español ("hace 5 min", "hace 2 h", "justo ahora").
pub fn texto_hace(antes: NaiveDateTime, ahora: NaiveDateTime) -> String {
    let segs = (ahora - antes).num_seconds().max(0);
    if segs < 60 {
        "justo ahora".to_string()
    } else if segs < 3600 {
        format!("hace {} min", segs / 60)
    } else if segs < 86_400 {
        format!("hace {} h", segs / 3600)
    } else {
        let dias = segs / 86_400;
        if dias == 1 {
            "hace 1 día".to_string()
        } else {
            format!("hace {dias} días")
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn guarda_carga_y_borra_sin_contrasena() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ruta = dir.path().join("moodle.toml");

        let mut cfg = MoodleConfig {
            sitio: "micampus.unal.edu.co".into(),
            token: "tok-secreto".into(),
            usuario: "Ana Estudiante".into(),
            userid: 42,
            ultima_sincronizacion: None,
            token_invalido: false,
        };
        let ahora = NaiveDate::from_ymd_opt(2026, 11, 22)
            .unwrap()
            .and_hms_opt(15, 0, 0)
            .unwrap();
        cfg.anotar_sincronizacion(ahora);
        guardar_en(&ruta, &cfg).expect("guardar");

        let crudo = std::fs::read_to_string(&ruta).expect("leer");
        assert!(!crudo.contains("password"), "{crudo}");
        assert!(!crudo.contains("contrasena"), "{crudo}");

        let leido = cargar_de(&ruta);
        assert_eq!(leido, cfg);

        let est = estado_en(&ruta, ahora + chrono::TimeDelta::minutes(12));
        assert!(est.conectado);
        assert!(!est.requiere_reingreso);
        assert!(est.texto_estado.contains("Ana Estudiante"), "{}", est.texto_estado);
        assert!(est.texto_estado.contains("hace 12 min"), "{}", est.texto_estado);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(&ruta).unwrap().permissions().mode() & 0o777;
            assert_eq!(modo, 0o600);
        }

        borrar_en(&ruta).expect("borrar");
        assert!(!ruta.exists());
        assert!(!estado_en(&ruta, ahora).conectado);
    }
}
