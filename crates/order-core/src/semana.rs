//! Cálculo de la vista **Semana** y de la lista de **Pendientes**.
//!
//! Aquí no hay UI: solo se convierte el contenido del archivo (`Datos`) en lo
//! que la ventana necesita dibujar. La semana se calcula a partir de su lunes
//! y las clases se repiten cada siete días mientras la fecha caiga dentro de
//! `[semestre]`; sin `[semestre]`, aparecen todas las semanas.

use crate::modelo::{Datos, Dia, Tarea};
use chrono::{Datelike, Days, NaiveDate, NaiveDateTime, NaiveTime, Weekday};

/// Una clase ya colocada en un día concreto de la semana mostrada.
#[derive(Debug, Clone, PartialEq)]
pub struct Bloque {
    pub clase_id: String,
    pub materia: String,
    pub inicio: NaiveTime,
    pub fin: NaiveTime,
    pub lugar: Option<String>,
}

/// Un día de la semana: sus clases y las tareas que vencen ese día.
#[derive(Debug, Clone, PartialEq)]
pub struct DiaVista {
    pub fecha: NaiveDate,
    pub bloques: Vec<Bloque>,
    /// Tareas (hechas o no) cuyo `vence` cae en `fecha`, por hora.
    pub tareas: Vec<Tarea>,
}

/// Un recordatorio que cae en una fecha concreta.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordatorioVisto {
    pub id: String,
    pub texto: String,
    pub fecha: NaiveDate,
    pub hora: NaiveTime,
}

/// Lo que dibuja la pestaña **Inicio**: el resumen del día y de lo que viene.
#[derive(Debug, Clone, PartialEq)]
pub struct InicioVista {
    /// Clases de hoy, por hora.
    pub clases: Vec<Bloque>,
    /// La próxima clase (hoy si aún no termina, o la primera de los próximos
    /// siete días) con la fecha en la que cae.
    pub siguiente: Option<(NaiveDate, Bloque)>,
    /// Tareas sin hacer cuyo vencimiento ya pasó.
    pub vencidas: Vec<Tarea>,
    /// Tareas sin hacer que vencen hoy.
    pub de_hoy: Vec<Tarea>,
    /// Tareas sin hacer que vencen de mañana a dentro de siete días.
    pub proximos: Vec<Tarea>,
    /// Los próximos recordatorios, en orden.
    pub recordatorios: Vec<RecordatorioVisto>,
    /// Cuántas tareas quedan por materia, de más a menos.
    pub por_materia: Vec<(String, usize)>,
}

/// El lunes de la semana que contiene `fecha` (la semana empieza en lunes).
pub fn lunes_de(fecha: NaiveDate) -> NaiveDate {
    fecha - Days::new(u64::from(fecha.weekday().num_days_from_monday()))
}

/// Los días de la semana que empieza en `lunes`: de lunes a viernes siempre, y
/// sábado o domingo solo si ese día tiene clases dentro del semestre.
pub fn semana(datos: &Datos, lunes: NaiveDate) -> Vec<DiaVista> {
    let mut dias = Vec::with_capacity(7);

    for atraso in 0..7u64 {
        let Some(fecha) = lunes.checked_add_days(Days::new(atraso)) else {
            break;
        };
        let dia = dia_de(fecha);
        let bloques = bloques_de(datos, fecha);

        // El fin de semana solo ocupa columna si de verdad hay clase.
        if matches!(dia, Dia::Sab | Dia::Dom) && bloques.is_empty() {
            continue;
        }

        dias.push(dia_completo(datos, fecha));
    }

    dias
}

