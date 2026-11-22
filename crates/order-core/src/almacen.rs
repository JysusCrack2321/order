//! Lectura y escritura segura del archivo de datos (`order.toml`).
//!
//! Dos reglas mandan aquí:
//!
//! 1. **Nunca pisar lo que no se ha leído.** Cada escritura vuelve a leer el
//!    disco justo antes de tocar nada, así los cambios hechos a mano o por
//!    Hermes sobreviven.
//! 2. **Nunca escribir algo que no se pueda volver a leer.** El texto nuevo se
//!    valida con `modelo::parsear` y solo entonces se guarda, de forma atómica
//!    (archivo temporal en el mismo directorio + renombrar). Si el archivo del
//!    disco está roto, no se toca en absoluto.

use crate::modelo::{self, Datos, ErrorDatos};
use crate::ops;
use chrono::Local;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::{DocumentMut, Item, Value, value};

/// Lo que se copia en el primer arranque.
const EJEMPLO: &str = include_str!("../ejemplo.toml");

/// Secciones que la app numera, con el prefijo de su `id`.
const IDS: [(&str, char); 3] = [("clase", 'c'), ("tarea", 't'), ("recordatorio", 'r')];

/// El archivo de datos y todo lo necesario para leerlo y reemplazarlo sin
/// perder nada. Se le puede dar cualquier ruta (las pruebas usan una temporal).
#[derive(Clone)]
pub struct Almacen {
    pub ruta: PathBuf,
}

impl Almacen {
    /// `~/.config/order/order.toml`.
    ///
    /// Si existe la variable de entorno `ORDER_ARCHIVO`, su valor reemplaza esa
    /// ruta: sirve para probar a mano con una copia sin tocar el archivo real
    /// del usuario (`ORDER_ARCHIVO=/tmp/copia.toml cargo run`). También mueve
    /// el historial, que vive junto al archivo elegido.
    pub fn por_defecto() -> Almacen {
        if let Some(ruta) = std::env::var_os("ORDER_ARCHIVO") {
            if !ruta.is_empty() {
                return Almacen {
                    ruta: PathBuf::from(ruta),
                };
            }
        }
        let base = dirs::config_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        Almacen {
            ruta: base.join("order").join("order.toml"),
        }
    }

    /// Lee el archivo y devuelve los datos. Si no existe, lo crea con el
    /// ejemplo. Si encuentra elementos sin `id`, se los asigna y los guarda.
    pub fn cargar(&self) -> Result<Datos, ErrorDatos> {
        if !self.ruta.exists() {
            self.escribir_atomico(EJEMPLO)?;
        }
        let texto = self.leer()?;
        let datos = modelo::parsear(&texto)?;
        if falta_algun_id(&datos) {
            return self.modificar(|_| Ok(()));
        }
        Ok(datos)
    }

    /// Aplica `f` sobre una copia recién leída del disco y guarda el resultado.
    /// Si el disco no parsea, o el texto resultante no se puede volver a leer,
    /// devuelve `Err` y deja el archivo exactamente como estaba.
    pub fn modificar<F>(&self, f: F) -> Result<Datos, ErrorDatos>
    where
        F: FnOnce(&mut DocumentMut) -> Result<(), String>,
    {
        let texto = self.leer()?;
        let mut doc = match texto.parse::<DocumentMut>() {
            Ok(doc) => doc,
            // El texto no es TOML; `parsear` ya sabe decir en qué línea está.
            Err(e) => {
                return match modelo::parsear(&texto) {
                    Err(error) => Err(error),
                    Ok(_) => Err(ErrorDatos {
                        linea: None,
                        mensaje: format!("el archivo no es TOML válido: {e}"),
                    }),
                };
            }
        };

        asignar_ids(&mut doc);
        f(&mut doc).map_err(|mensaje| ErrorDatos {
            linea: None,
            mensaje,
        })?;

        let nuevo = doc.to_string();
        let datos = modelo::parsear(&nuevo)?; // valida antes de tocar el disco
        self.escribir_atomico(&nuevo)?;
        Ok(datos)
    }

    /// El historial, junto al archivo principal (`historial.toml`).
    pub fn historial(&self) -> Almacen {
        Almacen {
            ruta: self.ruta.with_file_name("historial.toml"),
        }
    }

