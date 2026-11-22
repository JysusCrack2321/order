//! Preparación de adjuntos (PDF e imágenes) en escritorio.
//!
//! Usa `rfd` (portal XDG) para elegir archivos, `image` + `kamadak-exif` para
//! rotar/reducir fotos a JPEG, `pdftoppm -jpeg -r 110` para renderizar páginas
//! de PDF y `tesseract - - -l spa` para OCR cuando el modelo no ve imágenes.

use exif::{In, Tag};
use image::GenericImageView;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use order_core::agente::adjuntos::{
    self as core_adjuntos, Adjunto, CALIDAD_JPEG, ContenidoAdjunto, LADO_LARGO_IMAGEN,
    MAX_PAGINAS_PDF,
};
use order_core::agente::config::{AgenteConfig, Proveedor};
use std::io::{Cursor, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Resultado de preparar un archivo elegido por el usuario.
pub struct ResultadoPreparacion {
    pub adjunto: Adjunto,
    pub es_pdf: bool,
    pub aviso: Option<String>,
}

/// Abre el selector de archivos del sistema (portal XDG).
pub fn elegir_archivos() -> Vec<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Adjuntar PDF o imagen")
        .add_filter(
            "PDF e imágenes",
            &["pdf", "png", "jpg", "jpeg", "webp"],
        )
        .pick_files()
        .unwrap_or_default()
}

/// Prepara un archivo (PDF o imagen) según el proveedor configurado.
pub fn preparar_archivo(ruta: &Path, config: &AgenteConfig) -> Result<ResultadoPreparacion, String> {
    let nombre = ruta
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("archivo")
        .to_string();
    let ext = ruta
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let bytes = std::fs::read(ruta)
        .map_err(|e| format!("No se pudo leer «{nombre}»: {e}"))?;
    if bytes.is_empty() {
        return Err(format!("El archivo «{nombre}» está vacío."));
    }

    if ext == "pdf" {
        preparar_pdf(nombre, bytes, config)
    } else if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp") {
        preparar_imagen(nombre, &bytes, config.ve_imagenes())
    } else {
        Err(format!(
            "Formato no admitido en «{nombre}». Usa PDF, PNG, JPG o WEBP."
        ))
    }
}

fn preparar_imagen(
    nombre: String,
    bytes: &[u8],
    ve_imagenes: bool,
) -> Result<ResultadoPreparacion, String> {
    let jpeg = normalizar_imagen_jpeg(bytes)
        .map_err(|e| format!("No se pudo procesar la imagen «{nombre}»: {e}"))?;

    let texto_ocr = if ve_imagenes {
        None
    } else {
        let ocr = ocr_tesseract(&jpeg)?;
        if ocr.trim().is_empty() {
            return Err("No pude leer texto en este archivo; prueba con Gemini o Claude.".into());
        }
        Some(ocr)
    };

    Ok(ResultadoPreparacion {
        adjunto: Adjunto {
            nombre,
            contenido: ContenidoAdjunto::Imagen { jpeg, texto_ocr },
        },
        es_pdf: false,
        aviso: None,
    })
}

