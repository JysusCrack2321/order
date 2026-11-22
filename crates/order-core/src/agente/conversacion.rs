//! Memoria corta en RAM de la conversación con el agente.
//!
//! No se guarda en disco: vive mientras la ventana (o el proceso) esté viva y
//! se borra con «Nueva conversación». En cada petición se arma un bloque compacto
//! con el resumen que mantiene el propio modelo, los últimos 3 intercambios
//! (sin reenviar el contenido de los adjuntos) y lo que el usuario aplicó o
//! descartó desde el turno anterior.

use std::collections::VecDeque;

/// Máximo de intercambios recientes que se reenvían literales.
pub const MAX_TURNOS: usize = 3;
/// Tope de caracteres para cada texto (petición o respuesta) en un intercambio.
pub const MAX_CHARS_TURNO: usize = 500;
/// Tope de caracteres para el resumen acumulado.
pub const MAX_CHARS_RESUMEN: usize = 800;

/// Un intercambio usuario ↔ agente ya terminado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turno {
    pub peticion: String,
    pub nombres_adjuntos: Vec<String>,
    pub respuesta: String,
}

/// Estado en memoria de la conversación en curso.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conversacion {
    resumen: String,
    turnos: VecDeque<Turno>,
    resultados: Vec<String>,
}

impl Conversacion {
    /// Conversación limpia, sin resumen, turnos ni resultados previos.
    pub fn nueva() -> Self {
        Self::default()
    }

    /// `true` cuando no hay ningún historial ni resultados pendientes de reportar.
    pub fn esta_vacia(&self) -> bool {
        self.resumen.is_empty() && self.turnos.is_empty() && self.resultados.is_empty()
    }

    /// Resumen acumulado actual (para inspecciones o pruebas).
    pub fn resumen(&self) -> &str {
        &self.resumen
    }

