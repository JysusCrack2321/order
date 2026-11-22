//! Crear, editar, borrar y archivar sobre el documento TOML.
//!
//! Todas las funciones trabajan sobre un `DocumentMut` ya abierto y solo
//! cambian lo justo: al editar se **reemplazan los campos dentro de la tabla
//! existente** (nunca se tira y se vuelve a crear), así los comentarios de
//! arriba y al final de cada línea sobreviven. La lectura y la escritura en
//! disco las hace `almacen::Almacen::modificar`.
//!
//! Horas, fechas y vencimientos se escriben entre comillas y con los mismos
//! formatos que `modelo.rs`: `"HH:MM"`, `"YYYY-MM-DD"` y `"YYYY-MM-DDTHH:MM"`.

use crate::almacen::nuevo_id;
use crate::modelo::{
    Ajustes, Clase, Datos, Dia, FORMATO_FECHA, FORMATO_FECHA_HORA, FORMATO_HORA, Recordatorio,
    Semestre,
};
use chrono::{NaiveDate, NaiveDateTime};
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, Value};

/// Campos que el formulario de tarea entrega para crear o editar una tarea.
pub struct NuevaTarea {
    pub titulo: String,
    pub materia: Option<String>,
    pub vence: NaiveDateTime,
}

// ---------------------------------------------------------------------------
// Tareas
// ---------------------------------------------------------------------------

/// Añade una tarea al final de `[[tarea]]` y devuelve el `id` que se le puso.
pub fn agregar_tarea(doc: &mut DocumentMut, t: &NuevaTarea) -> Result<String, String> {
    let id = nuevo_id('t');
    let mut tabla = Table::new();
    poner(&mut tabla, "id", texto(&id));
    campos_tarea(&mut tabla, t);
    poner(&mut tabla, "hecha", Value::from(false));
    tablas_creadas(doc, "tarea")?.push(tabla);
    Ok(id)
}

/// Reemplaza título, materia y vencimiento. `hecha` y `hecha_en` no se tocan.
pub fn editar_tarea(doc: &mut DocumentMut, id: &str, t: &NuevaTarea) -> Result<(), String> {
    campos_tarea(por_id(doc, "tarea", "la tarea", id)?, t);
    Ok(())
}

/// Marca o desmarca. Al marcar pone `hecha_en = hoy`; al desmarcar la quita.
pub fn marcar_tarea(
    doc: &mut DocumentMut,
    id: &str,
    hecha: bool,
    hoy: NaiveDate,
) -> Result<(), String> {
    let tabla = por_id(doc, "tarea", "la tarea", id)?;
    poner(tabla, "hecha", Value::from(hecha));
    if hecha {
        poner(tabla, "hecha_en", texto(hoy.format(FORMATO_FECHA)));
    } else {
        tabla.remove("hecha_en");
    }
    Ok(())
}

pub fn borrar_tarea(doc: &mut DocumentMut, id: &str) -> Result<(), String> {
    borrar_por_id(doc, "tarea", "la tarea", id)
}

/// ¿Hay alguna tarea hecha hace más de 30 días que toque archivar? Sirve para
/// no reescribir el archivo al arrancar cuando no hace falta.
pub fn hay_que_archivar(datos: &Datos, hoy: NaiveDate) -> bool {
    datos.tareas.iter().any(|t| {
        t.hecha
            && t.hecha_en
                .is_some_and(|fecha| (hoy - fecha).num_days() > 30)
    })
}

