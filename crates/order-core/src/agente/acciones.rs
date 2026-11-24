//! Acciones que propone el agente y cómo se aplican al archivo.
//!
//! El modelo responde `{"respuesta": "...", "acciones": [...]}`. Aquí se lee
//! esa respuesta con tolerancia (bloques de código, texto alrededor, campos que
//! sobran) y se aplica **una** acción cuando el usuario pulsa Aplicar. Todo
//! pasa por `ops`, igual que los formularios: crear una tarea siempre por
//! `ops::agregar_tarea`.

use crate::almacen::Almacen;
use crate::modelo::{
    self, Clase, Datos, Dia, ErrorDatos, Evento, FORMATO_FECHA, FORMATO_FECHA_HORA, FORMATO_HORA,
    Recordatorio, Tarea,
};
use crate::ops::{self, NuevaTarea, NuevoEvento};
use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use toml_edit::DocumentMut;

/// Una acción propuesta. El campo `tipo` la distingue en el JSON:
/// `crear_tarea`, `editar_tarea`, `marcar_tarea`, `borrar_tarea`,
/// `guardar_clase`, `borrar_clase`, `guardar_recordatorio`, `borrar_recordatorio`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Accion {
    /// Crear una tarea. `vence` en `YYYY-MM-DDTHH:MM`.
    CrearTarea {
        titulo: String,
        #[serde(default)]
        materia: Option<String>,
        vence: String,
    },
    /// Cambiar solo los campos presentes; los que falten se conservan.
    EditarTarea {
        id: String,
        #[serde(default)]
        titulo: Option<String>,
        #[serde(default)]
        materia: Option<String>,
        #[serde(default)]
        vence: Option<String>,
    },
    MarcarTarea { id: String, hecha: bool },
    BorrarTarea { id: String },
    /// Con `id` edita la clase; sin `id` la crea.
    GuardarClase {
        #[serde(default)]
        id: Option<String>,
        materia: String,
        #[serde(default)]
        dias: Vec<String>,
        inicio: String,
        fin: String,
        #[serde(default)]
        lugar: Option<String>,
    },
    BorrarClase { id: String },
    GuardarRecordatorio {
        #[serde(default)]
        id: Option<String>,
        texto: String,
        #[serde(default)]
        dias: Vec<String>,
        hora: String,
    },
    BorrarRecordatorio { id: String },
    CrearEvento {
        #[serde(default)]
        titulo: String,
        fecha: String,
        inicio: String,
        fin: String,
        #[serde(default)]
        tarea: Option<String>,
        #[serde(default)]
        nota: Option<String>,
    },
    MoverEvento {
        id: String,
        #[serde(default)]
        fecha: Option<String>,
        #[serde(default)]
        inicio: Option<String>,
        #[serde(default)]
        fin: Option<String>,
        #[serde(default)]
        titulo: Option<String>,
        #[serde(default)]
        nota: Option<String>,
    },
    BorrarEvento { id: String },
}

