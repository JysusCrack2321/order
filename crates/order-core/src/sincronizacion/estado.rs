//! Estado CRDT por campo, lápidas y persistencia privada (`sync-estado.json`).

use super::hlc::{Marca, RelojHlc};
use crate::modelo::{Ajustes, Datos, Dia};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

/// Ventana tras la cual una lápida borrada se purga para no crecer sin límite.
pub const SESENTA_DIAS_MS: u64 = 60 * 24 * 60 * 60 * 1000;

/// Registro LWW individual para un campo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Campo<T> {
    pub valor: T,
    pub marca: Marca,
}

impl<T> Campo<T> {
    pub fn nuevo(valor: T, marca: Marca) -> Self {
        Campo { valor, marca }
    }
}

/// Campos de `[ajustes]` con marcas independientes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AjustesEstado {
    pub aviso_dia_antes: Campo<NaiveTime>,
    pub aviso_horas_antes: Campo<u32>,
}

impl Default for AjustesEstado {
    fn default() -> Self {
        let defecto = Ajustes::default();
        AjustesEstado {
            aviso_dia_antes: Campo::nuevo(defecto.aviso_dia_antes, Marca::cero()),
            aviso_horas_antes: Campo::nuevo(defecto.aviso_horas_antes, Marca::cero()),
        }
    }
}

impl AjustesEstado {
    pub fn max_marca(&self) -> Option<&Marca> {
        [&self.aviso_dia_antes.marca, &self.aviso_horas_antes.marca]
            .into_iter()
            .max()
    }
}

/// Campos de `[semestre]` con lápida opcional.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemestreEstado {
    pub inicio: Campo<NaiveDate>,
    pub fin: Campo<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub borrado: Option<Marca>,
}

impl SemestreEstado {
    pub fn max_campo(&self) -> &Marca {
        if self.inicio.marca >= self.fin.marca {
            &self.inicio.marca
        } else {
            &self.fin.marca
        }
    }

    pub fn max_marca(&self) -> &Marca {
        let mc = self.max_campo();
        match &self.borrado {
            Some(b) if b > mc => b,
            _ => mc,
        }
    }

    pub fn esta_vivo(&self) -> bool {
        match &self.borrado {
            None => true,
            Some(b) => self.max_campo() > b,
        }
    }
}

/// Campos de `[[clase]]` identificados por `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaseEstado {
    pub materia: Campo<String>,
    pub dias: Campo<Vec<Dia>>,
    pub inicio: Campo<NaiveTime>,
    pub fin: Campo<NaiveTime>,
    pub lugar: Campo<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub borrado: Option<Marca>,
}

impl ClaseEstado {
    pub fn max_campo(&self) -> &Marca {
        [
            &self.materia.marca,
            &self.dias.marca,
            &self.inicio.marca,
            &self.fin.marca,
            &self.lugar.marca,
        ]
        .into_iter()
        .max()
        .expect("lista fija no vacía")
    }

    pub fn max_marca(&self) -> &Marca {
        let mc = self.max_campo();
        match &self.borrado {
            Some(b) if b > mc => b,
            _ => mc,
        }
    }

    pub fn esta_vivo(&self) -> bool {
        match &self.borrado {
            None => true,
            Some(b) => self.max_campo() > b,
        }
    }
}

/// Campos de `[[tarea]]` identificados por `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TareaEstado {
    pub titulo: Campo<String>,
    pub materia: Campo<Option<String>>,
    pub vence: Campo<NaiveDateTime>,
    pub hecha: Campo<bool>,
    pub hecha_en: Campo<Option<NaiveDate>>,
    pub origen: Campo<Option<String>>,
    pub enlace: Campo<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub borrado: Option<Marca>,
}

impl TareaEstado {
    pub fn max_campo(&self) -> &Marca {
        [
            &self.titulo.marca,
            &self.materia.marca,
            &self.vence.marca,
            &self.hecha.marca,
            &self.hecha_en.marca,
            &self.origen.marca,
            &self.enlace.marca,
        ]
        .into_iter()
        .max()
        .expect("lista fija no vacía")
    }

    pub fn max_marca(&self) -> &Marca {
        let mc = self.max_campo();
        match &self.borrado {
            Some(b) if b > mc => b,
            _ => mc,
        }
    }

    pub fn esta_vivo(&self) -> bool {
        match &self.borrado {
            None => true,
            Some(b) => self.max_campo() > b,
        }
    }
}

/// Campos de `[[recordatorio]]` identificados por `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordatorioEstado {
    pub texto: Campo<String>,
    pub dias: Campo<Vec<Dia>>,
    pub hora: Campo<NaiveTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub borrado: Option<Marca>,
}

impl RecordatorioEstado {
    pub fn max_campo(&self) -> &Marca {
        [&self.texto.marca, &self.dias.marca, &self.hora.marca]
            .into_iter()
            .max()
            .expect("lista fija no vacía")
    }

    pub fn max_marca(&self) -> &Marca {
        let mc = self.max_campo();
        match &self.borrado {
            Some(b) if b > mc => b,
            _ => mc,
        }
    }

    pub fn esta_vivo(&self) -> bool {
        match &self.borrado {
            None => true,
            Some(b) => self.max_campo() > b,
        }
    }
}

