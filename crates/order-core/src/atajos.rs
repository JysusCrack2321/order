//! Modelo de atajos de teclado configurables y persistencia en `atajos.toml`.
//!
//! La configuración vive junto a `agente.toml` (`~/.config/order/atajos.toml`,
//! o junto a `ORDER_ARCHIVO`), con permisos `600` y sin sincronizarse entre
//! dispositivos. Solo se guardan los atajos que difieren del valor por defecto.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Acciones de ORDER que se pueden disparar con un atajo de teclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccionAtajo {
    Cerrar,
    Guardar,
    NuevaTarea,
    IrInicio,
    IrDia,
    IrSemana,
    IrPendientes,
    Anterior,
    Siguiente,
    Hoy,
    Agente,
    Voz,
    Sincronizar,
    Ajustes,
    AyudaAtajos,
}

impl AccionAtajo {
    /// Todas las acciones en el orden en que se muestran en Ajustes y en la ayuda.
    pub const TODAS: [AccionAtajo; 15] = [
        AccionAtajo::Cerrar,
        AccionAtajo::Guardar,
        AccionAtajo::NuevaTarea,
        AccionAtajo::IrInicio,
        AccionAtajo::IrDia,
        AccionAtajo::IrSemana,
        AccionAtajo::IrPendientes,
        AccionAtajo::Anterior,
        AccionAtajo::Siguiente,
        AccionAtajo::Hoy,
        AccionAtajo::Agente,
        AccionAtajo::Voz,
        AccionAtajo::Sincronizar,
        AccionAtajo::Ajustes,
        AccionAtajo::AyudaAtajos,
    ];

    /// Identificador estable usado en `atajos.toml`.
    pub fn id(self) -> &'static str {
        match self {
            AccionAtajo::Cerrar => "cerrar",
            AccionAtajo::Guardar => "guardar",
            AccionAtajo::NuevaTarea => "nueva_tarea",
            AccionAtajo::IrInicio => "ir_inicio",
            AccionAtajo::IrDia => "ir_dia",
            AccionAtajo::IrSemana => "ir_semana",
            AccionAtajo::IrPendientes => "ir_pendientes",
            AccionAtajo::Anterior => "anterior",
            AccionAtajo::Siguiente => "siguiente",
            AccionAtajo::Hoy => "hoy",
            AccionAtajo::Agente => "agente",
            AccionAtajo::Voz => "voz",
            AccionAtajo::Sincronizar => "sincronizar",
            AccionAtajo::Ajustes => "ajustes",
            AccionAtajo::AyudaAtajos => "ayuda_atajos",
        }
    }

    /// Busca una acción por su identificador de `atajos.toml`.
    pub fn desde_id(id: &str) -> Option<AccionAtajo> {
        let limpio = id.trim();
        AccionAtajo::TODAS
            .into_iter()
            .find(|accion| accion.id() == limpio)
    }

    /// Nombre corto de la acción para la lista de Ajustes y la ventana de ayuda.
    pub fn nombre(self) -> &'static str {
        match self {
            AccionAtajo::Cerrar => "Cerrar diálogo o quitar foco",
            AccionAtajo::Guardar => "Guardar formulario",
            AccionAtajo::NuevaTarea => "Nueva tarea",
            AccionAtajo::IrInicio => "Ir a Inicio",
            AccionAtajo::IrDia => "Ir a Día",
            AccionAtajo::IrSemana => "Ir a Semana",
            AccionAtajo::IrPendientes => "Ir a Pendientes",
            AccionAtajo::Anterior => "Día o semana anterior",
            AccionAtajo::Siguiente => "Día o semana siguiente",
            AccionAtajo::Hoy => "Ir a hoy",
            AccionAtajo::Agente => "Enfocar cuadro del agente",
            AccionAtajo::Voz => "Grabar o detener voz",
            AccionAtajo::Sincronizar => "Sincronizar",
            AccionAtajo::Ajustes => "Abrir Ajustes",
            AccionAtajo::AyudaAtajos => "Mostrar atajos de teclado",
        }
    }

    /// Combinación de teclado por defecto.
    pub fn por_defecto(self) -> Combinacion {
        match self {
            AccionAtajo::Cerrar => Combinacion::simple(Tecla::Esc),
            AccionAtajo::Guardar => Combinacion::ctrl(Tecla::Caracter('S')),
            AccionAtajo::NuevaTarea => Combinacion::ctrl(Tecla::Caracter('N')),
            AccionAtajo::IrInicio => Combinacion::ctrl(Tecla::Caracter('1')),
            AccionAtajo::IrDia => Combinacion::ctrl(Tecla::Caracter('2')),
            AccionAtajo::IrSemana => Combinacion::ctrl(Tecla::Caracter('3')),
            AccionAtajo::IrPendientes => Combinacion::ctrl(Tecla::Caracter('4')),
            AccionAtajo::Anterior => Combinacion::simple(Tecla::Izquierda),
            AccionAtajo::Siguiente => Combinacion::simple(Tecla::Derecha),
            AccionAtajo::Hoy => Combinacion::simple(Tecla::Caracter('T')),
            AccionAtajo::Agente => Combinacion::ctrl(Tecla::Caracter('K')),
            AccionAtajo::Voz => Combinacion::ctrl(Tecla::Caracter('M')),
            AccionAtajo::Sincronizar => Combinacion::ctrl(Tecla::Caracter('R')),
            AccionAtajo::Ajustes => Combinacion::ctrl(Tecla::Caracter(',')),
            AccionAtajo::AyudaAtajos => Combinacion::simple(Tecla::Caracter('?')),
        }
    }
}