impl Accion {
    /// Verbo corto para la tarjeta ("Crear tarea", "Borrar clase"…).
    pub fn etiqueta(&self) -> &'static str {
        match self {
            Accion::CrearTarea { .. } => "Crear tarea",
            Accion::EditarTarea { .. } => "Editar tarea",
            Accion::MarcarTarea { .. } => "Marcar tarea",
            Accion::BorrarTarea { .. } => "Borrar tarea",
            Accion::GuardarClase { id: None, .. } => "Crear clase",
            Accion::GuardarClase { .. } => "Editar clase",
            Accion::BorrarClase { .. } => "Borrar clase",
            Accion::GuardarRecordatorio { id: None, .. } => "Crear recordatorio",
            Accion::GuardarRecordatorio { .. } => "Editar recordatorio",
            Accion::BorrarRecordatorio { .. } => "Borrar recordatorio",
            Accion::CrearEvento { .. } => "Crear evento",
            Accion::MoverEvento { .. } => "Mover evento",
            Accion::BorrarEvento { .. } => "Borrar evento",
        }
    }

    /// Descripción legible en español, con los datos que ya existen (para
    /// nombrar la tarea o la clase que se va a tocar).
    pub fn descripcion(&self, datos: &Datos) -> String {
        match self {
            Accion::CrearTarea {
                titulo,
                materia,
                vence,
            } => {
                let materia = materia
                    .as_deref()
                    .map(|materia| format!(" de {materia}"))
                    .unwrap_or_default();
                format!(
                    "Crear la tarea «{}»{materia} para el {}.",
                    titulo.trim(),
                    fecha_legible(vence)
                )
            }
            Accion::EditarTarea {
                id,
                titulo,
                materia,
                vence,
            } => {
                let nombre = titulo
                    .clone()
                    .or_else(|| tarea_de(datos, id).map(|tarea| tarea.titulo.clone()))
                    .unwrap_or_else(|| id.clone());
                let mut cambios = Vec::new();
                if let Some(titulo) = titulo {
                    cambios.push(format!("título «{}»", titulo.trim()));
                }
                if let Some(materia) = materia {
                    cambios.push(format!("materia {}", materia.trim()));
                }
                if let Some(vence) = vence {
                    cambios.push(format!("vence el {}", fecha_legible(vence)));
                }
                if cambios.is_empty() {
                    format!("Editar la tarea «{nombre}» sin cambios.")
                } else {
                    format!("Editar la tarea «{nombre}»: {}.", cambios.join(", "))
                }
            }
            Accion::MarcarTarea { id, hecha } => {
                let nombre = tarea_de(datos, id)
                    .map(|tarea| tarea.titulo.clone())
                    .unwrap_or_else(|| id.clone());
                format!(
                    "{} la tarea «{}».",
                    if *hecha {
                        "Marcar como hecha"
                    } else {
                        "Desmarcar"
                    },
                    nombre
                )
            }
            Accion::BorrarTarea { id } => {
                let nombre = tarea_de(datos, id)
                    .map(|tarea| tarea.titulo.clone())
                    .unwrap_or_else(|| id.clone());
                format!("Borrar la tarea «{nombre}».")
            }
            Accion::GuardarClase { id, materia, .. } => {
                let nombre = id
                    .as_ref()
                    .and_then(|id| datos.clases.iter().find(|clase| &clase.id == id))
                    .map(|clase| clase.materia.clone())
                    .unwrap_or_else(|| materia.trim().to_string());
                format!(
                    "{} de {}.",
                    if id.is_some() {
                        "Editar la clase"
                    } else {
                        "Crear la clase"
                    },
                    nombre
                )
            }
            Accion::BorrarClase { id } => {
                let nombre = datos
                    .clases
                    .iter()
                    .find(|clase| &clase.id == id)
                    .map(|clase| clase.materia.clone())
                    .unwrap_or_else(|| id.clone());
                format!("Borrar la clase de {nombre}.")
            }
            Accion::GuardarRecordatorio { id, texto, .. } => {
                let nombre = id
                    .as_ref()
                    .and_then(|id| datos.recordatorios.iter().find(|r| &r.id == id))
                    .map(|recordatorio| recordatorio.texto.clone())
                    .unwrap_or_else(|| texto.trim().to_string());
                format!(
                    "{} «{}».",
                    if id.is_some() {
                        "Editar el recordatorio"
                    } else {
                        "Crear el recordatorio"
                    },
                    nombre
                )
            }
            Accion::BorrarRecordatorio { id } => {
                let nombre = datos
                    .recordatorios
                    .iter()
                    .find(|recordatorio| &recordatorio.id == id)
                    .map(|recordatorio| recordatorio.texto.clone())
                    .unwrap_or_else(|| id.clone());
                format!("Borrar el recordatorio «{nombre}».")
            }
            Accion::CrearEvento {
                titulo,
                fecha,
                inicio,
                fin,
                tarea,
                ..
            } => {
                let nombre = {
                    let limpio = titulo.trim();
                    if !limpio.is_empty() {
                        limpio.to_string()
                    } else if let Some(tid) = tarea.as_deref() {
                        tarea_de(datos, tid)
                            .map(|t| t.titulo.clone())
                            .unwrap_or_else(|| tid.to_string())
                    } else {
                        String::new()
                    }
                };
                let enlace = tarea
                    .as_deref()
                    .and_then(|tid| tarea_de(datos, tid))
                    .map(|t| format!(" (para la tarea «{}»)", t.titulo))
                    .unwrap_or_default();
                format!(
                    "Crear el evento «{nombre}» el {} de {} a {}{enlace}.",
                    fecha_solo_legible(fecha),
                    inicio.trim(),
                    fin.trim()
                )
            }
            Accion::MoverEvento {
                id,
                fecha,
                inicio,
                fin,
                titulo,
                ..
            } => {
                let nombre = titulo
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| evento_de(datos, id).map(|e| e.titulo.clone()))
                    .unwrap_or_else(|| id.clone());
                let mut cambios = Vec::new();
                if let Some(f) = fecha {
                    cambios.push(format!("fecha {}", fecha_solo_legible(f)));
                }
                if let (Some(i), Some(f)) = (inicio, fin) {
                    cambios.push(format!("{}–{}", i.trim(), f.trim()));
                } else {
                    if let Some(i) = inicio {
                        cambios.push(format!("inicio {}", i.trim()));
                    }
                    if let Some(f) = fin {
                        cambios.push(format!("fin {}", f.trim()));
                    }
                }
                if cambios.is_empty() {
                    format!("Mover el evento «{nombre}».")
                } else {
                    format!("Mover el evento «{nombre}»: {}.", cambios.join(", "))
                }
            }
            Accion::BorrarEvento { id } => {
                let nombre = evento_de(datos, id)
                    .map(|e| e.titulo.clone())
                    .unwrap_or_else(|| id.clone());
                format!("Borrar el evento «{nombre}».")
            }
        }
    }
}

/// Lo que devuelve el modelo ya leído.
#[derive(Debug, Clone, PartialEq)]
pub struct Respuesta {
    pub respuesta: String,
    pub acciones: Vec<Accion>,
    pub resumen: Option<String>,
}

/// Lee la respuesta del modelo: acepta el JSON solo, dentro de un bloque de
/// código (con o sin `json`) o rodeado de texto. Las acciones que no encajan
/// se descartan; si ninguna encaja, es un error legible.
pub fn parsear(texto: &str) -> Result<Respuesta, String> {
    let crudo = extraer_json(texto)
        .ok_or_else(|| "la respuesta del agente no trae ningún JSON".to_string())?;
    let valor: Value =
        serde_json::from_str(&crudo).map_err(|e| format!("la respuesta no es JSON válido: {e}"))?;

    let respuesta = valor
        .get("respuesta")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();

    let resumen = valor
        .get("resumen")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let lista = valor.get("acciones").and_then(Value::as_array);
    let mut acciones = Vec::new();
    let mut fallos = 0;
    for item in lista.into_iter().flatten() {
        match serde_json::from_value::<Accion>(item.clone()) {
            Ok(accion) => acciones.push(accion),
            Err(_) => fallos += 1,
        }
    }
    if acciones.is_empty() && fallos > 0 {
        return Err("el agente propuso acciones que no pude entender".to_string());
    }
    Ok(Respuesta {
        respuesta,
        acciones,
        resumen,
    })
}