/// Mensaje completo intercambiado entre dispositivos durante una sincronización.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MensajeSync {
    pub ajustes: AjustesEstado,
    pub semestre: Option<SemestreEstado>,
    pub clases: BTreeMap<String, ClaseEstado>,
    pub tareas: BTreeMap<String, TareaEstado>,
    pub recordatorios: BTreeMap<String, RecordatorioEstado>,
}

impl MensajeSync {
    /// Marca más alta presente en el mensaje, para adelantar el reloj local.
    pub fn max_marca(&self) -> Option<&Marca> {
        let mut max = self.ajustes.max_marca();
        if let Some(sem) = &self.semestre {
            let m = sem.max_marca();
            if max.is_none_or(|cur| m > cur) {
                max = Some(m);
            }
        }
        for c in self.clases.values() {
            let m = c.max_marca();
            if max.is_none_or(|cur| m > cur) {
                max = Some(m);
            }
        }
        for t in self.tareas.values() {
            let m = t.max_marca();
            if max.is_none_or(|cur| m > cur) {
                max = Some(m);
            }
        }
        for r in self.recordatorios.values() {
            let m = r.max_marca();
            if max.is_none_or(|cur| m > cur) {
                max = Some(m);
            }
        }
        max
    }

    /// Serializa el mensaje a JSON compacto para enviarlo por red.
    pub fn a_json(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|e| format!("no se pudo serializar el mensaje sync: {e}"))
    }

    /// Deserializa un mensaje recibido por red.
    pub fn desde_json(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes)
            .map_err(|e| format!("mensaje sync inválido: {e}"))
    }
}

/// Estado local persistente de sincronización (`sync-estado.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EstadoSync {
    pub version: u32,
    pub reloj: RelojHlc,
    pub crdt: MensajeSync,
    #[serde(with = "serde_foto", default)]
    pub foto: Option<Datos>,
}

mod serde_foto {
    use crate::modelo::{self, Datos};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<Datos>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(datos) => {
                let texto = toml::to_string(datos).map_err(serde::ser::Error::custom)?;
                s.serialize_some(&texto)
            }
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Datos>, D::Error> {
        let opt = Option::<String>::deserialize(d)?;
        match opt {
            Some(texto) => modelo::parsear(&texto)
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

impl EstadoSync {
    pub const VERSION_ACTUAL: u32 = 1;

    pub fn nuevo(dispositivo: impl Into<String>) -> Self {
        EstadoSync {
            version: Self::VERSION_ACTUAL,
            reloj: RelojHlc::nuevo(dispositivo),
            crdt: MensajeSync::default(),
            foto: None,
        }
    }

    /// Construye el mensaje de sincronización que se enviará a un par.
    pub fn mensaje(&self) -> MensajeSync {
        self.crdt.clone()
    }

    /// Elimina lápidas cuya fecha de borrado supera `SESENTA_DIAS_MS`.
    /// Devuelve cuántas lápidas se purgaron.
    pub fn purgar_lapidas(&mut self, ahora_ms: u64) -> usize {
        let expirada = |borrado: &Option<Marca>, vivo: bool| -> bool {
            if vivo {
                return false;
            }
            borrado
                .as_ref()
                .is_some_and(|b| ahora_ms.saturating_sub(b.tiempo_ms) > SESENTA_DIAS_MS)
        };

        let mut purgadas = 0usize;
        if let Some(sem) = &self.crdt.semestre {
            if expirada(&sem.borrado, sem.esta_vivo()) {
                self.crdt.semestre = None;
                purgadas += 1;
            }
        }
        let antes_c = self.crdt.clases.len();
        self.crdt
            .clases
            .retain(|_, c| !expirada(&c.borrado, c.esta_vivo()));
        purgadas += antes_c - self.crdt.clases.len();

        let antes_t = self.crdt.tareas.len();
        self.crdt
            .tareas
            .retain(|_, t| !expirada(&t.borrado, t.esta_vivo()));
        purgadas += antes_t - self.crdt.tareas.len();

        let antes_r = self.crdt.recordatorios.len();
        self.crdt
            .recordatorios
            .retain(|_, r| !expirada(&r.borrado, r.esta_vivo()));
        purgadas += antes_r - self.crdt.recordatorios.len();

        purgadas
    }

    pub fn a_json_texto(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| format!("no se pudo serializar sync-estado.json: {e}"))
    }

    pub fn desde_json_texto(texto: &str) -> Result<Self, String> {
        serde_json::from_str(texto)
            .map_err(|e| format!("sync-estado.json inválido: {e}"))
    }
}

/// Lee `sync-estado.json`. Si no existe o está dañado, devuelve uno vacío para `dispositivo`.
pub fn cargar_estado(ruta: &Path, dispositivo: &str) -> EstadoSync {
    let Ok(texto) = std::fs::read_to_string(ruta) else {
        return EstadoSync::nuevo(dispositivo);
    };
    match EstadoSync::desde_json_texto(&texto) {
        Ok(mut estado) => {
            if estado.reloj.dispositivo != dispositivo {
                estado.reloj.dispositivo = dispositivo.to_string();
            }
            estado
        }
        Err(_) => EstadoSync::nuevo(dispositivo),
    }
}

/// Escribe `sync-estado.json` de forma atómica con permisos `0600`.
pub fn guardar_estado(ruta: &Path, estado: &EstadoSync) -> Result<(), String> {
    let dir = ruta
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;

    let texto = estado.a_json_texto()?;
    let nombre = ruta
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sync-estado.json");
    let tmp = dir.join(format!(".{nombre}.{}.tmp", std::process::id()));

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
