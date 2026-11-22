//! Lectura y validación de los campos de formulario y de filtros.

use crate::agente::config::{
    AgenteConfig, LIMITE_CONTEXTO_POR_DEFECTO, Proveedor, Voz, VozProveedor,
};
use crate::filtros::TipoFecha;
use crate::modelo::{Ajustes, Clase, Dia, FORMATO_FECHA, FORMATO_HORA, Recordatorio, Semestre};
use crate::ops::NuevaTarea;
use crate::presentacion::materias::canonica;
use crate::reloj::a_24h;
use chrono::{NaiveDate, NaiveTime};

/// Campo inválido del formulario (1–4) y el mensaje para mostrar al usuario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorCampo {
    pub campo: i32,
    pub mensaje: String,
}

fn error(campo: i32, mensaje: impl Into<String>) -> ErrorCampo {
    ErrorCampo {
        campo,
        mensaje: mensaje.into(),
    }
}

/// Texto del campo, o `None` si quedó vacío (para los campos opcionales).
pub fn opcional(texto: &str) -> Option<String> {
    let texto = texto.trim();
    if texto.is_empty() {
        None
    } else {
        Some(texto.to_string())
    }
}

/// Texto escrito o, si está vacío, el valor por defecto.
pub fn valor_o(texto: &str, defecto: &str) -> String {
    let texto = texto.trim();
    if texto.is_empty() {
        defecto.to_string()
    } else {
        texto.to_string()
    }
}

pub fn interpretar_fecha(texto: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(texto.trim(), FORMATO_FECHA).ok()
}

/// Acepta lo que escriben los campos: "23:59" (24 h) y también "11:59 p. m.",
/// "7:00 a. m." o "7:00am", que es como se leen las horas en la ventana.
pub fn interpretar_hora(texto: &str) -> Option<NaiveTime> {
    let limpio = texto.trim();
    if let Ok(hora) = NaiveTime::parse_from_str(limpio, FORMATO_HORA) {
        return Some(hora);
    }
    let compacto: String = limpio
        .to_lowercase()
        .chars()
        .filter(|caracter| !caracter.is_whitespace() && *caracter != '.')
        .collect();
    let (reloj, tarde) = compacto
        .strip_suffix("am")
        .map(|reloj| (reloj, false))
        .or_else(|| compacto.strip_suffix("pm").map(|reloj| (reloj, true)))?;
    let (horas, minutos) = reloj.split_once(':')?;
    let horas: u32 = horas.parse().ok()?;
    let minutos: u32 = minutos.parse().ok()?;
    if !(1..=12).contains(&horas) || minutos > 59 {
        return None;
    }
    NaiveTime::from_hms_opt(a_24h(horas, tarde), minutos, 0)
}

/// La opción de fecha de la barra de filtros: 0 Todas · 1 Hoy · 2 Esta semana
/// · 3 Rango.
pub fn codigo_fecha(tipo: TipoFecha) -> i32 {
    match tipo {
        TipoFecha::Todas => 0,
        TipoFecha::Hoy => 1,
        TipoFecha::EstaSemana => 2,
        TipoFecha::Rango => 3,
    }
}

pub fn fecha_del_codigo(codigo: i32) -> TipoFecha {
    match codigo {
        1 => TipoFecha::Hoy,
        2 => TipoFecha::EstaSemana,
        3 => TipoFecha::Rango,
        _ => TipoFecha::Todas,
    }
}

pub fn validar_tarea(
    titulo: &str,
    fecha: &str,
    hora: &str,
    materia: &str,
    materias_existentes: &[String],
) -> Result<NuevaTarea, ErrorCampo> {
    let titulo = titulo.trim().to_string();
    if titulo.is_empty() {
        return Err(error(1, "El título no puede quedar vacío."));
    }
    let Some(fecha) = interpretar_fecha(fecha) else {
        return Err(error(2, "Elige la fecha de entrega."));
    };
    let Some(hora) = interpretar_hora(hora) else {
        return Err(error(3, "Elige la hora de entrega."));
    };
    Ok(NuevaTarea {
        titulo,
        materia: opcional(&canonica(materias_existentes, materia)),
        vence: fecha.and_time(hora),
    })
}

