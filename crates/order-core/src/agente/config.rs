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
        Proveedor::Gemini
    }
}

impl Proveedor {
    /// Los cuatro proveedores en el orden de las fichas de Ajustes.
    pub const TODOS: [Proveedor; 4] = [
        Proveedor::Gemini,
        Proveedor::DeepSeek,
        Proveedor::Claude,
        Proveedor::OpenAICompatible,
    ];

    /// Código de la ficha en Ajustes (el orden de `TODOS`).
    pub fn codigo(self) -> i32 {
        match self {
            Proveedor::Gemini => 0,
            Proveedor::DeepSeek => 1,
            Proveedor::Claude => 2,
            Proveedor::OpenAICompatible => 3,
        }
    }

    pub fn de_codigo(codigo: i32) -> Proveedor {
        *Proveedor::TODOS
            .get(codigo.max(0) as usize)
            .unwrap_or(&Proveedor::Gemini)
    }

    /// Etiqueta corta de la ficha (solo Gemini muestra «Gratis»).
    pub fn etiqueta(self) -> Option<&'static str> {
        match self {
            Proveedor::Gemini => Some("Gratis"),
            _ => None,
        }
    }

    /// Nombre de la ficha. El proveedor compatible es "Otro": ahí el usuario
    /// escribe su propia URL y su propio modelo.
    pub fn nombre(self) -> &'static str {
        match self {
            Proveedor::DeepSeek => "DeepSeek",
            Proveedor::Gemini => "Gemini",
            Proveedor::Claude => "Claude",
            Proveedor::OpenAICompatible => "Otro",
        }
    }

    /// Página donde el proveedor entrega la clave. "Otro" no tiene: no hay
    /// dónde pedirla.
    pub fn url_clave(self) -> Option<&'static str> {
        match self {
            Proveedor::DeepSeek => Some("https://platform.deepseek.com/api_keys"),
            Proveedor::Gemini => Some("https://aistudio.google.com/apikey"),
            Proveedor::Claude => Some("https://console.anthropic.com/settings/keys"),
            Proveedor::OpenAICompatible => None,
        }
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

/// Proveedor de la transcripción (Whisper por API).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VozProveedor {
    #[serde(rename = "openai", alias = "OpenAI")]
    OpenAI,
    #[serde(rename = "groq", alias = "Groq")]
    Groq,
}

impl Default for VozProveedor {
    fn default() -> Self {
        VozProveedor::OpenAI
    }
}

impl VozProveedor {
    /// Las dos fichas en el orden de Ajustes.
    pub const TODOS: [VozProveedor; 2] = [VozProveedor::OpenAI, VozProveedor::Groq];

    pub fn codigo(self) -> i32 {
        match self {
            VozProveedor::OpenAI => 0,
            VozProveedor::Groq => 1,
        }
    }

    pub fn de_codigo(codigo: i32) -> VozProveedor {
        *VozProveedor::TODOS
            .get(codigo.max(0) as usize)
            .unwrap_or(&VozProveedor::OpenAI)
    }

    pub fn nombre(self) -> &'static str {
        match self {
            VozProveedor::OpenAI => "OpenAI",
            VozProveedor::Groq => "Groq",
        }
    }

    /// URL base y modelo de Whisper para cada uno.
    pub fn por_defecto(self) -> (&'static str, &'static str) {
        match self {
            VozProveedor::OpenAI => ("https://api.openai.com/v1", "whisper-1"),
            VozProveedor::Groq => ("https://api.groq.com/openai/v1", "whisper-large-v3-turbo"),
        }
    }

    /// Página donde se pide la clave de la voz.
    pub fn url_clave(self) -> &'static str {
        match self {
            VozProveedor::OpenAI => "https://platform.openai.com/api-keys",
            VozProveedor::Groq => "https://console.groq.com/keys",
        }
    }
}

/// Voz por Whisper API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Voz {
    pub proveedor: VozProveedor,
    pub url_base: String,
    pub modelo: String,
    pub api_key: String,
}

impl Default for Voz {
    fn default() -> Self {
        let (url_base, modelo) = VozProveedor::default().por_defecto();
        Voz {
            proveedor: VozProveedor::default(),
            url_base: url_base.to_string(),
            modelo: modelo.to_string(),
            api_key: String::new(),
        }
    }
}

