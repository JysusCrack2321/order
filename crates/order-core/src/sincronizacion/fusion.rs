//! Detección de cambios locales, fusión LWW por campo y escritura segura sobre `order.toml`.

use super::estado::{
    AjustesEstado, Campo, ClaseEstado, EstadoSync, MensajeSync, RecordatorioEstado,
    SemestreEstado, TareaEstado,
};
use super::hlc::Marca;
use crate::modelo::{
    Ajustes, Clase, Datos, FORMATO_FECHA, FORMATO_FECHA_HORA, FORMATO_HORA, Recordatorio, Semestre,
    Tarea,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, Value};

/// Resultado de fusionar el estado local con el mensaje remoto.
#[derive(Debug, Clone, PartialEq)]
pub struct Resultado {
    pub estado: EstadoSync,
    pub cambio: bool,
}

/// Compara `datos` (leído de `order.toml`) contra `estado.foto`, asignando
/// marcas HLC nuevas únicamente a los campos que cambiaron y creando lápidas
/// para los elementos eliminados. Devuelve `true` si detectó algún cambio.
pub fn registrar_cambios_locales(datos: &Datos, estado: &mut EstadoSync, ahora_ms: u64) -> bool {
    let mut cambio = false;
    let prev = estado.foto.as_ref();

    // 1. Ajustes (si no hay foto previa, se compara contra Ajustes::default() para no
    //    pisar ajustes reales de otro dispositivo con valores por defecto).
    let prev_ajustes = prev.map(|d| d.ajustes.clone()).unwrap_or_default();
    if datos.ajustes.aviso_dia_antes != prev_ajustes.aviso_dia_antes {
        let marca = estado.reloj.tick(ahora_ms);
        estado.crdt.ajustes.aviso_dia_antes = Campo::nuevo(datos.ajustes.aviso_dia_antes, marca);
        cambio = true;
    }
    if datos.ajustes.aviso_horas_antes != prev_ajustes.aviso_horas_antes {
        let marca = estado.reloj.tick(ahora_ms);
        estado.crdt.ajustes.aviso_horas_antes =
            Campo::nuevo(datos.ajustes.aviso_horas_antes, marca);
        cambio = true;
    }

    // 2. Semestre
    let prev_sem = prev.and_then(|d| d.semestre.as_ref());
    match (prev_sem, datos.semestre.as_ref()) {
        (None, None) => {}
        (None, Some(s)) => {
            let m_inicio = estado.reloj.tick(ahora_ms);
            let m_fin = estado.reloj.tick(ahora_ms);
            let borrado = estado
                .crdt
                .semestre
                .as_ref()
                .and_then(|se| se.borrado.clone());
            estado.crdt.semestre = Some(SemestreEstado {
                inicio: Campo::nuevo(s.inicio, m_inicio),
                fin: Campo::nuevo(s.fin, m_fin),
                borrado,
            });
            cambio = true;
        }
        (Some(_), None) => {
            if let Some(se) = estado.crdt.semestre.as_mut() {
                if se.esta_vivo() {
                    se.borrado = Some(estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
            }
        }
        (Some(p), Some(s)) => {
            let se = estado.crdt.semestre.get_or_insert_with(|| SemestreEstado {
                inicio: Campo::nuevo(p.inicio, estado.reloj.tick(ahora_ms)),
                fin: Campo::nuevo(p.fin, estado.reloj.tick(ahora_ms)),
                borrado: None,
            });
            if s.inicio != p.inicio || !se.esta_vivo() {
                se.inicio = Campo::nuevo(s.inicio, estado.reloj.tick(ahora_ms));
                cambio = true;
            }
            if s.fin != p.fin || !se.esta_vivo() {
                se.fin = Campo::nuevo(s.fin, estado.reloj.tick(ahora_ms));
                cambio = true;
            }
        }
    }

    // 3. Clases
    let prev_clases: HashMap<&str, &Clase> = prev
        .map(|d| d.clases.as_slice())
        .unwrap_or(&[])
        .iter()
        .filter(|c| !c.id.trim().is_empty())
        .map(|c| (c.id.as_str(), c))
        .collect();
    let ids_clases: BTreeSet<&str> = datos
        .clases
        .iter()
        .map(|c| c.id.trim())
        .filter(|id| !id.is_empty())
        .collect();

    for id in prev_clases.keys() {
        if !ids_clases.contains(id) {
            if let Some(ce) = estado.crdt.clases.get_mut(*id) {
                if ce.esta_vivo() {
                    ce.borrado = Some(estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
            }
        }
    }
    for c in &datos.clases {
        let id = c.id.trim();
        if id.is_empty() {
            continue;
        }
        match prev_clases.get(id) {
            None => {
                let borrado = estado.crdt.clases.get(id).and_then(|ce| ce.borrado.clone());
                let ce = ClaseEstado {
                    materia: Campo::nuevo(c.materia.clone(), estado.reloj.tick(ahora_ms)),
                    dias: Campo::nuevo(c.dias.clone(), estado.reloj.tick(ahora_ms)),
                    inicio: Campo::nuevo(c.inicio, estado.reloj.tick(ahora_ms)),
                    fin: Campo::nuevo(c.fin, estado.reloj.tick(ahora_ms)),
                    lugar: Campo::nuevo(c.lugar.clone(), estado.reloj.tick(ahora_ms)),
                    borrado,
                };
                estado.crdt.clases.insert(id.to_string(), ce);
                cambio = true;
            }
            Some(p) => {
                let ce = estado.crdt.clases.entry(id.to_string()).or_insert_with(|| {
                    ClaseEstado {
                        materia: Campo::nuevo(p.materia.clone(), estado.reloj.tick(ahora_ms)),
                        dias: Campo::nuevo(p.dias.clone(), estado.reloj.tick(ahora_ms)),
                        inicio: Campo::nuevo(p.inicio, estado.reloj.tick(ahora_ms)),
                        fin: Campo::nuevo(p.fin, estado.reloj.tick(ahora_ms)),
                        lugar: Campo::nuevo(p.lugar.clone(), estado.reloj.tick(ahora_ms)),
                        borrado: None,
                    }
                });
                let vivo = ce.esta_vivo();
                if c.materia != p.materia || !vivo {
                    ce.materia = Campo::nuevo(c.materia.clone(), estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
                if c.dias != p.dias {
                    ce.dias = Campo::nuevo(c.dias.clone(), estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
                if c.inicio != p.inicio {
                    ce.inicio = Campo::nuevo(c.inicio, estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
                if c.fin != p.fin {
                    ce.fin = Campo::nuevo(c.fin, estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
                if c.lugar != p.lugar {
                    ce.lugar = Campo::nuevo(c.lugar.clone(), estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
            }
        }
    }

    // 4. Tareas
    let prev_tareas: HashMap<&str, &Tarea> = prev
        .map(|d| d.tareas.as_slice())
        .unwrap_or(&[])
        .iter()
        .filter(|t| !t.id.trim().is_empty())
        .map(|t| (t.id.as_str(), t))
        .collect();
    let ids_tareas: BTreeSet<&str> = datos
        .tareas
        .iter()
        .map(|t| t.id.trim())
        .filter(|id| !id.is_empty())
        .collect();

    for id in prev_tareas.keys() {
        if !ids_tareas.contains(id) {
            if let Some(te) = estado.crdt.tareas.get_mut(*id) {
                if te.esta_vivo() {
                    te.borrado = Some(estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
            }
        }
    }
    for t in &datos.tareas {
        let id = t.id.trim();
        if id.is_empty() {
            continue;
        }
        match prev_tareas.get(id) {
            None => {
                let borrado = estado.crdt.tareas.get(id).and_then(|te| te.borrado.clone());
                let te = TareaEstado {
                    titulo: Campo::nuevo(t.titulo.clone(), estado.reloj.tick(ahora_ms)),
                    materia: Campo::nuevo(t.materia.clone(), estado.reloj.tick(ahora_ms)),
                    vence: Campo::nuevo(t.vence, estado.reloj.tick(ahora_ms)),
                    hecha: Campo::nuevo(t.hecha, estado.reloj.tick(ahora_ms)),
                    hecha_en: Campo::nuevo(t.hecha_en, estado.reloj.tick(ahora_ms)),
                    origen: Campo::nuevo(t.origen.clone(), estado.reloj.tick(ahora_ms)),
                    enlace: Campo::nuevo(t.enlace.clone(), estado.reloj.tick(ahora_ms)),
                    borrado,
                };
                estado.crdt.tareas.insert(id.to_string(), te);
                cambio = true;
            }
            Some(p) => {
                let te = estado.crdt.tareas.entry(id.to_string()).or_insert_with(|| {
                    TareaEstado {
                        titulo: Campo::nuevo(p.titulo.clone(), estado.reloj.tick(ahora_ms)),
                        materia: Campo::nuevo(p.materia.clone(), estado.reloj.tick(ahora_ms)),
                        vence: Campo::nuevo(p.vence, estado.reloj.tick(ahora_ms)),
                        hecha: Campo::nuevo(p.hecha, estado.reloj.tick(ahora_ms)),
                        hecha_en: Campo::nuevo(p.hecha_en, estado.reloj.tick(ahora_ms)),
                        origen: Campo::nuevo(p.origen.clone(), estado.reloj.tick(ahora_ms)),
                        enlace: Campo::nuevo(p.enlace.clone(), estado.reloj.tick(ahora_ms)),
                        borrado: None,
                    }
                });
                let vivo = te.esta_vivo();
                if t.titulo != p.titulo || !vivo {
                    te.titulo = Campo::nuevo(t.titulo.clone(), estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
                if t.materia != p.materia {
                    te.materia = Campo::nuevo(t.materia.clone(), estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
                if t.vence != p.vence {
                    te.vence = Campo::nuevo(t.vence, estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
                if t.hecha != p.hecha {
                    te.hecha = Campo::nuevo(t.hecha, estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
                if t.hecha_en != p.hecha_en {
                    te.hecha_en = Campo::nuevo(t.hecha_en, estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
                if t.origen != p.origen {
                    te.origen = Campo::nuevo(t.origen.clone(), estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
                if t.enlace != p.enlace {
                    te.enlace = Campo::nuevo(t.enlace.clone(), estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
            }
        }
    }

    // 5. Recordatorios
    let prev_recs: HashMap<&str, &Recordatorio> = prev
        .map(|d| d.recordatorios.as_slice())
        .unwrap_or(&[])
        .iter()
        .filter(|r| !r.id.trim().is_empty())
        .map(|r| (r.id.as_str(), r))
        .collect();
    let ids_recs: BTreeSet<&str> = datos
        .recordatorios
        .iter()
        .map(|r| r.id.trim())
        .filter(|id| !id.is_empty())
        .collect();

    for id in prev_recs.keys() {
        if !ids_recs.contains(id) {
            if let Some(re) = estado.crdt.recordatorios.get_mut(*id) {
                if re.esta_vivo() {
                    re.borrado = Some(estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
            }
        }
    }
    for r in &datos.recordatorios {
        let id = r.id.trim();
        if id.is_empty() {
            continue;
        }
        match prev_recs.get(id) {
            None => {
                let borrado = estado
                    .crdt
                    .recordatorios
                    .get(id)
                    .and_then(|re| re.borrado.clone());
                let re = RecordatorioEstado {
                    texto: Campo::nuevo(r.texto.clone(), estado.reloj.tick(ahora_ms)),
                    dias: Campo::nuevo(r.dias.clone(), estado.reloj.tick(ahora_ms)),
                    hora: Campo::nuevo(r.hora, estado.reloj.tick(ahora_ms)),
                    borrado,
                };
                estado.crdt.recordatorios.insert(id.to_string(), re);
                cambio = true;
            }
            Some(p) => {
                let re = estado
                    .crdt
                    .recordatorios
                    .entry(id.to_string())
                    .or_insert_with(|| RecordatorioEstado {
                        texto: Campo::nuevo(p.texto.clone(), estado.reloj.tick(ahora_ms)),
                        dias: Campo::nuevo(p.dias.clone(), estado.reloj.tick(ahora_ms)),
                        hora: Campo::nuevo(p.hora, estado.reloj.tick(ahora_ms)),
                        borrado: None,
                    });
                let vivo = re.esta_vivo();
                if r.texto != p.texto || !vivo {
                    re.texto = Campo::nuevo(r.texto.clone(), estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
                if r.dias != p.dias {
                    re.dias = Campo::nuevo(r.dias.clone(), estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
                if r.hora != p.hora {
                    re.hora = Campo::nuevo(r.hora, estado.reloj.tick(ahora_ms));
                    cambio = true;
                }
            }
        }
    }

    estado.foto = Some(datos.clone());
    cambio
}

/// Fusiona el estado CRDT local con el mensaje remoto de forma conmutativa,
/// asociativa e idempotente. También consolida tareas distintas que compartan
/// el mismo `origen` (por ejemplo Moodle/iCal sincronizados en ambos equipos).
pub fn fusionar(local: &EstadoSync, remoto: &MensajeSync) -> Resultado {
    let mut nuevo = local.clone();
    if let Some(m) = remoto.max_marca() {
        nuevo.reloj.observar(m);
    }

    let mut cambio = false;
    cambio |= fusionar_ajustes(&mut nuevo.crdt.ajustes, &remoto.ajustes);
    cambio |= fusionar_semestre(&mut nuevo.crdt.semestre, &remoto.semestre);

    for (id, rem) in &remoto.clases {
        match nuevo.crdt.clases.get_mut(id) {
            Some(loc) => cambio |= fusionar_clase(loc, rem),
            None => {
                nuevo.crdt.clases.insert(id.clone(), rem.clone());
                cambio = true;
            }
        }
    }

    for (id, rem) in &remoto.tareas {
        match nuevo.crdt.tareas.get_mut(id) {
            Some(loc) => cambio |= fusionar_tarea(loc, rem),
            None => {
                nuevo.crdt.tareas.insert(id.clone(), rem.clone());
                cambio = true;
            }
        }
    }

    for (id, rem) in &remoto.recordatorios {
        match nuevo.crdt.recordatorios.get_mut(id) {
            Some(loc) => cambio |= fusionar_recordatorio(loc, rem),
            None => {
                nuevo.crdt.recordatorios.insert(id.clone(), rem.clone());
                cambio = true;
            }
        }
    }

    deduplicar_por_origen(&mut nuevo.crdt.tareas);
    let cambio_real = nuevo.crdt != local.crdt;
    let _ = cambio;

    Resultado {
        estado: nuevo,
        cambio: cambio_real,
    }
}

/// Reconstruye el modelo `Datos` a partir del estado CRDT fusionado.
pub fn aplicar(estado: &EstadoSync) -> Datos {
    let ajustes = Ajustes {
        aviso_dia_antes: estado.crdt.ajustes.aviso_dia_antes.valor,
        aviso_horas_antes: estado.crdt.ajustes.aviso_horas_antes.valor,
    };

    let semestre = estado
        .crdt
        .semestre
        .as_ref()
        .filter(|s| s.esta_vivo())
        .map(|s| Semestre {
            inicio: s.inicio.valor,
            fin: s.fin.valor,
        });

    let clases = estado
        .crdt
        .clases
        .iter()
        .filter(|(_, c)| c.esta_vivo())
        .map(|(id, c)| Clase {
            id: id.clone(),
            materia: c.materia.valor.clone(),
            dias: c.dias.valor.clone(),
            inicio: c.inicio.valor,
            fin: c.fin.valor,
            lugar: c.lugar.valor.clone(),
        })
        .collect();

    let tareas = estado
        .crdt
        .tareas
        .iter()
        .filter(|(_, t)| t.esta_vivo())
        .map(|(id, t)| Tarea {
            id: id.clone(),
            titulo: t.titulo.valor.clone(),
            materia: t.materia.valor.clone(),
            vence: t.vence.valor,
            hecha: t.hecha.valor,
            hecha_en: t.hecha_en.valor,
            origen: t.origen.valor.clone(),
            enlace: t.enlace.valor.clone(),
        })
        .collect();

    let recordatorios = estado
        .crdt
        .recordatorios
        .iter()
        .filter(|(_, r)| r.esta_vivo())
        .map(|(id, r)| Recordatorio {
            id: id.clone(),
            texto: r.texto.valor.clone(),
            dias: r.dias.valor.clone(),
            hora: r.hora.valor,
        })
        .collect();

    Datos {
        ajustes,
        semestre,
        clases,
        tareas,
        recordatorios,
    }
}

/// Comprueba si dos `Datos` representan exactamente la misma agenda (ignorando
/// el orden relativo de las tablas en `order.toml`).
pub fn datos_equivalentes(a: &Datos, b: &Datos) -> bool {
    if a.ajustes != b.ajustes || a.semestre != b.semestre {
        return false;
    }
    let clases_a: BTreeMap<&str, &Clase> = a.clases.iter().map(|c| (c.id.as_str(), c)).collect();
    let clases_b: BTreeMap<&str, &Clase> = b.clases.iter().map(|c| (c.id.as_str(), c)).collect();
    if clases_a != clases_b {
        return false;
    }
    let tareas_a: BTreeMap<&str, &Tarea> = a.tareas.iter().map(|t| (t.id.as_str(), t)).collect();
    let tareas_b: BTreeMap<&str, &Tarea> = b.tareas.iter().map(|t| (t.id.as_str(), t)).collect();
    if tareas_a != tareas_b {
        return false;
    }
    let recs_a: BTreeMap<&str, &Recordatorio> = a
        .recordatorios
        .iter()
        .map(|r| (r.id.as_str(), r))
        .collect();
    let recs_b: BTreeMap<&str, &Recordatorio> = b
        .recordatorios
        .iter()
        .map(|r| (r.id.as_str(), r))
        .collect();
    recs_a == recs_b
}

/// Aplica `nuevos` sobre un `DocumentMut` existente (usado dentro de
/// `Almacen::modificar`) conservando comentarios, decoración y el orden de los
/// elementos que no cambiaron.
pub fn aplicar_en_doc(doc: &mut DocumentMut, nuevos: &Datos) -> Result<(), String> {
    crate::ops::guardar_ajustes(doc, &nuevos.ajustes)?;
    crate::ops::guardar_semestre(doc, nuevos.semestre.as_ref())?;

    sincronizar_seccion(
        doc,
        "clase",
        &nuevos.clases,
        |c| c.id.as_str(),
        escribir_clase_en_tabla,
    )?;
    sincronizar_seccion(
        doc,
        "tarea",
        &nuevos.tareas,
        |t| t.id.as_str(),
        escribir_tarea_en_tabla,
    )?;
    sincronizar_seccion(
        doc,
        "recordatorio",
        &nuevos.recordatorios,
        |r| r.id.as_str(),
        escribir_recordatorio_en_tabla,
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Ayudantes LWW
// ---------------------------------------------------------------------------

fn fusionar_campo<T: Clone + PartialEq>(local: &mut Campo<T>, remoto: &Campo<T>) -> bool {
    if remoto.marca > local.marca {
        let cambio = local.valor != remoto.valor || local.marca != remoto.marca;
        local.valor = remoto.valor.clone();
        local.marca = remoto.marca.clone();
        cambio
    } else {
        false
    }
}

fn fusionar_borrado(local: &mut Option<Marca>, remoto: &Option<Marca>) -> bool {
    match (&*local, remoto) {
        (None, Some(r)) => {
            *local = Some(r.clone());
            true
        }
        (Some(l), Some(r)) if r > l => {
            *local = Some(r.clone());
            true
        }
        _ => false,
    }
}

fn fusionar_ajustes(local: &mut AjustesEstado, remoto: &AjustesEstado) -> bool {
    let mut c = false;
    c |= fusionar_campo(&mut local.aviso_dia_antes, &remoto.aviso_dia_antes);
    c |= fusionar_campo(&mut local.aviso_horas_antes, &remoto.aviso_horas_antes);
    c
}

fn fusionar_semestre(local: &mut Option<SemestreEstado>, remoto: &Option<SemestreEstado>) -> bool {
    match (local, remoto) {
        (_, None) => false,
        (loc @ None, Some(rem)) => {
            *loc = Some(rem.clone());
            true
        }
        (Some(loc), Some(rem)) => {
            let mut c = false;
            c |= fusionar_campo(&mut loc.inicio, &rem.inicio);
            c |= fusionar_campo(&mut loc.fin, &rem.fin);
            c |= fusionar_borrado(&mut loc.borrado, &rem.borrado);
            c
        }
    }
}

fn fusionar_clase(local: &mut ClaseEstado, remoto: &ClaseEstado) -> bool {
    let mut c = false;
    c |= fusionar_campo(&mut local.materia, &remoto.materia);
    c |= fusionar_campo(&mut local.dias, &remoto.dias);
    c |= fusionar_campo(&mut local.inicio, &remoto.inicio);
    c |= fusionar_campo(&mut local.fin, &remoto.fin);
    c |= fusionar_campo(&mut local.lugar, &remoto.lugar);
    c |= fusionar_borrado(&mut local.borrado, &remoto.borrado);
    c
}

fn fusionar_tarea(local: &mut TareaEstado, remoto: &TareaEstado) -> bool {
    let mut c = false;
    c |= fusionar_campo(&mut local.titulo, &remoto.titulo);
    c |= fusionar_campo(&mut local.materia, &remoto.materia);
    c |= fusionar_campo(&mut local.vence, &remoto.vence);
    c |= fusionar_campo(&mut local.hecha, &remoto.hecha);
    c |= fusionar_campo(&mut local.hecha_en, &remoto.hecha_en);
    c |= fusionar_campo(&mut local.origen, &remoto.origen);
    c |= fusionar_campo(&mut local.enlace, &remoto.enlace);
    c |= fusionar_borrado(&mut local.borrado, &remoto.borrado);
    c
}

fn fusionar_recordatorio(local: &mut RecordatorioEstado, remoto: &RecordatorioEstado) -> bool {
    let mut c = false;
    c |= fusionar_campo(&mut local.texto, &remoto.texto);
    c |= fusionar_campo(&mut local.dias, &remoto.dias);
    c |= fusionar_campo(&mut local.hora, &remoto.hora);
    c |= fusionar_borrado(&mut local.borrado, &remoto.borrado);
    c
}

/// Si dos tareas vivas tienen el mismo `origen` no vacío (p. ej. ambas llegaron
/// desde Moodle en dos equipos sin conexión), las consolida bajo el `id`
/// lexicográficamente menor usando LWW por campo.
fn deduplicar_por_origen(tareas: &mut BTreeMap<String, TareaEstado>) {
    let mut por_origen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, tarea) in tareas.iter() {
        if !tarea.esta_vivo() {
            continue;
        }
        if let Some(origen) = &tarea.origen.valor {
            let clave = origen.trim();
            if !clave.is_empty() {
                por_origen
                    .entry(clave.to_string())
                    .or_default()
                    .push(id.clone());
            }
        }
    }

    for (_, mut ids) in por_origen {
        if ids.len() <= 1 {
            continue;
        }
        ids.sort();
        let ganador_id = ids[0].clone();
        for perdedor_id in ids.into_iter().skip(1) {
            if let Some(perdedor) = tareas.remove(&perdedor_id) {
                if let Some(ganador) = tareas.get_mut(&ganador_id) {
                    fusionar_tarea(ganador, &perdedor);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Ayudantes de escritura sobre DocumentMut
// ---------------------------------------------------------------------------

fn sincronizar_seccion<T>(
    doc: &mut DocumentMut,
    seccion: &str,
    items: &[T],
    id_de: impl Fn(&T) -> &str,
    escribir: impl Fn(&mut Table, &T),
) -> Result<(), String> {
    let deseados: HashMap<&str, &T> = items.iter().map(|x| (id_de(x), x)).collect();

    if deseados.is_empty() {
        doc.remove(seccion);
        return Ok(());
    }

    // Normalizar si estaba en formato compacto `seccion = [{ ... }]`.
    if let Some(Item::Value(Value::Array(arr))) = doc.get(seccion) {
        let mut tablas = ArrayOfTables::new();
        for valor in arr.iter() {
            let Value::InlineTable(t) = valor else {
                return Err(format!("la sección «{seccion}» no tiene la forma esperada"));
            };
            tablas.push(t.clone().into_table());
        }
        doc.insert(seccion, Item::ArrayOfTables(tablas));
    }

    if doc.get(seccion).is_none() {
        doc.insert(seccion, Item::ArrayOfTables(ArrayOfTables::new()));
    }

    let tablas = doc
        .get_mut(seccion)
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| format!("la sección «{seccion}» no tiene la forma esperada"))?;

    let mut presentes = BTreeSet::new();
    let mut i = 0;
    while i < tablas.len() {
        let id = tablas
            .get(i)
            .and_then(|t| t.get("id"))
            .and_then(Item::as_str)
            .unwrap_or("")
            .to_string();
        if let Some(item) = deseados.get(id.as_str()) {
            if let Some(tabla) = tablas.get_mut(i) {
                escribir(tabla, item);
            }
            presentes.insert(id);
            i += 1;
        } else {
            tablas.remove(i);
        }
    }

    for item in items {
        let id = id_de(item);
        if !presentes.contains(id) {
            let mut tabla = Table::new();
            poner_valor(&mut tabla, "id", Value::from(id));
            escribir(&mut tabla, item);
            tablas.push(tabla);
        }
    }

    Ok(())
}

fn escribir_clase_en_tabla(tabla: &mut Table, c: &Clase) {
    poner_valor(tabla, "id", Value::from(c.id.as_str()));
    poner_valor(tabla, "materia", Value::from(c.materia.as_str()));
    let dias: Array = c.dias.iter().map(|d| d.texto()).collect();
    poner_valor(tabla, "dias", Value::Array(dias));
    tabla.remove("dia");
    poner_valor(
        tabla,
        "inicio",
        Value::from(c.inicio.format(FORMATO_HORA).to_string()),
    );
    poner_valor(
        tabla,
        "fin",
        Value::from(c.fin.format(FORMATO_HORA).to_string()),
    );
    poner_opcional(tabla, "lugar", c.lugar.as_deref().map(Value::from));
}

fn escribir_tarea_en_tabla(tabla: &mut Table, t: &Tarea) {
    poner_valor(tabla, "id", Value::from(t.id.as_str()));
    poner_valor(tabla, "titulo", Value::from(t.titulo.as_str()));
    poner_opcional(tabla, "materia", t.materia.as_deref().map(Value::from));
    poner_valor(
        tabla,
        "vence",
        Value::from(t.vence.format(FORMATO_FECHA_HORA).to_string()),
    );
    poner_valor(tabla, "hecha", Value::from(t.hecha));
    poner_opcional(
        tabla,
        "hecha_en",
        t.hecha_en
            .map(|f| Value::from(f.format(FORMATO_FECHA).to_string())),
    );
    poner_opcional(tabla, "origen", t.origen.as_deref().map(Value::from));
    poner_opcional(tabla, "enlace", t.enlace.as_deref().map(Value::from));
}

fn escribir_recordatorio_en_tabla(tabla: &mut Table, r: &Recordatorio) {
    poner_valor(tabla, "id", Value::from(r.id.as_str()));
    poner_valor(tabla, "texto", Value::from(r.texto.as_str()));
    let dias: Array = r.dias.iter().map(|d| d.texto()).collect();
    poner_valor(tabla, "dias", Value::Array(dias));
    poner_valor(
        tabla,
        "hora",
        Value::from(r.hora.format(FORMATO_HORA).to_string()),
    );
}

fn poner_valor(tabla: &mut Table, clave: &str, mut nuevo: Value) {
    if let Some(Item::Value(viejo)) = tabla.get(clave) {
        *nuevo.decor_mut() = viejo.decor().clone();
    }
    tabla.insert(clave, Item::Value(nuevo));
}

fn poner_opcional(tabla: &mut Table, clave: &str, valor: Option<Value>) {
    match valor {
        Some(v) => poner_valor(tabla, clave, v),
        None => {
            tabla.remove(clave);
        }
    }
}