pub fn validar_clase(
    id: String,
    materia: &str,
    dias: Vec<Dia>,
    hora_inicio: &str,
    hora_fin: &str,
    lugar: &str,
    materias_existentes: &[String],
) -> Result<Clase, ErrorCampo> {
    let materia = materia.trim().to_string();
    if materia.is_empty() {
        return Err(error(1, "La materia no puede quedar vacía."));
    }
    if dias.is_empty() {
        return Err(error(4, "Marca al menos un día para la clase."));
    }
    let Some(inicio) = interpretar_hora(hora_inicio) else {
        return Err(error(2, "Elige la hora de inicio."));
    };
    let Some(fin) = interpretar_hora(hora_fin) else {
        return Err(error(3, "Elige la hora de fin."));
    };
    if fin <= inicio {
        return Err(error(3, "La hora de fin debe ser posterior a la de inicio."));
    }
    Ok(Clase {
        id,
        materia: canonica(materias_existentes, &materia),
        dias,
        inicio,
        fin,
        lugar: opcional(lugar),
    })
}

pub fn validar_recordatorio(
    id: String,
    texto: &str,
    hora: &str,
    dias: Vec<Dia>,
) -> Result<Recordatorio, ErrorCampo> {
    let texto = texto.trim().to_string();
    if texto.is_empty() {
        return Err(error(1, "El texto del recordatorio no puede quedar vacío."));
    }
    let Some(hora) = interpretar_hora(hora) else {
        return Err(error(2, "Elige la hora del recordatorio."));
    };
    if dias.is_empty() {
        return Err(error(4, "Marca al menos un día para el recordatorio."));
    }
    Ok(Recordatorio {
        id,
        texto,
        dias,
        hora,
    })
}

pub fn validar_ajustes(
    sin_semestre: bool,
    inicio: &str,
    fin: &str,
    hora_aviso: &str,
    horas_antes: &str,
) -> Result<(Ajustes, Option<Semestre>), ErrorCampo> {
    let mut semestre = None;
    if !sin_semestre {
        let Some(inicio) = interpretar_fecha(inicio) else {
            return Err(error(1, "Elige el inicio del semestre."));
        };
        let Some(fin) = interpretar_fecha(fin) else {
            return Err(error(2, "Elige el fin del semestre."));
        };
        if fin < inicio {
            return Err(error(2, "El fin del semestre debe ser posterior al inicio."));
        }
        semestre = Some(Semestre { inicio, fin });
    }

    let Some(aviso) = interpretar_hora(hora_aviso) else {
        return Err(error(3, "Elige la hora del aviso."));
    };
    let Some(aviso_horas_antes) = horas_antes
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|horas| (1..=168).contains(horas))
    else {
        return Err(error(
            4,
            "Las horas de aviso deben ser un número entre 1 y 168.",
        ));
    };

    Ok((
        Ajustes {
            aviso_dia_antes: aviso,
            aviso_horas_antes,
        },
        semestre,
    ))
}

/// Una URL base solo vale si dice con qué viaja: `https://` o `http://`.
fn url_http(url: &str) -> bool {
    let url = url.trim();
    url.starts_with("https://") || url.starts_with("http://")
}