impl Voz {
    /// Cambia de proveedor y pone su URL y su modelo por defecto.
    pub fn con_proveedor(mut self, proveedor: VozProveedor) -> Voz {
        let (url_base, modelo) = proveedor.por_defecto();
        self.proveedor = proveedor;
        self.url_base = url_base.to_string();
        self.modelo = modelo.to_string();
        self
    }

    /// ¿Se puede transcribir? URL y modelo siempre tienen valor por defecto,
    /// así que en la práctica depende de la clave.
    pub fn lista(&self) -> bool {
        !self.api_key.trim().is_empty() && !self.url_base.trim().is_empty()
    }
}

/// Límite de contexto por defecto en tokens aproximados antes de compactar.
pub const LIMITE_CONTEXTO_POR_DEFECTO: u32 = 60_000;

fn limite_contexto_defecto() -> u32 {
    LIMITE_CONTEXTO_POR_DEFECTO
}

/// Lo que se guarda en `agente.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgenteConfig {
    pub proveedor: Proveedor,
    pub url_base: String,
    pub modelo: String,
    pub api_key: String,
    pub ve_imagenes: bool,
    #[serde(default = "limite_contexto_defecto")]
    pub limite_contexto_tokens: u32,
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
            ve_imagenes: false,
            limite_contexto_tokens: LIMITE_CONTEXTO_POR_DEFECTO,
            voz: Voz::default(),
        }
    }
}