/// Del primer `{` al último `}`: así el bloque ```json y el texto alrededor
/// quedan fuera sin tener que analizarlos.
fn extraer_json(texto: &str) -> Option<String> {
    let inicio = texto.find('{')?;
    let fin = texto.rfind('}')?;
    if fin <= inicio {
        return None;
    }
    Some(texto[inicio..=fin].to_string())
}

/// Esquema JSON de la respuesta, para el `output_config.format` de Claude.
pub fn esquema() -> Value {
    let texto = json!({ "type": "string" });
    let opcional = json!({ "type": ["string", "null"] });
    let dias = json!({ "type": "array", "items": { "type": "string" } });
    let accion = json!({
        "type": "object",
        "properties": {
            "tipo": {
                "type": "string",
                "enum": [
                    "crear_tarea", "editar_tarea", "marcar_tarea", "borrar_tarea",
                    "guardar_clase", "borrar_clase", "guardar_recordatorio", "borrar_recordatorio",
                    "crear_evento", "mover_evento", "borrar_evento"
                ]
            },
            "id": { "type": ["string", "null"] },
            "titulo": opcional,
            "texto": { "type": "string" },
            "materia": opcional,
            "vence": { "type": "string" },
            "hecha": { "type": "boolean" },
            "dias": dias,
            "inicio": opcional,
            "fin": opcional,
            "hora": { "type": "string" },
            "lugar": opcional,
            "fecha": opcional,
            "tarea": opcional,
            "nota": opcional
        },
        "required": ["tipo"],
        "additionalProperties": false
    });
    json!({
        "type": "object",
        "properties": {
            "respuesta": texto,
            "acciones": { "type": "array", "items": accion }
        },
        "required": ["respuesta", "acciones"],
        "additionalProperties": false
    })
}

// ---------------------------------------------------------------------------
// Aplicar
// ---------------------------------------------------------------------------

/// Resultado de aplicar en orden varias acciones propuestas.
#[derive(Debug, Clone)]
pub struct ResultadoAplicarTodas {
    pub aplicadas: usize,
    pub ultimos_datos: Option<Datos>,
    pub error: Option<String>,
}

/// Aplica una acción sobre el archivo real: relee, valida, cambia y guarda.
pub fn aplicar(almacen: &Almacen, accion: &Accion) -> Result<Datos, ErrorDatos> {
    almacen.modificar(|doc| aplicar_en(doc, accion))
}

/// Aplica en orden; si una falla, se detiene y dice cuántas quedaron hechas.
pub fn aplicar_todas(almacen: &Almacen, acciones: &[Accion]) -> ResultadoAplicarTodas {
    let mut aplicadas = 0;
    let mut ultimos_datos = None;
    for accion in acciones {
        match aplicar(almacen, accion) {
            Ok(datos) => {
                ultimos_datos = Some(datos);
                aplicadas += 1;
            }
            Err(error) => {
                return ResultadoAplicarTodas {
                    aplicadas,
                    ultimos_datos,
                    error: Some(format!(
                        "Aplicadas {aplicadas}; falló «{}» — {error}",
                        accion.etiqueta()
                    )),
                };
            }
        }
    }
    ResultadoAplicarTodas {
        aplicadas,
        ultimos_datos,
        error: None,
    }
}