/// Mueve a `historial` las tareas hechas hace más de 30 días y devuelve cuántas
/// movió. Las hechas sin `hecha_en` se quedan: no hay forma de saber su edad.
pub fn archivar_hechas(
    doc: &mut DocumentMut,
    historial: &mut DocumentMut,
    hoy: NaiveDate,
) -> usize {
    let destino = match tablas_creadas(historial, "tarea") {
        Ok(tablas) => tablas,
        // Si el historial no tiene la forma esperada, no se toca nada.
        Err(_) => return 0,
    };
    let Some(tareas) = tablas_de(doc, "tarea").ok().flatten() else {
        return 0;
    };

    let mut archivadas = 0;
    let mut i = 0;
    while i < tareas.len() {
        let vieja = tareas
            .get(i)
            .is_some_and(|tabla| hecha_hace_mas_de(tabla, hoy, 30));
        if vieja {
            // `remove` no devuelve la tabla: se copia antes de quitarla.
            if let Some(mut tabla) = tareas.get(i).cloned() {
                // El comentario que va encima de la tabla es decoración suya:
                // se queda en `order.toml` (encima de la siguiente tarea) para
                // no perder las notas de sección, y la tarea viaja al historial
                // con un salto de línea limpio.
                let prefijo = tabla
                    .decor()
                    .prefix()
                    .and_then(|prefijo| prefijo.as_str())
                    .unwrap_or_default()
                    .to_string();
                tareas.remove(i);
                if !prefijo.trim().is_empty() {
                    if let Some(siguiente) = tareas.get_mut(i) {
                        let actual = siguiente
                            .decor()
                            .prefix()
                            .and_then(|p| p.as_str())
                            .unwrap_or_default()
                            .to_string();
                        siguiente
                            .decor_mut()
                            .set_prefix(format!("{prefijo}{actual}"));
                    } else if i > 0 {
                        if let Some(anterior) = tareas.get_mut(i - 1) {
                            let actual = anterior
                                .decor()
                                .suffix()
                                .and_then(|p| p.as_str())
                                .unwrap_or_default()
                                .to_string();
                            anterior
                                .decor_mut()
                                .set_suffix(format!("{actual}{prefijo}"));
                        }
                    }
                }
                tabla.decor_mut().set_prefix("\n");
                destino.push(tabla);
                archivadas += 1;
            }
        } else {
            i += 1;
        }
    }
    archivadas
}

fn campos_tarea(tabla: &mut Table, t: &NuevaTarea) {
    poner(tabla, "titulo", texto(&t.titulo));
    poner_opcional(tabla, "materia", t.materia.as_deref().map(texto));
    poner(tabla, "vence", texto(t.vence.format(FORMATO_FECHA_HORA)));
}

/// ¿Está hecha y su `hecha_en` es de hace más de `dias` días?
fn hecha_hace_mas_de(tabla: &Table, hoy: NaiveDate, dias: i64) -> bool {
    if tabla.get("hecha").and_then(Item::as_bool) != Some(true) {
        return false;
    }
    let Some(fecha) = tabla
        .get("hecha_en")
        .and_then(Item::as_str)
        .and_then(|texto| NaiveDate::parse_from_str(texto, FORMATO_FECHA).ok())
    else {
        return false;
    };
    (hoy - fecha).num_days() > dias
}

// ---------------------------------------------------------------------------
// Clases, recordatorios, semestre y ajustes
// ---------------------------------------------------------------------------

/// Con `id` vacío crea la clase (y le asigna `id`); si no, reemplaza sus campos.
pub fn guardar_clase(doc: &mut DocumentMut, c: &Clase) -> Result<(), String> {
    if c.id.trim().is_empty() {
        let id = nuevo_id('c');
        let mut tabla = Table::new();
        poner(&mut tabla, "id", texto(&id));
        campos_clase(&mut tabla, c);
        tablas_creadas(doc, "clase")?.push(tabla);
        Ok(())
    } else {
        campos_clase(por_id(doc, "clase", "la clase", c.id.trim())?, c);
        Ok(())
    }
}

pub fn borrar_clase(doc: &mut DocumentMut, id: &str) -> Result<(), String> {
    borrar_por_id(doc, "clase", "la clase", id)
}

/// Con `id` vacío crea el recordatorio; si no, reemplaza sus campos.
pub fn guardar_recordatorio(doc: &mut DocumentMut, r: &Recordatorio) -> Result<(), String> {
    if r.id.trim().is_empty() {
        let id = nuevo_id('r');
        let mut tabla = Table::new();
        poner(&mut tabla, "id", texto(&id));
        campos_recordatorio(&mut tabla, r);
        tablas_creadas(doc, "recordatorio")?.push(tabla);
        Ok(())
    } else {
        campos_recordatorio(
            por_id(doc, "recordatorio", "el recordatorio", r.id.trim())?,
            r,
        );
        Ok(())
    }
}

