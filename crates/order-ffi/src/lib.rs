//! Puente UniFFI entre `order-core` y Kotlin/Android.
//!
//! Toda la lógica, los textos en español, las validaciones y el cálculo de
//! colores por materia viven en `order-core`. Este crate solo adapta los tipos
//! a registros planos para UniFFI y delega las operaciones en disco vía
//! `Almacen`.

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike};
use order_core::almacen::Almacen;
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
use std::fmt;
use std::path::PathBuf;

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
// Ayudantes internos
// ---------------------------------------------------------------------------

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
}