/// La misma aplicación sobre un documento ya abierto (por `modificar`). Las
/// validaciones se hacen contra el documento recién leído, así un id borrado
/// entre la propuesta y el clic se detecta antes de tocar nada.
pub fn aplicar_en(doc: &mut DocumentMut, accion: &Accion) -> Result<(), String> {
    let datos = modelo::parsear(&doc.to_string()).map_err(|error| error.to_string())?;
    let hoy = Local::now().date_naive();

    match accion {
        Accion::CrearTarea {
            titulo,
            materia,
            vence,
        } => {
            let titulo = titulo.trim();
            if titulo.is_empty() {
                return Err("la tarea nueva no tiene título".to_string());
            }
            let nueva = NuevaTarea {
                titulo: titulo.to_string(),
                materia: materia_de(&datos, materia.as_deref()),
                vence: fecha_hora(vence, "el vencimiento")?,
            };
            ops::agregar_tarea(doc, &nueva).map(|_| ())
        }
        Accion::EditarTarea {
            id,
            titulo,
            materia,
            vence,
        } => {
            let actual = tarea_de(&datos, id)
                .cloned()
                .ok_or_else(|| no_existe("la tarea", id))?;
            let nueva = NuevaTarea {
                titulo: titulo
                    .as_deref()
                    .map(str::trim)
                    .filter(|titulo| !titulo.is_empty())
                    .map(str::to_string)
                    .unwrap_or(actual.titulo),
                materia: match materia {
                    Some(materia) => materia_de(&datos, Some(materia)),
                    None => actual.materia,
                },
                vence: match vence {
                    Some(vence) => fecha_hora(vence, "el vencimiento")?,
                    None => actual.vence,
                },
            };
            ops::editar_tarea(doc, id, &nueva)
        }
        Accion::MarcarTarea { id, hecha } => {
            if tarea_de(&datos, id).is_none() {
                return Err(no_existe("la tarea", id));
            }
            ops::marcar_tarea(doc, id, *hecha, hoy)
        }
        Accion::BorrarTarea { id } => {
            if tarea_de(&datos, id).is_none() {
                return Err(no_existe("la tarea", id));
            }
            ops::borrar_tarea(doc, id)
        }
        Accion::GuardarClase {
            id,
            materia,
            dias,
            inicio,
            fin,
            lugar,
        } => {
            let actual = id
                .as_ref()
                .and_then(|id| datos.clases.iter().find(|clase| &clase.id == id).cloned());
            if let Some(id) = id {
                if actual.is_none() {
                    return Err(no_existe("la clase", id));
                }
            }
            let materia = materia.trim();
            if materia.is_empty() {
                return Err("la clase no tiene materia".to_string());
            }
            let dias = dias_de(dias, actual.as_ref().map(|clase| clase.dias.as_slice()))?;
            let inicio = hora(inicio, "la hora de inicio")?;
            let fin = hora(fin, "la hora de fin")?;
            if fin <= inicio {
                return Err("la hora de fin de la clase debe ser posterior a la de inicio".to_string());
            }
            let clase = Clase {
                id: id.clone().unwrap_or_default(),
                materia: modelo::canonica_materia(&materias(&datos), materia),
                dias,
                inicio,
                fin,
                lugar: match lugar {
                    Some(lugar) => opcional(lugar),
                    None => actual.and_then(|clase| clase.lugar),
                },
            };
            ops::guardar_clase(doc, &clase)
        }
        Accion::BorrarClase { id } => {
            if !datos.clases.iter().any(|clase| &clase.id == id) {
                return Err(no_existe("la clase", id));
            }
            ops::borrar_clase(doc, id)
        }
        Accion::GuardarRecordatorio {
            id,
            texto,
            dias,
            hora: cuando,
        } => {
            let actual = id.as_ref().and_then(|id| {
                datos
                    .recordatorios
                    .iter()
                    .find(|recordatorio| &recordatorio.id == id)
                    .cloned()
            });
            if let Some(id) = id {
                if actual.is_none() {
                    return Err(no_existe("el recordatorio", id));
                }
            }
            let texto = texto.trim();
            if texto.is_empty() {
                return Err("el recordatorio no tiene texto".to_string());
            }
            let dias = dias_de(dias, actual.as_ref().map(|r| r.dias.as_slice()))?;
            let recordatorio = Recordatorio {
                id: id.clone().unwrap_or_default(),
                texto: texto.to_string(),
                dias,
                hora: hora(cuando, "la hora del recordatorio")?,
            };
            ops::guardar_recordatorio(doc, &recordatorio)
        }
        Accion::BorrarRecordatorio { id } => {
            if !datos
                .recordatorios
                .iter()
                .any(|recordatorio| &recordatorio.id == id)
            {
                return Err(no_existe("el recordatorio", id));
            }
            ops::borrar_recordatorio(doc, id)
        }
        Accion::CrearEvento {
            titulo,
            fecha,
            inicio,
            fin,
            tarea,
            nota,
        } => {
            let tarea_id = match tarea.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(tid) => {
                    if tarea_de(&datos, tid).is_none() {
                        return Err(no_existe("la tarea", tid));
                    }
                    Some(tid.to_string())
                }
                None => None,
            };
            let titulo = {
                let limpio = titulo.trim();
                if !limpio.is_empty() {
                    limpio.to_string()
                } else if let Some(tid) = tarea_id.as_deref() {
                    tarea_de(&datos, tid)
                        .map(|t| t.titulo.clone())
                        .unwrap_or_default()
                } else {
                    return Err("el evento nuevo no tiene título".to_string());
                }
            };
            let fecha = fecha_simple(fecha, "la fecha del evento")?;
            let inicio = hora(inicio, "la hora de inicio")?;
            let fin = hora(fin, "la hora de fin")?;
            if fin <= inicio {
                return Err(
                    "la hora de fin del evento debe ser posterior a la de inicio".to_string(),
                );
            }
            let nuevo = NuevoEvento {
                titulo,
                fecha,
                inicio,
                fin,
                tarea: tarea_id,
                nota: nota.as_deref().and_then(opcional),
            };
            ops::agregar_evento(doc, &nuevo).map(|_| ())
        }
        Accion::MoverEvento {
            id,
            fecha,
            inicio,
            fin,
            titulo,
            nota,
        } => {
            let actual = evento_de(&datos, id)
                .cloned()
                .ok_or_else(|| no_existe("el evento", id))?;
            let nueva_fecha = match fecha {
                Some(f) => fecha_simple(f, "la fecha del evento")?,
                None => actual.fecha,
            };
            let nuevo_inicio = match inicio {
                Some(i) => hora(i, "la hora de inicio")?,
                None => actual.inicio,
            };
            let nueva_fin = match fin {
                Some(f) => hora(f, "la hora de fin")?,
                None => {
                    if inicio.is_some() && actual.fin <= nuevo_inicio {
                        let duracion = actual.fin - actual.inicio;
                        let candidato = nuevo_inicio + duracion;
                        if candidato > nuevo_inicio {
                            candidato
                        } else {
                            NaiveTime::from_hms_opt(23, 59, 0).unwrap_or(actual.fin)
                        }
                    } else {
                        actual.fin
                    }
                }
            };
            if nueva_fin <= nuevo_inicio {
                return Err(
                    "la hora de fin del evento debe ser posterior a la de inicio".to_string(),
                );
            }
            let nuevo_titulo = titulo
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            if nuevo_titulo.is_some() || nota.is_some() {
                let editado = NuevoEvento {
                    titulo: nuevo_titulo.unwrap_or(actual.titulo),
                    fecha: nueva_fecha,
                    inicio: nuevo_inicio,
                    fin: nueva_fin,
                    tarea: actual.tarea,
                    nota: match nota {
                        Some(n) => opcional(n),
                        None => actual.nota,
                    },
                };
                ops::editar_evento(doc, id, &editado)
            } else {
                ops::mover_evento(doc, id, nueva_fecha, nuevo_inicio, nueva_fin)
            }
        }
        Accion::BorrarEvento { id } => {
            if evento_de(&datos, id).is_none() {
                return Err(no_existe("el evento", id));
            }
            ops::borrar_evento(doc, id)
        }
    }
}