    /// Registra un turno completo. Conserva como máximo los 3 más recientes y
    /// recorta cada texto a 500 caracteres. Si `resumen` es `None` o viene
    /// vacío, mantiene el anterior; si trae texto, lo recorta a 800 caracteres.
    pub fn registrar_turno(
        &mut self,
        peticion: &str,
        nombres_adjuntos: &[String],
        respuesta: &str,
        resumen: Option<String>,
    ) {
        let peticion_limpia = recortar(peticion.trim(), MAX_CHARS_TURNO);
        let respuesta_limpia = recortar(respuesta.trim(), MAX_CHARS_TURNO);
        let nombres: Vec<String> = nombres_adjuntos
            .iter()
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .collect();

        if self.turnos.len() >= MAX_TURNOS {
            self.turnos.pop_front();
        }
        self.turnos.push_back(Turno {
            peticion: peticion_limpia,
            nombres_adjuntos: nombres,
            respuesta: respuesta_limpia,
        });

        if let Some(nuevo) = resumen {
            let limpio = nuevo.trim();
            if !limpio.is_empty() {
                self.resumen = recortar(limpio, MAX_CHARS_RESUMEN);
            }
        }
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

    /// Secciones 2–4 para incluir dentro del mensaje del usuario, en este orden:
    /// resumen, últimos intercambios y qué pasó con las propuestas.
    /// Devuelve una cadena vacía si no hay nada todavía.
    pub fn bloque_contexto(&self) -> String {
        let mut secciones = Vec::new();

        if !self.resumen.is_empty() {
            secciones.push(format!("Resumen de la conversación:\n{}", self.resumen));
        }

        if !self.turnos.is_empty() {
            let mut bloque = String::from("Últimos intercambios:\n");
            for (idx, turno) in self.turnos.iter().enumerate() {
                if idx > 0 {
                    bloque.push('\n');
                }
                let adjuntos = turno
                    .nombres_adjuntos
                    .iter()
                    .map(|nombre| format!("[adjuntó: {nombre}]"))
                    .collect::<Vec<_>>()
                    .join(" ");
                match (turno.peticion.is_empty(), adjuntos.is_empty()) {
                    (false, false) => {
                        bloque.push_str(&format!("Usuario: {} {adjuntos}\n", turno.peticion));
                    }
                    (false, true) => {
                        bloque.push_str(&format!("Usuario: {}\n", turno.peticion));
                    }
                    (true, false) => {
                        bloque.push_str(&format!("Usuario: {adjuntos}\n"));
                    }
                    (true, true) => {
                        bloque.push_str("Usuario:\n");
                    }
                }
                bloque.push_str(&format!("Agente: {}", turno.respuesta));
            }
            secciones.push(bloque);
        }

        if !self.resultados.is_empty() {
            let mut bloque = String::from("Qué pasó con tus propuestas:\n");
            bloque.push_str(&self.resultados.join("\n"));
            secciones.push(bloque);
        }

        secciones.join("\n\n")
    }
}

fn recortar(texto: &str, max_chars: usize) -> String {
    texto.chars().take(max_chars).collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn nueva_esta_vacia_y_devuelve_bloque_vacio() {
        let conv = Conversacion::nueva();
        assert!(conv.esta_vacia());
        assert_eq!(conv.bloque_contexto(), "");
    }

    #[test]
    fn solo_conserva_tres_turnos_mas_recientes() {
        let mut conv = Conversacion::nueva();
        for i in 1..=5 {
            conv.registrar_turno(
                &format!("petición {i}"),
                &[],
                &format!("respuesta {i}"),
                Some(format!("resumen {i}")),
            );
        }

        let bloque = conv.bloque_contexto();
        assert!(!bloque.contains("petición 1"), "{bloque}");
        assert!(!bloque.contains("petición 2"), "{bloque}");
        assert!(bloque.contains("petición 3"), "{bloque}");
        assert!(bloque.contains("petición 4"), "{bloque}");
        assert!(bloque.contains("petición 5"), "{bloque}");
        assert!(bloque.contains("respuesta 3"), "{bloque}");
        assert!(bloque.contains("respuesta 5"), "{bloque}");
        assert!(bloque.contains("resumen 5"), "{bloque}");
    }

    #[test]
    fn recorta_textos_de_turno_a_500_y_resumen_a_800() {
        let mut conv = Conversacion::nueva();
        let largo_peticion = "á".repeat(650);
        let largo_respuesta = "é".repeat(700);
        let largo_resumen = "ñ".repeat(950);

        conv.registrar_turno(&largo_peticion, &[], &largo_respuesta, Some(largo_resumen));

        assert_eq!(conv.turnos[0].peticion.chars().count(), 500);
        assert_eq!(conv.turnos[0].respuesta.chars().count(), 500);
        assert_eq!(conv.resumen().chars().count(), 800);

        let bloque = conv.bloque_contexto();
        assert!(!bloque.contains(&"á".repeat(501)));
        assert!(!bloque.contains(&"é".repeat(501)));
        assert!(!bloque.contains(&"ñ".repeat(801)));
    }

    #[test]
    fn adjuntos_aparecen_solo_por_nombre() {
        let mut conv = Conversacion::nueva();
        conv.registrar_turno(
            "agrega las clases",
            &["horario.pdf".into(), "tablero.jpg".into()],
            "Propuse 3 clases.",
            Some("Usuario compartió su horario.".into()),
        );

        let bloque = conv.bloque_contexto();
        assert!(bloque.contains("[adjuntó: horario.pdf]"), "{bloque}");
        assert!(bloque.contains("[adjuntó: tablero.jpg]"), "{bloque}");
    }

    #[test]
    fn resumen_ausente_o_vacio_conserva_el_anterior() {
        let mut conv = Conversacion::nueva();
        conv.registrar_turno("hola", &[], "hola", Some("Hablan de Control.".into()));
        conv.registrar_turno("y el viernes?", &[], "Sí.", None);
        assert_eq!(conv.resumen(), "Hablan de Control.");

        conv.registrar_turno("gracias", &[], "Con gusto.", Some("   ".into()));
        assert_eq!(conv.resumen(), "Hablan de Control.");
    }

    #[test]
    fn resultados_se_incluyen_en_orden_y_se_vacian_al_tomar() {
        let mut conv = Conversacion::nueva();
        conv.registrar_turno(
            "agrega taller",
            &[],
            "Propongo el taller.",
            Some("Quiere agregar taller de Control.".into()),
        );
        conv.anotar_resultado("Crear la tarea «Informe de Control» (miércoles)", true);
        conv.anotar_resultado("Borrar la tarea «Vieja»", false);

        let bloque = conv.bloque_contexto();
        let pos_resumen = bloque.find("Resumen de la conversación:").expect("resumen");
        let pos_turnos = bloque.find("Últimos intercambios:").expect("turnos");
        let pos_resultados = bloque.find("Qué pasó con tus propuestas:").expect("resultados");
        assert!(pos_resumen < pos_turnos && pos_turnos < pos_resultados);
        assert!(bloque.contains("aplicada: Crear la tarea «Informe de Control» (miércoles)"));
        assert!(bloque.contains("descartada: Borrar la tarea «Vieja»"));

        let tomados = conv.tomar_resultados();
        assert_eq!(tomados.len(), 2);
        let bloque_despues = conv.bloque_contexto();
        assert!(!bloque_despues.contains("Qué pasó con tus propuestas:"));
    }
}