/// Modificadores activos en una pulsación o combinación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modificadores {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modificadores {
    pub const NINGUNO: Modificadores = Modificadores {
        ctrl: false,
        shift: false,
        alt: false,
        meta: false,
    };

    pub const CTRL: Modificadores = Modificadores {
        ctrl: true,
        shift: false,
        alt: false,
        meta: false,
    };

    pub fn new(ctrl: bool, shift: bool, alt: bool, meta: bool) -> Self {
        Modificadores {
            ctrl,
            shift,
            alt,
            meta,
        }
    }

    /// ¿Hay algún modificador distinto de Shift (Ctrl, Alt o Meta)?
    pub fn tiene_comando(self) -> bool {
        self.ctrl || self.alt || self.meta
    }

    /// ¿Hay algún modificador activo?
    pub fn tiene_alguno(self) -> bool {
        self.ctrl || self.shift || self.alt || self.meta
    }
}

/// Tecla normalizada de una combinación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tecla {
    Esc,
    Enter,
    Tab,
    Espacio,
    Retroceso,
    Supr,
    Izquierda,
    Derecha,
    Arriba,
    Abajo,
    Inicio,
    Fin,
    RePag,
    AvPag,
    Insert,
    Funcion(u8),
    Caracter(char),
}

impl Tecla {
    /// Convierte el texto de tecla (Slint o nombre legible) a una tecla normalizada.
    /// Devuelve `None` si es vacía o corresponde únicamente a una tecla modificadora.
    pub fn desde_texto(texto: &str) -> Option<Tecla> {
        if texto.is_empty() {
            return None;
        }
        // Códigos especiales de Slint / Unicode antes de recortar espacios,
        // porque ' ' es la tecla Espacio y '\t' / '\n' son teclas de control.
        let mut chars = texto.chars();
        if let (Some(ch), None) = (chars.next(), chars.next()) {
            match ch {
                '\u{0010}'..='\u{0018}' => return None, // Shift, Control, Alt, AltGr, CapsLock, Meta
                '\u{001b}' => return Some(Tecla::Esc),
                '\u{000a}' | '\r' => return Some(Tecla::Enter),
                '\u{0009}' => return Some(Tecla::Tab),
                ' ' => return Some(Tecla::Espacio),
                '\u{0008}' => return Some(Tecla::Retroceso),
                '\u{007f}' => return Some(Tecla::Supr),
                '\u{F700}' | '↑' => return Some(Tecla::Arriba),
                '\u{F701}' | '↓' => return Some(Tecla::Abajo),
                '\u{F702}' | '←' => return Some(Tecla::Izquierda),
                '\u{F703}' | '→' => return Some(Tecla::Derecha),
                '\u{F727}' => return Some(Tecla::Insert),
                '\u{F729}' => return Some(Tecla::Inicio),
                '\u{F72B}' => return Some(Tecla::Fin),
                '\u{F72C}' => return Some(Tecla::RePag),
                '\u{F72D}' => return Some(Tecla::AvPag),
                c if ('\u{F704}'..='\u{F71B}').contains(&c) => {
                    let n = (c as u32 - '\u{F704}' as u32 + 1) as u8;
                    return Some(Tecla::Funcion(n));
                }
                c if c.is_control() || ('\u{F700}'..='\u{F7FF}').contains(&c) => return None,
                c => return Some(Tecla::Caracter(normalizar_caracter(c))),
            }
        }

        let limpio = texto.trim();
        if limpio.is_empty() {
            return None;
        }
        let mut chars_limpios = limpio.chars();
        if let (Some(ch), None) = (chars_limpios.next(), chars_limpios.next()) {
            return match ch {
                c if c.is_control() => None,
                c => Some(Tecla::Caracter(normalizar_caracter(c))),
            };
        }

        let minus = limpio.to_lowercase();
        match minus.as_str() {
            "esc" | "escape" => Some(Tecla::Esc),
            "enter" | "return" | "intro" => Some(Tecla::Enter),
            "tab" | "tabulador" => Some(Tecla::Tab),
            "space" | "espacio" => Some(Tecla::Espacio),
            "backspace" | "retroceso" => Some(Tecla::Retroceso),
            "delete" | "del" | "supr" | "suprimir" => Some(Tecla::Supr),
            "left" | "arrowleft" | "leftarrow" | "izquierda" | "izq" => Some(Tecla::Izquierda),
            "right" | "arrowright" | "rightarrow" | "derecha" | "der" => Some(Tecla::Derecha),
            "up" | "arrowup" | "uparrow" | "arriba" => Some(Tecla::Arriba),
            "down" | "arrowdown" | "downarrow" | "abajo" => Some(Tecla::Abajo),
            "home" | "inicio" => Some(Tecla::Inicio),
            "end" | "fin" => Some(Tecla::Fin),
            "pageup" | "repag" | "repág" => Some(Tecla::RePag),
            "pagedown" | "avpag" | "avpág" => Some(Tecla::AvPag),
            "insert" | "ins" => Some(Tecla::Insert),
            _ => {
                if let Some(resto) = minus.strip_prefix('f') {
                    if let Ok(n) = resto.parse::<u8>() {
                        if (1..=24).contains(&n) {
                            return Some(Tecla::Funcion(n));
                        }
                    }
                }
                None
            }
        }
    }