pub fn borrar_recordatorio(doc: &mut DocumentMut, id: &str) -> Result<(), String> {
    borrar_por_id(doc, "recordatorio", "el recordatorio", id)
}

fn campos_clase(tabla: &mut Table, c: &Clase) {
    poner(tabla, "materia", texto(&c.materia));
    poner(tabla, "dia", texto(dia_texto(c.dia)));
    poner(tabla, "inicio", texto(c.inicio.format(FORMATO_HORA)));
    poner(tabla, "fin", texto(c.fin.format(FORMATO_HORA)));
    poner_opcional(tabla, "lugar", c.lugar.as_deref().map(texto));
}

fn campos_recordatorio(tabla: &mut Table, r: &Recordatorio) {
    poner(tabla, "texto", texto(&r.texto));
    let dias: Array = r.dias.iter().map(|d| dia_texto(*d)).collect();
    poner(tabla, "dias", Value::Array(dias));
    poner(tabla, "hora", texto(r.hora.format(FORMATO_HORA)));
}

/// `None` borra la sección: sin semestre, las clases valen todo el año.
pub fn guardar_semestre(doc: &mut DocumentMut, s: Option<&Semestre>) -> Result<(), String> {
    match s {
        Some(s) => {
            let tabla = tabla_creada(doc, "semestre")?;
            poner(tabla, "inicio", texto(s.inicio.format(FORMATO_FECHA)));
            poner(tabla, "fin", texto(s.fin.format(FORMATO_FECHA)));
        }
        None => {
            doc.remove("semestre");
        }
    }
    Ok(())
}

pub fn guardar_ajustes(doc: &mut DocumentMut, a: &Ajustes) -> Result<(), String> {
    let tabla = tabla_creada(doc, "ajustes")?;
    poner(
        tabla,
        "aviso_dia_antes",
        texto(a.aviso_dia_antes.format(FORMATO_HORA)),
    );
    poner(
        tabla,
        "aviso_horas_antes",
        Value::from(a.aviso_horas_antes as i64),
    );
    Ok(())
}

fn dia_texto(dia: Dia) -> &'static str {
    match dia {
        Dia::Lun => "lun",
        Dia::Mar => "mar",
        Dia::Mie => "mie",
        Dia::Jue => "jue",
        Dia::Vie => "vie",
        Dia::Sab => "sab",
        Dia::Dom => "dom",
    }
}

// ---------------------------------------------------------------------------
// Ayudantes sobre el documento
// ---------------------------------------------------------------------------

/// Valor de texto (entre comillas en el archivo).
fn texto(valor: impl ToString) -> Value {
    Value::from(valor.to_string())
}

/// Escribe `clave` conservando la decoración de la línea: si el campo ya
/// existía, hereda sus espacios y su comentario final (`# …`).
fn poner(tabla: &mut Table, clave: &str, mut nuevo: Value) {
    if let Some(Item::Value(viejo)) = tabla.get(clave) {
        *nuevo.decor_mut() = viejo.decor().clone();
    }
    tabla.insert(clave, Item::Value(nuevo));
}

/// Igual que `poner`, pero un `None` quita el campo del archivo.
fn poner_opcional(tabla: &mut Table, clave: &str, valor: Option<Value>) {
    match valor {
        Some(valor) => poner(tabla, clave, valor),
        None => {
            tabla.remove(clave);
        }
    }
}

fn no_existe(que: &str, id: &str) -> String {
    format!("no existe {que} con id \"{id}\"")
}

fn indice(tablas: &ArrayOfTables, id: &str) -> Option<usize> {
    tablas
        .iter()
        .position(|tabla| tabla.get("id").and_then(Item::as_str) == Some(id))
}

/// La tabla con ese `id`, o `Err` si no está.
fn por_id<'a>(
    doc: &'a mut DocumentMut,
    seccion: &str,
    que: &str,
    id: &str,
) -> Result<&'a mut Table, String> {
    let tablas = tablas_de(doc, seccion)?.ok_or_else(|| no_existe(que, id))?;
    let i = indice(tablas, id).ok_or_else(|| no_existe(que, id))?;
    tablas.get_mut(i).ok_or_else(|| no_existe(que, id))
}

