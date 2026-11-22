//! Puente UniFFI entre `order-core` y Kotlin/Android.
//!
//! Toda la lógica, los textos en español, las validaciones y el cálculo de
//! colores por materia viven en `order-core`. Este crate solo adapta los tipos
//! a registros planos para UniFFI y delega las operaciones en disco vía
//! `Almacen`.

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike};
use order_core::agente::acciones::{self, Accion};
use order_core::agente::adjuntos::{self, Adjunto, ContenidoAdjunto};
use order_core::agente::cliente;
use order_core::agente::config::{self, AgenteConfig, Proveedor, VozProveedor};
use order_core::agente::contexto;
use order_core::agente::conversacion::Conversacion;
use order_core::agente::voz;
use order_core::almacen::Almacen;
use order_core::avisos::{self, Registro};
use order_core::conflictos;
use order_core::filtros::{Filtro, TipoFecha};
use order_core::modelo::{self, Ajustes, Datos, Dia, ErrorDatos, FORMATO_FECHA, Semestre, Tarea};
use order_core::ops;
use order_core::presentacion::formularios::{self, ErrorCampo, interpretar_fecha, interpretar_hora, opcional};
use order_core::presentacion::materias::{self, materias_de, tono_materia};
use order_core::presentacion::textos::{
    self, cuando_recordatorio, dias_texto, etiqueta_mes, etiqueta_semana, fecha_corta, fecha_larga,
    fecha_o_vacio, fecha_tarea, hora as construir_hora, hora_clase, hora_o_vacio, horas_texto,
    minutos, minutos_texto, nombre_dia, pie,
};
use order_core::reloj::{self, a_12h, a_24h, etiqueta_hora, hora_texto};
use order_core::semana;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

uniffi::setup_scaffolding!();

// ---------------------------------------------------------------------------
// Errores y tipos básicos
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum ErrorFfi {
    Datos { linea: Option<u32>, mensaje: String },
    Operacion { mensaje: String },
    Validacion { campo: i32, mensaje: String },
}

impl fmt::Display for ErrorFfi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorFfi::Datos {
                linea: Some(l),
                mensaje,
            } => write!(f, "línea {l}: {mensaje}"),
            ErrorFfi::Datos {
                linea: None,
                mensaje,
            } => write!(f, "{mensaje}"),
            ErrorFfi::Operacion { mensaje } | ErrorFfi::Validacion { mensaje, .. } => {
                write!(f, "{mensaje}")
            }
        }
    }
}

impl std::error::Error for ErrorFfi {}

impl From<ErrorDatos> for ErrorFfi {
    fn from(e: ErrorDatos) -> Self {
        ErrorFfi::Datos {
            linea: e.linea.map(|l| l as u32),
            mensaje: e.mensaje,
        }
    }
}