fn preparar_pdf(
    nombre: String,
    bytes: Vec<u8>,
    config: &AgenteConfig,
) -> Result<ResultadoPreparacion, String> {
    let total_paginas = core_adjuntos::contar_paginas_pdf(&bytes).map_err(|e| e.to_string())?;
    let aviso_paginas = if total_paginas > MAX_PAGINAS_PDF {
        Some(format!(
            "«{nombre}» tiene {total_paginas} páginas; se usarán solo las primeras {MAX_PAGINAS_PDF}."
        ))
    } else {
        None
    };
    let limite = total_paginas.min(MAX_PAGINAS_PDF);

    if config.ve_imagenes() {
        // Claude recibe el PDF crudo si tiene <= 20 páginas; si supera 20 o el
        // proveedor es Gemini/Otro con visión, renderizamos páginas JPEG con pdftoppm.
        let necesita_render = config.proveedor != Proveedor::Claude || total_paginas > MAX_PAGINAS_PDF;
        let paginas_jpeg = if necesita_render {
            renderizar_rango_pdf(&bytes, 1, limite)?
        } else {
            Vec::new()
        };

        return Ok(ResultadoPreparacion {
            adjunto: Adjunto {
                nombre,
                contenido: ContenidoAdjunto::Pdf {
                    bytes,
                    paginas_jpeg,
                    texto_ocr: None,
                },
            },
            es_pdf: true,
            aviso: aviso_paginas,
        });
    }

    let mut paginas_texto = core_adjuntos::extraer_texto_pdf(&bytes).map_err(|e| e.to_string())?;
    paginas_texto.truncate(MAX_PAGINAS_PDF);
    let sin_texto = core_adjuntos::paginas_sin_texto(&paginas_texto);

    let mut aviso_final = aviso_paginas;
    let mut texto_ocr = None;

    if !sin_texto.is_empty() {
        let todas_vacias = sin_texto.len() == paginas_texto.len();
        match ocr_paginas_pdf(&bytes, &sin_texto) {
            Ok(partes) => {
                let unido = partes.join("\u{000C}");
                if !unido.replace('\u{000C}', "").trim().is_empty() {
                    texto_ocr = Some(unido);
                } else if todas_vacias {
                    return Err(
                        "No pude leer texto en este archivo; prueba con Gemini o Claude.".into(),
                    );
                }
            }
            Err(err) => {
                if todas_vacias {
                    return Err(err);
                }
                // Hay páginas con texto embebido: avisamos y seguimos con lo que sí se extrajo.
                aviso_final = Some(match aviso_final {
                    Some(prev) => format!("{prev} {err}"),
                    None => err,
                });
            }
        }
    }

    Ok(ResultadoPreparacion {
        adjunto: Adjunto {
            nombre,
            contenido: ContenidoAdjunto::Pdf {
                bytes,
                paginas_jpeg: Vec::new(),
                texto_ocr,
            },
        },
        es_pdf: true,
        aviso: aviso_final,
    })
}

/// Normaliza una imagen: respeta orientación EXIF, reduce el lado largo a
/// `LADO_LARGO_IMAGEN` (1568 px) y codifica a JPEG calidad `CALIDAD_JPEG` (85).
pub fn normalizar_imagen_jpeg(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let orientacion = leer_orientacion_exif(bytes);
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let img = aplicar_orientacion(img, orientacion);

    let (ancho, alto) = img.dimensions();
    let lado_largo = ancho.max(alto);
    let img = if lado_largo > LADO_LARGO_IMAGEN {
        let escala = LADO_LARGO_IMAGEN as f64 / lado_largo as f64;
        let nuevo_ancho = ((ancho as f64 * escala).round() as u32).max(1);
        let nuevo_alto = ((alto as f64 * escala).round() as u32).max(1);
        img.resize_exact(nuevo_ancho, nuevo_alto, FilterType::Triangle)
    } else {
        img
    };

    let rgb = img.to_rgb8();
    let mut salida = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut salida, CALIDAD_JPEG);
    rgb.write_with_encoder(encoder).map_err(|e| e.to_string())?;
    Ok(salida)
}

fn leer_orientacion_exif(bytes: &[u8]) -> u32 {
    let mut cursor = Cursor::new(bytes);
    let Ok(datos) = exif::Reader::new().read_from_container(&mut cursor) else {
        return 1;
    };
    datos
        .get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .unwrap_or(1)
}