// ---------------------------------------------------------------------------
// Validación y textos
// ---------------------------------------------------------------------------

fn tarea_de<'a>(datos: &'a Datos, id: &str) -> Option<&'a Tarea> {
    datos.tareas.iter().find(|tarea| tarea.id == id)
}

fn evento_de<'a>(datos: &'a Datos, id: &str) -> Option<&'a Evento> {
    datos.eventos.iter().find(|evento| evento.id == id)
}

fn no_existe(que: &str, id: &str) -> String {
    format!("no existe {que} con id «{id}»")
}

/// Vencimiento tolerante: acepta `YYYY-MM-DDTHH:MM`, con espacio en vez de T y
/// solo la fecha (que se toma como 23:59).
fn fecha_hora(texto: &str, campo: &str) -> Result<NaiveDateTime, String> {
    let limpio = texto.trim().replace(' ', "T");
    if let Ok(momento) = NaiveDateTime::parse_from_str(&limpio, FORMATO_FECHA_HORA) {
        return Ok(momento);
    }
    if let Ok(fecha) = NaiveDate::parse_from_str(&limpio, "%Y-%m-%d") {
        return Ok(fecha.and_hms_opt(23, 59, 0).unwrap_or_default());
    }
    Err(format!(
        "{campo} «{}» no es una fecha válida (se espera YYYY-MM-DDTHH:MM)",
        texto.trim()
    ))
}

fn fecha_legible(texto: &str) -> String {
    match fecha_hora(texto, "la fecha") {
        Ok(momento) => momento.format("%d/%m/%Y a las %H:%M").to_string(),
        Err(_) => format!("«{}»", texto.trim()),
    }
}

fn fecha_simple(texto: &str, campo: &str) -> Result<NaiveDate, String> {
    let limpio = texto.trim();
    NaiveDate::parse_from_str(limpio, FORMATO_FECHA)
        .map_err(|_| format!("{campo} «{limpio}» no es una fecha válida (se espera YYYY-MM-DD)"))
}

fn fecha_solo_legible(texto: &str) -> String {
    match fecha_simple(texto, "la fecha") {
        Ok(fecha) => fecha.format("%d/%m/%Y").to_string(),
        Err(_) => format!("«{}»", texto.trim()),
    }
}

fn hora(texto: &str, campo: &str) -> Result<NaiveTime, String> {
    let limpio = texto.trim();
    NaiveTime::parse_from_str(limpio, FORMATO_HORA)
        .or_else(|_| NaiveTime::parse_from_str(&format!("0{limpio}"), FORMATO_HORA))
        .map_err(|_| format!("{campo} «{limpio}» no es una hora válida (se espera HH:MM)"))
}

/// Días en español a `Dia`, aceptando abreviaturas y nombres completos.
fn dia(texto: &str) -> Result<Dia, String> {
    let clave = modelo::clave_materia(texto);
    let clave = clave.trim_end_matches('.');
    let dia = match clave {
        "lun" | "lunes" => Dia::Lun,
        "mar" | "martes" => Dia::Mar,
        "mie" | "miercoles" => Dia::Mie,
        "jue" | "jueves" => Dia::Jue,
        "vie" | "viernes" => Dia::Vie,
        "sab" | "sabado" => Dia::Sab,
        "dom" | "domingo" => Dia::Dom,
        _ => return Err(format!("«{}» no es un día de la semana", texto.trim())),
    };
    Ok(dia)
}

/// Los días de la acción; si viene vacía y se está editando, se conservan.
fn dias_de(texto: &[String], actuales: Option<&[Dia]>) -> Result<Vec<Dia>, String> {
    if texto.is_empty() {
        if let Some(actuales) = actuales.filter(|dias| !dias.is_empty()) {
            return Ok(actuales.to_vec());
        }
        return Err("no hay ningún día marcado".to_string());
    }
    let mut dias = Vec::new();
    for nombre in texto {
        let dia = dia(nombre)?;
        if !dias.contains(&dia) {
            dias.push(dia);
        }
    }
    // Orden lun → dom, como los escribe el archivo.
    Ok(Dia::TODOS
        .iter()
        .copied()
        .filter(|dia| dias.contains(dia))
        .collect())
}

fn opcional(texto: &str) -> Option<String> {
    let texto = texto.trim();
    if texto.is_empty() {
        None
    } else {
        Some(texto.to_string())
    }
}

/// Materia con la grafía que ya existe; vacía significa "sin materia".
fn materia_de(datos: &Datos, texto: Option<&str>) -> Option<String> {
    let texto = texto?.trim();
    if texto.is_empty() {
        return None;
    }
    opcional(&modelo::canonica_materia(&materias(datos), texto))
}