    /// Etiqueta legible para mostrar al usuario y guardar en `atajos.toml`.
    pub fn etiqueta(self) -> String {
        match self {
            Tecla::Esc => "Esc".to_string(),
            Tecla::Enter => "Enter".to_string(),
            Tecla::Tab => "Tab".to_string(),
            Tecla::Espacio => "Espacio".to_string(),
            Tecla::Retroceso => "Retroceso".to_string(),
            Tecla::Supr => "Supr".to_string(),
            Tecla::Izquierda => "←".to_string(),
            Tecla::Derecha => "→".to_string(),
            Tecla::Arriba => "↑".to_string(),
            Tecla::Abajo => "↓".to_string(),
            Tecla::Inicio => "Inicio".to_string(),
            Tecla::Fin => "Fin".to_string(),
            Tecla::RePag => "RePág".to_string(),
            Tecla::AvPag => "AvPág".to_string(),
            Tecla::Insert => "Insert".to_string(),
            Tecla::Funcion(n) => format!("F{n}"),
            Tecla::Caracter(c) => c.to_string(),
        }
    }
}

fn normalizar_caracter(c: char) -> char {
    if c.is_ascii_alphabetic() {
        c.to_ascii_uppercase()
    } else {
        c
    }
}

/// Símbolos cuya escritura física ya implica la tecla Shift (por ejemplo `?`).
/// Para ellos, `Shift` se absorbe al construir la combinación desde un evento.
fn es_simbolo_con_shift_implicito(c: char) -> bool {
    matches!(
        c,
        '?' | '!'
            | '@'
            | '#'
            | '$'
            | '%'
            | '^'
            | '&'
            | '*'
            | '('
            | ')'
            | '_'
            | '+'
            | '{'
            | '}'
            | '|'
            | ':'
            | '"'
            | '<'
            | '>'
            | '~'
            | '¿'
            | '¡'
    )
}

