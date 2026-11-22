//! Lo que hace de ORDER un proceso de escritorio: instancia única, icono en la
//! bandeja, recarga del archivo cuando cambia y reloj de avisos.
//!
//! Nada de esto vive en el hilo de la ventana. La bandeja, el observador y el
//! reloj corren en hilos propios y se comunican con Slint por
//! `slint::invoke_from_event_loop`, que es la única forma segura de tocar la
//! interfaz desde fuera de su bucle de eventos.

use crate::almacen::Almacen;
use crate::avisos::{self, Registro};
use crate::modelo::{self, Datos};
use crate::puente;
use chrono::{Local, TimeDelta};
use ksni::blocking::TrayMethods;
use notify::{RecursiveMode, Watcher};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

/// Cada cuánto revisa el reloj si toca avisar.
const LATIDO: Duration = Duration::from_secs(30);

/// Los avisos con más de estos días se tiran del registro al arrancar.
const DIAS_DE_REGISTRO: i64 = 7;

/// Lo que vieron la última lectura válida. La ventana y el reloj lo comparten:
/// así el reloj avisa con los mismos datos que se están mostrando y un archivo
/// roto no borra lo último bueno.
static DATOS: Mutex<Option<Datos>> = Mutex::new(None);

/// El icono se queda vivo mientras viva el proceso.
static BANDEJA: OnceLock<ksni::blocking::Handle<Bandeja>> = OnceLock::new();

/// Publica los datos que acaba de leer la ventana.
pub fn publicar(datos: &Datos) {
    if let Ok(mut guardados) = DATOS.lock() {
        *guardados = Some(datos.clone());
    }
}

fn datos_actuales() -> Option<Datos> {
    DATOS.lock().ok().and_then(|guardados| guardados.clone())
}

// ---------------------------------------------------------------------------
// Instancia única
// ---------------------------------------------------------------------------

/// Cómo arrancó este proceso.
pub enum Arranque {
    /// Somos la instancia principal: hay que quedarse.
    Principal,
    /// Ya había otra: se le pidió mostrar la ventana y este proceso se va.
    Otra,
}

/// Toma el socket de instancia única. Si ya hay otra instancia viva, le pide
/// que muestre la ventana y devuelve `Arranque::Otra`.
///
/// El socket lleva el nombre de `ORDER_ARCHIVO` cuando esa variable existe, para
/// que una copia de prueba (otro archivo) no se confunda con la app real ni la
/// despierte.
pub fn reclamar_instancia() -> Arranque {
    let ruta = ruta_socket();

    if avisar_mostrar(&ruta) {
        return Arranque::Otra;
    }
    // El archivo existe pero nadie contesta: es de un cierre anterior.
    let _ = std::fs::remove_file(&ruta);

    match UnixListener::bind(&ruta) {
        Ok(oyente) => {
            escuchar(oyente);
            Arranque::Principal
        }
        Err(error) => {
            // Carrera: otra instancia tomó el socket entre la prueba y el bind.
            if avisar_mostrar(&ruta) {
                Arranque::Otra
            } else {
                // Sin socket la app sigue funcionando; solo pierde la instancia
                // única, y eso es mejor que no arrancar.
                eprintln!("ORDER: no se pudo abrir {}: {error}", ruta.display());
                Arranque::Principal
            }
        }
    }
}

/// `$XDG_RUNTIME_DIR/order.sock` (o el temporal del sistema si no hay).
fn ruta_socket() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|ruta| !ruta.as_os_str().is_empty())
        .unwrap_or_else(std::env::temp_dir);

    base.join(nombre_socket(std::env::var_os("ORDER_ARCHIVO").as_deref()))
}

