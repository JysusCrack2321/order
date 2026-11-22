//! Adjuntos (PDF e imágenes) para enviar al agente.
//!
//! La lógica de límites, extracción de texto de PDF y armado de bloques de
//! texto para modelos sin visión vive aquí. El renderizado de páginas a JPEG
//! y el OCR de imágenes corren en cada plataforma (Android o escritorio).

use crate::agente::config::Proveedor;
use pdf_extract::Document;
use std::fmt;
use std::panic::{self, AssertUnwindSafe};

/// Máximo de archivos adjuntos por petición.
pub const MAX_ADJUNTOS: usize = 5;
/// Máximo de páginas procesadas por cada PDF.
pub const MAX_PAGINAS_PDF: usize = 20;
/// Tamaño máximo combinado de los adjuntos en una petición (20 MB).
pub const MAX_BYTES_TOTAL: usize = 20 * 1024 * 1024;
/// Lado largo al que se reducen las imágenes antes de enviarlas.
pub const LADO_LARGO_IMAGEN: u32 = 1568;
/// Calidad JPEG (0–100) al comprimir páginas o fotos.
pub const CALIDAD_JPEG: u8 = 85;
/// Mínimo de caracteres no blancos para considerar que una página de PDF trae texto.
const MIN_CARACTERES_TEXTO_PAGINA: usize = 5;

/// Archivo adjunto ya preparado por la plataforma para enviar al agente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adjunto {
    pub nombre: String,
    pub contenido: ContenidoAdjunto,
}

/// Contenido del adjunto según su tipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContenidoAdjunto {
    Pdf {
        /// Bytes crudos del PDF (usados por Claude `document` y para extraer texto).
        bytes: Vec<u8>,
        /// Páginas renderizadas a JPEG (hasta 20 páginas, usadas por visión tipo Gemini).
        paginas_jpeg: Vec<Vec<u8>>,
        /// Texto obtenido por OCR en páginas sin texto embebido (separado por `\x0c` si hay varias).
        texto_ocr: Option<String>,
    },
    Imagen {
        /// Bytes JPEG ya rotados según EXIF y reducidos a `LADO_LARGO_IMAGEN`.
        jpeg: Vec<u8>,
        /// Texto obtenido por OCR para modelos sin visión.
        texto_ocr: Option<String>,
    },
}

/// Error legible al validar o procesar adjuntos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorAdjunto {
    DemasiadosAdjuntos {
        maximo: usize,
        recibidos: usize,
    },
    DemasiadasPaginas {
        nombre: String,
        maximo: usize,
        paginas: usize,
    },
    DemasiadoGrande {
        maximo_bytes: usize,
        actual_bytes: usize,
    },
    PdfConContrasena(String),
    PdfDanado(String),
    SinTextoLegible,
    Vacio(String),
}

impl fmt::Display for ErrorAdjunto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorAdjunto::DemasiadosAdjuntos { maximo, recibidos } => write!(
                f,
                "Puedes adjuntar como máximo {maximo} archivos por mensaje (enviaste {recibidos})."
            ),
            ErrorAdjunto::DemasiadasPaginas {
                nombre,
                maximo,
                paginas,
            } => write!(
                f,
                "El PDF «{nombre}» tiene {paginas} páginas y el máximo permitido es {maximo}."
            ),
            ErrorAdjunto::DemasiadoGrande {
                maximo_bytes,
                actual_bytes,
            } => {
                let max_mb = *maximo_bytes as f64 / (1024.0 * 1024.0);
                let act_mb = *actual_bytes as f64 / (1024.0 * 1024.0);
                write!(
                    f,
                    "Los adjuntos superan el límite de {max_mb:.0} MB ({act_mb:.1} MB). Quita algún archivo o reduce su tamaño."
                )
            }
            ErrorAdjunto::PdfConContrasena(nombre) if nombre.trim().is_empty() => write!(
                f,
                "El PDF está protegido con contraseña; quítala para poder leerlo."
            ),
            ErrorAdjunto::PdfConContrasena(nombre) => write!(
                f,
                "El PDF «{nombre}» está protegido con contraseña; quítala para poder leerlo."
            ),
            ErrorAdjunto::PdfDanado(detalle) => {
                write!(f, "No se pudo leer el PDF ({detalle}). Puede estar dañado.")
            }
            ErrorAdjunto::SinTextoLegible => write!(
                f,
                "No pude leer texto en este archivo; prueba con Gemini o Claude."
            ),
            ErrorAdjunto::Vacio(nombre) => {
                write!(f, "El archivo «{nombre}» está vacío.")
            }
        }
    }
}