/// Las clases de una fecha concreta, por hora. Fuera del semestre no hay.
pub fn bloques_de(datos: &Datos, fecha: NaiveDate) -> Vec<Bloque> {
    if !dentro_del_semestre(datos, fecha) {
        return Vec::new();
    }
    let dia = dia_de(fecha);
    let mut bloques: Vec<Bloque> = datos
        .clases
        .iter()
        .filter(|clase| clase.dias.contains(&dia))
        .map(|clase| Bloque {
            clase_id: clase.id.clone(),
            materia: clase.materia.clone(),
            inicio: clase.inicio,
            fin: clase.fin,
            lugar: clase.lugar.clone(),
        })
        .collect();
    bloques.sort_by(|a, b| {
        a.inicio
            .cmp(&b.inicio)
            .then(a.fin.cmp(&b.fin))
            .then(a.materia.cmp(&b.materia))
    });
    bloques
}

/// Un día suelto (lo que dibuja la pestaña **Día**): sus clases y las tareas
/// que vencen ese día, hechas o no.
pub fn dia(datos: &Datos, fecha: NaiveDate) -> DiaVista {
    dia_completo(datos, fecha)
}

fn dia_completo(datos: &Datos, fecha: NaiveDate) -> DiaVista {
    let mut tareas: Vec<Tarea> = datos
        .tareas
        .iter()
        .filter(|tarea| tarea.vence.date() == fecha)
        .cloned()
        .collect();
    tareas.sort_by_key(|tarea| tarea.vence);
    DiaVista {
        fecha,
        bloques: bloques_de(datos, fecha),
        tareas,
    }
}

/// Los recordatorios que tocan en una fecha, por hora y, a la misma hora, por
/// texto: el orden no depende de cómo estén escritos en el archivo.
pub fn recordatorios_de(datos: &Datos, fecha: NaiveDate) -> Vec<RecordatorioVisto> {
    let dia = dia_de(fecha);
    let mut lista: Vec<RecordatorioVisto> = datos
        .recordatorios
        .iter()
        .filter(|recordatorio| recordatorio.dias.contains(&dia))
        .map(|recordatorio| RecordatorioVisto {
            id: recordatorio.id.clone(),
            texto: recordatorio.texto.clone(),
            fecha,
            hora: recordatorio.hora,
        })
        .collect();
    lista.sort_by(|a, b| a.hora.cmp(&b.hora).then(a.texto.cmp(&b.texto)));
    lista
}

/// Los `cuantos` recordatorios que vienen después de `ahora`, en orden y con
/// cada recordatorio **una sola vez**: aparece solo su próxima ocurrencia. Un
/// "Revisar Moodle" de lunes a viernes sale una vez, no cinco, y el orden es
/// el de la ocurrencia que toca (el de hoy antes que el de mañana). Se buscan
/// en los próximos ocho días: con uno por semana basta para tener siempre
/// alguno si hay recordatorios.
pub fn proximos_recordatorios(
    datos: &Datos,
    ahora: NaiveDateTime,
    cuantos: usize,
) -> Vec<RecordatorioVisto> {
    let mut lista: Vec<RecordatorioVisto> = Vec::new();
    for adelanto in 0..8 {
        let fecha = ahora.date() + Days::new(adelanto);
        for recordatorio in recordatorios_de(datos, fecha) {
            if fecha.and_time(recordatorio.hora) <= ahora {
                continue;
            }
            // Ya sale más arriba con una ocurrencia anterior: no se repite.
            let repetido = lista.iter().any(|visto| {
                (!visto.id.is_empty() && visto.id == recordatorio.id)
                    || (visto.id.is_empty() && visto.texto == recordatorio.texto)
            });
            if repetido {
                continue;
            }
            lista.push(recordatorio);
            if lista.len() >= cuantos {
                return lista;
            }
        }
    }
    lista
}

