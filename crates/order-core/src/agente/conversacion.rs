//! Historial persistente de la conversación con el agente y compactación.
//!
//! Se guarda en `conversacion.json` junto a `agente.toml` (nunca en la carpeta
//! de datos ni en `order.toml`), con permisos `600`. Conserva:
//! - el resumen de compactación (vacío hasta la primera compactación),
//! - el texto completo de los documentos adjuntados (sin compactar salvo tope),
//! - todos los mensajes literales (`Usuario` / `ORDER`),
//! - los resultados pendientes de reportar (aplicada/descartada).

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Límite combinado de caracteres para los documentos guardados en la conversación.
pub const MAX_CHARS_DOCUMENTOS: usize = 200_000;
/// Tope máximo de caracteres para el resumen tras compactar.
pub const MAX_CHARS_RESUMEN_COMPACTACION: usize = 6_000;
/// Número de mensajes literales recientes que se conservan tras compactar.
pub const MENSAJES_RECIENTES_COMPACTACION: usize = 10;

/// Prompt enviado al modelo para compactar mensajes antiguos.
pub const PROMPT_COMPACTACION: &str = "\
Resume esta conversación entre una persona y ORDER (su agenda de clases, tareas y recordatorios) para poder seguirla sin el texto original. Escribe en español, con estas secciones y nada más:
1. **Qué quiere la persona**: objetivos y peticiones, en sus palabras cuando importe.
2. **Preferencias y datos personales que dio**: cómo le gusta que se hagan las cosas, nombres, materias, horarios, fechas.
3. **Lo que ya se hizo**: acciones aplicadas (qué tarea/clase/recordatorio, qué día) y las descartadas.
4. **Decisiones y correcciones**: lo que cambió de opinión (\"no, mejor el viernes\") y lo que dijo que estaba mal.
5. **Pendiente**: lo que pidió y aún no se hizo.
6. **Tema actual**: de qué se hablaba al final.
Incluye el resumen anterior si lo hay; no pierdas ningún dato concreto (fechas, horas, nombres). Máximo 4 000 caracteres.";

/// Quién emitió un mensaje de la conversación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rol {
    Usuario,
    Agente,
}

/// Texto completo extraído de un archivo adjunto en la conversación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Documento {
    pub nombre: String,
    #[serde(default)]
    pub texto: String,
}

/// Un mensaje literal dentro de la conversación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mensaje {
    pub rol: Rol,
    pub texto: String,
    #[serde(default)]
    pub adjuntos: Vec<String>,
}

/// Estado completo de la conversación con el agente.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Conversacion {
    resumen: String,
    documentos: Vec<Documento>,
    mensajes: Vec<Mensaje>,
    resultados: Vec<String>,
}

impl Conversacion {
    /// Conversación limpia, sin resumen, documentos, mensajes ni resultados previos.
    pub fn nueva() -> Self {
        Self::default()
    }

    /// Ruta de `conversacion.json` en la misma carpeta que `agente.toml`.
    pub fn ruta_junto_a(ruta_agente: &Path) -> PathBuf {
        ruta_agente.with_file_name("conversacion.json")
    }

    /// Carga `conversacion.json`. Si falta o está corrupto devuelve una
    /// conversación vacía sin fallar.
    pub fn cargar_de(ruta: &Path) -> Self {
        std::fs::read_to_string(ruta)
            .ok()
            .and_then(|texto| serde_json::from_str::<Conversacion>(&texto).ok())
            .unwrap_or_default()
    }

    /// Guarda de forma atómica (`temporal + rename`) con permisos `600` en Unix.
    pub fn guardar_en(&self, ruta: &Path) -> Result<(), String> {
        let dir = ruta
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("no se pudo serializar la conversación: {e}"))?;

