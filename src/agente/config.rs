//! Configuración del agente y de la voz: `~/.config/order/agente.toml`.
//!
//! Las claves **no** van en `order.toml`: ese archivo se sincroniza con el
//! celular y puede compartirse. Este otro se escribe con permisos `600` y, si
//! existe `ORDER_ARCHIVO` (copias de prueba), vive junto al archivo elegido.
//!
//! Todos los campos tienen valor por defecto: un archivo incompleto o ausente
//! deja la app funcionando, solo que sin clave hasta que se escriba una.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Proveedor del modelo de texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Proveedor {
    #[serde(rename = "deepseek", alias = "DeepSeek")]
    DeepSeek,
    #[serde(rename = "gemini", alias = "Gemini")]
    Gemini,
    #[serde(rename = "claude", alias = "Claude")]
    Claude,
    #[serde(
        rename = "openai_compatible",
        alias = "openai-compatible",
        alias = "compatible"
    )]
    OpenAICompatible,
}

impl Default for Proveedor {
    fn default() -> Self {
        Proveedor::DeepSeek
    }
}

impl Proveedor {
    /// Los cuatro proveedores en el orden de las fichas de Ajustes.
    pub const TODOS: [Proveedor; 4] = [
        Proveedor::DeepSeek,
        Proveedor::Gemini,
        Proveedor::Claude,
        Proveedor::OpenAICompatible,
    ];

    /// Código de la ficha en Ajustes (el orden de `TODOS`).
    pub fn codigo(self) -> i32 {
        match self {
            Proveedor::DeepSeek => 0,
            Proveedor::Gemini => 1,
            Proveedor::Claude => 2,
            Proveedor::OpenAICompatible => 3,
        }
    }

    pub fn de_codigo(codigo: i32) -> Proveedor {
        *Proveedor::TODOS
            .get(codigo.max(0) as usize)
            .unwrap_or(&Proveedor::DeepSeek)
    }

    /// URL base y modelo por defecto de la sección 12 del diseño.
    pub fn por_defecto(self) -> (&'static str, &'static str) {
        match self {
            Proveedor::DeepSeek => ("https://api.deepseek.com", "deepseek-v4-flash"),
            Proveedor::Gemini => (
                "https://generativelanguage.googleapis.com/v1beta/openai",
                "gemini-flash-latest",
            ),
            Proveedor::Claude => ("https://api.anthropic.com", "claude-opus-5-5"),
            Proveedor::OpenAICompatible => ("", ""),
        }
    }
}

/// Voz por Whisper API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Voz {
    pub url_base: String,
    pub modelo: String,
    pub api_key: String,
}

impl Default for Voz {
    fn default() -> Self {
        Voz {
            url_base: "https://api.openai.com/v1".to_string(),
            modelo: "whisper-1".to_string(),
            api_key: String::new(),
        }
    }
}

/// Lo que se guarda en `agente.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgenteConfig {
    pub proveedor: Proveedor,
    pub url_base: String,
    pub modelo: String,
    pub api_key: String,
    pub voz: Voz,
}

impl Default for AgenteConfig {
    fn default() -> Self {
        let (url_base, modelo) = Proveedor::default().por_defecto();
        AgenteConfig {
            proveedor: Proveedor::default(),
            url_base: url_base.to_string(),
            modelo: modelo.to_string(),
            api_key: String::new(),
            voz: Voz::default(),
        }
    }
}