fn borrar_por_id(doc: &mut DocumentMut, seccion: &str, que: &str, id: &str) -> Result<(), String> {
    let tablas = tablas_de(doc, seccion)?.ok_or_else(|| no_existe(que, id))?;
    let i = indice(tablas, id).ok_or_else(|| no_existe(que, id))?;
    tablas.remove(i);
    Ok(())
}

/// La sección `[[nombre]]`, si existe. Acepta también la forma compacta
/// `nombre = [{ … }]` y la convierte a tablas normales para poder editarla.
fn tablas_de<'a>(
    doc: &'a mut DocumentMut,
    nombre: &str,
) -> Result<Option<&'a mut ArrayOfTables>, String> {
    let conversion = match doc.get(nombre) {
        Some(Item::ArrayOfTables(_)) => None,
        Some(Item::Value(Value::Array(valores))) => {
            let mut tablas = ArrayOfTables::new();
            for valor in valores.iter() {
                let Value::InlineTable(tabla) = valor else {
                    return Err(forma_inesperada(nombre));
                };
                tablas.push(tabla.clone().into_table());
            }
            Some(Item::ArrayOfTables(tablas))
        }
        Some(_) => return Err(forma_inesperada(nombre)),
        None => None,
    };
    if let Some(item) = conversion {
        doc.insert(nombre, item);
    }
    Ok(doc.get_mut(nombre).and_then(Item::as_array_of_tables_mut))
}

/// Igual que `tablas_de`, pero crea la sección vacía si no existe.
fn tablas_creadas<'a>(
    doc: &'a mut DocumentMut,
    nombre: &str,
) -> Result<&'a mut ArrayOfTables, String> {
    if doc.get(nombre).is_none() {
        doc.insert(nombre, Item::ArrayOfTables(ArrayOfTables::new()));
    }
    tablas_de(doc, nombre)?.ok_or_else(|| forma_inesperada(nombre))
}

/// La tabla normal `[nombre]`, creándola (o convirtiendo la forma compacta) si
/// hace falta.
fn tabla_creada<'a>(doc: &'a mut DocumentMut, nombre: &str) -> Result<&'a mut Table, String> {
    let conversion = match doc.get(nombre) {
        Some(Item::Table(_)) => None,
        Some(Item::Value(Value::InlineTable(tabla))) => {
            Some(Item::Table(tabla.clone().into_table()))
        }
        Some(_) => return Err(forma_inesperada(nombre)),
        None => Some(Item::Table(Table::new())),
    };
    if let Some(item) = conversion {
        doc.insert(nombre, item);
    }
    doc.get_mut(nombre)
        .and_then(Item::as_table_mut)
        .ok_or_else(|| forma_inesperada(nombre))
}