impl std::error::Error for ErrorAdjunto {}

/// Extrae el texto de un PDF en memoria, devolviendo una entrada por página.
///
/// Si el PDF está cifrado con contraseña o dañado devuelve `ErrorAdjunto`
/// legible sin hacer pánico.
pub fn extraer_texto_pdf(bytes: &[u8]) -> Result<Vec<String>, ErrorAdjunto> {
    if bytes.is_empty() {
        return Err(ErrorAdjunto::PdfDanado("el archivo está vacío".into()));
    }

    let resultado = panic::catch_unwind(AssertUnwindSafe(|| -> Result<Vec<String>, ErrorAdjunto> {
        let mut doc = Document::load_mem(bytes).map_err(|err| mapear_error_lopdf("", &err))?;
        asegurar_descifrado(&mut doc, "")?;
        if doc.get_pages().is_empty() {
            return Err(ErrorAdjunto::PdfDanado("el PDF no tiene páginas".into()));
        }
        let paginas = pdf_extract::extract_text_from_mem_by_pages(bytes).map_err(|err| match err {
            pdf_extract::OutputError::PdfError(e) => mapear_error_lopdf("", &e),
            otro => ErrorAdjunto::PdfDanado(otro.to_string()),
        })?;
        Ok(paginas
            .into_iter()
            .map(|p| p.trim().to_string())
            .collect())
    }));

    match resultado {
        Ok(res) => res,
        Err(_) => Err(ErrorAdjunto::PdfDanado(
            "estructura interna inválida".to_string(),
        )),
    }
}

/// Cuenta las páginas de un PDF y verifica que no esté cifrado ni dañado.
pub fn contar_paginas_pdf(bytes: &[u8]) -> Result<usize, ErrorAdjunto> {
    if bytes.is_empty() {
        return Err(ErrorAdjunto::PdfDanado("el archivo está vacío".into()));
    }

    let resultado = panic::catch_unwind(AssertUnwindSafe(|| -> Result<usize, ErrorAdjunto> {
        let mut doc = Document::load_mem(bytes).map_err(|err| mapear_error_lopdf("", &err))?;
        asegurar_descifrado(&mut doc, "")?;
        let n = doc.get_pages().len();
        if n == 0 {
            return Err(ErrorAdjunto::PdfDanado("el PDF no tiene páginas".into()));
        }
        Ok(n)
    }));

    match resultado {
        Ok(res) => res,
        Err(_) => Err(ErrorAdjunto::PdfDanado(
            "estructura interna inválida".to_string(),
        )),
    }
}

/// Índices base 0 de las páginas que salieron vacías o casi vacías (necesitan OCR).
pub fn paginas_sin_texto(paginas: &[String]) -> Vec<usize> {
    paginas
        .iter()
        .enumerate()
        .filter_map(|(indice, texto)| {
            let caracteres = texto.chars().filter(|c| !c.is_whitespace()).count();
            (caracteres < MIN_CARACTERES_TEXTO_PAGINA).then_some(indice)
        })
        .collect()
}