pub fn validar_agente(
    proveedor: i32,
    url: &str,
    modelo: &str,
    api_key: &str,
    v_proveedor: i32,
    v_url: &str,
    v_modelo: &str,
    v_key: &str,
) -> Result<AgenteConfig, ErrorCampo> {
    // Un campo vacío no deja el agente sin URL ni modelo: vuelve el valor por
    // defecto del proveedor elegido. Con "Otro" no hay defecto que poner.
    let proveedor = Proveedor::de_codigo(proveedor);
    let (url_defecto, modelo_defecto) = proveedor.por_defecto();
    let url = valor_o(url, url_defecto);
    let modelo = valor_o(modelo, modelo_defecto);
    if url.is_empty() {
        return Err(error(1, "La URL base no puede quedar vacía."));
    }
    if !url_http(&url) {
        return Err(error(1, "La URL base debe empezar por https:// o http://."));
    }
    if modelo.is_empty() {
        return Err(error(2, "El modelo no puede quedar vacío."));
    }
    let api_key = api_key.trim().to_string();
    if api_key.is_empty() {
        return Err(error(5, "Falta la API key"));
    }

    let v_proveedor = VozProveedor::de_codigo(v_proveedor);
    let (v_url_defecto, v_modelo_defecto) = v_proveedor.por_defecto();
    let voz = Voz {
        proveedor: v_proveedor,
        url_base: valor_o(v_url, v_url_defecto),
        modelo: valor_o(v_modelo, v_modelo_defecto),
        api_key: v_key.trim().to_string(),
    };
    if !url_http(&voz.url_base) {
        return Err(error(
            3,
            "La URL de la voz debe empezar por https:// o http://.",
        ));
    }

    Ok(AgenteConfig {
        proveedor,
        url_base: url,
        modelo,
        api_key,
        ve_imagenes: false,
        limite_contexto_tokens: LIMITE_CONTEXTO_POR_DEFECTO,
        voz,
    })
}