impl AgenteConfig {
    /// ¿Hay clave para hablar con el modelo?
    pub fn lista(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    /// ¿El proveedor / modelo elegido recibe imágenes directamente?
    /// Claude y Gemini sí; DeepSeek no; «Otro» depende de `ve_imagenes`.
    pub fn ve_imagenes(&self) -> bool {
        match self.proveedor {
            Proveedor::Claude | Proveedor::Gemini => true,
            Proveedor::DeepSeek => false,
            Proveedor::OpenAICompatible => self.ve_imagenes,
        }
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

/// Lee la configuración de una ruta concreta. Si el archivo no existe o está
/// roto, devuelve los valores por defecto.
pub fn cargar_de(ruta: &Path) -> AgenteConfig {
    std::fs::read_to_string(ruta)
        .ok()
        .and_then(|texto| toml::from_str::<AgenteConfig>(&texto).ok())
        .unwrap_or_default()
}

/// Escribe la configuración con permisos `600`.
pub fn guardar(config: &AgenteConfig) -> Result<(), String> {
    guardar_en(&ruta(), config)
}

/// Escribe la configuración en una ruta concreta con permisos `600`.
pub fn guardar_en(ruta: &Path, config: &AgenteConfig) -> Result<(), String> {
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

/// Comprueba si un texto parece una clave de Gemini / Google AI Studio
/// (empieza por `AIza`, sin espacios ni saltos de línea).
pub fn parece_clave_gemini(texto: &str) -> bool {
    let limpio = texto.trim();
    limpio.starts_with("AIza")
        && limpio.len() >= 10
        && limpio
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Textos compartidos entre escritorio y Android para la tarjeta de conexión guiada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextosConexionAgente {
    pub titulo: String,
    pub paso_1: String,
    pub paso_2: String,
    pub paso_3: String,
    pub boton_abrir_studio: String,
    pub url_studio: String,
    pub boton_pegar: String,
    pub etiqueta_clave: String,
    pub pista_clave: String,
    pub boton_conectar: String,
    pub conectando: String,
    pub enlace_otro_proveedor: String,
    pub portapapeles_invalido: String,
    pub clave_vacia: String,
    pub oferta_portapapeles: String,
    pub conectado: String,
    pub clave_invalida: String,
    pub sin_internet: String,
}

/// Textos en español de la tarjeta «Conecta el agente gratis».
pub fn textos_conexion_agente() -> TextosConexionAgente {
    TextosConexionAgente {
        titulo: "Conecta el agente gratis".to_string(),
        paso_1: "1. Toca Abrir Google AI Studio y entra con tu cuenta de Google.".to_string(),
        paso_2: "2. Toca Create API key y copia la clave.".to_string(),
        paso_3: "3. Vuelve aquí y toca Pegar clave.".to_string(),
        boton_abrir_studio: "Abrir Google AI Studio".to_string(),
        url_studio: Proveedor::Gemini
            .url_clave()
            .unwrap_or("https://aistudio.google.com/apikey")
            .to_string(),
        boton_pegar: "Pegar clave".to_string(),
        etiqueta_clave: "API key".to_string(),
        pista_clave: "AIza…".to_string(),
        boton_conectar: "Conectar".to_string(),
        conectando: "Conectando…".to_string(),
        enlace_otro_proveedor: "Usar otro proveedor".to_string(),
        portapapeles_invalido: "Lo que copiaste no parece una clave de Google.".to_string(),
        clave_vacia: "Pega o escribe tu clave de Google.".to_string(),
        oferta_portapapeles: "¿Usar la clave que copiaste?".to_string(),
        conectado: "Listo, el agente está conectado.".to_string(),
        clave_invalida: "Esa clave no funciona; cópiala otra vez.".to_string(),
        sin_internet: "Sin internet; inténtalo luego.".to_string(),
    }
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
        assert_eq!(config.proveedor, Proveedor::Gemini);
        assert_eq!(
            config.url_base,
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
        assert_eq!(config.modelo, "gemini-flash-latest");
        assert!(!config.lista(), "sin clave no está listo");
        assert_eq!(config.voz.url_base, "https://api.openai.com/v1");
        assert_eq!(config.voz.modelo, "whisper-1");

        let deepseek = AgenteConfig::default().con_proveedor(Proveedor::DeepSeek);
        assert_eq!(deepseek.url_base, "https://api.deepseek.com");
        assert_eq!(deepseek.modelo, "deepseek-v4-flash");

        let claude = AgenteConfig::default().con_proveedor(Proveedor::Claude);
        assert_eq!(claude.url_base, "https://api.anthropic.com");
        assert_eq!(claude.modelo, "claude-opus-5-5");
    }

    #[test]
    fn archivo_incompleto_usa_defectos() {
        let (_dir, ruta) = temporal();
        std::fs::write(&ruta, "api_key = \"AIzaPrueba12345\"\n").expect("escribir");

        let config = cargar_de(&ruta);
        assert_eq!(config.api_key, "AIzaPrueba12345");
        assert_eq!(config.proveedor, Proveedor::Gemini);
        assert_eq!(config.modelo, "gemini-flash-latest");
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
    fn toml_viejo_con_deepseek_se_respeta() {
        let (_dir, ruta) = temporal();
        std::fs::write(
            &ruta,
            "proveedor = \"deepseek\"\n\
             url_base = \"https://api.deepseek.com\"\n\
             modelo = \"deepseek-v4-flash\"\n\
             api_key = \"sk-antigua-deepseek\"\n",
        )
        .expect("escribir");

        let config = cargar_de(&ruta);
        assert_eq!(config.proveedor, Proveedor::DeepSeek);
        assert_eq!(config.url_base, "https://api.deepseek.com");
        assert_eq!(config.modelo, "deepseek-v4-flash");
        assert_eq!(config.api_key, "sk-antigua-deepseek");
        assert!(config.lista());
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
            ve_imagenes: false,
            limite_contexto_tokens: 45_000,
            voz: Voz {
                proveedor: VozProveedor::Groq,
                url_base: "https://api.groq.com/openai/v1".into(),
                modelo: "whisper-large-v3-turbo".into(),
                api_key: "gsk-prueba".into(),
            },
        };
        guardar_en(&ruta, &config).expect("guardar");
        assert_eq!(cargar_de(&ruta), config);
    }

    #[test]
    fn ve_imagenes_segun_proveedor() {
        assert!(AgenteConfig::default().con_proveedor(Proveedor::Claude).ve_imagenes());
        assert!(AgenteConfig::default().con_proveedor(Proveedor::Gemini).ve_imagenes());
        assert!(!AgenteConfig::default().con_proveedor(Proveedor::DeepSeek).ve_imagenes());

        let mut otro = AgenteConfig::default().con_proveedor(Proveedor::OpenAICompatible);
        assert!(!otro.ve_imagenes());
        otro.ve_imagenes = true;
        assert!(otro.ve_imagenes());
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
    fn agente_toml_junto_a_order_archivo() {
        let dir = tempfile::tempdir().expect("directorio temporal");
        let order_toml = dir.path().join("order.toml");
        let anterior = std::env::var_os("ORDER_ARCHIVO");
        unsafe {
            std::env::set_var("ORDER_ARCHIVO", &order_toml);
        }
        let calculada = ruta();
        unsafe {
            match anterior {
                Some(valor) => std::env::set_var("ORDER_ARCHIVO", valor),
                None => std::env::remove_var("ORDER_ARCHIVO"),
            }
        }
        assert_eq!(calculada, dir.path().join("agente.toml"));
    }

    #[test]
    fn codigos_de_las_fichas() {
        assert_eq!(
            Proveedor::TODOS,
            [
                Proveedor::Gemini,
                Proveedor::DeepSeek,
                Proveedor::Claude,
                Proveedor::OpenAICompatible,
            ]
        );
        for proveedor in Proveedor::TODOS {
            assert_eq!(Proveedor::de_codigo(proveedor.codigo()), proveedor);
        }
        assert_eq!(Proveedor::de_codigo(99), Proveedor::Gemini);
        assert_eq!(Proveedor::Gemini.etiqueta(), Some("Gratis"));
        assert_eq!(Proveedor::DeepSeek.etiqueta(), None);
        assert_eq!(Proveedor::Claude.etiqueta(), None);
        assert_eq!(Proveedor::OpenAICompatible.etiqueta(), None);
    }

    #[test]
    fn parece_clave_gemini_valida_y_rechaza() {
        assert!(parece_clave_gemini("AIzaSyPruebaDeClave123456"));
        assert!(parece_clave_gemini("  AIzaSyPrueba-De_Clave123 \n"));
        assert!(!parece_clave_gemini(""));
        assert!(!parece_clave_gemini("AIza"));
        assert!(!parece_clave_gemini("sk-1234567890abcdef"));
        assert!(!parece_clave_gemini("AIza con espacios largos"));
        assert!(!parece_clave_gemini("hola mundo"));

        let textos = textos_conexion_agente();
        assert_eq!(textos.titulo, "Conecta el agente gratis");
        assert_eq!(textos.url_studio, "https://aistudio.google.com/apikey");
    }

    /// Cada ficha trae su URL base, su modelo y la página de la clave; "Otro"
    /// no tiene página porque no hay proveedor detrás.
    #[test]
    fn cada_ficha_trae_sus_valores_y_su_clave() {
        assert_eq!(
            Proveedor::DeepSeek.por_defecto(),
            ("https://api.deepseek.com", "deepseek-v4-flash")
        );
        assert_eq!(
            Proveedor::DeepSeek.url_clave(),
            Some("https://platform.deepseek.com/api_keys")
        );
        assert_eq!(
            Proveedor::Gemini.url_clave(),
            Some("https://aistudio.google.com/apikey")
        );
        assert_eq!(
            Proveedor::Claude.url_clave(),
            Some("https://console.anthropic.com/settings/keys")
        );
        assert_eq!(Proveedor::OpenAICompatible.nombre(), "Otro");
        assert_eq!(Proveedor::OpenAICompatible.url_clave(), None);
        assert_eq!(Proveedor::OpenAICompatible.por_defecto(), ("", ""));
    }

    /// La voz tiene dos fichas con su URL, su modelo y su página de clave.
    #[test]
    fn la_voz_trae_sus_dos_fichas() {
        for proveedor in VozProveedor::TODOS {
            assert_eq!(VozProveedor::de_codigo(proveedor.codigo()), proveedor);
        }
        assert_eq!(VozProveedor::de_codigo(9), VozProveedor::OpenAI);

        let openai = Voz::default();
        assert_eq!(openai.proveedor, VozProveedor::OpenAI);
        assert_eq!(openai.url_base, "https://api.openai.com/v1");
        assert_eq!(openai.modelo, "whisper-1");
        assert_eq!(
            VozProveedor::OpenAI.url_clave(),
            "https://platform.openai.com/api-keys"
        );

        let groq = Voz::default().con_proveedor(VozProveedor::Groq);
        assert_eq!(groq.proveedor, VozProveedor::Groq);
        assert_eq!(groq.url_base, "https://api.groq.com/openai/v1");
        assert_eq!(groq.modelo, "whisper-large-v3-turbo");
        assert_eq!(
            VozProveedor::Groq.url_clave(),
            "https://console.groq.com/keys"
        );
    }
}
