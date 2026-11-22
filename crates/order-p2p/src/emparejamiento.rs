//! Enlaces `order://emparejar?...`, código de 8 palabras, códigos QR (SVG y
//! matriz) y gestión del secreto de un solo uso (10 minutos).

use iroh::EndpointId;
use qrcode::{Color, QrCode};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Tiempo de validez de un código/QR de emparejamiento.
pub const TTL_EMPAREJAMIENTO: Duration = Duration::from_secs(10 * 60);

/// Mensaje exacto al presentar un secreto vencido o incorrecto.
pub const ERROR_CODIGO_INVALIDO: &str = "Este código ya no sirve; genera otro.";

/// Código listo para mostrar al usuario al agregar otro dispositivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodigoEmparejamiento {
    pub enlace: String,
    pub palabras: String,
    pub qr_svg: String,
    pub qr_tamano: u32,
    pub qr_modulos: Vec<bool>,
}

/// Datos extraídos de un enlace `order://emparejar?...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatosEnlace {
    pub nodo: EndpointId,
    pub secreto: [u8; 4],
    pub nombre: Option<String>,
}

/// Secreto temporal de un solo uso almacenado en el dispositivo que invita.
#[derive(Debug, Clone)]
pub struct InvitacionActiva {
    pub secreto: [u8; 4],
    pub creada: Instant,
    pub usada: bool,
}

impl InvitacionActiva {
    pub fn nueva() -> Self {
        InvitacionActiva {
            secreto: secreto_aleatorio(),
            creada: Instant::now(),
            usada: false,
        }
    }

    pub fn secreto_hex(&self) -> String {
        hex_4(self.secreto)
    }

    pub fn vigente(&self, ttl: Duration) -> bool {
        !self.usada && self.creada.elapsed() <= ttl
    }
}

pub fn generar_codigo(nodo: &EndpointId, nombre: &str, secreto: [u8; 4]) -> Result<CodigoEmparejamiento, String> {
    let enlace = construir_enlace(nodo, secreto, nombre);
    let palabras = codificar_palabras(nodo, secreto);
    let qr_svg = generar_qr_svg(&enlace)?;
    let (qr_tamano, qr_modulos) = generar_qr_matriz(&enlace)?;
    Ok(CodigoEmparejamiento {
        enlace,
        palabras,
        qr_svg,
        qr_tamano,
        qr_modulos,
    })
}

pub fn construir_enlace(nodo: &EndpointId, secreto: [u8; 4], nombre: &str) -> String {
    let s = hex_4(secreto);
    let d = codificar_pct(nombre.trim());
    format!("order://emparejar?n={nodo}&s={s}&d={d}")
}

pub fn parsear_enlace(entrada: &str) -> Result<DatosEnlace, String> {
    let limpio = entrada.trim();
    let resto = limpio
        .strip_prefix("order://emparejar?")
        .ok_or_else(|| "El enlace no es de emparejamiento de ORDER.".to_string())?;

    let mut nodo: Option<EndpointId> = None;
    let mut secreto: Option<[u8; 4]> = None;
    let mut nombre: Option<String> = None;

    for par in resto.split('&') {
        let Some((clave, valor)) = par.split_once('=') else {
            continue;
        };
        match clave {
            "n" => {
                let id = EndpointId::from_str(valor.trim())
                    .map_err(|_| "El identificador del dispositivo en el enlace no es válido.".to_string())?;
                nodo = Some(id);
            }
            "s" => {
                secreto = Some(parsear_hex_4(valor.trim())?);
            }
            "d" => {
                let dec = decodificar_pct(valor.trim());
                if !dec.is_empty() {
                    nombre = Some(dec);
                }
            }
            _ => {}
        }
    }

    Ok(DatosEnlace {
        nodo: nodo.ok_or_else(|| "El enlace no incluye el identificador del dispositivo.".to_string())?,
        secreto: secreto.ok_or_else(|| ERROR_CODIGO_INVALIDO.to_string())?,
        nombre,
    })
}

/// Codifica los primeros 4 bytes de `nodo` + los 4 bytes de `secreto` en 8 palabras.
pub fn codificar_palabras(nodo: &EndpointId, secreto: [u8; 4]) -> String {
    let bytes_nodo = nodo.as_bytes();
    let mut ocho = [0u8; 8];
    ocho[..4].copy_from_slice(&bytes_nodo[..4]);
    ocho[4..8].copy_from_slice(&secreto);
    ocho.iter()
        .map(|b| PALABRAS[*b as usize])
        .collect::<Vec<_>>()
        .join(" ")
}

/// Decodifica 8 palabras en `(prefijo_nodo_4_bytes, secreto_4_bytes)`.
pub fn decodificar_palabras(entrada: &str) -> Result<([u8; 4], [u8; 4]), String> {
    let tokens: Vec<String> = entrada
        .split(|c: char| c.is_whitespace() || c == '-' || c == ',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if tokens.len() != 8 {
        return Err("El código debe tener 8 palabras (o pega el enlace order://emparejar).".to_string());
    }
    let mut bytes = [0u8; 8];
    for (i, token) in tokens.iter().enumerate() {
        let idx = PALABRAS
            .iter()
            .position(|p| *p == token.as_str())
            .ok_or_else(|| format!("La palabra «{token}» no pertenece al código."))?;
        bytes[i] = idx as u8;
    }
    let mut prefijo = [0u8; 4];
    let mut secreto = [0u8; 4];
    prefijo.copy_from_slice(&bytes[..4]);
    secreto.copy_from_slice(&bytes[4..8]);
    Ok((prefijo, secreto))
}