impl AgenteConfig {
    /// ¿Hay clave para hablar con el modelo?
    pub fn lista(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    /// Cambia de proveedor y pone su URL y su modelo por defecto.
    pub fn con_proveedor(mut self, proveedor: Proveedor) -> AgenteConfig {
        let (url_base, modelo) = proveedor.por_defecto();
        self.proveedor = proveedor;
        self.url_base = url_base.to_string();
        self.modelo = modelo.to_string();
        self
    }
}

/// `~/.config/order/agente.toml` o, con `ORDER_ARCHIVO`, junto a ese archivo.
pub fn ruta() -> PathBuf {
    if let Some(archivo) = std::env::var_os("ORDER_ARCHIVO") {
        if !archivo.is_empty() {
            return junto_a(Path::new(&archivo));
        }
    }
    let base = dirs::config_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("order").join("agente.toml")
}

/// `agente.toml` en la misma carpeta que el archivo de datos.
pub fn junto_a(archivo: &Path) -> PathBuf {
    archivo.with_file_name("agente.toml")
}

/// Lee la configuración. Si el archivo no existe o está roto, devuelve los
/// valores por defecto: sin clave, la app avisa al enviar pero sigue viva.
pub fn cargar() -> AgenteConfig {
    cargar_de(&ruta())
}

fn cargar_de(ruta: &Path) -> AgenteConfig {
    std::fs::read_to_string(ruta)
        .ok()
        .and_then(|texto| toml::from_str::<AgenteConfig>(&texto).ok())
        .unwrap_or_default()
}

/// Escribe la configuración con permisos `600`.
pub fn guardar(config: &AgenteConfig) -> Result<(), String> {
    guardar_en(&ruta(), config)
}

fn guardar_en(ruta: &Path, config: &AgenteConfig) -> Result<(), String> {
    let dir = ruta
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;

    let cuerpo = toml::to_string_pretty(config)
        .map_err(|e| format!("no se pudo preparar agente.toml: {e}"))?;
    let texto = format!(
        "# Agente de ORDER: proveedor, modelo y claves.\n\
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

    // El temporal ya nace 600, pero un archivo previo con otros permisos no
    // debe contagiar al nuevo: se fija otra vez tras el renombrado.
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

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("directorio temporal");
        let ruta = dir.path().join("agente.toml");
        (dir, ruta)
    }

    #[test]
    fn valores_por_defecto() {
        let config = AgenteConfig::default();
        assert_eq!(config.proveedor, Proveedor::DeepSeek);
        assert_eq!(config.url_base, "https://api.deepseek.com");
        assert_eq!(config.modelo, "deepseek-v4-flash");
        assert!(!config.lista(), "sin clave no está listo");
        assert_eq!(config.voz.url_base, "https://api.openai.com/v1");
        assert_eq!(config.voz.modelo, "whisper-1");

        let claude = AgenteConfig::default().con_proveedor(Proveedor::Claude);
        assert_eq!(claude.url_base, "https://api.anthropic.com");
        assert_eq!(claude.modelo, "claude-opus-5-5");
    }

    #[test]
    fn archivo_incompleto_usa_defectos() {
        let (_dir, ruta) = temporal();
        std::fs::write(&ruta, "api_key = \"sk-prueba\"\n").expect("escribir");

        let config = cargar_de(&ruta);
        assert_eq!(config.api_key, "sk-prueba");
        assert_eq!(config.proveedor, Proveedor::DeepSeek);
        assert_eq!(config.modelo, "deepseek-v4-flash");
        assert!(config.lista());

        // Con la sección de voz escrita a medias.
        std::fs::write(
            &ruta,
            "proveedor = \"openai_compatible\"\nurl_base = \"http://localhost:1234/v1\"\n\
             modelo = \"local\"\n\n[voz]\nmodelo = \"whisper-large-v3-turbo\"\n",
        )
        .expect("escribir");
        let config = cargar_de(&ruta);
        assert_eq!(config.proveedor, Proveedor::OpenAICompatible);
        assert_eq!(config.url_base, "http://localhost:1234/v1");
        assert_eq!(config.voz.modelo, "whisper-large-v3-turbo");
        assert_eq!(config.voz.url_base, "https://api.openai.com/v1");
    }

    #[test]
    fn archivo_roto_no_rompe_la_app() {
        let (_dir, ruta) = temporal();
        std::fs::write(&ruta, "[voz\nmodelo = ").expect("escribir basura");
        assert_eq!(cargar_de(&ruta), AgenteConfig::default());
    }

    #[test]
    fn ida_y_vuelta() {
        let (_dir, ruta) = temporal();
        let config = AgenteConfig {
            proveedor: Proveedor::Gemini,
            url_base: "https://generativelanguage.googleapis.com/v1beta/openai".into(),
            modelo: "gemini-flash-latest".into(),
            api_key: "clave de prueba".into(),
            voz: Voz {
                url_base: "https://api.groq.com/openai/v1".into(),
                modelo: "whisper-large-v3-turbo".into(),
                api_key: "gsk-prueba".into(),
            },
        };
        guardar_en(&ruta, &config).expect("guardar");
        assert_eq!(cargar_de(&ruta), config);
    }

    /// La clave no puede quedar legible para los demás usuarios del equipo.
    #[cfg(unix)]
    #[test]
    fn permisos_600() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, ruta) = temporal();
        let config = AgenteConfig {
            api_key: "sk-secreta".into(),
            ..AgenteConfig::default()
        };

        // Un archivo previo con permisos abiertos no debe conservarlos.
        std::fs::write(&ruta, "").expect("escribir");
        std::fs::set_permissions(&ruta, std::fs::Permissions::from_mode(0o644)).expect("permisos");

        guardar_en(&ruta, &config).expect("guardar");
        let modo = std::fs::metadata(&ruta).expect("metadata").permissions().mode();
        assert_eq!(modo & 0o777, 0o600, "permisos del archivo: {modo:o}");
    }

    /// Con ORDER_ARCHIVO, la configuración vive junto a la copia de prueba.
    #[test]
    fn junto_al_archivo_de_prueba() {
        assert_eq!(
            junto_a(Path::new("/tmp/copia.toml")),
            PathBuf::from("/tmp/agente.toml")
        );
        assert_eq!(
            junto_a(Path::new("/tmp/order/order.toml")),
            PathBuf::from("/tmp/order/agente.toml")
        );
    }

    #[test]
    fn codigos_de_las_fichas() {
        for proveedor in Proveedor::TODOS {
            assert_eq!(Proveedor::de_codigo(proveedor.codigo()), proveedor);
        }
        assert_eq!(Proveedor::de_codigo(99), Proveedor::DeepSeek);
    }
}