    /// Mueve a `historial.toml` las tareas hechas hace más de 30 días. Se llama
    /// una vez al arrancar; si no hay nada que mover, no toca ningún archivo.
    pub fn archivar(&self) -> Result<Datos, ErrorDatos> {
        let hoy = Local::now().date_naive();
        let datos = modelo::parsear(&self.leer()?)?;
        if !ops::hay_que_archivar(&datos, hoy) {
            return Ok(datos);
        }

        let historial = self.historial();
        let mut doc_historial = if historial.ruta.exists() {
            historial
                .leer()?
                .parse::<DocumentMut>()
                .map_err(|e| ErrorDatos {
                    linea: None,
                    mensaje: format!("{} no es TOML válido: {e}", historial.ruta.display()),
                })?
        } else {
            DocumentMut::new()
        };

        let datos = self.modificar(|doc| {
            ops::archivar_hechas(doc, &mut doc_historial, hoy);
            Ok(())
        })?;
        historial.escribir_atomico(&doc_historial.to_string())?;
        Ok(datos)
    }

    fn leer(&self) -> Result<String, ErrorDatos> {
        std::fs::read_to_string(&self.ruta).map_err(|e| ErrorDatos {
            linea: None,
            mensaje: format!("no se pudo leer {}: {e}", self.ruta.display()),
        })
    }

    /// Escribe en un temporal del mismo directorio y lo renombra encima del
    /// original: quien lea el archivo ve la versión vieja o la nueva, nunca una
    /// a medias.
    fn escribir_atomico(&self, texto: &str) -> Result<(), ErrorDatos> {
        let dir = self
            .ruta
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(dir).map_err(|e| ErrorDatos {
            linea: None,
            mensaje: format!("no se pudo crear {}: {e}", dir.display()),
        })?;

        let tmp = dir.join(nombre_temporal(&self.ruta));
        let escritura = (|| -> std::io::Result<()> {
            let mut archivo = std::fs::File::create(&tmp)?;
            archivo.write_all(texto.as_bytes())?;
            archivo.sync_all()
        })();
        if let Err(e) = escritura {
            let _ = std::fs::remove_file(&tmp);
            return Err(ErrorDatos {
                linea: None,
                mensaje: format!("no se pudo escribir {}: {e}", tmp.display()),
            });
        }

        // El archivo nuevo hereda los permisos del que reemplaza.
        if let Ok(meta) = std::fs::metadata(&self.ruta) {
            let _ = std::fs::set_permissions(&tmp, meta.permissions());
        }

        std::fs::rename(&tmp, &self.ruta).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            ErrorDatos {
                linea: None,
                mensaje: format!("no se pudo reemplazar {}: {e}", self.ruta.display()),
            }
        })
    }
}

/// Nombre corto y único para el temporal (`.<archivo>.<pid>.<n>.tmp`).
fn nombre_temporal(ruta: &Path) -> String {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    let nombre = ruta
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("order.toml");
    format!(".{nombre}.{}.{n}.tmp", std::process::id())
}

/// `id` nuevo y corto, p. ej. `t-20261121-a3f901c2`. La fecha es la de hoy;
/// el sufijo lleva 32 bits aleatorios del sistema, sin crate extra.
pub fn nuevo_id(prefijo: char) -> String {
    let fecha = Local::now().format("%Y%m%d");
    let sufijo = sufijo_aleatorio();
    format!("{prefijo}-{fecha}-{sufijo:08x}")
}

fn sufijo_aleatorio() -> u32 {
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
    (h.finish() & 0xffff_ffff) as u32
}

