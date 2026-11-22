//! Extracción de texto de PDF respetando la disposición en filas y columnas.
//!
//! Implementa `pdf_extract::OutputDev` para recoger cada glifo con su posición
//! `(x, y)` y tamaño de fuente, agrupa los glifos por líneas (`y` dentro de la
//! mitad del tamaño de fuente), ordena cada línea por `x` y convierte los
//! huecos horizontales en espacios proporcionales al ancho medio de carácter.

use pdf_extract::{Document, MediaBox, OutputDev, OutputError, Transform};

/// Glifo o fragmento posicionado en la página (coordenadas con `y` hacia abajo).
#[derive(Debug, Clone)]
struct Fragmento {
    x: f64,
    y: f64,
    ancho: f64,
    tamano: f64,
    texto: String,
}

/// Colector `OutputDev` para una página de PDF.
struct ColectorPagina {
    alto_pagina: f64,
    fragmentos: Vec<Fragmento>,
}

impl ColectorPagina {
    fn nuevo() -> Self {
        Self {
            alto_pagina: 0.0,
            fragmentos: Vec::new(),
        }
    }
}

impl OutputDev for ColectorPagina {
    fn begin_page(
        &mut self,
        _page_num: u32,
        media_box: &MediaBox,
        _art_box: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), OutputError> {
        self.alto_pagina = media_box.ury - media_box.lly;
        self.fragmentos.clear();
        Ok(())
    }

    fn end_page(&mut self) -> Result<(), OutputError> {
        Ok(())
    }

    fn output_character(
        &mut self,
        trm: &Transform,
        width: f64,
        _spacing: f64,
        font_size: f64,
        ch: &str,
    ) -> Result<(), OutputError> {
        if ch.is_empty() || ch.chars().all(char::is_whitespace) {
            return Ok(());
        }

        let x = trm.m31;
        let y = self.alto_pagina - trm.m32;
        if !x.is_finite() || !y.is_finite() {
            return Ok(());
        }

        let escala = (trm.m11 * trm.m22 - trm.m12 * trm.m21).abs().sqrt();
        let tamano_raw = if font_size.is_finite() && font_size > 0.0 {
            if escala.is_finite() && escala > 0.0 {
                font_size * escala
            } else {
                font_size
            }
        } else {
            10.0
        };
        let tamano = tamano_raw.clamp(4.0, 200.0);

        let ancho_base = if width.is_finite() && width > 0.0 {
            if trm.m11.is_finite() && trm.m11 > 0.0 && font_size.is_finite() && font_size > 0.0 {
                width * font_size * trm.m11
            } else {
                width * tamano
            }
        } else {
            tamano * 0.5
        };

        self.fragmentos.push(Fragmento {
            x,
            y,
            ancho: ancho_base.max(0.0),
            tamano,
            texto: ch.to_string(),
        });
        Ok(())
    }

    fn begin_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }

    fn end_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }

    fn end_line(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
}

/// Extrae el texto maquetado por filas de todas las páginas de `doc`.
pub fn extraer_paginas(doc: &Document) -> Result<Vec<String>, OutputError> {
    let paginas = doc.get_pages();
    let mut salida = Vec::with_capacity(paginas.len());
    for &num_pag in paginas.keys() {
        let mut colector = ColectorPagina::nuevo();
        pdf_extract::output_doc_page(doc, &mut colector, num_pag)?;
        salida.push(reconstruir_pagina(&colector.fragmentos));
    }
    Ok(salida)
}

/// Reconstruye una página agrupando los fragmentos en filas y alineando columnas
/// con espacios proporcionales.
fn reconstruir_pagina(fragmentos: &[Fragmento]) -> String {
    if fragmentos.is_empty() {
        return String::new();
    }

    let mut ordenados: Vec<&Fragmento> = fragmentos.iter().collect();
    ordenados.sort_by(|a, b| {
        a.y.total_cmp(&b.y)
            .then_with(|| a.x.total_cmp(&b.x))
    });

    let x_min_pagina = ordenados
        .iter()
        .map(|f| f.x)
        .fold(f64::INFINITY, f64::min);

    let ancho_char_global = calcular_ancho_medio(&ordenados);

    // Agrupar en líneas por `y` con tolerancia ≈ mitad del tamaño de fuente.
    let mut lineas_crudo: Vec<Vec<&Fragmento>> = Vec::new();
    let mut y_linea = ordenados[0].y;
    let mut tam_linea = ordenados[0].tamano;
    let mut grupo_actual: Vec<&Fragmento> = Vec::new();

    for frag in ordenados {
        let tolerancia = (tam_linea.max(frag.tamano) * 0.5).max(1.5);
        if (frag.y - y_linea).abs() <= tolerancia {
            grupo_actual.push(frag);
        } else {
            if !grupo_actual.is_empty() {
                lineas_crudo.push(grupo_actual);
            }
            grupo_actual = vec![frag];
            y_linea = frag.y;
            tam_linea = frag.tamano;
        }
    }
    if !grupo_actual.is_empty() {
        lineas_crudo.push(grupo_actual);
    }

    let mut resultado_lineas: Vec<String> = Vec::with_capacity(lineas_crudo.len());
    let mut y_anterior: Option<f64> = None;
    let mut tam_anterior: f64 = 10.0;

    for mut linea in lineas_crudo {
        linea.sort_by(|a, b| a.x.total_cmp(&b.x));
        let y_actual = linea[0].y;
        let tam_actual = linea[0].tamano;

        // Si hay un salto vertical grande (> 1.8 * tamaño de fuente), insertar una línea vacía.
        if let Some(y_prev) = y_anterior {
            let salto = y_actual - y_prev;
            let paso_normal = tam_anterior.max(tam_actual);
            if salto > paso_normal * 1.8 {
                if !resultado_lineas.last().is_some_and(|l| l.is_empty()) {
                    resultado_lineas.push(String::new());
                }
            }
        }
        y_anterior = Some(y_actual);
        tam_anterior = tam_actual;

        let texto_linea = construir_linea(&linea, x_min_pagina, ancho_char_global);
        let limpio = texto_linea.trim_end().to_string();
        if limpio.is_empty() {
            if !resultado_lineas.last().is_some_and(|l| l.is_empty()) {
                resultado_lineas.push(String::new());
            }
        } else {
            resultado_lineas.push(limpio);
        }
    }

    while resultado_lineas.first().is_some_and(|l| l.is_empty()) {
        resultado_lineas.remove(0);
    }
    while resultado_lineas.last().is_some_and(|l| l.is_empty()) {
        resultado_lineas.pop();
    }

    resultado_lineas.join("\n")
}