/// El resumen de la pestaña **Inicio**: lo de hoy, lo próximo y lo atrasado.
pub fn inicio(datos: &Datos, ahora: NaiveDateTime) -> InicioVista {
    let hoy = ahora.date();

    // La siguiente clase: la de hoy que todavía no termina o, si ya no queda
    // ninguna, la primera de los próximos siete días.
    let mut siguiente = None;
    for adelanto in 0..8 {
        let Some(fecha) = hoy.checked_add_days(Days::new(adelanto)) else {
            break;
        };
        let bloque = bloques_de(datos, fecha)
            .into_iter()
            .find(|bloque| fecha > hoy || bloque.fin > ahora.time());
        if let Some(bloque) = bloque {
            siguiente = Some((fecha, bloque));
            break;
        }
    }

    let pendientes: Vec<&Tarea> = datos.tareas.iter().filter(|tarea| !tarea.hecha).collect();
    let limite = hoy.checked_add_days(Days::new(7)).unwrap_or(hoy);

    let por_fecha = |desde: NaiveDate, hasta: NaiveDate| -> Vec<Tarea> {
        let mut lista: Vec<Tarea> = pendientes
            .iter()
            .filter(|tarea| {
                let fecha = tarea.vence.date();
                fecha >= desde && fecha <= hasta
            })
            .map(|tarea| (*tarea).clone())
            .collect();
        lista.sort_by_key(|tarea| tarea.vence);
        lista
    };
    let de_hoy = por_fecha(hoy, hoy);
    let proximos = por_fecha(hoy + Days::new(1), limite);

    // "Vencidas" son las de días anteriores: las de hoy ya salen en su lista
    // aunque su hora haya pasado, para no verlas dos veces.
    let mut vencidas: Vec<Tarea> = pendientes
        .iter()
        .filter(|tarea| tarea.vence.date() < hoy)
        .map(|tarea| (*tarea).clone())
        .collect();
    vencidas.sort_by_key(|tarea| tarea.vence);

    InicioVista {
        clases: bloques_de(datos, hoy),
        siguiente,
        vencidas,
        de_hoy,
        proximos,
        recordatorios: proximos_recordatorios(datos, ahora, 5),
        por_materia: conteo_por_materia(&pendientes),
    }
}

/// Cuántas pendientes hay por materia, de más a menos y sin repetir la grafía.
fn conteo_por_materia(pendientes: &[&Tarea]) -> Vec<(String, usize)> {
    let mut lista: Vec<(String, usize)> = Vec::new();
    for tarea in pendientes {
        let materia = tarea
            .materia
            .as_deref()
            .map(str::trim)
            .filter(|materia| !materia.is_empty())
            .unwrap_or("Sin materia");
        match lista.iter_mut().find(|(nombre, _)| nombre == materia) {
            Some((_, cuantas)) => *cuantas += 1,
            None => lista.push((materia.to_string(), 1)),
        }
    }
    lista.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    lista
}

/// Tareas sin hacer, de la que vence antes a la que vence después.
pub fn pendientes(datos: &Datos) -> Vec<Tarea> {
    let mut lista: Vec<Tarea> = datos.tareas.iter().filter(|t| !t.hecha).cloned().collect();
    lista.sort_by_key(|t| t.vence);
    lista
}

/// ¿Pasó su vencimiento sin estar hecha?
pub fn vencida(t: &Tarea, ahora: NaiveDateTime) -> bool {
    !t.hecha && t.vence < ahora
}

/// Día del archivo (`lun` … `dom`) que corresponde a una fecha.
fn dia_de(fecha: NaiveDate) -> Dia {
    match fecha.weekday() {
        Weekday::Mon => Dia::Lun,
        Weekday::Tue => Dia::Mar,
        Weekday::Wed => Dia::Mie,
        Weekday::Thu => Dia::Jue,
        Weekday::Fri => Dia::Vie,
        Weekday::Sat => Dia::Sab,
        Weekday::Sun => Dia::Dom,
    }
}