/// Valida cantidad, páginas y tamaño total de los adjuntos según el proveedor.
pub fn validar(
    adjuntos: &[Adjunto],
    proveedor: Proveedor,
    ve_imagenes: bool,
) -> Result<(), ErrorAdjunto> {
    if adjuntos.len() > MAX_ADJUNTOS {
        return Err(ErrorAdjunto::DemasiadosAdjuntos {
            maximo: MAX_ADJUNTOS,
            recibidos: adjuntos.len(),
        });
    }

    let mut total_bytes: usize = 0;

    for adjunto in adjuntos {
        let nombre = adjunto.nombre.as_str();
        match &adjunto.contenido {
            ContenidoAdjunto::Pdf {
                bytes,
                paginas_jpeg,
                texto_ocr,
            } => {
                let tiene_ocr = texto_ocr.as_deref().is_some_and(|t| !t.trim().is_empty());
                if bytes.is_empty() && paginas_jpeg.is_empty() && !tiene_ocr {
                    return Err(ErrorAdjunto::Vacio(adjunto.nombre.clone()));
                }
                if paginas_jpeg.len() > MAX_PAGINAS_PDF {
                    return Err(ErrorAdjunto::DemasiadasPaginas {
                        nombre: adjunto.nombre.clone(),
                        maximo: MAX_PAGINAS_PDF,
                        paginas: paginas_jpeg.len(),
                    });
                }
                if !bytes.is_empty() && paginas_jpeg.is_empty() {
                    let paginas = contar_paginas_pdf(bytes).map_err(|e| con_nombre(e, nombre))?;
                    if paginas > MAX_PAGINAS_PDF {
                        return Err(ErrorAdjunto::DemasiadasPaginas {
                            nombre: adjunto.nombre.clone(),
                            maximo: MAX_PAGINAS_PDF,
                            paginas,
                        });
                    }
                }

                let peso = if !ve_imagenes {
                    texto_ocr.as_ref().map_or(0, |t| t.len())
                } else if proveedor == Proveedor::Claude
                    && !bytes.is_empty()
                    && (paginas_jpeg.is_empty()
                        || contar_paginas_pdf(bytes).is_ok_and(|n| n <= MAX_PAGINAS_PDF))
                {
                    bytes.len()
                } else if !paginas_jpeg.is_empty() {
                    paginas_jpeg.iter().map(Vec::len).sum()
                } else {
                    bytes.len()
                };
                total_bytes = total_bytes.saturating_add(peso);
            }
            ContenidoAdjunto::Imagen { jpeg, texto_ocr } => {
                let tiene_ocr = texto_ocr.as_deref().is_some_and(|t| !t.trim().is_empty());
                if jpeg.is_empty() && (!ve_imagenes && !tiene_ocr || ve_imagenes) {
                    return Err(ErrorAdjunto::Vacio(adjunto.nombre.clone()));
                }
                let peso = if ve_imagenes {
                    jpeg.len()
                } else {
                    texto_ocr.as_ref().map_or(0, |t| t.len())
                };
                total_bytes = total_bytes.saturating_add(peso);
            }
        }
    }

    if total_bytes > MAX_BYTES_TOTAL {
        return Err(ErrorAdjunto::DemasiadoGrande {
            maximo_bytes: MAX_BYTES_TOTAL,
            actual_bytes: total_bytes,
        });
    }

    Ok(())
}

/// Convierte los adjuntos en bloques de texto para modelos sin visión:
/// `--- archivo: nombre (página N) ---`.
pub fn a_texto(adjuntos: &[Adjunto]) -> Result<String, ErrorAdjunto> {
    let mut bloques = Vec::new();
    let mut total_bytes = 0usize;

    for adjunto in adjuntos {
        let nombre = if adjunto.nombre.trim().is_empty() {
            "adjunto"
        } else {
            adjunto.nombre.trim()
        };
        match &adjunto.contenido {
            ContenidoAdjunto::Pdf {
                bytes,
                paginas_jpeg: _,
                texto_ocr,
            } => {
                let mut paginas = if bytes.is_empty() {
                    Vec::new()
                } else {
                    let mut extraidas =
                        extraer_texto_pdf(bytes).map_err(|e| con_nombre(e, nombre))?;
                    extraidas.truncate(MAX_PAGINAS_PDF);
                    extraidas
                };
                let sin_texto = paginas_sin_texto(&paginas);
                let ocr_limpio = texto_ocr
                    .as_deref()
                    .map(str::trim)
                    .filter(|t| !t.is_empty());

                let mut bloques_archivo = Vec::new();

                if paginas.is_empty() {
                    if let Some(ocr) = ocr_limpio {
                        bloques_archivo.push(format!("--- archivo: {nombre} (página 1) ---\n{ocr}"));
                    }
                } else {
                    let ocr_partes: Vec<&str> = ocr_limpio
                        .map(|ocr| ocr.split('\u{000C}').map(str::trim).collect())
                        .unwrap_or_default();

                    if !ocr_partes.is_empty() && ocr_partes.len() == sin_texto.len() {
                        for (pos, &idx_pagina) in sin_texto.iter().enumerate() {
                            if !ocr_partes[pos].is_empty() {
                                paginas[idx_pagina] = ocr_partes[pos].to_string();
                            }
                        }
                        for (idx, texto) in paginas.iter().enumerate() {
                            if !texto.trim().is_empty() {
                                bloques_archivo.push(format!(
                                    "--- archivo: {nombre} (página {}) ---\n{}",
                                    idx + 1,
                                    texto.trim()
                                ));
                            }
                        }
                    } else {
                        for (idx, texto) in paginas.iter().enumerate() {
                            if !sin_texto.contains(&idx) && !texto.trim().is_empty() {
                                bloques_archivo.push(format!(
                                    "--- archivo: {nombre} (página {}) ---\n{}",
                                    idx + 1,
                                    texto.trim()
                                ));
                            }
                        }
                        if let Some(ocr) = ocr_limpio {
                            if sin_texto.len() == 1 {
                                let num = sin_texto[0] + 1;
                                bloques_archivo.push(format!(
                                    "--- archivo: {nombre} (página {num}) ---\n{ocr}"
                                ));
                            } else {
                                bloques_archivo.push(format!(
                                    "--- archivo: {nombre} (OCR) ---\n{ocr}"
                                ));
                            }
                        }
                    }
                }

                if bloques_archivo.is_empty() {
                    return Err(ErrorAdjunto::SinTextoLegible);
                }
                for b in &bloques_archivo {
                    total_bytes = total_bytes.saturating_add(b.len());
                }
                bloques.extend(bloques_archivo);
            }
            ContenidoAdjunto::Imagen { jpeg: _, texto_ocr } => {
                let Some(ocr) = texto_ocr
                    .as_deref()
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                else {
                    return Err(ErrorAdjunto::SinTextoLegible);
                };
                let bloque = format!("--- archivo: {nombre} ---\n{ocr}");
                total_bytes = total_bytes.saturating_add(bloque.len());
                bloques.push(bloque);
            }
        }
    }

    if total_bytes > MAX_BYTES_TOTAL {
        return Err(ErrorAdjunto::DemasiadoGrande {
            maximo_bytes: MAX_BYTES_TOTAL,
            actual_bytes: total_bytes,
        });
    }

    Ok(bloques.join("\n\n"))
}