fn forma_inesperada(nombre: &str) -> String {
    format!("la sección «{nombre}» no tiene la forma esperada")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::modelo::{self, FORMATO_FECHA, FORMATO_FECHA_HORA, FORMATO_HORA};
    use chrono::NaiveTime;

    const BASE: &str = "\
# archivo de prueba
[ajustes]
aviso_dia_antes = \"20:00\"
aviso_horas_antes = 3          # horas antes de vencer

[semestre]
inicio = \"2026-08-03\"
fin = \"2026-12-05\"

[[clase]]
id = \"c-uno\"
materia = \"Control\"
dia = \"lun\"
inicio = \"07:00\"
fin = \"09:00\"
lugar = \"453-201\"

[[tarea]]
# esta tarea es del taller
id = \"t-uno\"
titulo = \"Taller 2\"
materia = \"Control\"
vence = \"2026-11-28T23:59\"
hecha = false

[[recordatorio]]
id = \"r-uno\"
texto = \"Revisar Moodle\"
dias = [\"lun\", \"mie\", \"vie\"]
hora = \"18:00\"
";

    fn doc(texto: &str) -> DocumentMut {
        texto.parse().expect("TOML de prueba válido")
    }

    fn fecha(texto: &str) -> NaiveDate {
        NaiveDate::parse_from_str(texto, FORMATO_FECHA).expect("fecha de prueba")
    }

    fn momento(texto: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(texto, FORMATO_FECHA_HORA).expect("vencimiento de prueba")
    }

    fn hora(texto: &str) -> NaiveTime {
        NaiveTime::parse_from_str(texto, FORMATO_HORA).expect("hora de prueba")
    }

    fn tarea(titulo: &str, materia: Option<&str>, vence: &str) -> NuevaTarea {
        NuevaTarea {
            titulo: titulo.to_string(),
            materia: materia.map(str::to_string),
            vence: momento(vence),
        }
    }

    #[test]
    fn agregar_y_editar_tarea() {
        let mut doc = doc(BASE);
        let id = agregar_tarea(
            &mut doc,
            &tarea("Taller 3", Some("Control"), "2026-12-10T18:00"),
        )
        .expect("agregar la tarea");
        assert!(id.starts_with("t-"), "id: {id}");

        let datos = modelo::parsear(&doc.to_string()).expect("el documento sigue siendo válido");
        assert_eq!(datos.tareas.len(), 2, "la tarea nueva se añadió");
        assert_eq!(
            datos.tareas[1].vence,
            momento("2026-12-10T18:00"),
            "el vencimiento se escribe con el formato de modelo.rs"
        );

        editar_tarea(
            &mut doc,
            &id,
            &tarea("Taller 3 corregido", None, "2026-12-11T20:00"),
        )
        .expect("editar la tarea");

        let datos = modelo::parsear(&doc.to_string()).expect("el documento sigue siendo válido");
        assert_eq!(datos.tareas.len(), 2, "editar no duplica la tarea");
        let guardada = datos
            .tareas
            .iter()
            .find(|t| t.id == id)
            .expect("la tarea conserva su id");
        assert_eq!(guardada.titulo, "Taller 3 corregido");
        assert_eq!(guardada.materia, None);
        assert_eq!(guardada.vence, momento("2026-12-11T20:00"));
        assert!(
            doc.to_string().contains("# esta tarea es del taller"),
            "los comentarios de otra tarea sobreviven:\n{doc}"
        );
    }

    #[test]
    fn marcar_y_desmarcar() {
        let mut doc = doc(BASE);
        let hoy = fecha("2026-11-21");

        marcar_tarea(&mut doc, "t-uno", true, hoy).expect("marcar");
        let datos = modelo::parsear(&doc.to_string()).expect("válido");
        assert!(datos.tareas[0].hecha);
        assert_eq!(datos.tareas[0].hecha_en, Some(hoy));

        marcar_tarea(&mut doc, "t-uno", false, hoy).expect("desmarcar");
        let datos = modelo::parsear(&doc.to_string()).expect("válido");
        assert!(!datos.tareas[0].hecha);
        assert_eq!(datos.tareas[0].hecha_en, None);
        assert!(
            !doc.to_string().contains("hecha_en"),
            "al desmarcar no queda la fecha:\n{doc}"
        );
    }

    #[test]
    fn borrar_id_inexistente_falla() {
        let mut doc = doc(BASE);
        assert!(
            borrar_tarea(&mut doc, "x").is_err(),
            "borrar una tarea que no existe es un error"
        );
        assert!(
            borrar_clase(&mut doc, "x").is_err(),
            "borrar una clase que no existe es un error"
        );
        assert!(
            borrar_recordatorio(&mut doc, "x").is_err(),
            "borrar un recordatorio que no existe es un error"
        );
        let texto = doc.to_string();
        assert_eq!(
            modelo::parsear(&texto).expect("válido").tareas.len(),
            1,
            "el documento no cambió"
        );
    }

    #[test]
    fn crud_clase_y_recordatorio() {
        let mut doc = doc(BASE);
        let antes = modelo::parsear(&doc.to_string()).expect("válido");
        let clases = antes.clases.len();
        let recordatorios = antes.recordatorios.len();

        // Crear.
        let clase = Clase {
            id: String::new(),
            materia: "Robótica".to_string(),
            dia: Dia::Mie,
            inicio: hora("09:00"),
            fin: hora("11:00"),
            lugar: Some("Lab 3".to_string()),
        };
        guardar_clase(&mut doc, &clase).expect("crear la clase");
        let recordatorio = Recordatorio {
            id: String::new(),
            texto: "Revisar Classroom".to_string(),
            dias: vec![Dia::Sab],
            hora: hora("10:30"),
        };
        guardar_recordatorio(&mut doc, &recordatorio).expect("crear el recordatorio");

        let datos = modelo::parsear(&doc.to_string()).expect("válido");
        assert_eq!(datos.clases.len(), clases + 1);
        assert_eq!(datos.recordatorios.len(), recordatorios + 1);
        let nueva_clase = datos.clases.last().expect("la clase creada").clone();
        let nuevo_rec = datos
            .recordatorios
            .last()
            .expect("el recordatorio creado")
            .clone();
        assert!(nueva_clase.id.starts_with("c-"), "id: {}", nueva_clase.id);
        assert!(nuevo_rec.id.starts_with("r-"), "id: {}", nuevo_rec.id);

        // Reemplazar.
        let mut editada = nueva_clase.clone();
        editada.materia = "Robótica II".to_string();
        editada.dia = Dia::Jue;
        editada.lugar = None;
        guardar_clase(&mut doc, &editada).expect("reemplazar la clase");
        let mut editado = nuevo_rec.clone();
        editado.texto = "Revisar Classroom y Moodle".to_string();
        editado.dias = vec![Dia::Sab, Dia::Dom];
        guardar_recordatorio(&mut doc, &editado).expect("reemplazar el recordatorio");

        let datos = modelo::parsear(&doc.to_string()).expect("válido");
        assert_eq!(datos.clases.len(), clases + 1, "reemplazar no duplica");
        assert_eq!(
            datos.recordatorios.len(),
            recordatorios + 1,
            "reemplazar no duplica"
        );
        let guardada = datos
            .clases
            .iter()
            .find(|c| c.id == nueva_clase.id)
            .expect("la clase conserva su id");
        assert_eq!(guardada.materia, "Robótica II");
        assert_eq!(guardada.dia, Dia::Jue);
        assert_eq!(guardada.lugar, None);
        let guardado = datos
            .recordatorios
            .iter()
            .find(|r| r.id == nuevo_rec.id)
            .expect("el recordatorio conserva su id");
        assert_eq!(guardado.texto, "Revisar Classroom y Moodle");
        assert_eq!(guardado.dias, vec![Dia::Sab, Dia::Dom]);
        assert_eq!(guardado.hora, hora("10:30"));

        // Borrar: la cuenta vuelve a la inicial.
        borrar_clase(&mut doc, &nueva_clase.id).expect("borrar la clase");
        borrar_recordatorio(&mut doc, &nuevo_rec.id).expect("borrar el recordatorio");
        let datos = modelo::parsear(&doc.to_string()).expect("válido");
        assert_eq!(datos.clases.len(), clases);
        assert_eq!(datos.recordatorios.len(), recordatorios);
    }

    #[test]
    fn archiva_solo_mayores_de_30() {
        let mut doc = doc("\
[[tarea]]
id = \"t-vieja\"
titulo = \"De octubre\"
vence = \"2026-10-05T23:59\"
hecha = true
hecha_en = \"2026-10-01\"

[[tarea]]
id = \"t-reciente\"
titulo = \"De noviembre\"
vence = \"2026-11-16T23:59\"
hecha = true
hecha_en = \"2026-11-15\"

[[tarea]]
id = \"t-pendiente\"
titulo = \"Sin hacer\"
vence = \"2026-11-30T23:59\"
hecha = false
");
        let mut historial = DocumentMut::new();
        let hoy = fecha("2026-11-21");

        let movidas = archivar_hechas(&mut doc, &mut historial, hoy);
        assert_eq!(movidas, 1, "solo la de hace más de 30 días");

        let actual = modelo::parsear(&doc.to_string()).expect("válido");
        assert!(
            !actual.tareas.iter().any(|t| t.id == "t-vieja"),
            "sale de order.toml"
        );
        assert!(
            actual.tareas.iter().any(|t| t.id == "t-reciente"),
            "la reciente se queda"
        );
        assert!(
            actual.tareas.iter().any(|t| t.id == "t-pendiente"),
            "la pendiente se queda"
        );

        let pasado = modelo::parsear(&historial.to_string()).expect("historial válido");
        assert_eq!(pasado.tareas.len(), 1);
        assert_eq!(pasado.tareas[0].id, "t-vieja");
        assert_eq!(pasado.tareas[0].hecha_en, Some(fecha("2026-10-01")));
    }
}