/// Sin `[semestre]` todo vale; con él, solo las fechas entre `inicio` y `fin`.
fn dentro_del_semestre(datos: &Datos, fecha: NaiveDate) -> bool {
    datos
        .semestre
        .as_ref()
        .is_none_or(|s| fecha >= s.inicio && fecha <= s.fin)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::modelo::{Ajustes, Clase, Recordatorio, Semestre};

    fn fecha(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).unwrap()
    }

    fn hora(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn momento(anio: i32, mes: u32, dia: u32, h: u32, m: u32) -> NaiveDateTime {
        fecha(anio, mes, dia).and_hms_opt(h, m, 0).unwrap()
    }

    fn datos(clases: Vec<Clase>, tareas: Vec<Tarea>, semestre: Option<Semestre>) -> Datos {
        Datos {
            ajustes: Ajustes::default(),
            semestre,
            clases,
            tareas,
            recordatorios: Vec::new(),
        }
    }

    fn clase(materia: &str, dia: Dia) -> Clase {
        Clase {
            id: String::new(),
            materia: materia.to_string(),
            dias: vec![dia],
            inicio: hora(7, 0),
            fin: hora(9, 0),
            lugar: None,
        }
    }

    fn clase_en(materia: &str, dias: &[Dia]) -> Clase {
        Clase {
            dias: dias.to_vec(),
            ..clase(materia, Dia::Lun)
        }
    }

    fn tarea(titulo: &str, vence: NaiveDateTime, hecha: bool) -> Tarea {
        Tarea {
            id: String::new(),
            titulo: titulo.to_string(),
            materia: None,
            vence,
            hecha,
            hecha_en: hecha.then(|| vence.date()),
            origen: None,
            enlace: None,
        }
    }

    fn fechas(dias: &[DiaVista]) -> Vec<NaiveDate> {
        dias.iter().map(|d| d.fecha).collect()
    }

    fn recordatorio(texto: &str, dias: &[Dia], h: u32, m: u32) -> Recordatorio {
        Recordatorio {
            id: texto.to_lowercase().replace(' ', "-"),
            texto: texto.to_string(),
            dias: dias.to_vec(),
            hora: hora(h, m),
        }
    }

    #[test]
    fn semana_respeta_semestre() {
        let d = datos(
            vec![clase("Control", Dia::Lun)],
            Vec::new(),
            Some(Semestre {
                inicio: fecha(2026, 8, 3),
                fin: fecha(2026, 12, 4),
            }),
        );

        let dentro = semana(&d, fecha(2026, 11, 30));
        let lunes = &dentro[0];
        assert_eq!(lunes.fecha, fecha(2026, 11, 30));
        assert_eq!(
            lunes.bloques.len(),
            1,
            "el lunes 30-nov aún cae en semestre"
        );
        assert_eq!(lunes.bloques[0].materia, "Control");

        let despues = semana(&d, fecha(2026, 12, 7));
        assert_eq!(
            despues.len(),
            5,
            "la semana se dibuja aunque no haya clases"
        );
        assert!(
            despues.iter().all(|dia| dia.bloques.is_empty()),
            "pasado el fin del semestre no se dibuja ninguna clase"
        );
    }

    #[test]
    fn semana_cruza_anio() {
        assert_eq!(lunes_de(fecha(2027, 1, 1)), fecha(2026, 12, 28));

        let d = datos(vec![clase("Control", Dia::Vie)], Vec::new(), None);
        let dias = semana(&d, lunes_de(fecha(2027, 1, 1)));
        assert_eq!(
            fechas(&dias),
            vec![
                fecha(2026, 12, 28),
                fecha(2026, 12, 29),
                fecha(2026, 12, 30),
                fecha(2026, 12, 31),
                fecha(2027, 1, 1),
            ]
        );
        assert_eq!(dias[4].bloques.len(), 1, "la clase del viernes es el 1-ene");
    }

    #[test]
    fn fin_de_semana_solo_si_hay_clases() {
        let entre_semana = datos(vec![clase("Control", Dia::Lun)], Vec::new(), None);
        assert_eq!(semana(&entre_semana, fecha(2026, 11, 30)).len(), 5);

        let con_sabado = datos(
            vec![clase("Control", Dia::Lun), clase("Taller", Dia::Sab)],
            Vec::new(),
            None,
        );
        let dias = semana(&con_sabado, fecha(2026, 11, 30));
        assert_eq!(dias.len(), 6);
        assert_eq!(dias[5].fecha, fecha(2026, 12, 5));
        assert_eq!(dias[5].bloques[0].materia, "Taller");
    }

    /// Una clase con varios días ocupa una columna por cada día.
    #[test]
    fn clase_en_varios_dias() {
        let d = datos(
            vec![clase_en("Control", &[Dia::Lun, Dia::Mie, Dia::Vie])],
            Vec::new(),
            None,
        );
        let dias = semana(&d, fecha(2026, 11, 30));
        let con_clase: Vec<NaiveDate> = dias
            .iter()
            .filter(|dia| !dia.bloques.is_empty())
            .map(|dia| dia.fecha)
            .collect();
        assert_eq!(
            con_clase,
            vec![fecha(2026, 11, 30), fecha(2026, 12, 2), fecha(2026, 12, 4)],
            "lunes, miércoles y viernes"
        );
        assert_eq!(
            dias[2].bloques[0].clase_id, "",
            "el bloque conserva la clase que lo originó"
        );
    }

    /// Un miércoles con clase, una tarea atrasada, una de hoy, una próxima y
    /// una lejana: el resumen de Inicio las separa y cuenta por materia.
    fn miércoles(ahora_h: u32, ahora_m: u32) -> (Datos, NaiveDateTime) {
        let mut d = datos(
            vec![
                {
                    let mut c = clase_en("Control", &[Dia::Mie]);
                    c.inicio = hora(7, 0);
                    c.fin = hora(9, 0);
                    c
                },
                {
                    let mut c = clase_en("Robótica", &[Dia::Jue]);
                    c.inicio = hora(9, 0);
                    c.fin = hora(11, 0);
                    c
                },
            ],
            vec![
                {
                    let mut t = tarea("Atrasada", momento(2026, 11, 24, 23, 59), false);
                    t.materia = None;
                    t
                },
                {
                    let mut t = tarea("Taller", momento(2026, 11, 25, 23, 59), false);
                    t.materia = Some("Control".into());
                    t
                },
                {
                    let mut t = tarea("Informe", momento(2026, 12, 1, 18, 0), false);
                    t.materia = Some("Control".into());
                    t
                },
                tarea("Examen", momento(2026, 12, 10, 9, 0), false),
                {
                    let mut t = tarea("Quiz", momento(2026, 11, 26, 10, 0), true);
                    t.materia = Some("Control".into());
                    t
                },
            ],
            Some(Semestre {
                inicio: fecha(2026, 8, 3),
                fin: fecha(2026, 12, 5),
            }),
        );
        d.recordatorios = vec![
            recordatorio("Revisar Moodle", &[Dia::Mie], 18, 0),
            recordatorio("Revisar Classroom", &[Dia::Lun], 9, 0),
        ];
        (d, momento(2026, 11, 25, ahora_h, ahora_m))
    }

    #[test]
    fn inicio_resume_el_dia() {
        let (d, ahora) = miércoles(8, 0); // la clase de 7–9 todavía no termina
        let resumen = inicio(&d, ahora);

        assert_eq!(resumen.clases.len(), 1);
        assert_eq!(resumen.clases[0].materia, "Control");
        let (fecha_siguiente, siguiente) = resumen.siguiente.clone().expect("hay siguiente clase");
        assert_eq!(fecha_siguiente, fecha(2026, 11, 25));
        assert_eq!(siguiente.materia, "Control", "la de hoy aún no termina");

        assert_eq!(
            resumen.vencidas.iter().map(|t| t.titulo.as_str()).collect::<Vec<_>>(),
            vec!["Atrasada"]
        );
        assert_eq!(
            resumen.de_hoy.iter().map(|t| t.titulo.as_str()).collect::<Vec<_>>(),
            vec!["Taller"],
            "la hecha no entra"
        );
        assert_eq!(
            resumen.proximos.iter().map(|t| t.titulo.as_str()).collect::<Vec<_>>(),
            vec!["Informe"],
            "el examen del 10-dic queda fuera de los siete días"
        );

        assert_eq!(
            resumen.recordatorios[0].texto, "Revisar Moodle",
            "el de hoy va primero"
        );
        assert_eq!(resumen.recordatorios[0].fecha, fecha(2026, 11, 25));
        assert_eq!(resumen.recordatorios[1].fecha, fecha(2026, 11, 30), "el lunes siguiente");

        assert_eq!(
            resumen.por_materia,
            vec![("Control".to_string(), 2), ("Sin materia".to_string(), 2)],
            "cuenta todas las pendientes, no solo las de la semana"
        );
    }

    /// Una tarea de hoy con la hora ya pasada sigue siendo "de hoy", no
    /// "vencida": así no aparece dos veces en el tablero.
    #[test]
    fn lo_de_hoy_no_se_repite_en_vencidas() {
        let (mut d, ahora) = miércoles(10, 0);
        let mut temprano = tarea("Exposición", momento(2026, 11, 25, 8, 0), false);
        temprano.materia = Some("Control".into());
        d.tareas.push(temprano);
        let resumen = inicio(&d, ahora);
        assert_eq!(
            resumen.vencidas.iter().map(|t| t.titulo.as_str()).collect::<Vec<_>>(),
            vec!["Atrasada"]
        );
        assert_eq!(
            resumen.de_hoy.iter().map(|t| t.titulo.as_str()).collect::<Vec<_>>(),
            vec!["Exposición", "Taller"],
            "las de hoy, por hora"
        );
    }

    /// Si la clase de hoy ya terminó, la siguiente es la del día siguiente.
    #[test]
    fn siguiente_clase_salta_al_dia_que_viene() {
        let (d, ahora) = miércoles(10, 0);
        let resumen = inicio(&d, ahora);
        let (fecha_siguiente, siguiente) = resumen.siguiente.expect("hay siguiente clase");
        assert_eq!(fecha_siguiente, fecha(2026, 11, 26));
        assert_eq!(siguiente.materia, "Robótica");
    }

    /// Sin clases en los próximos días no hay "siguiente".
    #[test]
    fn sin_clases_no_hay_siguiente() {
        let d = datos(Vec::new(), Vec::new(), None);
        let resumen = inicio(&d, momento(2026, 11, 25, 8, 0));
        assert!(resumen.siguiente.is_none());
        assert!(resumen.clases.is_empty());
        assert!(resumen.por_materia.is_empty());
    }

    /// La pestaña Día trae las clases y las tareas de esa fecha.
    #[test]
    fn vista_de_un_dia() {
        let (d, _) = miércoles(8, 0);
        let vista = dia(&d, fecha(2026, 11, 25));
        assert_eq!(vista.fecha, fecha(2026, 11, 25));
        assert_eq!(vista.bloques.len(), 1);
        assert_eq!(vista.bloques[0].materia, "Control");
        assert_eq!(
            vista.tareas.iter().map(|t| t.titulo.as_str()).collect::<Vec<_>>(),
            vec!["Taller"]
        );

        let recordatorios = recordatorios_de(&d, fecha(2026, 11, 25));
        assert_eq!(recordatorios.len(), 1);
        assert_eq!(recordatorios[0].texto, "Revisar Moodle");

        let jueves = dia(&d, fecha(2026, 11, 26));
        assert_eq!(
            jueves.tareas.iter().map(|t| t.titulo.as_str()).collect::<Vec<_>>(),
            vec!["Quiz"],
            "la vista del día muestra también lo hecho"
        );
        assert_eq!(jueves.bloques[0].materia, "Robótica");
    }

    /// Los próximos recordatorios salen en orden, sin pasados y con cada uno
    /// una sola vez: solo su próxima ocurrencia.
    #[test]
    fn proximos_recordatorios_en_orden() {
        let (d, ahora) = miércoles(8, 0);
        let lista = proximos_recordatorios(&d, ahora, 5);
        assert_eq!(
            lista.iter().map(|r| r.texto.as_str()).collect::<Vec<_>>(),
            vec!["Revisar Moodle", "Revisar Classroom"],
            "miércoles 18:00 y lunes 9:00; el miércoles siguiente no repite Moodle"
        );
        assert_eq!(lista[0].fecha, fecha(2026, 11, 25), "primero el de hoy");
        assert_eq!(lista[1].fecha, fecha(2026, 11, 30), "después el del lunes");
        assert_eq!(proximos_recordatorios(&d, ahora, 1).len(), 1, "respeta el tope");

        // Después de la hora del recordatorio de hoy, el de hoy ya no cuenta.
        let tarde = proximos_recordatorios(&d, momento(2026, 11, 25, 19, 0), 1);
        assert_eq!(tarde[0].texto, "Revisar Classroom");
    }

    /// Un recordatorio de lunes a viernes aparece una sola vez, con la
    /// ocurrencia que viene antes; pasada esa hora, con la del día siguiente.
    #[test]
    fn recordatorio_repetido_aparece_una_vez() {
        let mut d = datos(Vec::new(), Vec::new(), None);
        d.recordatorios = vec![recordatorio(
            "Revisar Moodle",
            &[Dia::Lun, Dia::Mar, Dia::Mie, Dia::Jue, Dia::Vie],
            18,
            0,
        )];

        // Miércoles 25-nov-2026, 8:00: la próxima es hoy a las 18:00.
        let lista = proximos_recordatorios(&d, momento(2026, 11, 25, 8, 0), 5);
        assert_eq!(lista.len(), 1, "no se repite aunque toque cinco días");
        assert_eq!(lista[0].texto, "Revisar Moodle");
        assert_eq!(lista[0].fecha, fecha(2026, 11, 25));

        // Pasadas las 18:00 de hoy, la próxima ocurrencia es mañana.
        let tarde = proximos_recordatorios(&d, momento(2026, 11, 25, 19, 0), 5);
        assert_eq!(tarde.len(), 1);
        assert_eq!(tarde[0].fecha, fecha(2026, 11, 26));

        // En Inicio también sale una sola vez.
        let resumen = inicio(&d, momento(2026, 11, 25, 8, 0));
        assert_eq!(resumen.recordatorios.len(), 1);
        assert_eq!(resumen.recordatorios[0].texto, "Revisar Moodle");
    }

    /// Dos recordatorios a la misma hora salen ordenados por su texto, uno
    /// junto al otro, sin desaparecer ninguno.
    #[test]
    fn recordatorios_distintos_a_la_misma_hora() {
        let mut d = datos(Vec::new(), Vec::new(), None);
        d.recordatorios = vec![
            recordatorio("Revisar Moodle", &[Dia::Mie], 18, 0),
            recordatorio("Revisar Classroom", &[Dia::Mie], 18, 0),
        ];
        let lista = proximos_recordatorios(&d, momento(2026, 11, 25, 8, 0), 5);
        assert_eq!(
            lista.iter().map(|r| r.texto.as_str()).collect::<Vec<_>>(),
            vec!["Revisar Classroom", "Revisar Moodle"],
            "mismo momento: manda el orden de la hora, luego el texto"
        );
    }

    #[test]
    fn pendientes_ordenados_sin_hechas() {
        let d = datos(
            Vec::new(),
            vec![
                tarea("Informe", momento(2026, 12, 2, 18, 0), false),
                tarea("Taller 2", momento(2026, 11, 28, 23, 59), false),
                tarea("Quiz viejo", momento(2026, 11, 25, 10, 0), true),
            ],
            None,
        );

        let lista = pendientes(&d);
        let titulos: Vec<&str> = lista.iter().map(|t| t.titulo.as_str()).collect();
        assert_eq!(titulos, vec!["Taller 2", "Informe"]);
        assert!(
            lista.windows(2).all(|par| par[0].vence <= par[1].vence),
            "van de la fecha más próxima a la más lejana"
        );
    }
}