/// Combinación de modificadores y tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Combinacion {
    pub modificadores: Modificadores,
    pub tecla: Tecla,
}

impl Combinacion {
    pub fn simple(tecla: Tecla) -> Self {
        Combinacion {
            modificadores: Modificadores::NINGUNO,
            tecla,
        }
    }

    pub fn ctrl(tecla: Tecla) -> Self {
        Combinacion {
            modificadores: Modificadores::CTRL,
            tecla,
        }
    }

    pub fn new(mut modificadores: Modificadores, tecla: Tecla) -> Self {
        if let Tecla::Caracter(c) = tecla {
            if es_simbolo_con_shift_implicito(c) {
                modificadores.shift = false;
            }
        }
        Combinacion {
            modificadores,
            tecla,
        }
    }

    /// Construye una combinación a partir de un evento de teclado.
    /// Devuelve `None` si el evento corresponde solo a un modificador o no es reconocible.
    pub fn desde_evento(tecla: &str, modificadores: Modificadores) -> Option<Self> {
        let tecla = Tecla::desde_texto(tecla)?;
        Some(Combinacion::new(modificadores, tecla))
    }

    /// Parsea textos como `"Ctrl+Shift+K"`, `"Esc"`, `"←"`, `"Ctrl+,"` o `"?"`.
    pub fn parsear(texto: &str) -> Result<Self, String> {
        let recortado = texto.trim();
        if recortado.is_empty() {
            return Err("el atajo no puede estar vacío".to_string());
        }
        if recortado == "+" {
            return Ok(Combinacion::simple(Tecla::Caracter('+')));
        }

        // Si termina en "++" (por ejemplo "Ctrl++"), la tecla final es '+'.
        let (prefijo, tecla_mas) = if let Some(antes) = recortado.strip_suffix("++") {
            (antes, true)
        } else {
            (recortado, false)
        };

        let partes: Vec<&str> = prefijo.split('+').map(str::trim).collect();
        if partes.iter().any(|p| p.is_empty()) {
            return Err(format!("combinación inválida: «{recortado}»"));
        }

        let mut mods = Modificadores::NINGUNO;
        let modificadores_partes = if tecla_mas {
            &partes[..]
        } else {
            &partes[..partes.len() - 1]
        };

        for parte in modificadores_partes {
            match parte.to_lowercase().as_str() {
                "ctrl" | "control" => mods.ctrl = true,
                "shift" | "mayus" | "mayús" => mods.shift = true,
                "alt" => mods.alt = true,
                "meta" | "super" | "win" | "cmd" => mods.meta = true,
                otra => return Err(format!("modificador desconocido: «{otra}»")),
            }
        }

        let tecla = if tecla_mas {
            Tecla::Caracter('+')
        } else {
            let ultima = partes[partes.len() - 1];
            Tecla::desde_texto(ultima)
                .ok_or_else(|| format!("tecla desconocida: «{ultima}»"))?
        };

        Ok(Combinacion::new(mods, tecla))
    }

    /// Formato canónico legible (p. ej. `"Ctrl+Shift+K"`).
    pub fn formatear(self) -> String {
        let mut partes: Vec<String> = Vec::with_capacity(5);
        if self.modificadores.ctrl {
            partes.push("Ctrl".to_string());
        }
        if self.modificadores.alt {
            partes.push("Alt".to_string());
        }
        if self.modificadores.shift {
            partes.push("Shift".to_string());
        }
        if self.modificadores.meta {
            partes.push("Meta".to_string());
        }
        partes.push(self.tecla.etiqueta());
        partes.join("+")
    }

    /// Indica si la combinación puede ejecutarse mientras el foco está en un campo de texto.
    pub fn permitida_escribiendo(self) -> bool {
        if self.modificadores.tiene_comando() {
            return true;
        }
        matches!(self.tecla, Tecla::Esc) && !self.modificadores.shift
    }
}

impl fmt::Display for Combinacion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.formatear())
    }
}

/// Mapa completo de atajos de teclado de ORDER.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtajosConfig {
    mapa: BTreeMap<AccionAtajo, Combinacion>,
}

impl Default for AtajosConfig {
    fn default() -> Self {
        let mut mapa = BTreeMap::new();
        for accion in AccionAtajo::TODAS {
            mapa.insert(accion, accion.por_defecto());
        }
        AtajosConfig { mapa }
    }
}

