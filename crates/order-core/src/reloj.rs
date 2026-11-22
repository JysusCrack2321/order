//! El reloj de la ventana: horas en 12 h con a. m. / p. m.
//!
//! En el archivo y en todo el estado interno la hora va siempre en 24 h
//! (`HH:MM`), que es lo que se guarda en `order.toml` y lo que entiende
//! `chrono`. En la ventana, en cambio, se lee como se dice aquí: "7:00 a. m.",
//! "12:00 p. m.", "11:59 p. m.".
//!
//! Aquí solo hay cuentas y textos: nada de interfaz. La conversión tiene dos
//! casos que se equivocan siempre y que están cubiertos por las pruebas:
//! 12 a. m. es medianoche (00:00) y 12 p. m. es mediodía (12:00).

use chrono::{NaiveTime, Timelike};

/// La hora en formato de 12 h: (1–12, ¿es de la tarde?). 00:00 es 12 a. m. y
/// 12:00 es 12 p. m.
pub fn a_12h(hora24: u32) -> (u32, bool) {
    let hora24 = hora24 % 24;
    let tarde = hora24 >= 12;
    let hora12 = match hora24 % 12 {
        0 => 12,
        otra => otra,
    };
    (hora12, tarde)
}

/// Vuelve a 24 h: 12 a. m. → 0 y 12 p. m. → 12.
pub fn a_24h(hora12: u32, tarde: bool) -> u32 {
    let hora = hora12.clamp(1, 12) % 12;
    if tarde {
        hora + 12
    } else {
        hora
    }
}

/// "a. m." o "p. m." según la hora.
pub fn sufijo(hora24: u32) -> &'static str {
    if a_12h(hora24).1 {
        "p. m."
    } else {
        "a. m."
    }
}

/// "7:05 a. m." — una hora como se lee en la ventana.
pub fn hora_texto(hora: NaiveTime) -> String {
    let (hora12, _) = a_12h(hora.hour());
    format!("{hora12}:{:02} {}", hora.minute(), sufijo(hora.hour()))
}

/// "7 a. m." — la hora en punto, para la columna de la rejilla.
pub fn etiqueta_hora(hora: u32) -> String {
    let (hora12, _) = a_12h(hora);
    format!("{hora12} {}", sufijo(hora))
}

/// "7:00–9:00 a. m." cuando las dos horas son de la misma mitad del día, o
/// "11:00 a. m. – 1:00 p. m." cuando cruza el mediodía.
pub fn rango_texto(inicio: NaiveTime, fin: NaiveTime) -> String {
    let (hora_inicio, tarde_inicio) = a_12h(inicio.hour());
    let (hora_fin, tarde_fin) = a_12h(fin.hour());
    if tarde_inicio == tarde_fin {
        format!(
            "{hora_inicio}:{:02}–{hora_fin}:{:02} {}",
            inicio.minute(),
            fin.minute(),
            sufijo(inicio.hour())
        )
    } else {
        format!("{} – {}", hora_texto(inicio), hora_texto(fin))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use chrono::NaiveTime;

    fn hora(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    /// La conversión 12 h ↔ 24 h, con los dos casos que se equivocan siempre.
    #[test]
    fn doce_y_veinticuatro_horas() {
        assert_eq!(a_12h(0), (12, false), "medianoche es 12 a. m.");
        assert_eq!(a_12h(1), (1, false));
        assert_eq!(a_12h(11), (11, false));
        assert_eq!(a_12h(12), (12, true), "mediodía es 12 p. m.");
        assert_eq!(a_12h(13), (1, true));
        assert_eq!(a_12h(23), (11, true));

        assert_eq!(a_24h(12, false), 0, "12 a. m. es medianoche");
        assert_eq!(a_24h(12, true), 12, "12 p. m. es mediodía");
        assert_eq!(a_24h(1, true), 13);
        assert_eq!(a_24h(11, false), 11);
        assert_eq!(a_24h(13, true), 12, "13 no existe en 12 h: se acota a 12");

        for hora in 0..24 {
            let (hora12, tarde) = a_12h(hora);
            assert!((1..=12).contains(&hora12), "{hora} → {hora12}");
            assert_eq!(a_24h(hora12, tarde), hora, "ida y vuelta de {hora}");
        }
    }

    #[test]
    fn horas_escritas_en_doce_horas() {
        assert_eq!(hora_texto(hora(7, 0)), "7:00 a. m.");
        assert_eq!(hora_texto(hora(7, 5)), "7:05 a. m.");
        assert_eq!(hora_texto(hora(0, 5)), "12:05 a. m.");
        assert_eq!(hora_texto(hora(12, 0)), "12:00 p. m.");
        assert_eq!(hora_texto(hora(23, 59)), "11:59 p. m.");
        assert_eq!(etiqueta_hora(0), "12 a. m.");
        assert_eq!(etiqueta_hora(7), "7 a. m.");
        assert_eq!(etiqueta_hora(12), "12 p. m.");
        assert_eq!(etiqueta_hora(13), "1 p. m.");
    }

    #[test]
    fn rangos_de_hora() {
        assert_eq!(rango_texto(hora(7, 0), hora(9, 0)), "7:00–9:00 a. m.");
        assert_eq!(rango_texto(hora(13, 0), hora(15, 30)), "1:00–3:30 p. m.");
        assert_eq!(
            rango_texto(hora(11, 30), hora(13, 0)),
            "11:30 a. m. – 1:00 p. m.",
            "cuando cruza el mediodía se dice cada hora con su mitad"
        );
        assert_eq!(
            rango_texto(hora(23, 0), hora(0, 30)),
            "11:00 p. m. – 12:30 a. m."
        );
    }
}