        let tmp = ruta.with_extension("json.tmp");
        let escritura = (|| -> std::io::Result<()> {
            let mut opciones = std::fs::OpenOptions::new();
            opciones.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opciones.mode(0o600);
            }
            let mut archivo = opciones.open(&tmp)?;
            archivo.write_all(json.as_bytes())?;
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

    /// Borra el archivo de conversación si existe.
    pub fn borrar_de(ruta: &Path) {
        let _ = std::fs::remove_file(ruta);
    }

    /// `true` cuando no hay resumen, documentos, mensajes ni resultados pendientes.
    pub fn esta_vacia(&self) -> bool {
        self.resumen.is_empty()
            && self.documentos.is_empty()
            && self.mensajes.is_empty()
            && self.resultados.is_empty()
    }

    /// Resumen acumulado de compactación.
    pub fn resumen(&self) -> &str {
        &self.resumen
    }

    /// Documentos guardados en la conversación.
    pub fn documentos(&self) -> &[Documento] {
        &self.documentos
    }

    /// Mensajes literales de la conversación.
    pub fn mensajes(&self) -> &[Mensaje] {
        &self.mensajes
    }

    /// Agrega un documento a la conversación. Si la suma supera `200 000`
    /// caracteres, retira los documentos más viejos reemplazándolos por
    /// `"[documento retirado: nombre]"` y devuelve sus nombres.
    pub fn agregar_documento(&mut self, nombre: &str, texto: &str) -> Vec<String> {
        let nombre_limpio = {
            let t = nombre.trim();
            if t.is_empty() { "adjunto" } else { t }
        };
        self.documentos.push(Documento {
            nombre: nombre_limpio.to_string(),
            texto: texto.trim().to_string(),
        });

        let mut retirados = Vec::new();
        while self.chars_documentos() > MAX_CHARS_DOCUMENTOS {
            let limite = if self.documentos.len() > 1 {
                self.documentos.len() - 1
            } else {
                self.documentos.len()
            };
            let Some(idx) = self.documentos[..limite]
                .iter()
                .position(|d| !es_documento_retirado(d))
            else {
                if let Some(ultimo) = self.documentos.last_mut() {
                    let aviso = "\n[documento recortado por tamaño]";
                    let max_texto = MAX_CHARS_DOCUMENTOS.saturating_sub(aviso.chars().count());
                    let mut recortado: String = ultimo.texto.chars().take(max_texto).collect();
                    recortado.push_str(aviso);
                    ultimo.texto = recortado;
                }
                break;
            };
            let nombre_viejo = self.documentos[idx].nombre.clone();
            self.documentos[idx].texto = marcador_retirado(&nombre_viejo);
            retirados.push(nombre_viejo);
        }

        retirados
    }

    /// Registra un intercambio completo (usuario + agente) de forma literal.
    pub fn registrar_turno(&mut self, peticion: &str, nombres_adjuntos: &[String], respuesta: &str) {
        let nombres: Vec<String> = nombres_adjuntos
            .iter()
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .collect();

        self.mensajes.push(Mensaje {
            rol: Rol::Usuario,
            texto: peticion.trim().to_string(),
            adjuntos: nombres,
        });
        self.mensajes.push(Mensaje {
            rol: Rol::Agente,
            texto: respuesta.trim().to_string(),
            adjuntos: Vec::new(),
        });
    }

    /// Anota qué hizo el usuario con una propuesta (Aplicar o Descartar) para
    /// contárselo al modelo en la siguiente petición.
    pub fn anotar_resultado(&mut self, descripcion: &str, aplicada: bool) {
        let limpio = descripcion.trim();
        if limpio.is_empty() {
            return;
        }
        let prefijo = if aplicada { "aplicada" } else { "descartada" };
        self.resultados.push(format!("{prefijo}: {limpio}"));
    }

    /// Devuelve y vacía los resultados acumulados tras haberse enviado al modelo.
    pub fn tomar_resultados(&mut self) -> Vec<String> {
        std::mem::take(&mut self.resultados)
    }

    /// Secciones 2–5 para incluir dentro del mensaje del usuario, en este orden:
    /// Documentos, Resumen, Conversación y Qué pasó con tus propuestas.
    /// Devuelve una cadena vacía si no hay nada todavía.
    pub fn bloque_contexto(&self) -> String {
        let mut secciones = Vec::new();

        if !self.documentos.is_empty() {
            let mut bloque = String::from("## Documentos de la conversación\n");
            for (idx, doc) in self.documentos.iter().enumerate() {
                if idx > 0 {
                    bloque.push_str("\n\n");
                }
                if doc.texto.is_empty() {
                    bloque.push_str(&format!("--- archivo: {} ---", doc.nombre));
                } else {
                    bloque.push_str(&format!("--- archivo: {} ---\n{}", doc.nombre, doc.texto));
                }
            }
            secciones.push(bloque);
        }

        if !self.resumen.is_empty() {
            secciones.push(format!("## Resumen de lo hablado antes\n{}", self.resumen));
        }

        if !self.mensajes.is_empty() {
            let mut bloque = String::from("## Conversación\n");
            for (idx, msg) in self.mensajes.iter().enumerate() {
                if idx > 0 {
                    bloque.push('\n');
                }
                bloque.push_str(&formatear_mensaje(msg));
            }
            secciones.push(bloque);
        }

        if !self.resultados.is_empty() {
            let mut bloque = String::from("## Qué pasó con tus propuestas\n");
            bloque.push_str(&self.resultados.join("\n"));
            secciones.push(bloque);
        }

        secciones.join("\n\n")
    }

    /// Estimación de tokens de las secciones 2–5 (`caracteres / 4`).
    pub fn tokens_estimados(&self) -> usize {
        self.bloque_contexto().chars().count() / 4
    }

    /// `true` cuando las secciones 2–5 superan `limite_tokens` y hay más de
    /// 10 mensajes (es decir, hay mensajes antiguos que compactar).
    pub fn necesita_compactar(&self, limite_tokens: u32) -> bool {
        limite_tokens > 0
            && self.mensajes.len() > MENSAJES_RECIENTES_COMPACTACION
            && self.tokens_estimados() > limite_tokens as usize
    }

    /// Arma el texto (resumen anterior + mensajes anteriores a los últimos 10)
    /// para pedirle al modelo que lo resuma. Devuelve `None` si no hay
    /// mensajes antiguos que compactar.
    pub fn texto_para_compactar(&self) -> Option<String> {
        if self.mensajes.len() <= MENSAJES_RECIENTES_COMPACTACION {
            return None;
        }
        let corte = self.mensajes.len() - MENSAJES_RECIENTES_COMPACTACION;
        let mut partes = Vec::new();
        if !self.resumen.is_empty() {
            partes.push(format!("## Resumen anterior\n{}", self.resumen));
        }
        let mut bloque_msgs = String::from("## Mensajes a resumir\n");
        for (idx, msg) in self.mensajes[..corte].iter().enumerate() {
            if idx > 0 {
                bloque_msgs.push('\n');
            }
            bloque_msgs.push_str(&formatear_mensaje(msg));
        }
        partes.push(bloque_msgs);
        Some(partes.join("\n\n"))
    }

    /// Guarda el nuevo resumen (recortado a 6 000 caracteres) y conserva solo
    /// los últimos 10 mensajes literales. Los documentos permanecen intactos.
    pub fn aplicar_compactacion(&mut self, resumen: &str) {
        self.resumen = resumen
            .trim()
            .chars()
            .take(MAX_CHARS_RESUMEN_COMPACTACION)
            .collect();
        if self.mensajes.len() > MENSAJES_RECIENTES_COMPACTACION {
            let quitar = self.mensajes.len() - MENSAJES_RECIENTES_COMPACTACION;
            self.mensajes.drain(..quitar);
        }
    }

    fn chars_documentos(&self) -> usize {
        self.documentos
            .iter()
            .map(|d| d.texto.chars().count())
            .sum()
    }
}

fn marcador_retirado(nombre: &str) -> String {
    format!("[documento retirado: {nombre}]")
}

fn es_documento_retirado(doc: &Documento) -> bool {
    doc.texto == marcador_retirado(&doc.nombre)
}

fn formatear_mensaje(msg: &Mensaje) -> String {
    match msg.rol {
        Rol::Usuario => {
            let adjuntos = msg
                .adjuntos
                .iter()
                .map(|nombre| format!("[adjuntó: {nombre}]"))
                .collect::<Vec<_>>()
                .join(" ");
            match (msg.texto.is_empty(), adjuntos.is_empty()) {
                (false, false) => format!("Usuario: {} {adjuntos}", msg.texto),
                (false, true) => format!("Usuario: {}", msg.texto),
                (true, false) => format!("Usuario: {adjuntos}"),
                (true, true) => "Usuario:".to_string(),
            }
        }
        Rol::Agente => format!("ORDER: {}", msg.texto),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn nueva_esta_vacia_y_devuelve_bloque_vacio() {
        let conv = Conversacion::nueva();
        assert!(conv.esta_vacia());
        assert_eq!(conv.bloque_contexto(), "");
        assert_eq!(conv.tokens_estimados(), 0);
    }

    #[test]
    fn documentos_sobreviven_a_muchos_turnos_y_a_compactacion() {
        let mut conv = Conversacion::nueva();
        let retirados = conv.agregar_documento("horario.pdf", "LUNES  Control  11:00 - 13:00");
        assert!(retirados.is_empty());

        for i in 1..=8 {
            let adj = if i == 1 { vec!["horario.pdf".into()] } else { vec![] };
            conv.registrar_turno(&format!("pregunta {i}"), &adj, &format!("respuesta {i}"));
        }
        assert_eq!(conv.mensajes().len(), 16);

        conv.aplicar_compactacion("Resumen previo de Control.");
        assert_eq!(conv.mensajes().len(), 10);
        assert_eq!(conv.documentos().len(), 1);

        let bloque = conv.bloque_contexto();
        assert!(bloque.contains("--- archivo: horario.pdf ---\nLUNES  Control  11:00 - 13:00"), "{bloque}");
        assert!(bloque.contains("## Resumen de lo hablado antes\nResumen previo de Control."), "{bloque}");
        // Los últimos 10 mensajes corresponden a los turnos 4..=8.
        assert!(!bloque.contains("pregunta 3"), "{bloque}");
        assert!(bloque.contains("Usuario: pregunta 4"), "{bloque}");
        assert!(bloque.contains("ORDER: respuesta 8"), "{bloque}");
    }

    #[test]
    fn orden_de_secciones_y_resultados() {
        let mut conv = Conversacion::nueva();
        conv.agregar_documento("notas.pdf", "Contenido literal");
        conv.registrar_turno("agrega taller", &["notas.pdf".into()], "Propongo el taller.");
        conv.anotar_resultado("Crear la tarea «Informe»", true);
        conv.anotar_resultado("Borrar la tarea «Vieja»", false);
        conv.resumen = "Resumen anterior.".into();

        let bloque = conv.bloque_contexto();
        let p_docs = bloque.find("## Documentos de la conversación").expect("docs");
        let p_res = bloque.find("## Resumen de lo hablado antes").expect("resumen");
        let p_conv = bloque.find("## Conversación").expect("conv");
        let p_prop = bloque.find("## Qué pasó con tus propuestas").expect("prop");
        assert!(p_docs < p_res && p_res < p_conv && p_conv < p_prop, "{bloque}");
        assert!(bloque.contains("Usuario: agrega taller [adjuntó: notas.pdf]"), "{bloque}");
        assert!(bloque.contains("ORDER: Propongo el taller."), "{bloque}");
        assert!(bloque.contains("aplicada: Crear la tarea «Informe»"));
        assert!(bloque.contains("descartada: Borrar la tarea «Vieja»"));

        let tomados = conv.tomar_resultados();
        assert_eq!(tomados.len(), 2);
        assert!(!conv.bloque_contexto().contains("## Qué pasó con tus propuestas"));
    }

    #[test]
    fn persistencia_json_ida_y_vuelta_y_corrupto() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ruta_agente = dir.path().join("agente.toml");
        let ruta_conv = Conversacion::ruta_junto_a(&ruta_agente);
        assert_eq!(ruta_conv, dir.path().join("conversacion.json"));

        let mut conv = Conversacion::nueva();
        conv.agregar_documento("horario.pdf", "LUNES Control 11:00 - 13:00");
        conv.registrar_turno("hola", &["horario.pdf".into()], "Hola, listo.");
        conv.anotar_resultado("Crear tarea", true);
        conv.guardar_en(&ruta_conv).expect("guardar");

        let leida = Conversacion::cargar_de(&ruta_conv);
        assert_eq!(leida, conv);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(&ruta_conv).expect("meta").permissions().mode();
            assert_eq!(modo & 0o777, 0o600);
        }

        // JSON corrupto no falla ni hace pánico: devuelve vacía.
        std::fs::write(&ruta_conv, "{corrupto").expect("escribir corrupto");
        assert_eq!(Conversacion::cargar_de(&ruta_conv), Conversacion::nueva());

        Conversacion::borrar_de(&ruta_conv);
        assert!(!ruta_conv.exists());
    }