pub fn coincide_prefijo(nodo: &EndpointId, prefijo: [u8; 4]) -> bool {
    nodo.as_bytes()[..4] == prefijo
}

pub fn generar_qr_svg(enlace: &str) -> Result<String, String> {
    let qr = QrCode::new(enlace.as_bytes())
        .map_err(|e| format!("no se pudo generar el código QR: {e}"))?;
    Ok(qr
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(220, 220)
        .dark_color(qrcode::render::svg::Color("#101014"))
        .light_color(qrcode::render::svg::Color("#e9e4d8"))
        .build())
}

pub fn generar_qr_matriz(enlace: &str) -> Result<(u32, Vec<bool>), String> {
    let qr = QrCode::new(enlace.as_bytes())
        .map_err(|e| format!("no se pudo generar el código QR: {e}"))?;
    let tamano = qr.width() as u32;
    let modulos = qr
        .to_colors()
        .into_iter()
        .map(|c| matches!(c, Color::Dark))
        .collect();
    Ok((tamano, modulos))
}

pub fn hex_4(bytes: [u8; 4]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3]
    )
}

pub fn parsear_hex_4(texto: &str) -> Result<[u8; 4], String> {
    let t = texto.trim();
    if t.len() != 8 {
        return Err(ERROR_CODIGO_INVALIDO.to_string());
    }
    let mut out = [0u8; 4];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&t[i * 2..i * 2 + 2], 16)
            .map_err(|_| ERROR_CODIGO_INVALIDO.to_string())?;
    }
    Ok(out)
}

fn secreto_aleatorio() -> [u8; 4] {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let mut h = RandomState::new().build_hasher();
    h.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default(),
    );
    h.write_u32(std::process::id());
    h.write_u64(CONTADOR.fetch_add(1, Ordering::Relaxed));
    (h.finish() as u32).to_be_bytes()
}

fn codificar_pct(texto: &str) -> String {
    let mut out = String::with_capacity(texto.len());
    for b in texto.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn decodificar_pct(texto: &str) -> String {
    let bytes = texto.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_digito(bytes[i + 1]), hex_digito(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_digito(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn palabras_lista() -> &'static [&'static str; 256] {
    &PALABRAS
}

/// 256 palabras cortas en español sin tildes para códigos de 8 palabras.
const PALABRAS: [&str; 256] = [
    "abril", "acero", "agua", "aire", "ala", "alba", "alce", "aldea", "alga", "alma", "alto", "ambar",
    "amigo", "ancla", "anda", "angel", "anillo", "antena", "apoyo", "arado", "arbol", "arco", "arena", "aroma",
    "arpa", "arte", "asfalto", "astro", "atlas", "atril", "aula", "aura", "ave", "avena", "avion", "ayuda",
    "azul", "bahia", "baile", "bala", "balsa", "bambu", "banco", "banda", "barco", "barra", "barro", "base",
    "baston", "bata", "batea", "bazar", "beber", "beca", "bella", "beso", "bicho", "biela", "bison", "blanco",
    "bloque", "blusa", "boca", "boda", "boina", "bola", "bolsa", "bomba", "borde", "bosque", "bota", "bote",
    "boton", "brasa", "brazo", "breve", "brisa", "broca", "broma", "bronce", "brote", "bruja", "bruma", "bueno",
    "bufanda", "buho", "bulto", "buque", "burro", "busca", "buzon", "cabal", "cabra", "cacao", "cacto", "cadena",
    "cafe", "caida", "caja", "cal", "caldo", "calma", "calor", "calle", "cama", "cambio", "camino", "campo",
    "canal", "canasta", "cancha", "candil", "canela", "canoa", "canto", "caoba", "capa", "capaz", "caracol", "carbon",
    "carga", "carne", "caro", "carpa", "carro", "carta", "casa", "casco", "caso", "casta", "caucho", "causa",
    "caza", "cebra", "cedro", "ceja", "celda", "cena", "ceniza", "centro", "cera", "cerca", "cerdo", "cereza",
    "cerro", "cesta", "cetro", "chapa", "charco", "chico", "chile", "chisme", "chispa", "choza", "churro", "ciclo",
    "cielo", "cien", "ciervo", "cifra", "cigarro", "cima", "cincel", "cine", "cinta", "circo", "ciruela", "cisne",
    "cita", "ciudad", "claro", "clase", "clave", "clavo", "clima", "cloro", "club", "cobre", "cocina", "coco",
    "codo", "cofre", "cohete", "cola", "colcha", "colina", "collar", "colmo", "color", "coma", "cometa", "comida",
    "compas", "conde", "conejo", "coral", "corazon", "corcho", "cordel", "corona", "corral", "correo", "corte", "cortina",
    "cosa", "coser", "costa", "costo", "crater", "crema", "cresta", "cria", "crimen", "cristal", "cromo", "cronica",
    "cruce", "crudo", "cruel", "cruz", "cuadro", "cuarto", "cuatro", "cubo", "cuchara", "cuello", "cuenca", "cuenta",
    "cuerda", "cuerno", "cuero", "cuesta", "cueva", "cuidar", "culpa", "cumbre", "cuna", "cuota", "cupula", "cura",
    "curso", "curva", "dado", "daga", "dalia", "dama", "danza", "dardo", "dato", "deber", "debil", "dedal",
    "dedo", "defensa", "dejar", "delta",
];