fn aplicar_orientacion(img: image::DynamicImage, orientacion: u32) -> image::DynamicImage {
    match orientacion {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

fn renderizar_rango_pdf(bytes: &[u8], desde_1: usize, hasta_1: usize) -> Result<Vec<Vec<u8>>, String> {
    let dir = tempfile::tempdir().map_err(|e| format!("No se pudo crear carpeta temporal: {e}"))?;
    let pdf_ruta = dir.path().join("entrada.pdf");
    std::fs::write(&pdf_ruta, bytes).map_err(|e| format!("No se pudo escribir PDF temporal: {e}"))?;
    let prefijo = dir.path().join("pag");

    let salida = Command::new("pdftoppm")
        .arg("-jpeg")
        .arg("-r")
        .arg("110")
        .arg("-f")
        .arg(desde_1.to_string())
        .arg("-l")
        .arg(hasta_1.to_string())
        .arg(&pdf_ruta)
        .arg(&prefijo)
        .output();

    let salida = match salida {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(
                "Falta «pdftoppm» (paquete poppler) para convertir páginas del PDF a imagen."
                    .into(),
            );
        }
        Err(e) => return Err(format!("No se pudo ejecutar pdftoppm: {e}")),
    };

    if !salida.status.success() {
        let detalle = String::from_utf8_lossy(&salida.stderr).trim().to_string();
        return Err(if detalle.is_empty() {
            "pdftoppm falló al renderizar el PDF.".into()
        } else {
            format!("pdftoppm falló al renderizar el PDF: {detalle}")
        });
    }

    let mut archivos: Vec<PathBuf> = std::fs::read_dir(dir.path())
        .map_err(|e| format!("No se pudo leer salida de pdftoppm: {e}"))?
        .filter_map(|e| e.ok().map(|entry| entry.path()))
        .filter(|p| {
            p.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg"))
        })
        .collect();
    archivos.sort();

    if archivos.is_empty() {
        return Err("pdftoppm no generó ninguna página JPEG.".into());
    }

    let mut paginas = Vec::with_capacity(archivos.len());
    for archivo in archivos {
        let crudo = std::fs::read(&archivo)
            .map_err(|e| format!("No se pudo leer página generada: {e}"))?;
        let normalizada = normalizar_imagen_jpeg(&crudo)?;
        paginas.push(normalizada);
    }
    Ok(paginas)
}

fn ocr_paginas_pdf(bytes: &[u8], indices_base0: &[usize]) -> Result<Vec<String>, String> {
    let mut partes = Vec::with_capacity(indices_base0.len());
    for &idx in indices_base0 {
        let pag = idx + 1;
        let imagenes = renderizar_rango_pdf(bytes, pag, pag)?;
        let Some(jpeg) = imagenes.first() else {
            partes.push(String::new());
            continue;
        };
        partes.push(ocr_tesseract(jpeg)?);
    }
    Ok(partes)
}

fn ocr_tesseract(jpeg: &[u8]) -> Result<String, String> {
    let mut child = match Command::new("tesseract")
        .arg("-")
        .arg("-")
        .arg("-l")
        .arg("spa")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(
                "Falta «tesseract» (con idioma español) para leer texto de imágenes o PDFs escaneados."
                    .into(),
            );
        }
        Err(e) => return Err(format!("No se pudo ejecutar tesseract: {e}")),
    };

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(jpeg)
            .map_err(|e| format!("No se pudo enviar la imagen a tesseract: {e}"))?;
    }

    let salida = child
        .wait_with_output()
        .map_err(|e| format!("Error esperando a tesseract: {e}"))?;

    if !salida.status.success() {
        let detalle = String::from_utf8_lossy(&salida.stderr).trim().to_string();
        return Err(if detalle.is_empty() {
            "tesseract falló al hacer OCR.".into()
        } else {
            format!("tesseract falló al hacer OCR: {detalle}")
        });
    }

    Ok(String::from_utf8_lossy(&salida.stdout).trim().to_string())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use image::{ImageBuffer, Rgb};

    #[test]
    fn reduce_imagen_larga_a_1568_jpeg() {
        let img = ImageBuffer::from_pixel(3000, 1500, Rgb([200u8, 180u8, 120u8]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("codificar png");

        let jpeg = normalizar_imagen_jpeg(&png).expect("normalizar");
        let dec = image::load_from_memory(&jpeg).expect("decodificar jpeg");
        assert_eq!(dec.dimensions(), (1568, 784));
    }

    #[test]
    fn pdftoppm_y_tesseract_funcionan_si_estan_instalados() {
        // Verifica el flujo real de pdftoppm cuando está instalado en el sistema.
        let pdf = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Kids [ 4 0 R ] /Count 1 >>\nendobj\n\
3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n\
4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents 5 0 R >>\nendobj\n\
5 0 obj\n<< /Length 44 >>\nstream\nBT /F1 18 Tf 72 720 Td (Hola ORDER) Tj ET\nendstream\nendobj\n\
xref\n0 6\n\
0000000000 65535 f \n\
0000000009 00000 n \n\
0000000058 00000 n \n\
0000000115 00000 n \n\
0000000185 00000 n \n\
0000000309 00000 n \n\
trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n403\n%%EOF\n";
        let pags = renderizar_rango_pdf(pdf, 1, 1).expect("pdftoppm render");
        assert_eq!(pags.len(), 1);
        let texto = ocr_tesseract(&pags[0]).expect("tesseract ocr");
        assert!(texto.contains("ORDER") || texto.contains("Hola"), "{texto}");
    }
}