fn asegurar_descifrado(doc: &mut Document, nombre: &str) -> Result<(), ErrorAdjunto> {
    if doc.is_encrypted() {
        doc.decrypt("")
            .map_err(|_| ErrorAdjunto::PdfConContrasena(nombre.to_string()))?;
    }
    Ok(())
}

fn mapear_error_lopdf(nombre: &str, err: &pdf_extract::Error) -> ErrorAdjunto {
    match err {
        pdf_extract::Error::Decryption(_) => ErrorAdjunto::PdfConContrasena(nombre.to_string()),
        otro => {
            let msg = otro.to_string();
            let lower = msg.to_lowercase();
            if lower.contains("encrypt") || lower.contains("password") || lower.contains("decrypt") {
                ErrorAdjunto::PdfConContrasena(nombre.to_string())
            } else {
                ErrorAdjunto::PdfDanado(msg)
            }
        }
    }
}

fn con_nombre(err: ErrorAdjunto, nombre: &str) -> ErrorAdjunto {
    match err {
        ErrorAdjunto::PdfConContrasena(n) if n.is_empty() => {
            ErrorAdjunto::PdfConContrasena(nombre.to_string())
        }
        otro => otro,
    }
}

#[cfg(test)]
pub(crate) mod pruebas_ayuda {
    use std::fmt::Write as _;