/// Nombre del socket. Con `ORDER_ARCHIVO` se deriva de ese archivo: dos copias
/// de prueba distintas no se pisan ni despiertan a la app real.
fn nombre_socket(archivo: Option<&std::ffi::OsStr>) -> String {
    match archivo.filter(|archivo| !archivo.is_empty()) {
        Some(archivo) => {
            let mut hasher = DefaultHasher::new();
            archivo.hash(&mut hasher);
            format!("order-{:08x}.sock", hasher.finish() as u32)
        }
        None => "order.sock".to_string(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn el_socket_de_la_copia_no_es_el_de_la_app() {
        let principal = nombre_socket(None);
        assert_eq!(principal, "order.sock");

        let copia = nombre_socket(Some(OsStr::new("/tmp/copia.toml")));
        assert!(copia.starts_with("order-") && copia.ends_with(".sock"), "{copia}");
        assert_ne!(copia, principal, "una copia no despierta a la app real");

        let otra = nombre_socket(Some(OsStr::new("/tmp/otra.toml")));
        assert_ne!(otra, copia, "cada archivo tiene su socket");

        assert_eq!(nombre_socket(Some(OsStr::new(""))), principal);
    }

    /// La instancia única se reconoce por el socket: sin nadie escuchando no
    /// hay otra instancia; con un oyente vivo, el aviso llega.
    #[test]
    fn avisa_a_la_instancia_viva() {
        let dir = tempfile::tempdir().expect("directorio temporal");
        let ruta = dir.path().join("order.sock");

        assert!(!avisar_mostrar(&ruta), "sin socket no hay otra instancia");

        let oyente = UnixListener::bind(&ruta).expect("abrir el socket");
        assert!(avisar_mostrar(&ruta), "con oyente sí se avisa");

        let (mut conexion, _) = oyente.accept().expect("aceptar la conexión");
        let mut orden = [0_u8; 32];
        let leidos = conexion.read(&mut orden).expect("leer la orden");
        assert_eq!(
            String::from_utf8_lossy(&orden[..leidos]).trim(),
            "mostrar",
            "la orden es mostrar"
        );
    }
}

/// Intenta hablar con la instancia que ya corre. `true` si contestó.
fn avisar_mostrar(ruta: &PathBuf) -> bool {
    let Ok(mut flujo) = UnixStream::connect(ruta) else {
        return false;
    };
    let _ = flujo.set_write_timeout(Some(Duration::from_millis(500)));
    flujo.write_all(b"mostrar\n").is_ok()
}

/// Acepta órdenes en un hilo: `mostrar`, `alternar` y `salir`.
fn escuchar(oyente: UnixListener) {
    thread::spawn(move || {
        for conexion in oyente.incoming() {
            let Ok(mut flujo) = conexion else { continue };
            let _ = flujo.set_read_timeout(Some(Duration::from_secs(2)));
            let mut orden = [0_u8; 32];
            let leidos = flujo.read(&mut orden).unwrap_or(0);
            let orden = String::from_utf8_lossy(&orden[..leidos]);
            match orden.trim() {
                "mostrar" => {
                    let _ = slint::invoke_from_event_loop(puente::mostrar_ventana);
                }
                "alternar" => {
                    let _ = slint::invoke_from_event_loop(puente::alternar_ventana);
                }
                "salir" => cerrar(),
                _ => {}
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Bandeja
// ---------------------------------------------------------------------------

/// El icono de DMS. Clic izquierdo alterna la ventana; el menú abre o sale.
struct Bandeja;

impl ksni::Tray for Bandeja {
    fn id(&self) -> String {
        "order".to_string()
    }

    fn title(&self) -> String {
        "ORDER".to_string()
    }

    fn icon_name(&self) -> String {
        "view-calendar".to_string()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = slint::invoke_from_event_loop(puente::alternar_ventana);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: "Abrir".to_string(),
                activate: Box::new(|_| {
                    let _ = slint::invoke_from_event_loop(puente::mostrar_ventana);
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Salir".to_string(),
                activate: Box::new(|_| cerrar()),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Arranca el icono. Si el escritorio no tiene SNI, se avisa y se sigue: la
/// ventana y los avisos funcionan igual.
pub fn arrancar_bandeja() {
    // Al iniciar sesión DMS puede tardar un poco en ofrecer SNI: se tolera que
    // el vigilante no esté y el icono aparece cuando llegue, en vez de perderlo
    // para siempre por arrancar temprano.
    match Bandeja.assume_sni_available(true).spawn() {
        Ok(handle) => {
            let _ = BANDEJA.set(handle);
        }
        Err(error) => eprintln!("ORDER: sin icono en la bandeja: {error}"),
    }
}

/// Salir de verdad: se para el bucle de Slint y el proceso termina. El socket
/// queda en disco, pero el próximo arranque lo reconoce como viejo (nadie
/// contesta) y lo reemplaza.
fn cerrar() {
    let _ = slint::invoke_from_event_loop(|| {
        let _ = slint::quit_event_loop();
    });
}

// ---------------------------------------------------------------------------
// Observador del archivo
// ---------------------------------------------------------------------------

/// Vigila la carpeta del archivo. Cada cambio de `order.toml` se relee en el
/// hilo de la ventana; si el archivo quedó roto, se muestra el error y la
/// ventana conserva lo último válido.
pub fn vigilar_archivo(almacen: Almacen) {
    thread::spawn(move || {
        // Un ORDER_ARCHIVO relativo (`copia.toml`) tiene padre vacío: se vigila
        // la carpeta actual.
        let carpeta = match almacen.ruta.parent() {
            Some(carpeta) if !carpeta.as_os_str().is_empty() => carpeta.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let Some(nombre) = almacen.ruta.file_name().map(|n| n.to_os_string()) else {
            return;
        };

        let (emisor, receptor) = std::sync::mpsc::channel();
        let mut vigilante = match notify::recommended_watcher(move |resultado| {
            let _ = emisor.send(resultado);
        }) {
            Ok(vigilante) => vigilante,
            Err(error) => {
                eprintln!("ORDER: no se pudo vigilar el archivo: {error}");
                return;
            }
        };
        if let Err(error) = vigilante.watch(&carpeta, RecursiveMode::NonRecursive) {
            eprintln!("ORDER: no se pudo vigilar {}: {error}", carpeta.display());
            return;
        }

        // Una sola escritura produce varios eventos (temporal, renombrar…).
        let mut ultimo = Instant::now();
        for resultado in receptor {
            let Ok(evento) = resultado else { continue };
            if !evento
                .paths
                .iter()
                .any(|ruta| ruta.file_name() == Some(nombre.as_os_str()))
            {
                continue;
            }
            if ultimo.elapsed() < Duration::from_millis(150) {
                continue;
            }
            ultimo = Instant::now();

            let almacen = almacen.clone();
            let _ = slint::invoke_from_event_loop(move || leer_y_mostrar(&almacen));
        }
    });
}

/// Relee el archivo y actualiza la ventana sin mover la semana ni cerrar el
/// formulario abierto.
fn leer_y_mostrar(almacen: &Almacen) {
    match almacen.cargar() {
        Ok(datos) => {
            publicar(&datos);
            puente::recargar(&datos);
            puente::mostrar_advertencias(&advertencias(almacen));
            puente::mostrar_error(None);
        }
        Err(error) => puente::mostrar_error(Some(&error)),
    }
}

fn advertencias(almacen: &Almacen) -> Vec<modelo::ErrorDatos> {
    std::fs::read_to_string(&almacen.ruta)
        .map(|texto| modelo::advertencias(&texto))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Reloj de avisos
// ---------------------------------------------------------------------------

/// Revisa cada 30 s qué toca avisar y lo manda. Al arrancar da un solo resumen
/// (lo perdido con el equipo apagado no se dispara en ráfaga) y poda el
/// registro de claves viejas.
pub fn arrancar_reloj() {
    thread::spawn(move || {
        let registro = registro();
        let hoy = Local::now().date_naive();
        registro.podar(hoy - TimeDelta::days(DIAS_DE_REGISTRO));
        let mut enviados = registro.leer();
        let mut resumen_hecho = false;

        loop {
            let ahora = Local::now().naive_local();
            if let Some(datos) = datos_actuales() {
                if !resumen_hecho {
                    resumen_hecho = true;
                    if let Some(resumen) = avisos::resumen_arranque(&datos, ahora) {
                        if !enviados.contains(&resumen.clave) {
                            mandar(&resumen);
                            registro.anotar(&resumen.clave);
                            enviados.insert(resumen.clave);
                        }
                    }
                }

                for aviso in avisos::avisos_debidos(&datos, ahora, &enviados) {
                    mandar(&aviso);
                    registro.anotar(&aviso.clave);
                    enviados.insert(aviso.clave);
                }
            }
            thread::sleep(LATIDO);
        }
    });
}

/// Envía el aviso. Si el escritorio no tiene servidor de notificaciones, se
/// apunta igual: reintentarlo cada 30 s sería peor que perderlo.
fn mandar(aviso: &avisos::Aviso) {
    let resultado = notify_rust::Notification::new()
        .appname("ORDER")
        .summary(&aviso.titulo)
        .body(&aviso.cuerpo)
        .timeout(notify_rust::Timeout::Milliseconds(8000))
        .show();
    if let Err(error) = resultado {
        eprintln!(
            "ORDER: no se pudo mostrar el aviso «{}»: {error}",
            aviso.titulo
        );
    }
}

/// El registro vive junto al archivo cuando se usa `ORDER_ARCHIVO`, para que
/// una copia de prueba no marque como enviados los avisos de la app real.
fn registro() -> Registro {
    match std::env::var_os("ORDER_ARCHIVO") {
        Some(archivo) if !archivo.is_empty() => Registro {
            ruta: PathBuf::from(archivo).with_file_name("enviados.txt"),
        },
        _ => Registro::por_defecto(),
    }
}