/// Pone `id` a toda clase, tarea y recordatorio que no lo tenga. Es lo que hace
/// que lo escrito por Hermes sin `id` siga siendo editable desde la ventana.
fn asignar_ids(doc: &mut DocumentMut) {
    for (seccion, prefijo) in IDS {
        match doc.get_mut(seccion) {
            Some(Item::ArrayOfTables(tablas)) => {
                for tabla in tablas.iter_mut() {
                    let falta = tabla
                        .get("id")
                        .and_then(Item::as_str)
                        .is_none_or(|s| s.trim().is_empty());
                    if falta {
                        tabla.insert("id", value(nuevo_id(prefijo)));
                    }
                }
            }
            // Forma compacta: `clase = [{ materia = "..." }]`.
            Some(Item::Value(Value::Array(valores))) => {
                for valor in valores.iter_mut() {
                    if let Value::InlineTable(tabla) = valor {
                        let falta = tabla
                            .get("id")
                            .and_then(Value::as_str)
                            .is_none_or(|s| s.trim().is_empty());
                        if falta {
                            tabla.insert("id", nuevo_id(prefijo).into());
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn falta_algun_id(datos: &Datos) -> bool {
    datos.clases.iter().any(|c| c.id.trim().is_empty())
        || datos.tareas.iter().any(|t| t.id.trim().is_empty())
        || datos.recordatorios.iter().any(|r| r.id.trim().is_empty())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    /// Un almacén sobre un directorio temporal, que se borra al terminar.
    fn temporal() -> (tempfile::TempDir, Almacen) {
        let dir = tempfile::tempdir().expect("directorio temporal");
        let almacen = Almacen {
            ruta: dir.path().join("order.toml"),
        };
        (dir, almacen)
    }

    const UNA_TAREA: &str = "\
# mi nota
[ajustes]
aviso_horas_antes = 3

[[tarea]]
id = \"t-uno\"
titulo = \"Taller\"
vence = \"2026-11-28T23:59\"
hecha = false
";

    #[test]
    fn crea_desde_ejemplo() {
        let (_dir, almacen) = temporal();
        assert!(!almacen.ruta.exists(), "la ruta empieza sin archivo");

        let datos = almacen.cargar().expect("cargar crea el archivo");
        assert!(almacen.ruta.exists(), "el archivo debe existir tras cargar");
        assert!(!datos.clases.is_empty(), "trae el ejemplo");
        assert!(!datos.tareas.is_empty(), "trae el ejemplo");
        assert!(!datos.recordatorios.is_empty(), "trae el ejemplo");
    }

    #[test]
    fn conserva_comentarios() {
        let (_dir, almacen) = temporal();
        fs::write(&almacen.ruta, UNA_TAREA).expect("escribir el archivo");

        let datos = almacen
            .modificar(|doc| {
                doc["ajustes"]["aviso_horas_antes"] = value(5);
                Ok(())
            })
            .expect("modificar un archivo válido");

        assert_eq!(datos.ajustes.aviso_horas_antes, 5);
        let leido = fs::read_to_string(&almacen.ruta).expect("leer el archivo");
        assert!(
            leido.contains("# mi nota"),
            "el comentario debe sobrevivir, archivo:\n{leido}"
        );
    }

    #[test]
    fn modificar_relee_disco() {
        let (_dir, almacen) = temporal();
        fs::write(&almacen.ruta, UNA_TAREA).expect("escribir el archivo");
        almacen.cargar().expect("cargar el archivo inicial");

        // Alguien (Hermes) agrega una tarea mientras la app está abierta.
        let mut externo = fs::read_to_string(&almacen.ruta).expect("leer el archivo");
        externo.push_str(
            "\n[[tarea]]\nid = \"t-dos\"\ntitulo = \"Segunda\"\n\
             vence = \"2026-12-02T18:00\"\nhecha = false\n",
        );
        fs::write(&almacen.ruta, externo).expect("escribir el cambio externo");

        let datos = almacen
            .modificar(|doc| {
                let tareas = doc["tarea"]
                    .as_array_of_tables_mut()
                    .expect("las tareas son [[tarea]]");
                let primera = tareas.get_mut(0).expect("existe la primera tarea");
                primera["hecha"] = value(true);
                Ok(())
            })
            .expect("modificar releyendo el disco");

        assert_eq!(datos.tareas.len(), 2, "el cambio externo sobrevive");
        assert!(
            datos.tareas.iter().any(|t| t.id == "t-dos"),
            "la tarea externa está en el resultado"
        );
        assert!(
            datos.tareas.iter().any(|t| t.id == "t-uno" && t.hecha),
            "la tarea marcada quedó hecha"
        );
    }

    #[test]
    fn no_sobrescribe_archivo_roto() {
        let (_dir, almacen) = temporal();

        // TOML roto a mano: ni siquiera se puede interpretar.
        let roto = "# roto\n[ajustes\naviso_horas_antes = 3\n";
        fs::write(&almacen.ruta, roto).expect("escribir el archivo roto");
        let error = almacen
            .modificar(|_| Ok(()))
            .expect_err("un TOML roto no se puede modificar");
        assert!(
            error.linea.is_some(),
            "el error debe traer la línea: {}",
            error.mensaje
        );
        assert_eq!(
            fs::read_to_string(&almacen.ruta).expect("leer el archivo"),
            roto,
            "el archivo roto debe quedar intacto"
        );

        // TOML válido con un dato imposible: se valida antes de escribir.
        let invalido = "[[tarea]]\ntitulo = \"X\"\nvence = \"2026-13-45T99:99\"\n";
        fs::write(&almacen.ruta, invalido).expect("escribir el archivo inválido");
        assert!(
            almacen.modificar(|_| Ok(())).is_err(),
            "un vencimiento inválido no se guarda"
        );
        assert_eq!(
            fs::read_to_string(&almacen.ruta).expect("leer el archivo"),
            invalido,
            "el archivo inválido debe quedar intacto"
        );
    }

    #[test]
    fn asigna_ids_faltantes() {
        let (_dir, almacen) = temporal();
        fs::write(
            &almacen.ruta,
            "[[clase]]\nmateria = \"Control\"\ndia = \"lun\"\n\
             inicio = \"07:00\"\nfin = \"09:00\"\n",
        )
        .expect("escribir una clase sin id");

        let datos = almacen.cargar().expect("cargar asigna el id");
        let id = datos.clases[0].id.clone();
        assert!(id.starts_with("c-"), "id: {id}");

        let leido = fs::read_to_string(&almacen.ruta).expect("leer el archivo");
        assert!(leido.contains(&id), "el id debe quedar en disco:\n{leido}");
    }

    /// El historial se escribe junto al archivo y solo se lleva lo viejo.
    #[test]
    fn archiva_a_historial() {
        let (_dir, almacen) = temporal();
        fs::write(
            &almacen.ruta,
            "\
# mi nota
[[tarea]]
id = \"t-vieja\"
titulo = \"De octubre\"
vence = \"2026-10-05T23:59\"
hecha = true
hecha_en = \"2026-09-20\"

[[tarea]]
id = \"t-nueva\"
titulo = \"Sin hacer\"
vence = \"2026-12-01T23:59\"
hecha = false
",
        )
        .expect("escribir el archivo con una tarea vieja");

        let datos = almacen.archivar().expect("archivar");
        assert!(
            !datos.tareas.iter().any(|t| t.id == "t-vieja"),
            "la tarea vieja sale de order.toml"
        );
        assert!(
            datos.tareas.iter().any(|t| t.id == "t-nueva"),
            "la pendiente se queda"
        );

        let principal = fs::read_to_string(&almacen.ruta).expect("leer el archivo");
        assert!(
            principal.contains("# mi nota"),
            "los comentarios sobreviven:\n{principal}"
        );

        let historial = fs::read_to_string(almacen.historial().ruta).expect("leer el historial");
        assert!(historial.contains("t-vieja"), "historial:\n{historial}");
    }

    /// Sin tareas viejas no se reescribe nada (el historial no aparece).
    #[test]
    fn no_archiva_si_no_hace_falta() {
        let (_dir, almacen) = temporal();
        almacen.cargar().expect("crear desde el ejemplo");
        let antes = fs::read_to_string(&almacen.ruta).expect("leer el archivo");

        almacen.archivar().expect("archivar sin nada que mover");

        assert_eq!(
            fs::read_to_string(&almacen.ruta).expect("leer el archivo"),
            antes,
            "el archivo no cambia"
        );
        assert!(
            !almacen.historial().ruta.exists(),
            "no se crea historial vacío"
        );
    }

    #[test]
    fn nuevo_id_usa_32_bits_y_no_colisiona() {
        let mut ids = std::collections::HashSet::new();
        for _ in 0..64 {
            let id = nuevo_id('t');
            let partes: Vec<&str> = id.split('-').collect();
            assert_eq!(partes.len(), 3, "id inesperado: {id}");
            assert_eq!(partes[0], "t");
            assert_eq!(partes[2].len(), 8, "el sufijo debe tener 8 hex (32 bits): {id}");
            assert!(
                u32::from_str_radix(partes[2], 16).is_ok(),
                "sufijo hexadecimal válido: {id}"
            );
            assert!(ids.insert(id), "no deben repetirse ids seguidos");
        }
    }
}