impl AtajosConfig {
    /// Combinación actualmente asignada a una acción.
    pub fn combinacion(&self, accion: AccionAtajo) -> Combinacion {
        self.mapa
            .get(&accion)
            .copied()
            .unwrap_or_else(|| accion.por_defecto())
    }

    /// Lista ordenada de todas las acciones con su combinación actual.
    pub fn entradas(&self) -> Vec<(AccionAtajo, Combinacion)> {
        AccionAtajo::TODAS
            .into_iter()
            .map(|accion| (accion, self.combinacion(accion)))
            .collect()
    }

    /// Acción que usa actualmente la combinación dada, si existe.
    pub fn accion_de(&self, combinacion: Combinacion) -> Option<AccionAtajo> {
        AccionAtajo::TODAS
            .into_iter()
            .find(|&accion| self.combinacion(accion) == combinacion)
    }

    /// Comprueba si asignar `nueva` a `accion` choca con otra acción.
    /// Devuelve `Some(otra)` si otra acción ya usa `nueva`.
    pub fn choque(&self, accion: AccionAtajo, nueva: Combinacion) -> Option<AccionAtajo> {
        AccionAtajo::TODAS
            .into_iter()
            .find(|&otra| otra != accion && self.combinacion(otra) == nueva)
    }

    /// Asigna `nueva` a `accion`. Si otra acción ya usaba `nueva`, intercambia
    /// ambas combinaciones y devuelve la acción con la que se intercambió.
    pub fn asignar(&mut self, accion: AccionAtajo, nueva: Combinacion) -> Option<AccionAtajo> {
        let anterior = self.combinacion(accion);
        if anterior == nueva {
            return None;
        }
        let choque = self.choque(accion, nueva);
        if let Some(otra) = choque {
            self.mapa.insert(otra, anterior);
        }
        self.mapa.insert(accion, nueva);
        choque
    }

    /// Restaura todas las combinaciones a sus valores por defecto.
    pub fn restaurar_por_defecto(&mut self) {
        *self = AtajosConfig::default();
    }

    /// Resuelve qué acción corresponde a una pulsación de teclado.
    ///
    /// Si `escribiendo` es `true`, las combinaciones sin modificadores de comando
    /// (`Ctrl`, `Alt`, `Meta`) no actúan, salvo `Esc`.
    pub fn accion_para(
        &self,
        tecla: &str,
        modificadores: Modificadores,
        escribiendo: bool,
    ) -> Option<AccionAtajo> {
        let combinacion = Combinacion::desde_evento(tecla, modificadores)?;
        if escribiendo && !combinacion.permitida_escribiendo() {
            return None;
        }
        self.accion_de(combinacion)
    }