fn materias(datos: &Datos) -> Vec<String> {
    let mut lista: Vec<String> = Vec::new();
    for materia in datos
        .clases
        .iter()
        .map(|clase| &clase.materia)
        .chain(datos.tareas.iter().filter_map(|tarea| tarea.materia.as_ref()))
    {
        let materia = materia.trim();
        if materia.is_empty() {
            continue;
        }
        let clave = modelo::clave_materia(materia);
        if lista
            .iter()
            .any(|vista| modelo::clave_materia(vista) == clave)
        {
            continue;
        }
        lista.push(materia.to_string());
    }
    lista
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::modelo::Dia;
    use std::fs;

    const BASE: &str = "\
[ajustes]
aviso_dias = 2

[[clase]]
id = \"c-uno\"
materia = \"Control\"
dias = [\"lun\", \"mie\"]
inicio = \"07:00\"
fin = \"09:00\"

[[tarea]]
id = \"t-uno\"
titulo = \"Taller 2\"
materia = \"Control\"
vence = \"2026-11-28T23:59\"
hecha = false

[[recordatorio]]
id = \"r-uno\"
texto = \"Revisar Moodle\"
dias = [\"lun\", \"vie\"]
hora = \"18:00\"
";

    fn temporal() -> (tempfile::TempDir, Almacen) {
        let dir = tempfile::tempdir().expect("directorio temporal");
        let almacen = Almacen {
            ruta: dir.path().join("order.toml"),
        };
        (dir, almacen)
    }

    fn almacen_con_base() -> (tempfile::TempDir, Almacen) {
        let (dir, almacen) = temporal();
        fs::write(&almacen.ruta, BASE).expect("escribir la base");
        (dir, almacen)
    }

    fn datos_base() -> Datos {
        modelo::parsear(BASE).expect("base válida")
    }

    // --- Lectura de la respuesta -----------------------------------------

    #[test]
    fn lee_json_limpio() {
        let respuesta = parsear(
            r#"{"respuesta": "Listo.", "acciones": [
                {"tipo": "crear_tarea", "titulo": "Taller 3", "materia": "Control",
                 "vence": "2026-12-10T18:00"}]}"#,
        )
        .expect("JSON válido");
        assert_eq!(respuesta.respuesta, "Listo.");
        assert_eq!(
            respuesta.acciones,
            vec![Accion::CrearTarea {
                titulo: "Taller 3".into(),
                materia: Some("Control".into()),
                vence: "2026-12-10T18:00".into(),
            }]
        );
    }

    /// El modelo suele envolver el JSON en ```json … ``` y saludar antes.
    #[test]
    fn lee_bloque_de_codigo_rodeado_de_texto() {
        let texto = "Claro, aquí va:\n\n```json\n{\n  \"respuesta\": \"Agregué el taller.\",\n  \
                     \"acciones\": [{\"tipo\": \"crear_tarea\", \"titulo\": \"Taller 3\", \
                     \"vence\": \"2026-12-10T18:00\"}]\n}\n```\n\n¿Algo más?";
        let respuesta = parsear(texto).expect("bloque de código");
        assert_eq!(respuesta.respuesta, "Agregué el taller.");
        assert_eq!(respuesta.acciones.len(), 1);
    }

    /// Un solo objeto, sin `acciones`: es una pregunta contestada.
    #[test]
    fn pregunta_sin_acciones() {
        let respuesta = parsear(r#"{"respuesta": "Tienes 1 tarea pendiente."}"#)
            .expect("respuesta sin acciones");
        assert!(respuesta.acciones.is_empty());
        assert!(respuesta.respuesta.contains("1 tarea"));
    }

    #[test]
    fn accion_invalida_es_error_legible() {
        let error = parsear(r#"{"respuesta": "x", "acciones": [{"tipo": "volar"}]}"#)
            .expect_err("un tipo desconocido no se entiende");
        assert!(error.contains("no pude entender"), "{error}");
    }

    #[test]
    fn accion_valida_sobrevive_a_una_invalida() {
        // Tolerancia: se queda la que sí se entiende.
        let respuesta = parsear(
            r#"{"respuesta": "x", "acciones": [
                {"tipo": "borrar_tarea", "id": "t-uno"},
                {"tipo": "inventada"}]}"#,
        )
        .expect("una acción válida basta");
        assert_eq!(respuesta.acciones.len(), 1);
    }

    #[test]
    fn texto_sin_json_es_error() {
        assert!(parsear("No hay nada que hacer.").is_err());
    }

    // --- Descripciones ----------------------------------------------------

    #[test]
    fn descripciones_en_espanol() {
        let datos = datos_base();
        let crear = Accion::CrearTarea {
            titulo: "Taller 3".into(),
            materia: Some("Control".into()),
            vence: "2026-12-10T18:00".into(),
        };
        assert_eq!(
            crear.descripcion(&datos),
            "Crear la tarea «Taller 3» de Control para el 10/12/2026 a las 18:00."
        );

        let borrar = Accion::BorrarTarea {
            id: "t-uno".into(),
        };
        assert_eq!(borrar.descripcion(&datos), "Borrar la tarea «Taller 2».");

        let marcar = Accion::MarcarTarea {
            id: "t-uno".into(),
            hecha: true,
        };
        assert_eq!(marcar.descripcion(&datos), "Marcar como hecha la tarea «Taller 2».");

        let clase = Accion::GuardarClase {
            id: Some("c-uno".into()),
            materia: "Control".into(),
            dias: vec!["lun".into()],
            inicio: "07:00".into(),
            fin: "09:00".into(),
            lugar: None,
        };
        assert_eq!(clase.descripcion(&datos), "Editar la clase de Control.");

        let recordatorio = Accion::BorrarRecordatorio {
            id: "r-uno".into(),
        };
        assert_eq!(
            recordatorio.descripcion(&datos),
            "Borrar el recordatorio «Revisar Moodle»."
        );
    }

    // --- Aplicar ----------------------------------------------------------

    #[test]
    fn crear_tarea_pasa_por_agregar_tarea() {
        let (_dir, almacen) = almacen_con_base();
        let accion = Accion::CrearTarea {
            titulo: "Taller 3".into(),
            materia: Some("control".into()),
            vence: "2026-12-10T18:00".into(),
        };
        let datos = aplicar(&almacen, &accion).expect("crear");
        assert_eq!(datos.tareas.len(), 2);
        let nueva = datos.tareas.last().expect("la nueva");
        assert!(nueva.id.starts_with("t-"), "id: {}", nueva.id);
        assert_eq!(nueva.titulo, "Taller 3");
        assert_eq!(nueva.materia.as_deref(), Some("Control"), "grafía existente");
        assert_eq!(nueva.vence.format("%Y-%m-%dT%H:%M").to_string(), "2026-12-10T18:00");
    }

    #[test]
    fn editar_tarea_conserva_lo_que_falta() {
        let (_dir, almacen) = almacen_con_base();
        let accion = Accion::EditarTarea {
            id: "t-uno".into(),
            titulo: None,
            materia: None,
            vence: Some("2026-12-01T20:30".into()),
        };
        let datos = aplicar(&almacen, &accion).expect("editar");
        let tarea = datos.tareas.iter().find(|t| t.id == "t-uno").expect("tarea");
        assert_eq!(tarea.titulo, "Taller 2", "el título se conserva");
        assert_eq!(tarea.materia.as_deref(), Some("Control"));
        assert_eq!(tarea.vence.format("%H:%M").to_string(), "20:30");
    }

    #[test]
    fn marcar_y_borrar_tarea() {
        let (_dir, almacen) = almacen_con_base();
        let datos = aplicar(
            &almacen,
            &Accion::MarcarTarea {
                id: "t-uno".into(),
                hecha: true,
            },
        )
        .expect("marcar");
        assert!(datos.tareas[0].hecha);
        assert!(datos.tareas[0].hecha_en.is_some(), "anota la fecha de hoy");

        let datos = aplicar(
            &almacen,
            &Accion::BorrarTarea {
                id: "t-uno".into(),
            },
        )
        .expect("borrar");
        assert!(datos.tareas.is_empty());
    }

    #[test]
    fn clase_nueva_y_editada() {
        let (_dir, almacen) = almacen_con_base();
        let datos = aplicar(
            &almacen,
            &Accion::GuardarClase {
                id: None,
                materia: "Robótica".into(),
                dias: vec!["martes".into(), "jueves".into()],
                inicio: "9:00".into(),
                fin: "11:00".into(),
                lugar: Some("B-301".into()),
            },
        )
        .expect("crear clase");
        assert_eq!(datos.clases.len(), 2);
        let nueva = &datos.clases[1];
        assert_eq!(nueva.dias, vec![Dia::Mar, Dia::Jue]);
        assert_eq!(nueva.inicio.format("%H:%M").to_string(), "09:00");
        assert_eq!(nueva.lugar.as_deref(), Some("B-301"));

        // Editar sin repetir los días los conserva.
        let datos = aplicar(
            &almacen,
            &Accion::GuardarClase {
                id: Some(nueva.id.clone()),
                materia: "Robotica".into(),
                dias: Vec::new(),
                inicio: "10:00".into(),
                fin: "12:00".into(),
                lugar: None,
            },
        )
        .expect("editar clase");
        let editada = datos.clases.iter().find(|c| c.id == nueva.id).expect("clase");
        assert_eq!(editada.materia, "Robótica", "grafía existente");
        assert_eq!(editada.dias, vec![Dia::Mar, Dia::Jue]);
        assert_eq!(editada.lugar.as_deref(), Some("B-301"));
    }

    #[test]
    fn recordatorio_creado_y_borrado() {
        let (_dir, almacen) = almacen_con_base();
        let datos = aplicar(
            &almacen,
            &Accion::GuardarRecordatorio {
                id: None,
                texto: "Revisar Classroom".into(),
                dias: vec!["Lun".into(), "mié".into()],
                hora: "20:00".into(),
            },
        )
        .expect("crear recordatorio");
        assert_eq!(datos.recordatorios.len(), 2);
        assert_eq!(datos.recordatorios[1].dias, vec![Dia::Lun, Dia::Mie]);

        let datos = aplicar(
            &almacen,
            &Accion::BorrarRecordatorio {
                id: "r-uno".into(),
            },
        )
        .expect("borrar recordatorio");
        assert_eq!(datos.recordatorios.len(), 1);
    }

    #[test]
    fn errores_legibles() {
        let (_dir, almacen) = almacen_con_base();

        let error = aplicar(
            &almacen,
            &Accion::BorrarTarea {
                id: "t-fantasma".into(),
            },
        )
        .expect_err("id inexistente");
        assert!(error.mensaje.contains("no existe la tarea con id «t-fantasma»"), "{}", error.mensaje);

        let error = aplicar(
            &almacen,
            &Accion::CrearTarea {
                titulo: "X".into(),
                materia: None,
                vence: "mañana".into(),
            },
        )
        .expect_err("fecha inválida");
        assert!(error.mensaje.contains("no es una fecha válida"), "{}", error.mensaje);

        let error = aplicar(
            &almacen,
            &Accion::GuardarClase {
                id: Some("c-fantasma".into()),
                materia: "X".into(),
                dias: vec!["lun".into()],
                inicio: "07:00".into(),
                fin: "09:00".into(),
                lugar: None,
            },
        )
        .expect_err("clase inexistente");
        assert!(error.mensaje.contains("no existe la clase"), "{}", error.mensaje);

        // Nada de eso tocó el archivo.
        let texto = fs::read_to_string(&almacen.ruta).expect("leer");
        assert_eq!(texto, BASE, "el archivo queda intacto");
    }

    #[test]
    fn fecha_solo_dia_y_hora_con_un_digito() {
        assert_eq!(
            fecha_hora("2026-12-10", "la fecha")
                .expect("solo fecha")
                .format("%Y-%m-%dT%H:%M")
                .to_string(),
            "2026-12-10T23:59"
        );
        assert_eq!(
            fecha_hora("2026-12-10 18:00", "la fecha")
                .expect("con espacio")
                .format("%H:%M")
                .to_string(),
            "18:00"
        );
        assert_eq!(hora("7:05", "la hora").expect("hora").format("%H:%M").to_string(), "07:05");
        assert!(fecha_hora("10/12/2026", "la fecha").is_err());
        assert!(dia("festivo").is_err());
    }

    /// El esquema que se le da a Claude nombra todos los tipos de acción y ya no pide `resumen`.
    #[test]
    fn esquema_completo() {
        let esquema = esquema();
        assert!(esquema["properties"].get("resumen").is_none());
        let requeridos = esquema["required"].as_array().expect("required");
        assert!(!requeridos.iter().any(|v| v == "resumen"));
        let tipos = esquema["properties"]["acciones"]["items"]["properties"]["tipo"]["enum"]
            .as_array()
            .expect("enum de tipos");
        assert_eq!(tipos.len(), 11);
        for accion in [
            "crear_tarea",
            "editar_tarea",
            "marcar_tarea",
            "borrar_tarea",
            "guardar_clase",
            "borrar_clase",
            "guardar_recordatorio",
            "borrar_recordatorio",
            "crear_evento",
            "mover_evento",
            "borrar_evento",
        ] {
            assert!(
                tipos.iter().any(|valor| valor == accion),
                "falta {accion}"
            );
        }
    }

    #[test]
    fn parsear_expone_resumen_si_viene_y_none_si_falta_o_vacio() {
        let con = parsear(r#"{"respuesta":"Ok","acciones":[],"resumen":"Hablan de Control."}"#)
            .expect("con resumen");
        assert_eq!(con.resumen.as_deref(), Some("Hablan de Control."));

        let sin = parsear(r#"{"respuesta":"Ok","acciones":[]}"#).expect("sin resumen");
        assert_eq!(sin.resumen, None);

        let vacio =
            parsear(r#"{"respuesta":"Ok","acciones":[],"resumen":"   "}"#).expect("resumen vacío");
        assert_eq!(vacio.resumen, None);
    }

    #[test]
    fn aplicar_todas_se_detiene_en_error() {
        let (_dir, almacen) = almacen_con_base();
        let acciones = vec![
            Accion::CrearTarea {
                titulo: "Taller 3".into(),
                materia: Some("Control".into()),
                vence: "2026-12-10T18:00".into(),
            },
            Accion::BorrarTarea {
                id: "t-fantasma".into(),
            },
            Accion::CrearTarea {
                titulo: "Taller 4".into(),
                materia: None,
                vence: "2026-12-12T18:00".into(),
            },
        ];
        let resultado = aplicar_todas(&almacen, &acciones);
        assert_eq!(resultado.aplicadas, 1);
        let datos = resultado.ultimos_datos.expect("la primera se aplicó");
        assert_eq!(datos.tareas.len(), 2);
        let err = resultado.error.expect("la segunda falló");
        assert!(err.starts_with("Aplicadas 1; falló «Borrar tarea»"), "{err}");
    }

    #[test]
    fn eventos_crear_mover_y_borrar() {
        let (_dir, almacen) = almacen_con_base();

        // Crear evento ligado a tarea existente sin repetir título: toma el de la tarea.
        let datos = aplicar(
            &almacen,
            &Accion::CrearEvento {
                titulo: String::new(),
                fecha: "2026-11-26".into(),
                inicio: "10:00".into(),
                fin: "11:00".into(),
                tarea: Some("t-uno".into()),
                nota: Some("Capítulo 2".into()),
            },
        )
        .expect("crear evento");
        assert_eq!(datos.eventos.len(), 1);
        let id = datos.eventos[0].id.clone();
        assert!(id.starts_with("e-"));
        assert_eq!(datos.eventos[0].titulo, "Taller 2");
        assert_eq!(datos.eventos[0].tarea.as_deref(), Some("t-uno"));

        // Mover solo la hora de inicio conserva la duración (1h -> 15:00 a 16:00).
        let datos = aplicar(
            &almacen,
            &Accion::MoverEvento {
                id: id.clone(),
                fecha: Some("2026-11-27".into()),
                inicio: Some("15:00".into()),
                fin: None,
                titulo: None,
                nota: None,
            },
        )
        .expect("mover evento");
        assert_eq!(datos.eventos[0].fecha.format("%Y-%m-%d").to_string(), "2026-11-27");
        assert_eq!(datos.eventos[0].inicio.format("%H:%M").to_string(), "15:00");
        assert_eq!(datos.eventos[0].fin.format("%H:%M").to_string(), "16:00");

        // Borrar evento.
        let datos = aplicar(&almacen, &Accion::BorrarEvento { id }).expect("borrar evento");
        assert!(datos.eventos.is_empty());
    }
}