    #[test]
    fn tope_de_documentos_retira_el_mas_viejo() {
        let mut conv = Conversacion::nueva();
        let r1 = conv.agregar_documento("viejo.pdf", &"a".repeat(120_000));
        assert!(r1.is_empty());
        let r2 = conv.agregar_documento("nuevo.pdf", &"b".repeat(120_000));
        assert_eq!(r2, vec!["viejo.pdf"]);
        assert_eq!(conv.documentos()[0].texto, "[documento retirado: viejo.pdf]");
        assert_eq!(conv.documentos()[1].texto.chars().count(), 120_000);
    }

    #[test]
    fn necesita_compactar_y_texto_para_compactar() {
        let mut conv = Conversacion::nueva();
        for i in 1..=6 {
            conv.registrar_turno(&format!("peticion {i} {}", "x".repeat(40)), &[], "ok");
        }
        assert_eq!(conv.mensajes().len(), 12);
        assert!(conv.necesita_compactar(20));
        assert!(!conv.necesita_compactar(100_000));

        let para_compactar = conv.texto_para_compactar().expect("hay mensajes antiguos");
        assert!(para_compactar.contains("peticion 1"));
        assert!(!para_compactar.contains("peticion 2"));

        conv.aplicar_compactacion(&"z".repeat(7_000));
        assert_eq!(conv.resumen().chars().count(), MAX_CHARS_RESUMEN_COMPACTACION);
        assert_eq!(conv.mensajes().len(), 10);
        assert!(!conv.necesita_compactar(20));
    }
}