    /// Devuelve solo los atajos que difieren del valor por defecto.
    pub fn diferencias(&self) -> BTreeMap<String, String> {
        let mut salida = BTreeMap::new();
        for accion in AccionAtajo::TODAS {
            let actual = self.combinacion(accion);
            if actual != accion.por_defecto() {
                salida.insert(accion.id().to_string(), actual.formatear());
            }
        }
        salida
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct AtajosArchivo {
    #[serde(flatten)]
    atajos: BTreeMap<String, String>,
}

/// `~/.config/order/atajos.toml` o, con `ORDER_ARCHIVO`, junto a ese archivo.
pub fn ruta() -> PathBuf {
    if let Some(archivo) = std::env::var_os("ORDER_ARCHIVO") {
        if !archivo.is_empty() {
            return junto_a(Path::new(&archivo));
        }
    }
    let base = dirs::config_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("order").join("atajos.toml")
}

/// `atajos.toml` en la misma carpeta que el archivo de datos o configuración.
pub fn junto_a(archivo: &Path) -> PathBuf {
    archivo.with_file_name("atajos.toml")
}

/// Lee `atajos.toml`. Si no existe o está roto, devuelve los valores por defecto.
pub fn cargar() -> AtajosConfig {
    cargar_de(&ruta())
}

/// Lee los atajos desde una ruta concreta. Entradas desconocidas o inválidas
/// se ignoran para no impedir el arranque.
pub fn cargar_de(ruta: &Path) -> AtajosConfig {
    let Ok(texto) = std::fs::read_to_string(ruta) else {
        return AtajosConfig::default();
    };
    parsear_toml(&texto)
}

fn parsear_toml(texto: &str) -> AtajosConfig {
    let Ok(crudo) = toml::from_str::<AtajosArchivo>(texto) else {
        return AtajosConfig::default();
    };

    let mut config = AtajosConfig::default();
    let mut personalizados: Vec<(AccionAtajo, Combinacion)> = Vec::new();
    for (id, valor) in crudo.atajos {
        let Some(accion) = AccionAtajo::desde_id(&id) else {
            continue;
        };
        let Ok(combinacion) = Combinacion::parsear(&valor) else {
            continue;
        };
        personalizados.push((accion, combinacion));
    }

    // Se aplican primero los explícitos; si un archivo editado a mano deja un
    // choque contra otra acción no listada, se intercambia para conservar la
    // unicidad de combinaciones.
    let mut fijados = std::collections::HashSet::new();
    for (accion, combinacion) in personalizados {
        if let Some(otra) = config.choque(accion, combinacion) {
            if fijados.contains(&otra) {
                continue;
            }
        }
        config.asignar(accion, combinacion);
        fijados.insert(accion);
    }

    config
}

/// Guarda `atajos.toml` con permisos `600`, escribiendo solo los atajos que
/// difieren de los valores por defecto.
pub fn guardar(config: &AtajosConfig) -> Result<(), String> {
    guardar_en(&ruta(), config)
}

/// Guarda los atajos en una ruta concreta con permisos `600`.
pub fn guardar_en(ruta: &Path, config: &AtajosConfig) -> Result<(), String> {
    let dir = ruta
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;

    let diffs = config.diferencias();
    let cuerpo = if diffs.is_empty() {
        String::new()
    } else {
        toml::to_string_pretty(&diffs)
            .map_err(|e| format!("no se pudo preparar atajos.toml: {e}"))?
    };

    let texto = format!(
        "# Atajos de teclado de ORDER (privado, por dispositivo).\n\
         # Solo se guardan los que difieren del valor por defecto. Permisos 600.\n\n{cuerpo}"
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

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("directorio temporal");
        let ruta = dir.path().join("atajos.toml");
        (dir, ruta)
    }

    #[test]
    fn valores_por_defecto_unicos_y_completos() {
        let cfg = AtajosConfig::default();
        let entradas = cfg.entradas();
        assert_eq!(entradas.len(), 15);

        let mut vistos = std::collections::HashSet::new();
        for (accion, comb) in &entradas {
            assert!(
                vistos.insert(*comb),
                "combinación por defecto duplicada para {}: {comb}",
                accion.id()
            );
            assert_eq!(AccionAtajo::desde_id(accion.id()), Some(*accion));
        }

        assert_eq!(cfg.combinacion(AccionAtajo::Cerrar).formatear(), "Esc");
        assert_eq!(cfg.combinacion(AccionAtajo::Guardar).formatear(), "Ctrl+S");
        assert_eq!(cfg.combinacion(AccionAtajo::NuevaTarea).formatear(), "Ctrl+N");
        assert_eq!(cfg.combinacion(AccionAtajo::IrInicio).formatear(), "Ctrl+1");
        assert_eq!(cfg.combinacion(AccionAtajo::IrDia).formatear(), "Ctrl+2");
        assert_eq!(cfg.combinacion(AccionAtajo::IrSemana).formatear(), "Ctrl+3");
        assert_eq!(cfg.combinacion(AccionAtajo::IrPendientes).formatear(), "Ctrl+4");
        assert_eq!(cfg.combinacion(AccionAtajo::Anterior).formatear(), "←");
        assert_eq!(cfg.combinacion(AccionAtajo::Siguiente).formatear(), "→");
        assert_eq!(cfg.combinacion(AccionAtajo::Hoy).formatear(), "T");
        assert_eq!(cfg.combinacion(AccionAtajo::Agente).formatear(), "Ctrl+K");
        assert_eq!(cfg.combinacion(AccionAtajo::Voz).formatear(), "Ctrl+M");
        assert_eq!(cfg.combinacion(AccionAtajo::Sincronizar).formatear(), "Ctrl+R");
        assert_eq!(cfg.combinacion(AccionAtajo::Ajustes).formatear(), "Ctrl+,");
        assert_eq!(cfg.combinacion(AccionAtajo::AyudaAtajos).formatear(), "?");
        assert!(cfg.diferencias().is_empty());
    }

    #[test]
    fn parseo_y_formato_ida_y_vuelta() {
        let casos = [
            ("Ctrl+Shift+K", "Ctrl+Shift+K"),
            ("ctrl + shift + k", "Ctrl+Shift+K"),
            ("Esc", "Esc"),
            ("escape", "Esc"),
            ("←", "←"),
            ("Left", "←"),
            ("Derecha", "→"),
            ("?", "?"),
            ("Ctrl+,", "Ctrl+,"),
            ("t", "T"),
            ("Alt+F4", "Alt+F4"),
            ("Ctrl++", "Ctrl++"),
            ("+", "+"),
            ("Ctrl+Space", "Ctrl+Espacio"),
        ];
        for (entrada, esperado) in casos {
            let comb = Combinacion::parsear(entrada).expect(entrada);
            assert_eq!(comb.formatear(), esperado, "entrada: {entrada}");
            let de_nuevo = Combinacion::parsear(&comb.formatear()).expect("reparsear");
            assert_eq!(comb, de_nuevo);
        }

        assert!(Combinacion::parsear("").is_err());
        assert!(Combinacion::parsear("Ctrl+").is_err());
        assert!(Combinacion::parsear("Foo+K").is_err());
        assert!(Combinacion::parsear("Ctrl+TeclaInexistente").is_err());
    }

    #[test]
    fn accion_para_respeta_escribiendo_y_codigos_slint() {
        let cfg = AtajosConfig::default();

        // Sin foco de texto: funcionan todas las teclas por defecto.
        assert_eq!(
            cfg.accion_para("\u{001b}", Modificadores::NINGUNO, false),
            Some(AccionAtajo::Cerrar)
        );
        assert_eq!(
            cfg.accion_para("k", Modificadores::CTRL, false),
            Some(AccionAtajo::Agente)
        );
        assert_eq!(
            cfg.accion_para("\u{F702}", Modificadores::NINGUNO, false),
            Some(AccionAtajo::Anterior)
        );
        assert_eq!(
            cfg.accion_para("\u{F703}", Modificadores::NINGUNO, false),
            Some(AccionAtajo::Siguiente)
        );
        assert_eq!(
            cfg.accion_para("t", Modificadores::NINGUNO, false),
            Some(AccionAtajo::Hoy)
        );
        // Al pulsar '?' en un teclado físico, Shift suele venir activo.
        assert_eq!(
            cfg.accion_para(
                "?",
                Modificadores {
                    shift: true,
                    ..Modificadores::NINGUNO
                },
                false
            ),
            Some(AccionAtajo::AyudaAtajos)
        );
        assert_eq!(
            cfg.accion_para(",", Modificadores::CTRL, false),
            Some(AccionAtajo::Ajustes)
        );

        // Escribiendo en un campo de texto:
        // - Sin modificador (←, →, T, ?) NO actúan.
        assert_eq!(cfg.accion_para("t", Modificadores::NINGUNO, true), None);
        assert_eq!(
            cfg.accion_para(
                "T",
                Modificadores {
                    shift: true,
                    ..Modificadores::NINGUNO
                },
                true
            ),
            None
        );
        assert_eq!(
            cfg.accion_para(
                "?",
                Modificadores {
                    shift: true,
                    ..Modificadores::NINGUNO
                },
                true
            ),
            None
        );
        assert_eq!(cfg.accion_para("\u{F702}", Modificadores::NINGUNO, true), None);
        assert_eq!(cfg.accion_para("\u{F703}", Modificadores::NINGUNO, true), None);
        // - Esc SÍ actúa.
        assert_eq!(
            cfg.accion_para("\u{001b}", Modificadores::NINGUNO, true),
            Some(AccionAtajo::Cerrar)
        );
        // - Ctrl+* SÍ actúa.
        assert_eq!(
            cfg.accion_para("s", Modificadores::CTRL, true),
            Some(AccionAtajo::Guardar)
        );
        // - Teclas modificadoras sueltas se ignoran.
        assert_eq!(cfg.accion_para("\u{0011}", Modificadores::CTRL, false), None);
    }

    #[test]
    fn choques_e_intercambio() {
        let mut cfg = AtajosConfig::default();
        let ctrl_n = Combinacion::parsear("Ctrl+N").unwrap();
        let ctrl_k = Combinacion::parsear("Ctrl+K").unwrap();
        let ctrl_shift_n = Combinacion::parsear("Ctrl+Shift+N").unwrap();

        assert_eq!(cfg.choque(AccionAtajo::NuevaTarea, ctrl_n), None);
        assert_eq!(cfg.choque(AccionAtajo::NuevaTarea, ctrl_shift_n), None);
        assert_eq!(
            cfg.choque(AccionAtajo::NuevaTarea, ctrl_k),
            Some(AccionAtajo::Agente)
        );

        // Asignar una combinación ocupada intercambia ambas acciones.
        let intercambiada = cfg.asignar(AccionAtajo::NuevaTarea, ctrl_k);
        assert_eq!(intercambiada, Some(AccionAtajo::Agente));
        assert_eq!(cfg.combinacion(AccionAtajo::NuevaTarea), ctrl_k);
        assert_eq!(cfg.combinacion(AccionAtajo::Agente), ctrl_n);

        cfg.restaurar_por_defecto();
        assert_eq!(cfg.combinacion(AccionAtajo::NuevaTarea), ctrl_n);
        assert_eq!(cfg.combinacion(AccionAtajo::Agente), ctrl_k);
    }

    #[test]
    fn guardar_y_cargar_solo_diferencias() {
        let (_dir, ruta) = temporal();
        let mut cfg = AtajosConfig::default();
        cfg.asignar(
            AccionAtajo::NuevaTarea,
            Combinacion::parsear("Ctrl+Shift+N").unwrap(),
        );
        cfg.asignar(
            AccionAtajo::AyudaAtajos,
            Combinacion::parsear("F1").unwrap(),
        );

        guardar_en(&ruta, &cfg).expect("guardar");
        let contenido = std::fs::read_to_string(&ruta).expect("leer archivo");
        assert!(contenido.contains("nueva_tarea = \"Ctrl+Shift+N\""));
        assert!(contenido.contains("ayuda_atajos = \"F1\""));
        // Los que siguen por defecto no deben escribirse.
        assert!(!contenido.contains("cerrar ="));
        assert!(!contenido.contains("guardar ="));
        assert!(!contenido.contains("agente ="));

        let leido = cargar_de(&ruta);
        assert_eq!(leido, cfg);

        // Restaurar borra las diferencias del archivo.
        cfg.restaurar_por_defecto();
        guardar_en(&ruta, &cfg).expect("guardar restaurado");
        let contenido_vacio = std::fs::read_to_string(&ruta).expect("leer restaurado");
        assert!(!contenido_vacio.contains('='));
        assert_eq!(cargar_de(&ruta), AtajosConfig::default());
    }

    #[test]
    fn archivo_roto_o_con_claves_invalidas_no_rompe() {
        let (_dir, ruta) = temporal();
        std::fs::write(&ruta, "esto = [no es toml").expect("escribir");
        assert_eq!(cargar_de(&ruta), AtajosConfig::default());

        std::fs::write(
            &ruta,
            "desconocida = \"Ctrl+Q\"\n\
             nueva_tarea = \"TeclaMala+XYZ\"\n\
             agente = \"Ctrl+Shift+K\"\n",
        )
        .expect("escribir parcial");
        let cfg = cargar_de(&ruta);
        assert_eq!(
            cfg.combinacion(AccionAtajo::Agente),
            Combinacion::parsear("Ctrl+Shift+K").unwrap()
        );
        assert_eq!(
            cfg.combinacion(AccionAtajo::NuevaTarea),
            AccionAtajo::NuevaTarea.por_defecto()
        );
    }

    #[cfg(unix)]
    #[test]
    fn permisos_600_y_junto_a_order_archivo() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, ruta) = temporal();
        std::fs::write(&ruta, "").expect("crear");
        std::fs::set_permissions(&ruta, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        guardar_en(&ruta, &AtajosConfig::default()).expect("guardar");
        let modo = std::fs::metadata(&ruta).expect("meta").permissions().mode();
        assert_eq!(modo & 0o777, 0o600);

        assert_eq!(
            junto_a(Path::new("/tmp/order/order.toml")),
            PathBuf::from("/tmp/order/atajos.toml")
        );
    }
}