/// Valida el campo de límite de contexto (en tokens); vacío usa `60 000`.
pub fn validar_limite_contexto(texto: &str) -> Result<u32, ErrorCampo> {
    let limpio = texto.trim();
    if limpio.is_empty() {
        return Ok(LIMITE_CONTEXTO_POR_DEFECTO);
    }
    let valor = limpio
        .parse::<u32>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| {
            error(
                6,
                "El límite de contexto debe ser un número entero mayor que cero.",
            )
        })?;
    Ok(valor)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::presentacion::textos::hora;
    use crate::reloj::hora_texto;

    fn fecha(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).unwrap()
    }

    /// Los formularios aceptan los formatos del archivo y las horas tal como
    /// se leen en la ventana (12 h con a. m. / p. m.).
    #[test]
    fn solo_acepta_los_formatos_del_archivo() {
        assert_eq!(interpretar_fecha("2026-11-28"), Some(fecha(2026, 11, 28)));
        assert_eq!(interpretar_fecha(" 2026-11-28 "), Some(fecha(2026, 11, 28)));
        assert_eq!(interpretar_fecha("28/11/2026"), None);
        assert_eq!(interpretar_fecha(""), None);

        assert_eq!(
            interpretar_hora("23:59"),
            Some(NaiveTime::from_hms_opt(23, 59, 0).unwrap())
        );
        assert_eq!(interpretar_hora("25:00"), None);
        assert_eq!(interpretar_hora("7"), None);
    }

    /// El campo de hora entiende lo que la ventana muestra (12 h) y lo que
    /// hay escrito en el archivo (24 h).
    #[test]
    fn el_campo_de_hora_entiende_las_dos_formas() {
        for (texto, esperada) in [
            ("7:00 a. m.", hora(7, 0)),
            ("07:00 a.m.", hora(7, 0)),
            ("7:00am", hora(7, 0)),
            ("12:00 a. m.", hora(0, 0)),
            ("12:00 p. m.", hora(12, 0)),
            ("11:59 p. m.", hora(23, 59)),
            ("2:30 p. m.", hora(14, 30)),
            ("23:59", hora(23, 59)),
        ] {
            assert_eq!(interpretar_hora(texto), Some(esperada), "{texto}");
        }
        assert_eq!(interpretar_hora("13:00 p. m."), None, "13 no existe en 12 h");
        assert_eq!(interpretar_hora("7:60 a. m."), None);
        assert_eq!(interpretar_hora("7:00 x. m."), None);
        assert_eq!(hora_texto(hora(14, 30)), "2:30 p. m.");
    }

    /// Un campo vacío no se guarda como cadena vacía en el TOML.
    #[test]
    fn campos_opcionales_vacios_son_none() {
        assert_eq!(opcional("   "), None);
        assert_eq!(opcional(" Control "), Some("Control".to_string()));
    }

    /// Las fichas de fecha de la barra de filtros y su código van de la mano.
    #[test]
    fn codigos_de_fecha_del_filtro() {
        for (codigo, tipo) in [
            (0, TipoFecha::Todas),
            (1, TipoFecha::Hoy),
            (2, TipoFecha::EstaSemana),
            (3, TipoFecha::Rango),
        ] {
            assert_eq!(codigo_fecha(tipo), codigo);
            assert_eq!(fecha_del_codigo(codigo), tipo);
        }
        assert_eq!(fecha_del_codigo(99), TipoFecha::Todas, "código raro = todas");
    }

    #[test]
    fn validaciones_de_formularios_devuelven_campo_y_mensaje() {
        assert_eq!(
            validar_tarea(" ", "2026-11-28", "23:59", "", &[]).unwrap_err().campo,
            1
        );
        assert_eq!(
            validar_clase(
                String::new(),
                "Control",
                vec![Dia::Lun],
                "09:00",
                "07:00",
                "",
                &[]
            )
            .unwrap_err()
            .campo,
            3
        );
        assert_eq!(
            validar_recordatorio(String::new(), "Algo", "18:00", vec![])
                .unwrap_err()
                .campo,
            4
        );
        assert_eq!(
            validar_ajustes(false, "2026-11-20", "2026-11-10", "20:00", "24")
                .unwrap_err()
                .campo,
            2
        );
    }

    /// El agente se valida con los valores de su ficha: URL sin esquema, key
    /// ausente y voz con sus propios defectos.
    #[test]
    fn validaciones_del_agente_y_de_la_voz() {
        let err = validar_agente(3, "localhost:1234", "local", "k", 0, "", "", "").unwrap_err();
        assert_eq!(err.campo, 1);
        assert_eq!(
            err.mensaje,
            "La URL base debe empezar por https:// o http://."
        );

        let err = validar_agente(0, "", "", "   ", 0, "", "", "").unwrap_err();
        assert_eq!(err.campo, 5);
        assert_eq!(err.mensaje, "Falta la API key");

        // Campos vacíos = valores por defecto del proveedor elegido.
        let agente = validar_agente(1, "", "", "clave", 0, "", "", "").unwrap();
        assert_eq!(agente.proveedor, Proveedor::Gemini);
        assert_eq!(
            agente.url_base,
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
        assert_eq!(agente.modelo, "gemini-flash-latest");
        assert_eq!(agente.voz.proveedor, VozProveedor::OpenAI);
        assert_eq!(agente.voz.url_base, "https://api.openai.com/v1");
        assert_eq!(agente.voz.modelo, "whisper-1");

        let con_groq = validar_agente(0, "", "", "clave", 1, "", "", "gsk").unwrap();
        assert_eq!(con_groq.voz.proveedor, VozProveedor::Groq);
        assert_eq!(con_groq.voz.url_base, "https://api.groq.com/openai/v1");
        assert_eq!(con_groq.voz.modelo, "whisper-large-v3-turbo");
        assert!(con_groq.voz.lista());

        let err = validar_agente(0, "", "", "clave", 0, "api.openai.com", "", "").unwrap_err();
        assert_eq!(err.campo, 3);
        assert_eq!(
            err.mensaje,
            "La URL de la voz debe empezar por https:// o http://."
        );
    }

    #[test]
    fn valida_limite_contexto() {
        assert_eq!(validar_limite_contexto("").unwrap(), 60_000);
        assert_eq!(validar_limite_contexto("  12000 ").unwrap(), 12_000);
        assert_eq!(validar_limite_contexto("0").unwrap_err().campo, 6);
        assert_eq!(validar_limite_contexto("abc").unwrap_err().campo, 6);
    }
}