fn calcular_ancho_medio(fragmentos: &[&Fragmento]) -> f64 {
    let mut suma_anchos = 0.0;
    let mut total_chars = 0usize;
    for f in fragmentos {
        let chars = f.texto.chars().count().max(1);
        if f.ancho > 0.2 {
            suma_anchos += f.ancho;
            total_chars += chars;
        }
    }
    if total_chars > 0 {
        (suma_anchos / total_chars as f64).max(1.5)
    } else {
        5.0
    }
}

fn construir_linea(fragmentos: &[&Fragmento], x_min_pagina: f64, ancho_char_global: f64) -> String {
    let mut linea = String::new();
    let ancho_char = {
        let local = calcular_ancho_medio(fragmentos);
        if local > 1.5 {
            (local + ancho_char_global) * 0.5
        } else {
            ancho_char_global
        }
    };

    let mut cursor_x = x_min_pagina;
    for (idx, frag) in fragmentos.iter().enumerate() {
        let hueco = frag.x - cursor_x;
        if idx == 0 {
            let sangria = ((hueco / ancho_char).round().max(0.0) as usize).min(160);
            for _ in 0..sangria {
                linea.push(' ');
            }
        } else if hueco >= ancho_char * 0.25 {
            let espacios = ((hueco / ancho_char).round().max(1.0) as usize).min(80);
            for _ in 0..espacios {
                linea.push(' ');
            }
        }
        linea.push_str(&frag.texto);
        cursor_x = (frag.x + frag.ancho).max(cursor_x);
    }

    linea
}

#[cfg(test)]
mod pruebas {
    use crate::agente::adjuntos::extraer_texto_pdf;

    /// Construye un PDF de una página con un stream de contenido arbitrario.
    fn pdf_con_stream(stream: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n");
        let mut offsets = vec![0usize];

        offsets.push(out.len());
        out.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

        offsets.push(out.len());
        out.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [ 4 0 R ] /Count 1 >>\nendobj\n");

        offsets.push(out.len());
        out.extend_from_slice(
            b"3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
        );

        offsets.push(out.len());
        out.extend_from_slice(
            b"4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents 5 0 R >>\nendobj\n",
        );

        offsets.push(out.len());
        out.extend_from_slice(
            format!(
                "5 0 obj\n<< /Length {} >>\nstream\n{stream}endstream\nendobj\n",
                stream.len()
            )
            .as_bytes(),
        );

        let xref_pos = out.len();
        out.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
        for off in offsets.iter().skip(1) {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n")
                .as_bytes(),
        );
        out
    }

    /// Tabla 3x3 escrita columna por columna en el stream (primero todos los
    /// días, luego todas las materias, luego todas las horas). Debe salir por filas.
    #[test]
    fn tabla_en_orden_de_columnas_sale_por_filas() {
        let stream = "\
BT /F1 10 Tf
1 0 0 1 50 700 Tm (LUNES) Tj
1 0 0 1 50 680 Tm (MARTES) Tj
1 0 0 1 50 660 Tm (MIERCOLES) Tj
1 0 0 1 160 700 Tm (Control) Tj
1 0 0 1 160 680 Tm (Robotica) Tj
1 0 0 1 160 660 Tm (Finanzas) Tj
1 0 0 1 320 700 Tm (11:00 - 13:00) Tj
1 0 0 1 320 680 Tm (09:00 - 11:00) Tj
1 0 0 1 320 660 Tm (19:00 - 21:00) Tj
ET\n";
        let pdf = pdf_con_stream(stream);
        let pags = extraer_texto_pdf(&pdf).expect("extrae tabla");
        assert_eq!(pags.len(), 1);
        let lineas: Vec<&str> = pags[0].lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lineas.len(), 3, "texto:\n{}", pags[0]);
        assert!(lineas[0].contains("LUNES") && lineas[0].contains("Control") && lineas[0].contains("11:00 - 13:00"), "{:?}", lineas);
        assert!(lineas[1].contains("MARTES") && lineas[1].contains("Robotica") && lineas[1].contains("09:00 - 11:00"), "{:?}", lineas);
        assert!(lineas[2].contains("MIERCOLES") && lineas[2].contains("Finanzas") && lineas[2].contains("19:00 - 21:00"), "{:?}", lineas);
    }

    #[test]
    fn colapsa_lineas_vacias_repetidas() {
        let stream = "\
BT /F1 12 Tf
1 0 0 1 50 750 Tm (Encabezado) Tj
1 0 0 1 50 600 Tm (Pie de pagina) Tj
ET\n";
        let pdf = pdf_con_stream(stream);
        let pags = extraer_texto_pdf(&pdf).expect("extrae");
        assert_eq!(pags.len(), 1);
        assert!(!pags[0].contains("\n\n\n"), "{:?}", pags[0]);
        assert!(pags[0].contains("Encabezado"));
        assert!(pags[0].contains("Pie de pagina"));
    }
}