    /// Genera un PDF mínimo válido con una página por cada cadena de `paginas`.
    pub fn pdf_con_paginas(paginas: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n");

        // Objeto 1: Catalog
        // Objeto 2: Pages
        // Objeto 3: Font Helvetica
        // Luego por cada página i: Page (4 + 2*i) y Content (5 + 2*i)
        let num_pags = paginas.len().max(1);
        let total_objs = 3 + num_pags * 2;
        let mut offsets = Vec::with_capacity(total_objs + 1);
        offsets.push(0usize);

        let mut kids = String::new();
        for i in 0..num_pags {
            let page_id = 4 + i * 2;
            let _ = write!(kids, "{page_id} 0 R ");
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

        for i in 0..num_pags {
            let page_id = 4 + i * 2;
            let content_id = page_id + 1;
            let texto = paginas.get(i).copied().unwrap_or("");
            let stream = if texto.is_empty() {
                String::new()
            } else {
                let escapado = texto
                    .replace('\\', "\\\\")
                    .replace('(', "\\(")
                    .replace(')', "\\)");
                format!("BT /F1 12 Tf 72 720 Td ({escapado}) Tj ET\n")
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
        out.extend_from_slice(format!("xref\n0 {}\n", total_objs + 1).as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in offsets.iter().skip(1) {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n",
                total_objs + 1
            )
            .as_bytes(),
        );
        out
    }
}

#[cfg(test)]
mod pruebas {
    use super::pruebas_ayuda::pdf_con_paginas;
    use super::*;

    #[test]
    fn extrae_texto_pdf_por_pagina_y_detecta_vacias() {
        let pdf = pdf_con_paginas(&["Horario de Control lunes 7am", "", "Taller 3 entrega viernes"]);
        let paginas = extraer_texto_pdf(&pdf).expect("PDF válido");
        assert_eq!(paginas.len(), 3);
        assert!(paginas[0].contains("Horario de Control"), "{:?}", paginas);
        assert!(paginas[1].is_empty(), "{:?}", paginas);
        assert!(paginas[2].contains("Taller 3"), "{:?}", paginas);

        assert_eq!(paginas_sin_texto(&paginas), vec![1]);
    }

    #[test]
    fn pdf_danado_o_vacio_da_error_legible_sin_panico() {
        let err_vacio = extraer_texto_pdf(&[]).unwrap_err();
        assert!(err_vacio.to_string().contains("PDF"), "{err_vacio}");

        let err_basura = extraer_texto_pdf(b"esto no es un pdf").unwrap_err();
        assert!(matches!(err_basura, ErrorAdjunto::PdfDanado(_)));
        assert!(err_basura.to_string().contains("dañado"), "{err_basura}");
    }

    #[test]
    fn limites_adjuntos_paginas_y_peso() {
        // Más de 5 adjuntos.
        let seis: Vec<Adjunto> = (0..6)
            .map(|i| Adjunto {
                nombre: format!("foto-{i}.jpg"),
                contenido: ContenidoAdjunto::Imagen {
                    jpeg: vec![0xFF, 0xD8, 0xFF],
                    texto_ocr: Some("hola".into()),
                },
            })
            .collect();
        let err = validar(&seis, Proveedor::Gemini, true).unwrap_err();
        assert!(matches!(err, ErrorAdjunto::DemasiadosAdjuntos { maximo: 5, recibidos: 6 }));

        // PDF de 21 páginas sin recorte.
        let paginas_21 = vec!["Texto de prueba"; 21];
        let pdf_21 = pdf_con_paginas(&paginas_21);
        let adj_21 = vec![Adjunto {
            nombre: "largo.pdf".into(),
            contenido: ContenidoAdjunto::Pdf {
                bytes: pdf_21,
                paginas_jpeg: vec![],
                texto_ocr: None,
            },
        }];
        let err_pags = validar(&adj_21, Proveedor::Claude, true).unwrap_err();
        assert!(matches!(
            err_pags,
            ErrorAdjunto::DemasiadasPaginas {
                maximo: 20,
                paginas: 21,
                ..
            }
        ));

        // Más de 20 MB en imágenes.
        let gigante = vec![Adjunto {
            nombre: "pesada.jpg".into(),
            contenido: ContenidoAdjunto::Imagen {
                jpeg: vec![1u8; MAX_BYTES_TOTAL + 1],
                texto_ocr: None,
            },
        }];
        let err_peso = validar(&gigante, Proveedor::Gemini, true).unwrap_err();
        assert!(matches!(err_peso, ErrorAdjunto::DemasiadoGrande { .. }));
    }

    #[test]
    fn sin_texto_legible_devuelve_mensaje_esperado() {
        let pdf_escaneado = pdf_con_paginas(&[""]);
        let adjuntos = vec![Adjunto {
            nombre: "escaneado.pdf".into(),
            contenido: ContenidoAdjunto::Pdf {
                bytes: pdf_escaneado,
                paginas_jpeg: vec![vec![1, 2, 3]],
                texto_ocr: Some("   ".into()),
            },
        }];
        let err = a_texto(&adjuntos).unwrap_err();
        assert_eq!(
            err.to_string(),
            "No pude leer texto en este archivo; prueba con Gemini o Claude."
        );
    }

    #[test]
    fn mezcla_texto_embebido_y_ocr_por_pagina() {
        let pdf = pdf_con_paginas(&["Clase de Calculo", "", "Laboratorio 2"]);
        let adjuntos = vec![Adjunto {
            nombre: "programa.pdf".into(),
            contenido: ContenidoAdjunto::Pdf {
                bytes: pdf,
                paginas_jpeg: vec![],
                texto_ocr: Some("Parcial el 28 de noviembre".into()),
            },
        }];
        let texto = a_texto(&adjuntos).expect("a texto");
        assert!(texto.contains("--- archivo: programa.pdf (página 1) ---\nClase de Calculo"));
        assert!(texto.contains("--- archivo: programa.pdf (página 2) ---\nParcial el 28 de noviembre"));
        assert!(texto.contains("--- archivo: programa.pdf (página 3) ---\nLaboratorio 2"));
    }
}