impl From<ErrorCampo> for ErrorFfi {
    fn from(e: ErrorCampo) -> Self {
        ErrorFfi::Validacion {
            campo: e.campo,
            mensaje: e.mensaje,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DiaFfi {
    Lun,
    Mar,
    Mie,
    Jue,
    Vie,
    Sab,
    Dom,
}

impl From<Dia> for DiaFfi {
    fn from(d: Dia) -> Self {
        match d {
            Dia::Lun => DiaFfi::Lun,
            Dia::Mar => DiaFfi::Mar,
            Dia::Mie => DiaFfi::Mie,
            Dia::Jue => DiaFfi::Jue,
            Dia::Vie => DiaFfi::Vie,
            Dia::Sab => DiaFfi::Sab,
            Dia::Dom => DiaFfi::Dom,
        }
    }
}

impl From<DiaFfi> for Dia {
    fn from(d: DiaFfi) -> Self {
        match d {
            DiaFfi::Lun => Dia::Lun,
            DiaFfi::Mar => Dia::Mar,
            DiaFfi::Mie => Dia::Mie,
            DiaFfi::Jue => Dia::Jue,
            DiaFfi::Vie => Dia::Vie,
            DiaFfi::Sab => Dia::Sab,
            DiaFfi::Dom => Dia::Dom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TipoFechaFfi {
    Todas,
    Hoy,
    EstaSemana,
    Rango,
}

impl From<TipoFecha> for TipoFechaFfi {
    fn from(t: TipoFecha) -> Self {
        match t {
            TipoFecha::Todas => TipoFechaFfi::Todas,
            TipoFecha::Hoy => TipoFechaFfi::Hoy,
            TipoFecha::EstaSemana => TipoFechaFfi::EstaSemana,
            TipoFecha::Rango => TipoFechaFfi::Rango,
        }
    }
}

impl From<TipoFechaFfi> for TipoFecha {
    fn from(t: TipoFechaFfi) -> Self {
        match t {
            TipoFechaFfi::Todas => TipoFecha::Todas,
            TipoFechaFfi::Hoy => TipoFecha::Hoy,
            TipoFechaFfi::EstaSemana => TipoFecha::EstaSemana,
            TipoFechaFfi::Rango => TipoFecha::Rango,
        }
    }
}

// ---------------------------------------------------------------------------
// Registros de datos, advertencias, conflictos y vistas
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ConflictoFfi {
    pub ruta: String,
    pub mensaje: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ResumenArchivoFfi {
    pub ruta: String,
    pub num_clases: u32,
    pub num_tareas: u32,
    pub num_tareas_pendientes: u32,
    pub num_recordatorios: u32,
    pub advertencias: Vec<String>,
    pub conflictos: Vec<ConflictoFfi>,
    pub pie: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TareaFfi {
    pub id: String,
    pub titulo: String,
    pub materia: String,
    pub fecha_texto: String,
    pub fecha_iso: String,
    pub hora_texto: String,
    pub vencida: bool,
    pub hecha: bool,
    pub tono: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ClaseInicioFfi {
    pub id: String,
    pub materia: String,
    pub lugar: String,
    pub hora: String,
    pub tono: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RecordatorioInicioFfi {
    pub id: String,
    pub texto: String,
    pub cuando: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ConteoMateriaFfi {
    pub materia: String,
    pub cuantas: i32,
    pub tono: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct InicioVistaFfi {
    pub fecha_larga: String,
    pub clases: Vec<ClaseInicioFfi>,
    pub siguiente: Option<ClaseInicioFfi>,
    pub vencidas: Vec<TareaFfi>,
    pub de_hoy: Vec<TareaFfi>,
    pub proximas: Vec<TareaFfi>,
    pub recordatorios: Vec<RecordatorioInicioFfi>,
    pub por_materia: Vec<ConteoMateriaFfi>,
    pub pie: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DiaVistaFfi {
    pub fecha_iso: String,
    pub etiqueta: String,
    pub es_hoy: bool,
    pub clases: Vec<ClaseInicioFfi>,
    pub tareas: Vec<TareaFfi>,
    pub recordatorios: Vec<RecordatorioInicioFfi>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BloqueSemanaFfi {
    pub id: String,
    pub materia: String,
    pub lugar: String,
    pub hora: String,
    pub inicio_min: i32,
    pub fin_min: i32,
    pub tono: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TareaDiaSemanaFfi {
    pub id: String,
    pub titulo: String,
    pub hora: String,
    pub hecha: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DiaSemanaFfi {
    pub fecha_iso: String,
    pub nombre: String,
    pub fecha_corta: String,
    pub es_hoy: bool,
    pub bloques: Vec<BloqueSemanaFfi>,
    pub tareas: Vec<TareaDiaSemanaFfi>,
    pub extra_tareas: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SemanaVistaFfi {
    pub lunes_iso: String,
    pub etiqueta: String,
    pub es_actual: bool,
    pub base_hora: i32,
    pub fin_hora: i32,
    pub etiquetas_hora: Vec<String>,
    pub mostrar_ahora: bool,
    pub ahora_min: i32,
    pub hay_tareas: bool,
    pub dias: Vec<DiaSemanaFfi>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FiltroPendientesFfi {
    pub materia: Option<String>,
    pub tipo_fecha: TipoFechaFfi,
    pub desde_iso: Option<String>,
    pub hasta_iso: Option<String>,
    pub hora_desde: Option<String>,
    pub hora_hasta: Option<String>,
    pub mostrar_hechas: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PendientesVistaFfi {
    pub tareas: Vec<TareaFfi>,
    pub resumen: String,
    pub filtro_activo: bool,
    pub filtro_desde_texto: String,
    pub filtro_hasta_texto: String,
    pub filtro_hora_desde_texto: String,
    pub filtro_hora_hasta_texto: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MateriaConTonoFfi {
    pub nombre: String,
    pub tono: i32,
}

// ---------------------------------------------------------------------------
// Registros para formularios y selectores
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TareaFormFfi {
    pub id: String,
    pub titulo: String,
    pub materia: String,
    pub fecha_iso: String,
    pub hora_texto: String,
    pub hecha: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ClaseFormFfi {
    pub id: String,
    pub materia: String,
    pub dias: Vec<DiaFfi>,
    pub hora_inicio: String,
    pub hora_fin: String,
    pub lugar: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RecordatorioFormFfi {
    pub id: String,
    pub texto: String,
    pub dias: Vec<DiaFfi>,
    pub hora: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RecordatorioListaFfi {
    pub id: String,
    pub texto: String,
    pub dias_texto: String,
    pub hora_texto: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AjustesFormFfi {
    pub sin_semestre: bool,
    pub semestre_inicio_iso: String,
    pub semestre_fin_iso: String,
    pub aviso_dia_antes: String,
    pub aviso_horas_antes: String,
    pub recordatorios: Vec<RecordatorioListaFfi>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CeldaCalendarioFfi {
    pub dia: i32,
    pub fila: i32,
    pub columna: i32,
    pub hoy: bool,
    pub elegida: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CalendarioMesFfi {
    pub anio: i32,
    pub mes: u32,
    pub etiqueta_mes: String,
    pub celdas: Vec<CeldaCalendarioFfi>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AnioMesFfi {
    pub anio: i32,
    pub mes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Hora12Ffi {
    pub hora12: u32,
    pub minuto: u32,
    pub tarde: bool,
    pub texto: String,
}

// ---------------------------------------------------------------------------
// Registros para avisos, agente y voz
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AvisoFfi {
    pub clave: String,
    pub titulo: String,
    pub cuerpo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AvisoProgramableFfi {
    pub clave: String,
    pub momento_iso: String,
    pub titulo: String,
    pub cuerpo: String,
    pub es_recordatorio: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ProveedorInfoFfi {
    pub codigo: i32,
    pub nombre: String,
    pub url_base: String,
    pub modelo: String,
    pub url_clave: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VozProveedorInfoFfi {
    pub codigo: i32,
    pub nombre: String,
    pub url_base: String,
    pub modelo: String,
    pub url_clave: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AgenteConfigFfi {
    pub proveedor: i32,
    pub url_base: String,
    pub modelo: String,
    pub api_key: String,
    pub ve_imagenes: bool,
    pub voz_proveedor: i32,
    pub voz_url_base: String,
    pub voz_modelo: String,
    pub voz_api_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TipoAdjuntoFfi {
    Pdf,
    Imagen,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AdjuntoFfi {
    pub nombre: String,
    pub tipo: TipoAdjuntoFfi,
    pub bytes: Vec<u8>,
    pub paginas_jpeg: Vec<Vec<u8>>,
    pub texto_ocr: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct LimitesAdjuntosFfi {
    pub max_adjuntos: u32,
    pub max_paginas_pdf: u32,
    pub max_bytes_total: u64,
    pub lado_largo_imagen: u32,
    pub calidad_jpeg: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccionVistaFfi {
    pub id: String,
    pub descripcion: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RespuestaAgenteFfi {
    pub respuesta: String,
    pub acciones: Vec<AccionVistaFfi>,
    pub nota: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ResultadoAplicarTodasFfi {
    pub aplicadas: u32,
    pub resumen: Option<ResumenArchivoFfi>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Rutas, carga, advertencias y conflictos
// ---------------------------------------------------------------------------

/// Ruta por defecto de `order.toml`. En Android vive en `/storage/emulated/0/ORDER/order.toml`.
#[uniffi::export]
pub fn ruta_por_defecto() -> String {
    if cfg!(target_os = "android") {
        "/storage/emulated/0/ORDER/order.toml".to_string()
    } else {
        Almacen::por_defecto().ruta.to_string_lossy().into_owned()
    }
}

/// Carpeta por defecto donde viven `order.toml`, `historial.toml` y `respaldos/`.
#[uniffi::export]
pub fn carpeta_por_defecto() -> String {
    let ruta = PathBuf::from(ruta_por_defecto());
    ruta.parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/storage/emulated/0/ORDER".to_string())
}

/// Carga (o crea desde el ejemplo si no existe) `order.toml` y devuelve el
/// resumen junto con advertencias y conflictos detectados.
#[uniffi::export]
pub fn cargar(ruta: String) -> Result<ResumenArchivoFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    let ahora = Local::now().naive_local();
    Ok(construir_resumen(&almacen, &datos, ahora))
}

/// Mueve a `historial.toml` las tareas hechas hace más de 30 días.
#[uniffi::export]
pub fn archivar(ruta: String) -> Result<ResumenArchivoFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.archivar()?;
    let ahora = Local::now().naive_local();
    Ok(construir_resumen(&almacen, &datos, ahora))
}

/// Advertencias por claves desconocidas en `order.toml` (máximo 4 líneas + resumen).
#[uniffi::export]
pub fn advertencias(ruta: String) -> Result<Vec<String>, ErrorFfi> {
    let texto = std::fs::read_to_string(&ruta).map_err(|e| ErrorFfi::Operacion {
        mensaje: format!("no se pudo leer {ruta}: {e}"),
    })?;
    Ok(formatear_advertencias(&modelo::advertencias(&texto)))
}

/// Detecta copias en conflicto de Syncthing junto a `ruta`.
#[uniffi::export]
pub fn detectar_conflictos(ruta: String) -> Vec<ConflictoFfi> {
    conflictos::detectar(&PathBuf::from(&ruta))
        .into_iter()
        .map(|c| ConflictoFfi {
            mensaje: conflictos::mensaje_banner(&c),
            ruta: c.ruta.to_string_lossy().into_owned(),
        })
        .collect()
}

/// Resuelve un conflicto moviendo a `respaldos/` la copia descartada o el
/// archivo actual (si `usar_otra` es true).
#[uniffi::export]
pub fn resolver_conflicto(
    ruta: String,
    ruta_conflicto: String,
    usar_otra: bool,
) -> Result<(), ErrorFfi> {
    let ruta_order = PathBuf::from(&ruta);
    let conflicto = conflictos::detectar(&ruta_order)
        .into_iter()
        .find(|c| c.ruta == PathBuf::from(&ruta_conflicto))
        .unwrap_or(conflictos::Conflicto {
            ruta: PathBuf::from(&ruta_conflicto),
            fecha: None,
        });

    let res = if usar_otra {
        conflictos::usar_la_otra(&ruta_order, &conflicto)
    } else {
        conflictos::quedarme_con_actual(&ruta_order, &conflicto)
    };
    res.map_err(|mensaje| ErrorFfi::Operacion { mensaje })
}

// ---------------------------------------------------------------------------
// Vistas listas para pintar
// ---------------------------------------------------------------------------

#[uniffi::export]
pub fn vista_inicio(ruta: String) -> Result<InicioVistaFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    let ahora = Local::now().naive_local();
    Ok(construir_inicio(&datos, ahora))
}

#[uniffi::export]
pub fn vista_dia(ruta: String, fecha_iso: Option<String>) -> Result<DiaVistaFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    let ahora = Local::now().naive_local();
    let hoy = ahora.date();
    let fecha = fecha_iso
        .as_deref()
        .and_then(interpretar_fecha)
        .unwrap_or(hoy);
    Ok(construir_dia(&datos, fecha, ahora))
}

#[uniffi::export]
pub fn vista_semana(ruta: String, lunes_iso: Option<String>) -> Result<SemanaVistaFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    let ahora = Local::now().naive_local();
    let hoy = ahora.date();
    let lunes = lunes_iso
        .as_deref()
        .and_then(interpretar_fecha)
        .map(semana::lunes_de)
        .unwrap_or_else(|| semana::lunes_de(hoy));
    Ok(construir_semana(&datos, lunes, ahora))
}

#[uniffi::export]
pub fn vista_pendientes(
    ruta: String,
    filtro: FiltroPendientesFfi,
) -> Result<PendientesVistaFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    let ahora = Local::now().naive_local();
    Ok(construir_pendientes(&datos, &filtro, ahora))
}

#[uniffi::export]
pub fn materias_con_tono(ruta: String) -> Result<Vec<MateriaConTonoFfi>, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    let lista = materias_de(&datos);
    Ok(materias::materias_con_tono(&lista)
        .into_iter()
        .map(|m| MateriaConTonoFfi {
            nombre: m.nombre,
            tono: m.tono,
        })
        .collect())
}

#[uniffi::export]
pub fn tono_de_materia(nombre: String) -> i32 {
    tono_materia(&nombre)
}

#[uniffi::export]
pub fn canonica_materia(ruta: String, texto: String) -> Result<String, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    let lista = materias_de(&datos);
    Ok(materias::canonica(&lista, &texto))
}

// ---------------------------------------------------------------------------
// Formularios: lectura de estado inicial y validaciones
// ---------------------------------------------------------------------------

#[uniffi::export]
pub fn obtener_tarea_form(ruta: String, id: Option<String>) -> Result<TareaFormFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    match id.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(id_buscado) => {
            let tarea = datos
                .tareas
                .into_iter()
                .find(|t| t.id == id_buscado)
                .ok_or_else(|| ErrorFfi::Operacion {
                    mensaje: "Esa tarea ya no está en el archivo.".to_string(),
                })?;
            Ok(TareaFormFfi {
                id: tarea.id,
                titulo: tarea.titulo,
                materia: tarea.materia.unwrap_or_default(),
                fecha_iso: tarea.vence.format(FORMATO_FECHA).to_string(),
                hora_texto: hora_texto(tarea.vence.time()),
                hecha: tarea.hecha,
            })
        }
        None => {
            let defecto = Local::now().date_naive() + TimeDelta::days(7);
            Ok(TareaFormFfi {
                id: String::new(),
                titulo: String::new(),
                materia: String::new(),
                fecha_iso: defecto.format(FORMATO_FECHA).to_string(),
                hora_texto: hora_texto(construir_hora(23, 59)),
                hecha: false,
            })
        }
    }
}

#[uniffi::export]
pub fn obtener_clase_form(ruta: String, id: Option<String>) -> Result<ClaseFormFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    match id.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(id_buscado) => {
            let clase = datos
                .clases
                .into_iter()
                .find(|c| c.id == id_buscado)
                .ok_or_else(|| ErrorFfi::Operacion {
                    mensaje: "Esa clase ya no está en el archivo.".to_string(),
                })?;
            Ok(ClaseFormFfi {
                id: clase.id,
                materia: clase.materia,
                dias: clase.dias.into_iter().map(DiaFfi::from).collect(),
                hora_inicio: hora_texto(clase.inicio),
                hora_fin: hora_texto(clase.fin),
                lugar: clase.lugar.unwrap_or_default(),
            })
        }
        None => Ok(ClaseFormFfi {
            id: String::new(),
            materia: String::new(),
            dias: vec![DiaFfi::Lun],
            hora_inicio: hora_texto(construir_hora(7, 0)),
            hora_fin: hora_texto(construir_hora(9, 0)),
            lugar: String::new(),
        }),
    }
}

#[uniffi::export]
pub fn obtener_recordatorio_form(
    ruta: String,
    id: Option<String>,
) -> Result<RecordatorioFormFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    match id.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(id_buscado) => {
            let rec = datos
                .recordatorios
                .into_iter()
                .find(|r| r.id == id_buscado)
                .ok_or_else(|| ErrorFfi::Operacion {
                    mensaje: "Ese recordatorio ya no está en el archivo.".to_string(),
                })?;
            Ok(RecordatorioFormFfi {
                id: rec.id,
                texto: rec.texto,
                dias: rec.dias.into_iter().map(DiaFfi::from).collect(),
                hora: hora_texto(rec.hora),
            })
        }
        None => Ok(RecordatorioFormFfi {
            id: String::new(),
            texto: String::new(),
            dias: Vec::new(),
            hora: hora_texto(construir_hora(18, 0)),
        }),
    }
}

#[uniffi::export]
pub fn obtener_ajustes_form(ruta: String) -> Result<AjustesFormFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    let hoy = Local::now().date_naive();
    let (inicio, fin) = match &datos.semestre {
        Some(s) => (s.inicio, s.fin),
        None => (hoy, hoy + TimeDelta::days(120)),
    };
    let recordatorios = datos
        .recordatorios
        .iter()
        .map(|r| RecordatorioListaFfi {
            id: r.id.clone(),
            texto: r.texto.clone(),
            dias_texto: dias_texto(&r.dias),
            hora_texto: hora_texto(r.hora),
        })
        .collect();
    Ok(AjustesFormFfi {
        sin_semestre: datos.semestre.is_none(),
        semestre_inicio_iso: inicio.format(FORMATO_FECHA).to_string(),
        semestre_fin_iso: fin.format(FORMATO_FECHA).to_string(),
        aviso_dia_antes: hora_texto(datos.ajustes.aviso_dia_antes),
        aviso_horas_antes: datos.ajustes.aviso_horas_antes.to_string(),
        recordatorios,
    })
}

#[uniffi::export]
pub fn validar_tarea_campos(
    titulo: String,
    fecha_iso: String,
    hora: String,
    materia: String,
    materias_existentes: Vec<String>,
) -> Result<(), ErrorFfi> {
    formularios::validar_tarea(&titulo, &fecha_iso, &hora, &materia, &materias_existentes)?;
    Ok(())
}

#[uniffi::export]
pub fn validar_clase_campos(
    id: String,
    materia: String,
    dias: Vec<DiaFfi>,
    hora_inicio: String,
    hora_fin: String,
    lugar: String,
    materias_existentes: Vec<String>,
) -> Result<(), ErrorFfi> {
    let dias_modelo: Vec<Dia> = dias.into_iter().map(Dia::from).collect();
    formularios::validar_clase(
        id,
        &materia,
        dias_modelo,
        &hora_inicio,
        &hora_fin,
        &lugar,
        &materias_existentes,
    )?;
    Ok(())
}

#[uniffi::export]
pub fn validar_recordatorio_campos(
    id: String,
    texto: String,
    hora: String,
    dias: Vec<DiaFfi>,
) -> Result<(), ErrorFfi> {
    let dias_modelo: Vec<Dia> = dias.into_iter().map(Dia::from).collect();
    formularios::validar_recordatorio(id, &texto, &hora, dias_modelo)?;
    Ok(())
}

#[uniffi::export]
pub fn validar_ajustes_campos(
    sin_semestre: bool,
    inicio_iso: String,
    fin_iso: String,
    hora_aviso: String,
    horas_antes: String,
) -> Result<(), ErrorFfi> {
    formularios::validar_ajustes(
        sin_semestre,
        &inicio_iso,
        &fin_iso,
        &hora_aviso,
        &horas_antes,
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Operaciones de escritura vía Almacen::modificar
// ---------------------------------------------------------------------------

#[uniffi::export]
pub fn guardar_tarea(
    ruta: String,
    id: Option<String>,
    titulo: String,
    fecha_iso: String,
    hora: String,
    materia: String,
    hecha: bool,
) -> Result<ResumenArchivoFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let materias = materias_existentes_o_vacio(&almacen);
    let nueva = formularios::validar_tarea(&titulo, &fecha_iso, &hora, &materia, &materias)?;
    let hoy = Local::now().date_naive();
    let id_limpio = id.unwrap_or_default().trim().to_string();

    let datos = almacen.modificar(|doc| {
        if id_limpio.is_empty() {
            let nuevo_id = ops::agregar_tarea(doc, &nueva)?;
            if hecha {
                ops::marcar_tarea(doc, &nuevo_id, true, hoy)?;
            }
            Ok(())
        } else {
            let antes = modelo::parsear(&doc.to_string())
                .ok()
                .and_then(|d| d.tareas.into_iter().find(|t| t.id == id_limpio))
                .map(|t| t.hecha);
            ops::editar_tarea(doc, &id_limpio, &nueva)?;
            if antes.is_some_and(|a| a != hecha) {
                ops::marcar_tarea(doc, &id_limpio, hecha, hoy)?;
            }
            Ok(())
        }
    })?;

    let ahora = Local::now().naive_local();
    Ok(construir_resumen(&almacen, &datos, ahora))
}

#[uniffi::export]
pub fn marcar_tarea(
    ruta: String,
    id: String,
    hecha: bool,
) -> Result<ResumenArchivoFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let hoy = Local::now().date_naive();
    let datos = almacen.modificar(|doc| ops::marcar_tarea(doc, &id, hecha, hoy))?;
    let ahora = Local::now().naive_local();
    Ok(construir_resumen(&almacen, &datos, ahora))
}

#[uniffi::export]
pub fn borrar_tarea(ruta: String, id: String) -> Result<ResumenArchivoFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.modificar(|doc| ops::borrar_tarea(doc, &id))?;
    let ahora = Local::now().naive_local();
    Ok(construir_resumen(&almacen, &datos, ahora))
}

#[uniffi::export]
pub fn guardar_clase(
    ruta: String,
    id: Option<String>,
    materia: String,
    dias: Vec<DiaFfi>,
    hora_inicio: String,
    hora_fin: String,
    lugar: String,
) -> Result<ResumenArchivoFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let materias = materias_existentes_o_vacio(&almacen);
    let dias_modelo: Vec<Dia> = orden_dias(dias);
    let clase = formularios::validar_clase(
        id.unwrap_or_default(),
        &materia,
        dias_modelo,
        &hora_inicio,
        &hora_fin,
        &lugar,
        &materias,
    )?;
    let datos = almacen.modificar(|doc| ops::guardar_clase(doc, &clase))?;
    let ahora = Local::now().naive_local();
    Ok(construir_resumen(&almacen, &datos, ahora))
}

#[uniffi::export]
pub fn borrar_clase(ruta: String, id: String) -> Result<ResumenArchivoFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.modificar(|doc| ops::borrar_clase(doc, &id))?;
    let ahora = Local::now().naive_local();
    Ok(construir_resumen(&almacen, &datos, ahora))
}

#[uniffi::export]
pub fn guardar_recordatorio(
    ruta: String,
    id: Option<String>,
    texto: String,
    hora: String,
    dias: Vec<DiaFfi>,
) -> Result<ResumenArchivoFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let dias_modelo: Vec<Dia> = orden_dias(dias);
    let rec =
        formularios::validar_recordatorio(id.unwrap_or_default(), &texto, &hora, dias_modelo)?;
    let datos = almacen.modificar(|doc| ops::guardar_recordatorio(doc, &rec))?;
    let ahora = Local::now().naive_local();
    Ok(construir_resumen(&almacen, &datos, ahora))
}

#[uniffi::export]
pub fn borrar_recordatorio(ruta: String, id: String) -> Result<ResumenArchivoFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.modificar(|doc| ops::borrar_recordatorio(doc, &id))?;
    let ahora = Local::now().naive_local();
    Ok(construir_resumen(&almacen, &datos, ahora))
}

#[uniffi::export]
pub fn guardar_ajustes(
    ruta: String,
    sin_semestre: bool,
    inicio_iso: String,
    fin_iso: String,
    hora_aviso: String,
    horas_antes: String,
) -> Result<ResumenArchivoFfi, ErrorFfi> {
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let (ajustes, semestre): (Ajustes, Option<Semestre>) = formularios::validar_ajustes(
        sin_semestre,
        &inicio_iso,
        &fin_iso,
        &hora_aviso,
        &horas_antes,
    )?;
    let datos = almacen.modificar(|doc| {
        ops::guardar_ajustes(doc, &ajustes)?;
        ops::guardar_semestre(doc, semestre.as_ref())
    })?;
    let ahora = Local::now().naive_local();
    Ok(construir_resumen(&almacen, &datos, ahora))
}

// ---------------------------------------------------------------------------
// Textos y selectores (calendario y reloj 12 h)
// ---------------------------------------------------------------------------

#[uniffi::export]
pub fn fecha_hoy_iso() -> String {
    Local::now()
        .date_naive()
        .format(FORMATO_FECHA)
        .to_string()
}

#[uniffi::export]
pub fn lunes_actual_iso() -> String {
    semana::lunes_de(Local::now().date_naive())
        .format(FORMATO_FECHA)
        .to_string()
}

#[uniffi::export]
pub fn sumar_dias_iso(fecha_iso: String, delta: i32) -> String {
    let base = interpretar_fecha(&fecha_iso).unwrap_or_else(|| Local::now().date_naive());
    let movida = base + TimeDelta::days(i64::from(delta));
    movida.format(FORMATO_FECHA).to_string()
}

#[uniffi::export]
pub fn calendario_mes(anio: i32, mes: u32, elegido: u32) -> CalendarioMesFfi {
    let hoy = Local::now().date_naive();
    let celdas = textos::celdas_mes(anio, mes, elegido, hoy)
        .into_iter()
        .map(|c| CeldaCalendarioFfi {
            dia: c.dia,
            fila: c.fila,
            columna: c.columna,
            hoy: c.hoy,
            elegida: c.elegida,
        })
        .collect();
    CalendarioMesFfi {
        anio,
        mes,
        etiqueta_mes: etiqueta_mes(anio, mes),
        celdas,
    }
}

#[uniffi::export]
pub fn mes_vecino(anio: i32, mes: u32, delta: i32) -> AnioMesFfi {
    let (a, m) = textos::mes_vecino(anio, mes, delta);
    AnioMesFfi { anio: a, mes: m }
}

#[uniffi::export]
pub fn dias_del_mes(anio: i32, mes: u32) -> i32 {
    textos::dias_del_mes(anio, mes)
}

#[uniffi::export]
pub fn horas_12_texto() -> Vec<String> {
    horas_texto()
}

#[uniffi::export]
pub fn minutos_60_texto() -> Vec<String> {
    minutos_texto()
}

#[uniffi::export]
pub fn formatear_hora_12(hora12: u32, minuto: u32, tarde: bool) -> String {
    let h24 = a_24h(hora12, tarde);
    let m = minuto.min(59);
    hora_texto(construir_hora(h24, m))
}

#[uniffi::export]
pub fn parsear_hora_12(texto: String) -> Hora12Ffi {
    let t = interpretar_hora(&texto).unwrap_or_else(|| construir_hora(23, 59));
    let (hora12, tarde) = a_12h(t.hour());
    Hora12Ffi {
        hora12,
        minuto: t.minute(),
        tarde,
        texto: hora_texto(t),
    }
}

#[uniffi::export]
pub fn dias_semana_etiquetas() -> Vec<String> {
    textos::DIAS_SEMANA
        .iter()
        .map(|d| textos::texto_dia(*d).to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Avisos programables, registro y resumen del día
// ---------------------------------------------------------------------------

const DIAS_PODA_REGISTRO: i64 = 7;
const FORMATO_ISO_SEGUNDOS: &str = "%Y-%m-%dT%H:%M:%S";

#[uniffi::export]
pub fn proximos_avisos(
    ruta: String,
    desde_iso: String,
    hasta_iso: String,
    registro_ruta: String,
) -> Result<Vec<AvisoProgramableFfi>, ErrorFfi> {
    let desde = interpretar_fecha_hora(&desde_iso).ok_or_else(|| ErrorFfi::Operacion {
        mensaje: format!("fecha inicial no válida: {desde_iso}"),
    })?;
    let hasta = interpretar_fecha_hora(&hasta_iso).ok_or_else(|| ErrorFfi::Operacion {
        mensaje: format!("fecha final no válida: {hasta_iso}"),
    })?;

    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    let registro = Registro {
        ruta: PathBuf::from(&registro_ruta),
    };
    registro.podar(desde.date() - TimeDelta::days(DIAS_PODA_REGISTRO));
    let enviados = registro.leer();

    Ok(avisos::proximos_avisos(&datos, desde, hasta, &enviados)
        .into_iter()
        .map(|item| AvisoProgramableFfi {
            clave: item.aviso.clave,
            momento_iso: item.momento.format(FORMATO_ISO_SEGUNDOS).to_string(),
            titulo: item.aviso.titulo,
            cuerpo: item.aviso.cuerpo,
            es_recordatorio: item.es_recordatorio,
        })
        .collect())
}

#[uniffi::export]
pub fn anotar_aviso(registro_ruta: String, clave: String) {
    let registro = Registro {
        ruta: PathBuf::from(&registro_ruta),
    };
    registro.anotar(&clave);
}

#[uniffi::export]
pub fn resumen_del_dia(
    ruta: String,
    ahora_iso: String,
    registro_ruta: String,
) -> Result<Option<AvisoFfi>, ErrorFfi> {
    let ahora = if ahora_iso.trim().is_empty() {
        Local::now().naive_local()
    } else {
        interpretar_fecha_hora(&ahora_iso).ok_or_else(|| ErrorFfi::Operacion {
            mensaje: format!("fecha/hora no válida: {ahora_iso}"),
        })?
    };

    let almacen = Almacen {
        ruta: PathBuf::from(&ruta),
    };
    let datos = almacen.cargar()?;
    let registro = Registro {
        ruta: PathBuf::from(&registro_ruta),
    };
    registro.podar(ahora.date() - TimeDelta::days(DIAS_PODA_REGISTRO));
    let enviados = registro.leer();

    let Some(resumen) = avisos::resumen_arranque(&datos, ahora) else {
        return Ok(None);
    };
    if enviados.contains(&resumen.clave) {
        return Ok(None);
    }
    registro.anotar(&resumen.clave);
    Ok(Some(AvisoFfi {
        clave: resumen.clave,
        titulo: resumen.titulo,
        cuerpo: resumen.cuerpo,
    }))
}

// ---------------------------------------------------------------------------
// Agente y voz
// ---------------------------------------------------------------------------

struct AccionPendiente {
    accion: Accion,
    descripcion: String,
}

static SIGUIENTE_ACCION_ID: AtomicU64 = AtomicU64::new(1);
static ACCIONES_PENDIENTES: LazyLock<Mutex<HashMap<String, AccionPendiente>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static CONVERSACION: LazyLock<Mutex<Conversacion>> =
    LazyLock::new(|| Mutex::new(Conversacion::nueva()));

#[uniffi::export]
pub fn proveedores_agente() -> Vec<ProveedorInfoFfi> {
    Proveedor::TODOS
        .iter()
        .map(|p| {
            let (url_base, modelo) = p.por_defecto();
            ProveedorInfoFfi {
                codigo: p.codigo(),
                nombre: p.nombre().to_string(),
                url_base: url_base.to_string(),
                modelo: modelo.to_string(),
                url_clave: p.url_clave().map(str::to_string),
            }
        })
        .collect()
}

#[uniffi::export]
pub fn proveedores_voz() -> Vec<VozProveedorInfoFfi> {
    VozProveedor::TODOS
        .iter()
        .map(|p| {
            let (url_base, modelo) = p.por_defecto();
            VozProveedorInfoFfi {
                codigo: p.codigo(),
                nombre: p.nombre().to_string(),
                url_base: url_base.to_string(),
                modelo: modelo.to_string(),
                url_clave: p.url_clave().to_string(),
            }
        })
        .collect()
}

#[uniffi::export]
pub fn leer_agente_config(ruta_config: String) -> AgenteConfigFfi {
    let cfg = config::cargar_de(Path::new(&ruta_config));
    agente_config_a_ffi(&cfg)
}

#[uniffi::export]
pub fn validar_agente(
    proveedor: i32,
    url: String,
    modelo: String,
    api_key: String,
    voz_proveedor: i32,
    voz_url: String,
    voz_modelo: String,
    voz_key: String,
) -> Result<(), ErrorFfi> {
    formularios::validar_agente(
        proveedor,
        &url,
        &modelo,
        &api_key,
        voz_proveedor,
        &voz_url,
        &voz_modelo,
        &voz_key,
    )?;
    Ok(())
}

#[uniffi::export]
pub fn guardar_agente_config(
    ruta_config: String,
    proveedor: i32,
    url: String,
    modelo: String,
    api_key: String,
    ve_imagenes: bool,
    voz_proveedor: i32,
    voz_url: String,
    voz_modelo: String,
    voz_key: String,
) -> Result<AgenteConfigFfi, ErrorFfi> {
    let mut cfg = formularios::validar_agente(
        proveedor,
        &url,
        &modelo,
        &api_key,
        voz_proveedor,
        &voz_url,
        &voz_modelo,
        &voz_key,
    )?;
    cfg.ve_imagenes = ve_imagenes;
    config::guardar_en(Path::new(&ruta_config), &cfg)
        .map_err(|mensaje| ErrorFfi::Operacion { mensaje })?;
    Ok(agente_config_a_ffi(&cfg))
}

#[uniffi::export]
pub fn agente_ve_imagenes(ruta_config: String) -> bool {
    config::cargar_de(Path::new(&ruta_config)).ve_imagenes()
}

#[uniffi::export]
pub fn texto_de_pdf(bytes: Vec<u8>) -> Result<Vec<String>, ErrorFfi> {
    adjuntos::extraer_texto_pdf(&bytes).map_err(|e| ErrorFfi::Operacion {
        mensaje: e.to_string(),
    })
}

#[uniffi::export]
pub fn paginas_sin_texto(paginas: Vec<String>) -> Vec<u32> {
    adjuntos::paginas_sin_texto(&paginas)
        .into_iter()
        .map(|i| i as u32)
        .collect()
}

#[uniffi::export]
pub fn limites_adjuntos() -> LimitesAdjuntosFfi {
    LimitesAdjuntosFfi {
        max_adjuntos: adjuntos::MAX_ADJUNTOS as u32,
        max_paginas_pdf: adjuntos::MAX_PAGINAS_PDF as u32,
        max_bytes_total: adjuntos::MAX_BYTES_TOTAL as u64,
        lado_largo_imagen: adjuntos::LADO_LARGO_IMAGEN,
        calidad_jpeg: u32::from(adjuntos::CALIDAD_JPEG),
    }
}

#[uniffi::export]
pub fn enviar_al_agente(
    ruta_datos: String,
    ruta_config: String,
    texto: String,
    adjuntos: Vec<AdjuntoFfi>,
) -> Result<RespuestaAgenteFfi, ErrorFfi> {
    let pregunta = texto.trim();
    if pregunta.is_empty() && adjuntos.is_empty() {
        return Err(ErrorFfi::Operacion {
            mensaje: "Escribe una petición para el agente.".to_string(),
        });
    }
    let pregunta_efectiva = if pregunta.is_empty() {
        "Revisa los archivos adjuntos y propón las tareas, clases o recordatorios correspondientes."
    } else {
        pregunta
    };

    let cfg = config::cargar_de(Path::new(&ruta_config));
    if !cfg.lista() {
        return Err(ErrorFfi::Operacion {
            mensaje: "El agente no tiene clave. Ábrela en Ajustes → Agente y guárdala.".to_string(),
        });
    }

    let almacen = Almacen {
        ruta: PathBuf::from(&ruta_datos),
    };
    let datos = almacen.cargar()?;
    let ahora = Local::now();
    let resumen_ctx = contexto::resumen(
        &datos,
        ahora.naive_local(),
        &format!("UTC{}", ahora.format("%:z")),
    );

    let nombres_adjuntos: Vec<String> = adjuntos.iter().map(|a| a.nombre.clone()).collect();
    let adjuntos_core: Vec<Adjunto> = adjuntos.into_iter().map(adjunto_de_ffi).collect();
    let conv_snapshot = CONVERSACION
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| Conversacion::nueva());
    let salida = cliente::consultar(
        &cfg,
        &resumen_ctx,
        pregunta_efectiva,
        &adjuntos_core,
        &conv_snapshot,
    )
    .map_err(|e| ErrorFfi::Operacion {
        mensaje: e.to_string(),
    })?;

    Ok(construir_respuesta_agente(
        &datos,
        &salida,
        pregunta_efectiva,
        &nombres_adjuntos,
    ))
}

#[uniffi::export]
pub fn nueva_conversacion() {
    if let Ok(mut guard) = CONVERSACION.lock() {
        *guard = Conversacion::nueva();
    }
    if let Ok(mut guard) = ACCIONES_PENDIENTES.lock() {
        guard.clear();
    }
}

#[uniffi::export]
pub fn hay_conversacion() -> bool {
    CONVERSACION
        .lock()
        .map(|guard| !guard.esta_vacia())
        .unwrap_or(false)
}

#[uniffi::export]
pub fn aplicar_accion(ruta_datos: String, accion_id: String) -> Result<ResumenArchivoFfi, ErrorFfi> {
    let pendiente = {
        let guard = ACCIONES_PENDIENTES
            .lock()
            .map_err(|_| ErrorFfi::Operacion {
                mensaje: "No se pudo acceder a las acciones pendientes.".to_string(),
            })?;
        guard.get(&accion_id).map(|p| (p.accion.clone(), p.descripcion.clone()))
    }
    .ok_or_else(|| ErrorFfi::Operacion {
        mensaje: "Esa acción ya no está pendiente.".to_string(),
    })?;

    let almacen = Almacen {
        ruta: PathBuf::from(&ruta_datos),
    };
    let datos = acciones::aplicar(&almacen, &pendiente.0)?;
    if let Ok(mut guard) = ACCIONES_PENDIENTES.lock() {
        guard.remove(&accion_id);
    }
    if let Ok(mut conv) = CONVERSACION.lock() {
        conv.anotar_resultado(&pendiente.1, true);
    }

    let ahora = Local::now().naive_local();
    Ok(construir_resumen(&almacen, &datos, ahora))
}

#[uniffi::export]
pub fn descartar_accion(accion_id: String) {
    let descartada = ACCIONES_PENDIENTES
        .lock()
        .ok()
        .and_then(|mut guard| guard.remove(&accion_id));
    if let Some(pendiente) = descartada {
        if let Ok(mut conv) = CONVERSACION.lock() {
            conv.anotar_resultado(&pendiente.descripcion, false);
        }
    }
}

#[uniffi::export]
pub fn aplicar_todas(
    ruta_datos: String,
    accion_ids: Vec<String>,
) -> Result<ResultadoAplicarTodasFfi, ErrorFfi> {
    let acciones_pares: Vec<(String, Accion, String)> = {
        let guard = ACCIONES_PENDIENTES
            .lock()
            .map_err(|_| ErrorFfi::Operacion {
                mensaje: "No se pudo acceder a las acciones pendientes.".to_string(),
            })?;
        accion_ids
            .iter()
            .filter_map(|id| {
                guard
                    .get(id)
                    .map(|p| (id.clone(), p.accion.clone(), p.descripcion.clone()))
            })
            .collect()
    };

    if acciones_pares.is_empty() {
        return Err(ErrorFfi::Operacion {
            mensaje: "No hay acciones pendientes por aplicar.".to_string(),
        });
    }

    let solo_acciones: Vec<Accion> = acciones_pares.iter().map(|(_, a, _)| a.clone()).collect();
    let almacen = Almacen {
        ruta: PathBuf::from(&ruta_datos),
    };
    let resultado = acciones::aplicar_todas(&almacen, &solo_acciones);

    if let Ok(mut guard) = ACCIONES_PENDIENTES.lock() {
        for (id, _, _) in acciones_pares.iter().take(resultado.aplicadas) {
            guard.remove(id);
        }
    }
    if let Ok(mut conv) = CONVERSACION.lock() {
        for (_, _, descripcion) in acciones_pares.iter().take(resultado.aplicadas) {
            conv.anotar_resultado(descripcion, true);
        }
    }

    let ahora = Local::now().naive_local();
    Ok(ResultadoAplicarTodasFfi {
        aplicadas: resultado.aplicadas as u32,
        resumen: resultado
            .ultimos_datos
            .as_ref()
            .map(|d| construir_resumen(&almacen, d, ahora)),
        error: resultado.error,
    })
}

#[uniffi::export]
pub fn wav_de_pcm16(muestras: Vec<i16>, frecuencia: u32) -> Vec<u8> {
    voz::wav_de_pcm16(&muestras, frecuencia)
}

#[uniffi::export]
pub fn transcribir(ruta_config: String, wav: Vec<u8>) -> Result<String, ErrorFfi> {
    let cfg = config::cargar_de(Path::new(&ruta_config));
    if !cfg.voz.lista() {
        return Err(ErrorFfi::Operacion {
            mensaje: "La voz no tiene clave: Ajustes → Agente → Voz.".to_string(),
        });
    }
    voz::transcribir(&cfg.voz, &wav).map_err(|e| ErrorFfi::Operacion {
        mensaje: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Ayudantes internos
// ---------------------------------------------------------------------------

fn interpretar_fecha_hora(texto: &str) -> Option<NaiveDateTime> {
    let limpio = texto.trim();
    for fmt in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(limpio, fmt) {
            return Some(dt);
        }
    }
    None
}

fn agente_config_a_ffi(cfg: &AgenteConfig) -> AgenteConfigFfi {
    AgenteConfigFfi {
        proveedor: cfg.proveedor.codigo(),
        url_base: cfg.url_base.clone(),
        modelo: cfg.modelo.clone(),
        api_key: cfg.api_key.clone(),
        ve_imagenes: cfg.ve_imagenes,
        voz_proveedor: cfg.voz.proveedor.codigo(),
        voz_url_base: cfg.voz.url_base.clone(),
        voz_modelo: cfg.voz.modelo.clone(),
        voz_api_key: cfg.voz.api_key.clone(),
    }
}

fn adjunto_de_ffi(a: AdjuntoFfi) -> Adjunto {
    let contenido = match a.tipo {
        TipoAdjuntoFfi::Pdf => ContenidoAdjunto::Pdf {
            bytes: a.bytes,
            paginas_jpeg: a.paginas_jpeg,
            texto_ocr: a.texto_ocr,
        },
        TipoAdjuntoFfi::Imagen => ContenidoAdjunto::Imagen {
            jpeg: a.bytes,
            texto_ocr: a.texto_ocr,
        },
    };
    Adjunto {
        nombre: a.nombre,
        contenido,
    }
}

fn construir_respuesta_agente(
    datos: &Datos,
    crudo: &str,
    pregunta: &str,
    nombres_adjuntos: &[String],
) -> RespuestaAgenteFfi {
    match acciones::parsear(crudo) {
        Ok(lectura) => {
            let respuesta = if lectura.respuesta.is_empty() {
                "(sin comentario)".to_string()
            } else {
                lectura.respuesta
            };
            let mut vistas = Vec::with_capacity(lectura.acciones.len());
            if let Ok(mut guard) = ACCIONES_PENDIENTES.lock() {
                for accion in lectura.acciones {
                    let num = SIGUIENTE_ACCION_ID.fetch_add(1, Ordering::Relaxed);
                    let id = format!("accion-{num}");
                    let descripcion = accion.descripcion(datos);
                    guard.insert(
                        id.clone(),
                        AccionPendiente {
                            accion,
                            descripcion: descripcion.clone(),
                        },
                    );
                    vistas.push(AccionVistaFfi { id, descripcion });
                }
            }
            if let Ok(mut conv) = CONVERSACION.lock() {
                conv.tomar_resultados();
                conv.registrar_turno(pregunta, nombres_adjuntos, &respuesta, lectura.resumen);
            }
            RespuestaAgenteFfi {
                respuesta,
                acciones: vistas,
                nota: None,
            }
        }
        Err(mensaje) => {
            if let Ok(mut conv) = CONVERSACION.lock() {
                conv.tomar_resultados();
            }
            RespuestaAgenteFfi {
                respuesta: crudo.to_string(),
                acciones: Vec::new(),
                nota: Some(format!("No pude leer la propuesta: {mensaje}")),
            }
        }
    }
}

fn construir_resumen(almacen: &Almacen, datos: &Datos, ahora: NaiveDateTime) -> ResumenArchivoFfi {
    let pendientes = semana::pendientes(datos);
    let adv = std::fs::read_to_string(&almacen.ruta)
        .map(|texto| formatear_advertencias(&modelo::advertencias(&texto)))
        .unwrap_or_default();
    let conf = conflictos::detectar(&almacen.ruta)
        .into_iter()
        .map(|c| ConflictoFfi {
            mensaje: conflictos::mensaje_banner(&c),
            ruta: c.ruta.to_string_lossy().into_owned(),
        })
        .collect();

    ResumenArchivoFfi {
        ruta: almacen.ruta.to_string_lossy().into_owned(),
        num_clases: datos.clases.len() as u32,
        num_tareas: datos.tareas.len() as u32,
        num_tareas_pendientes: pendientes.len() as u32,
        num_recordatorios: datos.recordatorios.len() as u32,
        advertencias: adv,
        conflictos: conf,
        pie: pie(datos, pendientes.len(), ahora),
    }
}

fn formatear_advertencias(advertencias: &[ErrorDatos]) -> Vec<String> {
    const MAXIMO: usize = 4;
    let mut lineas: Vec<String> = advertencias
        .iter()
        .take(MAXIMO)
        .map(|a| a.to_string())
        .collect();
    if advertencias.len() > MAXIMO {
        lineas.push(format!("… y {} más", advertencias.len() - MAXIMO));
    }
    lineas
}

fn materias_existentes_o_vacio(almacen: &Almacen) -> Vec<String> {
    almacen
        .cargar()
        .map(|d| materias_de(&d))
        .unwrap_or_default()
}

fn orden_dias(dias: Vec<DiaFfi>) -> Vec<Dia> {
    let mut salida = Vec::new();
    for dia in textos::DIAS_SEMANA {
        if dias.iter().any(|d| Dia::from(*d) == dia) {
            salida.push(dia);
        }
    }
    salida
}

fn tarea_a_ffi(t: &Tarea, ahora: NaiveDateTime) -> TareaFfi {
    TareaFfi {
        id: t.id.clone(),
        titulo: t.titulo.clone(),
        materia: t.materia.clone().unwrap_or_default(),
        fecha_texto: fecha_tarea(t.vence),
        fecha_iso: t.vence.date().format(FORMATO_FECHA).to_string(),
        hora_texto: hora_texto(t.vence.time()),
        vencida: semana::vencida(t, ahora),
        hecha: t.hecha,
        tono: tono_materia(t.materia.as_deref().unwrap_or("")),
    }
}

fn clase_inicio_a_ffi(
    b: &semana::Bloque,
    fecha: NaiveDate,
    hoy: NaiveDate,
) -> ClaseInicioFfi {
    ClaseInicioFfi {
        id: b.clase_id.clone(),
        materia: b.materia.clone(),
        lugar: b.lugar.clone().unwrap_or_default(),
        hora: hora_clase(b, fecha, hoy),
        tono: tono_materia(&b.materia),
    }
}

fn recordatorio_inicio_a_ffi(
    r: &semana::RecordatorioVisto,
    hoy: NaiveDate,
) -> RecordatorioInicioFfi {
    RecordatorioInicioFfi {
        id: r.id.clone(),
        texto: r.texto.clone(),
        cuando: cuando_recordatorio(r, hoy),
    }
}

fn construir_inicio(datos: &Datos, ahora: NaiveDateTime) -> InicioVistaFfi {
    let hoy = ahora.date();
    let resumen = semana::inicio(datos, ahora);
    let pendientes = semana::pendientes(datos);

    InicioVistaFfi {
        fecha_larga: fecha_larga(hoy),
        clases: resumen
            .clases
            .iter()
            .map(|b| clase_inicio_a_ffi(b, hoy, hoy))
            .collect(),
        siguiente: resumen
            .siguiente
            .as_ref()
            .map(|(f, b)| clase_inicio_a_ffi(b, *f, hoy)),
        vencidas: resumen
            .vencidas
            .iter()
            .map(|t| tarea_a_ffi(t, ahora))
            .collect(),
        de_hoy: resumen
            .de_hoy
            .iter()
            .map(|t| tarea_a_ffi(t, ahora))
            .collect(),
        proximas: resumen
            .proximos
            .iter()
            .map(|t| tarea_a_ffi(t, ahora))
            .collect(),
        recordatorios: resumen
            .recordatorios
            .iter()
            .map(|r| recordatorio_inicio_a_ffi(r, hoy))
            .collect(),
        por_materia: resumen
            .por_materia
            .iter()
            .map(|(m, cuantas)| ConteoMateriaFfi {
                materia: m.clone(),
                cuantas: *cuantas as i32,
                tono: tono_materia(m),
            })
            .collect(),
        pie: pie(datos, pendientes.len(), ahora),
    }
}

fn construir_dia(datos: &Datos, fecha: NaiveDate, ahora: NaiveDateTime) -> DiaVistaFfi {
    let hoy = ahora.date();
    let vista = semana::dia(datos, fecha);
    DiaVistaFfi {
        fecha_iso: fecha.format(FORMATO_FECHA).to_string(),
        etiqueta: fecha_larga(fecha),
        es_hoy: fecha == hoy,
        clases: vista
            .bloques
            .iter()
            .map(|b| clase_inicio_a_ffi(b, fecha, hoy))
            .collect(),
        tareas: vista
            .tareas
            .iter()
            .map(|t| tarea_a_ffi(t, ahora))
            .collect(),
        recordatorios: semana::recordatorios_de(datos, fecha)
            .iter()
            .map(|r| recordatorio_inicio_a_ffi(r, hoy))
            .collect(),
    }
}

fn construir_semana(datos: &Datos, lunes: NaiveDate, ahora: NaiveDateTime) -> SemanaVistaFfi {
    let hoy = ahora.date();
    let dias_vista = semana::semana(datos, lunes);

    let mut base = 7;
    let mut fin = 19;
    for d in &dias_vista {
        for b in &d.bloques {
            base = base.min(b.inicio.hour() as i32);
            let termina = b.fin.hour() as i32 + i32::from(b.fin.minute() > 0);
            fin = fin.max(termina);
        }
    }

    let ahora_min = ahora.hour() as i32 * 60 + ahora.minute() as i32;
    let es_actual = lunes == semana::lunes_de(hoy);
    let mostrar_ahora = es_actual && ahora_min >= base * 60 && ahora_min <= fin * 60;
    let hay_tareas = dias_vista.iter().any(|d| !d.tareas.is_empty());

    let dias = dias_vista
        .iter()
        .map(|d| {
            let mut tareas: Vec<TareaDiaSemanaFfi> = d
                .tareas
                .iter()
                .map(|t| TareaDiaSemanaFfi {
                    id: t.id.clone(),
                    titulo: t.titulo.clone(),
                    hora: hora_texto(t.vence.time()),
                    hecha: t.hecha,
                })
                .collect();
            let extra_tareas = if tareas.len() > 2 {
                let resto = tareas.len() - 2;
                tareas.truncate(2);
                format!("+{resto} más")
            } else {
                String::new()
            };

            DiaSemanaFfi {
                fecha_iso: d.fecha.format(FORMATO_FECHA).to_string(),
                nombre: nombre_dia(d.fecha).to_string(),
                fecha_corta: fecha_corta(d.fecha),
                es_hoy: d.fecha == hoy,
                bloques: d
                    .bloques
                    .iter()
                    .map(|b| BloqueSemanaFfi {
                        id: b.clase_id.clone(),
                        materia: b.materia.clone(),
                        lugar: b.lugar.clone().unwrap_or_default(),
                        hora: reloj::rango_texto(b.inicio, b.fin),
                        inicio_min: minutos(b.inicio),
                        fin_min: minutos(b.fin),
                        tono: tono_materia(&b.materia),
                    })
                    .collect(),
                tareas,
                extra_tareas,
            }
        })
        .collect();

    SemanaVistaFfi {
        lunes_iso: lunes.format(FORMATO_FECHA).to_string(),
        etiqueta: etiqueta_semana(lunes, &dias_vista),
        es_actual,
        base_hora: base,
        fin_hora: fin,
        etiquetas_hora: (base..=fin).map(|h| etiqueta_hora(h as u32)).collect(),
        mostrar_ahora,
        ahora_min,
        hay_tareas,
        dias,
    }
}

fn construir_pendientes(
    datos: &Datos,
    filtro_ffi: &FiltroPendientesFfi,
    ahora: NaiveDateTime,
) -> PendientesVistaFfi {
    let desde: Option<NaiveDate> = filtro_ffi.desde_iso.as_deref().and_then(interpretar_fecha);
    let hasta: Option<NaiveDate> = filtro_ffi.hasta_iso.as_deref().and_then(interpretar_fecha);
    let hora_desde: Option<NaiveTime> = filtro_ffi
        .hora_desde
        .as_deref()
        .and_then(interpretar_hora);
    let hora_hasta: Option<NaiveTime> = filtro_ffi
        .hora_hasta
        .as_deref()
        .and_then(interpretar_hora);

    let filtro = Filtro {
        materia: filtro_ffi.materia.as_deref().and_then(opcional),
        tipo_fecha: TipoFecha::from(filtro_ffi.tipo_fecha),
        desde,
        hasta,
        hora_desde,
        hora_hasta,
        mostrar_hechas: filtro_ffi.mostrar_hechas,
    };

    let visibles = filtro.aplicar(&datos.tareas, ahora.date());
    let tareas: Vec<TareaFfi> = visibles.iter().map(|t| tarea_a_ffi(t, ahora)).collect();
    let hechas: Vec<bool> = tareas.iter().map(|t| t.hecha).collect();

    PendientesVistaFfi {
        resumen: textos::resumen(&hechas, filtro.activo()),
        filtro_activo: filtro.activo(),
        filtro_desde_texto: fecha_o_vacio(filtro.desde),
        filtro_hasta_texto: fecha_o_vacio(filtro.hasta),
        filtro_hora_desde_texto: hora_o_vacio(filtro.hora_desde),
        filtro_hora_hasta_texto: hora_o_vacio(filtro.hora_hasta),
        tareas,
    }
}

// ---------------------------------------------------------------------------
// Pruebas unitarias
// ---------------------------------------------------------------------------

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn ruta_temp(dir: &TempDir) -> String {
        dir.path().join("order.toml").to_string_lossy().into_owned()
    }

    #[test]
    fn carga_ejemplo_y_expone_rutas_y_advertencias() {
        let dir = TempDir::new().expect("tempdir");
        let ruta = ruta_temp(&dir);

        assert!(!ruta_por_defecto().is_empty());
        assert!(!carpeta_por_defecto().is_empty());

        let resumen = cargar(ruta.clone()).expect("cargar crea el ejemplo");
        assert!(resumen.num_clases > 0);
        assert!(resumen.num_tareas > 0);
        assert!(resumen.num_recordatorios > 0);
        assert!(resumen.advertencias.is_empty());
        assert!(resumen.conflictos.is_empty());
        assert!(resumen.pie.starts_with("Hoy:"));

        // Advertencias por clave desconocida.
        let mut contenido = fs::read_to_string(&ruta).expect("leer ejemplo");
        contenido.push_str("\nclave_rara = 1\n");
        fs::write(&ruta, contenido).expect("escribir advertencia");
        let adv = advertencias(ruta.clone()).expect("leer advertencias");
        assert_eq!(adv.len(), 1);
        assert!(adv[0].contains("clave_rara"));

        // Archivar no falla aunque no haya tareas de >30 días.
        let tras_archivar = archivar(ruta).expect("archivar");
        assert!(tras_archivar.num_tareas > 0);
    }

    #[test]
    fn crear_editar_marcar_y_borrar_tarea() {
        let dir = TempDir::new().expect("tempdir");
        let ruta = ruta_temp(&dir);
        let inicial = cargar(ruta.clone()).expect("cargar");

        let defecto = obtener_tarea_form(ruta.clone(), None).expect("form nueva tarea");
        assert!(defecto.id.is_empty());
        assert_eq!(defecto.hora_texto, "11:59 p. m.");

        // Validación con error de campo.
        let err = validar_tarea_campos(
            "   ".into(),
            "2026-12-01".into(),
            "11:59 p. m.".into(),
            "Control".into(),
            vec![],
        )
        .unwrap_err();
        assert!(matches!(err, ErrorFfi::Validacion { campo: 1, .. }));

        // Crear tarea.
        let tras_crear = guardar_tarea(
            ruta.clone(),
            None,
            "Proyecto FFI".into(),
            "2026-12-01".into(),
            "8:30 p. m.".into(),
            "control".into(),
            false,
        )
        .expect("crear tarea");
        assert_eq!(tras_crear.num_tareas, inicial.num_tareas + 1);

        let pendientes = vista_pendientes(
            ruta.clone(),
            FiltroPendientesFfi {
                materia: None,
                tipo_fecha: TipoFechaFfi::Todas,
                desde_iso: None,
                hasta_iso: None,
                hora_desde: None,
                hora_hasta: None,
                mostrar_hechas: true,
            },
        )
        .expect("pendientes");
        let creada = pendientes
            .tareas
            .iter()
            .find(|t| t.titulo == "Proyecto FFI")
            .expect("la tarea aparece en pendientes");
        assert_eq!(creada.materia, "Control");

        // Editar tarea.
        let form = obtener_tarea_form(ruta.clone(), Some(creada.id.clone())).expect("obtener form");
        assert_eq!(form.titulo, "Proyecto FFI");
        guardar_tarea(
            ruta.clone(),
            Some(creada.id.clone()),
            "Proyecto FFI v2".into(),
            "2026-12-02".into(),
            "9:00 p. m.".into(),
            "Robótica".into(),
            false,
        )
        .expect("editar tarea");

        // Marcar y borrar.
        let tras_marcar = marcar_tarea(ruta.clone(), creada.id.clone(), true).expect("marcar");
        assert_eq!(tras_marcar.num_tareas_pendientes, inicial.num_tareas_pendientes);

        let tras_borrar = borrar_tarea(ruta, creada.id.clone()).expect("borrar");
        assert_eq!(tras_borrar.num_tareas, inicial.num_tareas);
    }

    #[test]
    fn crud_clase_recordatorio_y_ajustes() {
        let dir = TempDir::new().expect("tempdir");
        let ruta = ruta_temp(&dir);
        let inicial = cargar(ruta.clone()).expect("cargar");

        // Validaciones de clase, recordatorio y ajustes.
        let err_clase = validar_clase_campos(
            String::new(),
            "Física".into(),
            vec![DiaFfi::Lun],
            "10:00 a. m.".into(),
            "8:00 a. m.".into(),
            String::new(),
            vec![],
        )
        .unwrap_err();
        assert!(matches!(err_clase, ErrorFfi::Validacion { campo: 3, .. }));

        let err_rec = validar_recordatorio_campos(
            String::new(),
            "Leer".into(),
            "6:00 p. m.".into(),
            vec![],
        )
        .unwrap_err();
        assert!(matches!(err_rec, ErrorFfi::Validacion { campo: 4, .. }));

        let err_aj = validar_ajustes_campos(
            false,
            "2026-12-10".into(),
            "2026-12-01".into(),
            "8:00 p. m.".into(),
            "3".into(),
        )
        .unwrap_err();
        assert!(matches!(err_aj, ErrorFfi::Validacion { campo: 2, .. }));

        // Clase.
        let _ = obtener_clase_form(ruta.clone(), None).expect("form clase");
        let tras_clase = guardar_clase(
            ruta.clone(),
            None,
            "Física".into(),
            vec![DiaFfi::Vie, DiaFfi::Lun],
            "2:00 p. m.".into(),
            "4:00 p. m.".into(),
            "Aula 10".into(),
        )
        .expect("crear clase");
        assert_eq!(tras_clase.num_clases, inicial.num_clases + 1);

        let dia_lun = vista_dia(ruta.clone(), Some("2026-11-23".into())).expect("vista dia");
        let fisica = dia_lun
            .clases
            .iter()
            .find(|c| c.materia == "Física")
            .expect("clase física el lunes");
        let form_clase =
            obtener_clase_form(ruta.clone(), Some(fisica.id.clone())).expect("form clase edit");
        assert_eq!(
            form_clase.dias,
            vec![DiaFfi::Lun, DiaFfi::Vie],
            "días normalizados en orden lun..dom"
        );
        borrar_clase(ruta.clone(), fisica.id.clone()).expect("borrar clase");

        // Recordatorio.
        let _ = obtener_recordatorio_form(ruta.clone(), None).expect("form rec");
        let tras_rec = guardar_recordatorio(
            ruta.clone(),
            None,
            "Entregar lab".into(),
            "7:30 a. m.".into(),
            vec![DiaFfi::Mar],
        )
        .expect("crear recordatorio");
        assert_eq!(tras_rec.num_recordatorios, inicial.num_recordatorios + 1);

        let aj_form = obtener_ajustes_form(ruta.clone()).expect("obtener ajustes");
        let rec_creado = aj_form
            .recordatorios
            .iter()
            .find(|r| r.texto == "Entregar lab")
            .expect("recordatorio en ajustes");
        let form_rec =
            obtener_recordatorio_form(ruta.clone(), Some(rec_creado.id.clone())).expect("edit rec");
        assert_eq!(form_rec.hora, "7:30 a. m.");
        borrar_recordatorio(ruta.clone(), rec_creado.id.clone()).expect("borrar recordatorio");

        // Ajustes.
        guardar_ajustes(
            ruta.clone(),
            true,
            "2026-08-03".into(),
            "2026-12-05".into(),
            "9:00 p. m.".into(),
            "5".into(),
        )
        .expect("guardar ajustes");
        let aj_actualizados = obtener_ajustes_form(ruta).expect("leer ajustes");
        assert!(aj_actualizados.sin_semestre);
        assert_eq!(aj_actualizados.aviso_dia_antes, "9:00 p. m.");
        assert_eq!(aj_actualizados.aviso_horas_antes, "5");
    }

    #[test]
    fn vistas_inicio_dia_semana_y_filtros_pendientes() {
        let dir = TempDir::new().expect("tempdir");
        let ruta = ruta_temp(&dir);
        cargar(ruta.clone()).expect("cargar ejemplo");

        let inicio = vista_inicio(ruta.clone()).expect("vista inicio");
        assert!(!inicio.fecha_larga.is_empty());
        assert!(!inicio.pie.is_empty());

        let semana = vista_semana(ruta.clone(), Some("2026-11-23".into())).expect("vista semana");
        assert_eq!(semana.lunes_iso, "2026-11-23");
        assert!(semana.dias.len() >= 5);
        assert!(!semana.etiquetas_hora.is_empty());

        let dia = vista_dia(ruta.clone(), Some("2026-11-23".into())).expect("vista dia");
        assert_eq!(dia.fecha_iso, "2026-11-23");
        assert!(!dia.clases.is_empty());

        // Filtro por materia y canonica.
        let materias = materias_con_tono(ruta.clone()).expect("materias");
        assert!(!materias.is_empty());
        let primera = &materias[0];
        assert_eq!(primera.tono, tono_de_materia(primera.nombre.clone()));
        assert_eq!(
            canonica_materia(ruta.clone(), primera.nombre.to_lowercase()).expect("canonica"),
            primera.nombre
        );

        let filtradas = vista_pendientes(
            ruta,
            FiltroPendientesFfi {
                materia: Some(primera.nombre.clone()),
                tipo_fecha: TipoFechaFfi::Rango,
                desde_iso: Some("2026-01-01".into()),
                hasta_iso: Some("2027-12-31".into()),
                hora_desde: Some("12:00 a. m.".into()),
                hora_hasta: Some("11:59 p. m.".into()),
                mostrar_hechas: false,
            },
        )
        .expect("pendientes filtrados");
        assert!(filtradas.filtro_activo);
        assert!(filtradas.resumen.contains("(filtrado)") || filtradas.resumen.contains("sin resultados"));
    }

    #[test]
    fn detecta_y_resuelve_conflicto() {
        let dir = TempDir::new().expect("tempdir");
        let ruta = ruta_temp(&dir);
        cargar(ruta.clone()).expect("cargar ejemplo");

        let copia = dir
            .path()
            .join("order.sync-conflict-20261122-014500-ABCDEFG.toml");
        fs::write(&copia, fs::read_to_string(&ruta).expect("leer")).expect("escribir conflicto");

        let conflictos = detectar_conflictos(ruta.clone());
        assert_eq!(conflictos.len(), 1);
        assert!(conflictos[0].mensaje.contains("Hay una copia en conflicto"));

        resolver_conflicto(ruta.clone(), conflictos[0].ruta.clone(), false)
            .expect("quedarme con la actual");
        assert!(detectar_conflictos(ruta.clone()).is_empty());

        // Probar usar_otra.
        let copia2 = dir
            .path()
            .join("order.sync-conflict-20261122-020000-ABCDEFG.toml");
        fs::write(
            &copia2,
            "[[tarea]]\nid = \"t-otra\"\ntitulo = \"Desde conflicto\"\nvence = \"2026-12-01T12:00\"\nhecha = false\n",
        )
        .expect("escribir segundo conflicto");
        let conflictos2 = detectar_conflictos(ruta.clone());
        assert_eq!(conflictos2.len(), 1);
        resolver_conflicto(ruta.clone(), conflictos2[0].ruta.clone(), true)
            .expect("usar la otra");
        let res = cargar(ruta).expect("cargar tras conflicto");
        assert_eq!(res.num_tareas, 1);
    }

    #[test]
    fn selectores_calendario_hora_y_fechas() {
        let hoy = fecha_hoy_iso();
        assert_eq!(hoy.len(), 10);
        let lunes = lunes_actual_iso();
        assert_eq!(lunes.len(), 10);
        assert_eq!(sumar_dias_iso("2026-11-22".into(), 1), "2026-11-23");

        let cal = calendario_mes(2026, 11, 22);
        assert_eq!(cal.celdas.len(), 42);
        assert_eq!(cal.etiqueta_mes, "noviembre de 2026");

        let sig = mes_vecino(2026, 12, 1);
        assert_eq!((sig.anio, sig.mes), (2027, 1));
        assert_eq!(dias_del_mes(2028, 2), 29);

        assert_eq!(horas_12_texto().len(), 12);
        assert_eq!(minutos_60_texto().len(), 60);
        assert_eq!(formatear_hora_12(12, 0, false), "12:00 a. m.");
        assert_eq!(formatear_hora_12(12, 30, true), "12:30 p. m.");

        let parsed = parsear_hora_12("2:15 p. m.".into());
        assert_eq!((parsed.hora12, parsed.minuto, parsed.tarde), (2, 15, true));
        assert_eq!(dias_semana_etiquetas().len(), 7);
    }

    fn servidor_http_con_captura(
        status: u16,
        cuerpo: String,
    ) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local");
        let addr = listener.local_addr().expect("local_addr");
        let url = format!("http://{addr}");

        let handle = std::thread::spawn(move || {
            let mut recibido = String::new();
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
                let mut buf = [0u8; 65536];
                while let Ok(n) = stream.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    recibido.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if let Some(fin_hdr) = recibido.find("\r\n\r\n") {
                        let cabeceras = &recibido[..fin_hdr];
                        let content_len = cabeceras
                            .lines()
                            .find_map(|l| {
                                let (k, v) = l.split_once(':')?;
                                if k.eq_ignore_ascii_case("content-length") {
                                    v.trim().parse::<usize>().ok()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0);
                        let cuerpo_len = recibido.len() - (fin_hdr + 4);
                        if cuerpo_len >= content_len {
                            break;
                        }
                    }
                }
                let razon = match status {
                    200 => "OK",
                    401 => "Unauthorized",
                    429 => "Too Many Requests",
                    _ => "Error",
                };
                let resp = format!(
                    "HTTP/1.1 {status} {razon}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
                    cuerpo.len()
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
            recibido
        });
        (url, handle)
    }

    fn servidor_http_simulado(status: u16, cuerpo: String) -> (String, std::thread::JoinHandle<()>) {
        let (url, handle) = servidor_http_con_captura(status, cuerpo);
        let plano = std::thread::spawn(move || {
            let _ = handle.join();
        });
        (url, plano)
    }

    #[test]
    fn avisos_proximos_anotar_y_resumen_del_dia() {
        let dir = TempDir::new().expect("tempdir");
        let ruta = ruta_temp(&dir);
        let registro = dir.path().join("enviados.txt").to_string_lossy().into_owned();

        // Archivo controlado con una tarea que vence el 2026-11-28T23:59 y un recordatorio el sábado.
        fs::write(
            &ruta,
            r#"[ajustes]
aviso_dia_antes = "20:00"
aviso_horas_antes = 3

[[tarea]]
id = "t-uno"
titulo = "Parcial"
materia = "Control"
vence = "2026-11-28T23:59"
hecha = false

[[recordatorio]]
id = "r-uno"
texto = "Repasar"
dias = ["sab"]
hora = "18:00"
"#,
        )
        .expect("escribir");

        let lista = proximos_avisos(
            ruta.clone(),
            "2026-11-27T08:00:00".into(),
            "2026-11-29T08:00:00".into(),
            registro.clone(),
        )
        .expect("proximos avisos");
        assert_eq!(lista.len(), 3);
        assert_eq!(lista[0].clave, "t:t-uno:dia:2026-11-28");
        assert_eq!(lista[0].momento_iso, "2026-11-27T20:00:00");
        assert!(!lista[0].es_recordatorio);
        assert_eq!(lista[1].clave, "r:r-uno:2026-11-28");
        assert!(lista[1].es_recordatorio);

        // Anotar uno lo excluye de la siguiente consulta.
        anotar_aviso(registro.clone(), lista[0].clave.clone());
        let lista2 = proximos_avisos(
            ruta.clone(),
            "2026-11-27T08:00:00".into(),
            "2026-11-29T08:00:00".into(),
            registro.clone(),
        )
        .expect("sin repetidos");
        assert_eq!(lista2.len(), 2);

        // Resumen del día se entrega una sola vez para esa fecha.
        let res1 = resumen_del_dia(
            ruta.clone(),
            "2026-11-28T09:00:00".into(),
            registro.clone(),
        )
        .expect("resumen 1");
        assert!(res1.is_some());
        assert_eq!(res1.unwrap().clave, "resumen:2026-11-28");

        let res2 = resumen_del_dia(ruta, "2026-11-28T14:00:00".into(), registro)
            .expect("resumen 2");
        assert!(res2.is_none(), "no se repite el resumen el mismo día");
    }

    static PRUEBA_AGENTE_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn agente_config_proveedores_enviar_y_aplicar() {
        let _guard = PRUEBA_AGENTE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        nueva_conversacion();
        let dir = TempDir::new().expect("tempdir");
        let ruta_datos = ruta_temp(&dir);
        let ruta_cfg = dir.path().join("agente.toml").to_string_lossy().into_owned();
        let inicial = cargar(ruta_datos.clone()).expect("cargar");

        let provs = proveedores_agente();
        assert_eq!(provs.len(), 4);
        assert_eq!(provs[0].nombre, "DeepSeek");
        assert!(provs[0].url_clave.is_some());
        assert!(provs[3].url_clave.is_none());

        let voz_provs = proveedores_voz();
        assert_eq!(voz_provs.len(), 2);
        assert_eq!(voz_provs[0].nombre, "OpenAI");

        // Config inexistente devuelve valores por defecto sin clave.
        let cfg0 = leer_agente_config(ruta_cfg.clone());
        assert!(cfg0.api_key.is_empty());

        // Sin clave, enviar_al_agente falla de forma legible sin red.
        let err_sin_clave = enviar_al_agente(
            ruta_datos.clone(),
            ruta_cfg.clone(),
            "hola".into(),
            vec![],
        )
        .unwrap_err();
        assert!(err_sin_clave.to_string().contains("clave"));

        // Validación con error.
        let err_val = validar_agente(
            3,
            "sin-esquema".into(),
            "m".into(),
            "sk-1".into(),
            0,
            "".into(),
            "".into(),
            "".into(),
        )
        .unwrap_err();
        assert!(matches!(err_val, ErrorFfi::Validacion { campo: 1, .. }));

        // Servidor local simulado para enviar_al_agente con acciones propuestas.
        let respuesta_json = r#"{"choices":[{"message":{"role":"assistant","content":"{\"respuesta\":\"Listo, propongo dos tareas.\",\"acciones\":[{\"tipo\":\"crear_tarea\",\"titulo\":\"Tarea agente 1\",\"materia\":\"Control\",\"vence\":\"2026-12-05T20:00\"},{\"tipo\":\"crear_tarea\",\"titulo\":\"Tarea agente 2\",\"materia\":null,\"vence\":\"2026-12-06T20:00\"}]}"}}]}"#.to_string();
        let (url, handle) = servidor_http_simulado(200, respuesta_json);

        guardar_agente_config(
            ruta_cfg.clone(),
            3,
            url,
            "modelo-local".into(),
            "sk-test".into(),
            false,
            0,
            "".into(),
            "".into(),
            "".into(),
        )
        .expect("guardar config");

        let resp = enviar_al_agente(
            ruta_datos.clone(),
            ruta_cfg.clone(),
            "agrega dos tareas".into(),
            vec![],
        )
        .expect("enviar al agente");
        let _ = handle.join();
        assert_eq!(resp.respuesta, "Listo, propongo dos tareas.");
        assert_eq!(resp.acciones.len(), 2);
        assert!(resp.nota.is_none());

        // Aplicar una acción individual.
        let res1 = aplicar_accion(ruta_datos.clone(), resp.acciones[0].id.clone())
            .expect("aplicar primera");
        assert_eq!(res1.num_tareas, inicial.num_tareas + 1);

        // Aplicar todas (la segunda restante).
        let res_todas =
            aplicar_todas(ruta_datos.clone(), vec![resp.acciones[1].id.clone()])
                .expect("aplicar todas");
        assert_eq!(res_todas.aplicadas, 1);
        assert!(res_todas.error.is_none());
        assert_eq!(res_todas.resumen.unwrap().num_tareas, inicial.num_tareas + 2);

        // Error 401 simulado.
        let (url_401, h401) = servidor_http_simulado(401, "{}".into());
        guardar_agente_config(
            ruta_cfg.clone(),
            3,
            url_401,
            "m".into(),
            "sk-mal".into(),
            false,
            0,
            "".into(),
            "".into(),
            "".into(),
        )
        .expect("guardar 401");
        let err_401 =
            enviar_al_agente(ruta_datos, ruta_cfg, "hola".into(), vec![]).unwrap_err();
        let _ = h401.join();
        assert!(err_401.to_string().contains("401"));
    }

    fn pdf_prueba(paginas: &[&str]) -> Vec<u8> {
        use std::fmt::Write as _;
        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n");
        let num_pags = paginas.len().max(1);
        let total_objs = 3 + num_pags * 2;
        let mut offsets = vec![0usize];
        let mut kids = String::new();
        for i in 0..num_pags {
            let _ = write!(kids, "{} 0 R ", 4 + i * 2);
        }
        offsets.push(out.len());
        out.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        offsets.push(out.len());
        out.extend_from_slice(
            format!("2 0 obj\n<< /Type /Pages /Kids [ {kids}] /Count {num_pags} >>\nendobj\n")
                .as_bytes(),
        );
        offsets.push(out.len());
        out.extend_from_slice(
            b"3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
        );
        for (i, texto) in paginas.iter().enumerate() {
            let page_id = 4 + i * 2;
            let content_id = page_id + 1;
            let stream = if texto.is_empty() {
                String::new()
            } else {
                format!("BT /F1 12 Tf 72 720 Td ({texto}) Tj ET\n")
            };
            offsets.push(out.len());
            out.extend_from_slice(
                format!(
                    "{page_id} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {content_id} 0 R >>\nendobj\n"
                )
                .as_bytes(),
            );
            offsets.push(out.len());
            out.extend_from_slice(
                format!(
                    "{content_id} 0 obj\n<< /Length {} >>\nstream\n{stream}endstream\nendobj\n",
                    stream.len()
                )
                .as_bytes(),
            );
        }
        let xref_pos = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", total_objs + 1).as_bytes());
        for off in offsets.iter().skip(1) {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n", total_objs + 1)
                .as_bytes(),
        );
        out
    }

    #[test]
    fn adjuntos_ffi_texto_pdf_vision_y_envio_simulado() {
        let _guard = PRUEBA_AGENTE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        nueva_conversacion();
        let dir = TempDir::new().expect("tempdir");
        let ruta_datos = ruta_temp(&dir);
        let ruta_cfg = dir.path().join("agente.toml").to_string_lossy().into_owned();
        cargar(ruta_datos.clone()).expect("cargar");

        // Límites expuestos.
        let lim = limites_adjuntos();
        assert_eq!(lim.max_adjuntos, 5);
        assert_eq!(lim.max_paginas_pdf, 20);
        assert_eq!(lim.lado_largo_imagen, 1568);
        assert_eq!(lim.calidad_jpeg, 85);

        // PDF con una página con texto y otra vacía: texto_de_pdf + paginas_sin_texto.
        let pdf = pdf_prueba(&["Parcial de Control", ""]);

        let pags = texto_de_pdf(pdf.clone()).expect("texto de pdf");
        assert_eq!(pags.len(), 2);
        assert!(pags[0].contains("Parcial de Control"));
        assert_eq!(paginas_sin_texto(pags), vec![1]);
        assert!(texto_de_pdf(b"basura".to_vec()).is_err());

        // agente_ve_imagenes según proveedor y casilla de Otro.
        let respuesta_json =
            r#"{"choices":[{"message":{"role":"assistant","content":"{\"respuesta\":\"Recibido\",\"acciones\":[]}"}}]}"#
                .to_string();
        let (url_txt, h_txt) = servidor_http_con_captura(200, respuesta_json.clone());
        guardar_agente_config(
            ruta_cfg.clone(),
            3,
            url_txt,
            "m".into(),
            "sk-test".into(),
            false,
            0,
            "".into(),
            "".into(),
            "".into(),
        )
        .expect("config sin vision");
        assert!(!agente_ve_imagenes(ruta_cfg.clone()));

        let resp_txt = enviar_al_agente(
            ruta_datos.clone(),
            ruta_cfg.clone(),
            "".into(),
            vec![AdjuntoFfi {
                nombre: "control.pdf".into(),
                tipo: TipoAdjuntoFfi::Pdf,
                bytes: pdf,
                paginas_jpeg: vec![],
                texto_ocr: Some("Entrega 5 diciembre".into()),
            }],
        )
        .expect("envío sin visión");
        assert_eq!(resp_txt.respuesta, "Recibido");
        let recibido_txt = h_txt.join().expect("join");
        assert!(recibido_txt.contains("Parcial de Control"), "{recibido_txt}");
        assert!(recibido_txt.contains("Entrega 5 diciembre"), "{recibido_txt}");

        // Con Otro + ve_imagenes = true manda image_url data URL.
        let (url_vis, h_vis) = servidor_http_con_captura(200, respuesta_json);
        guardar_agente_config(
            ruta_cfg.clone(),
            3,
            url_vis,
            "m-vision".into(),
            "sk-test".into(),
            true,
            0,
            "".into(),
            "".into(),
            "".into(),
        )
        .expect("config con vision");
        assert!(agente_ve_imagenes(ruta_cfg.clone()));

        let resp_vis = enviar_al_agente(
            ruta_datos,
            ruta_cfg,
            "mira la foto".into(),
            vec![AdjuntoFfi {
                nombre: "tablero.jpg".into(),
                tipo: TipoAdjuntoFfi::Imagen,
                bytes: vec![0xFF, 0xD8, 0xFF, 0xD9],
                paginas_jpeg: vec![],
                texto_ocr: None,
            }],
        )
        .expect("envío con visión");
        assert_eq!(resp_vis.respuesta, "Recibido");
        let recibido_vis = h_vis.join().expect("join");
        assert!(recibido_vis.contains("data:image/jpeg;base64,"), "{recibido_vis}");
    }

    #[test]
    fn voz_wav_y_transcribir_simulado() {
        let dir = TempDir::new().expect("tempdir");
        let ruta_cfg = dir.path().join("agente.toml").to_string_lossy().into_owned();

        let wav = wav_de_pcm16(vec![0i16; 1600], 16_000);
        assert_eq!(wav.len(), 44 + 3200);

        // Sin clave de voz no toca la red.
        let err_sin_clave = transcribir(ruta_cfg.clone(), wav.clone()).unwrap_err();
        assert!(err_sin_clave.to_string().contains("clave de voz") || err_sin_clave.to_string().contains("clave"));

        // Con servidor local simulado.
        let (url, handle) = servidor_http_simulado(
            200,
            r#"{"text":"taller de control para el viernes"}"#.into(),
        );
        guardar_agente_config(
            ruta_cfg.clone(),
            0,
            "".into(),
            "".into(),
            "sk-agent".into(),
            false,
            0,
            url,
            "whisper-1".into(),
            "sk-voz".into(),
        )
        .expect("guardar config voz");

        let texto = transcribir(ruta_cfg, wav).expect("transcribir");
        let _ = handle.join();
        assert_eq!(texto, "taller de control para el viernes");
    }

    #[test]
    fn conversacion_ffi_resumen_descartar_y_nueva() {
        let _guard = PRUEBA_AGENTE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        nueva_conversacion();
        assert!(!hay_conversacion());

        let dir = TempDir::new().expect("tempdir");
        let ruta_datos = ruta_temp(&dir);
        let ruta_cfg = dir.path().join("agente.toml").to_string_lossy().into_owned();
        cargar(ruta_datos.clone()).expect("cargar");

        // 1. Primera petición devuelve resumen + dos acciones.
        let resp1_json = r#"{"choices":[{"message":{"role":"assistant","content":"{\"respuesta\":\"Propongo dos tareas.\",\"acciones\":[{\"tipo\":\"crear_tarea\",\"titulo\":\"Informe de Control\",\"materia\":\"Control\",\"vence\":\"2026-12-04T18:00\"},{\"tipo\":\"crear_tarea\",\"titulo\":\"Lectura opcional\",\"materia\":null,\"vence\":\"2026-12-05T18:00\"}],\"resumen\":\"El usuario pidió tareas para Control y una lectura.\"}"}}]}"#.to_string();
        let (url1, h1) = servidor_http_con_captura(200, resp1_json);
        guardar_agente_config(
            ruta_cfg.clone(),
            3,
            url1,
            "m".into(),
            "sk-test".into(),
            false,
            0,
            "".into(),
            "".into(),
            "".into(),
        )
        .expect("config 1");

        let resp1 = enviar_al_agente(
            ruta_datos.clone(),
            ruta_cfg.clone(),
            "crea dos tareas".into(),
            vec![],
        )
        .expect("primera");
        let _ = h1.join().expect("join 1");
        assert!(hay_conversacion());
        assert_eq!(resp1.acciones.len(), 2);

        // Aplicar la primera y descartar la segunda.
        aplicar_accion(ruta_datos.clone(), resp1.acciones[0].id.clone()).expect("aplicar");
        descartar_accion(resp1.acciones[1].id.clone());

        // 2. Segunda petición debe incluir el resumen de la primera y ambos resultados.
        let resp2_json = r#"{"choices":[{"message":{"role":"assistant","content":"{\"respuesta\":\"Entendido.\",\"acciones\":[],\"resumen\":\"Se creó Informe de Control y se descartó la lectura.\"}"}}]}"#.to_string();
        let (url2, h2) = servidor_http_con_captura(200, resp2_json);
        guardar_agente_config(
            ruta_cfg.clone(),
            3,
            url2,
            "m".into(),
            "sk-test".into(),
            false,
            0,
            "".into(),
            "".into(),
            "".into(),
        )
        .expect("config 2");

        let _resp2 = enviar_al_agente(
            ruta_datos.clone(),
            ruta_cfg.clone(),
            "no, mejor solo la primera".into(),
            vec![],
        )
        .expect("segunda");
        let recibido2 = h2.join().expect("join 2");
        assert!(
            recibido2.contains("El usuario pidió tareas para Control y una lectura."),
            "{recibido2}"
        );
        assert!(
            recibido2.contains("aplicada: Crear la tarea «Informe de Control»"),
            "{recibido2}"
        );
        assert!(
            recibido2.contains("descartada: Crear la tarea «Lectura opcional»"),
            "{recibido2}"
        );

        // 3. Nueva conversación limpia todo.
        nueva_conversacion();
        assert!(!hay_conversacion());

        let resp3_json = r#"{"choices":[{"message":{"role":"assistant","content":"{\"respuesta\":\"Hola.\",\"acciones\":[],\"resumen\":\"Saludo inicial.\"}"}}]}"#.to_string();
        let (url3, h3) = servidor_http_con_captura(200, resp3_json);
        guardar_agente_config(
            ruta_cfg.clone(),
            3,
            url3,
            "m".into(),
            "sk-test".into(),
            false,
            0,
            "".into(),
            "".into(),
            "".into(),
        )
        .expect("config 3");

        let _resp3 = enviar_al_agente(ruta_datos, ruta_cfg, "hola".into(), vec![]).expect("tercera");
        let recibido3 = h3.join().expect("join 3");
        assert!(!recibido3.contains("Resumen de la conversación:"), "{recibido3}");
        assert!(!recibido3.contains("Últimos intercambios:"), "{recibido3}");
        assert!(!recibido3.contains("Qué pasó con tus propuestas:"), "{recibido3}");
        nueva_conversacion();
    }
}
