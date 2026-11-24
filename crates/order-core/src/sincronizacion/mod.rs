//! Fusión CRDT LWW por campo para sincronizar `order.toml` entre dispositivos.

pub mod estado;
pub mod fusion;
pub mod hlc;

pub use estado::{
    AjustesEstado, Campo, ClaseEstado, EstadoSync, EventoEstado, MensajeSync, RecordatorioEstado,
    SESENTA_DIAS_MS, SemestreEstado, TareaEstado, cargar_estado, guardar_estado,
};
pub use fusion::{
    Resultado, aplicar, aplicar_en_doc, datos_equivalentes, fusionar, registrar_cambios_locales,
};
pub use hlc::{Marca, RelojHlc, ahora_ms};

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::almacen::Almacen;
    use crate::modelo::{
        self, Ajustes, Clase, Datos, Dia, Evento, FORMATO_FECHA, FORMATO_FECHA_HORA, FORMATO_HORA,
        Recordatorio, Semestre, Tarea,
    };
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

    fn fecha(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, FORMATO_FECHA).unwrap()
    }

    fn fecha_hora(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, FORMATO_FECHA_HORA).unwrap()
    }

    fn hora(s: &str) -> NaiveTime {
        NaiveTime::parse_from_str(s, FORMATO_HORA).unwrap()
    }

    fn datos_vacios() -> Datos {
        Datos {
            ajustes: Ajustes::default(),
            semestre: None,
            clases: Vec::new(),
            tareas: Vec::new(),
            recordatorios: Vec::new(),
            eventos: Vec::new(),
        }
    }

    fn tarea_base(id: &str, titulo: &str, vence: &str) -> Tarea {
        Tarea {
            id: id.to_string(),
            titulo: titulo.to_string(),
            materia: Some("Control".to_string()),
            vence: fecha_hora(vence),
            hecha: false,
            hecha_en: None,
            origen: None,
            enlace: None,
        }
    }

    /// Punto de revisión 1:
    /// PC y celular editan la misma tarea: uno el título, otro la fecha, sin
    /// conexión -> al conectar quedan los dos cambios.
    #[test]
    fn edicion_campos_distintos_conserva_ambos() {
        let mut base = datos_vacios();
        base.tareas
            .push(tarea_base("t-1", "Taller 1", "2026-11-28T23:59"));

        let mut pc = EstadoSync::nuevo("pc");
        registrar_cambios_locales(&base, &mut pc, 1_000);

        let mut cel = EstadoSync::nuevo("cel");
        let res = fusionar(&cel, &pc.mensaje());
        cel = res.estado;
        cel.foto = Some(aplicar(&cel));

        // PC cambia título; Cel cambia fecha de vencimiento (sin conexión).
        let mut datos_pc = aplicar(&pc);
        datos_pc.tareas[0].titulo = "Taller 1 corregido".to_string();
        assert!(registrar_cambios_locales(&datos_pc, &mut pc, 2_000));

        let mut datos_cel = aplicar(&cel);
        datos_cel.tareas[0].vence = fecha_hora("2026-12-01T18:00");
        assert!(registrar_cambios_locales(&datos_cel, &mut cel, 2_100));

        let pc_tras = fusionar(&pc, &cel.mensaje());
        let cel_tras = fusionar(&cel, &pc.mensaje());
        assert!(pc_tras.cambio);
        assert!(cel_tras.cambio);
        assert_eq!(pc_tras.estado.crdt, cel_tras.estado.crdt);

        let final_pc = aplicar(&pc_tras.estado);
        let final_cel = aplicar(&cel_tras.estado);
        assert_eq!(final_pc, final_cel);
        assert_eq!(final_pc.tareas[0].titulo, "Taller 1 corregido");
        assert_eq!(
            final_pc.tareas[0].vence,
            fecha_hora("2026-12-01T18:00")
        );
    }

    /// Punto de revisión 2:
    /// Mismo campo editado en ambos -> queda el más reciente, en los dos.
    #[test]
    fn edicion_mismo_campo_gana_mas_reciente() {
        let mut base = datos_vacios();
        base.tareas
            .push(tarea_base("t-1", "Original", "2026-11-28T23:59"));

        let mut pc = EstadoSync::nuevo("pc");
        registrar_cambios_locales(&base, &mut pc, 1_000);
        let mut cel = fusionar(&EstadoSync::nuevo("cel"), &pc.mensaje()).estado;
        cel.foto = Some(aplicar(&cel));

        let mut d_pc = aplicar(&pc);
        d_pc.tareas[0].titulo = "Título PC (antes)".to_string();
        registrar_cambios_locales(&d_pc, &mut pc, 2_000);

        let mut d_cel = aplicar(&cel);
        d_cel.tareas[0].titulo = "Título Cel (después)".to_string();
        registrar_cambios_locales(&d_cel, &mut cel, 3_000);

        let m_pc = fusionar(&pc, &cel.mensaje());
        let m_cel = fusionar(&cel, &pc.mensaje());
        assert_eq!(m_pc.estado.crdt, m_cel.estado.crdt);
        assert_eq!(aplicar(&m_pc.estado).tareas[0].titulo, "Título Cel (después)");
        assert_eq!(
            aplicar(&m_cel.estado).tareas[0].titulo,
            "Título Cel (después)"
        );
    }

    /// Punto de revisión 3:
    /// Borrar en uno y editar antes en el otro -> queda borrada;
    /// editar después del borrado -> reaparece con la edición.
    #[test]
    fn lapida_vence_a_edicion_anterior_y_pierde_contra_edicion_posterior() {
        let mut base = datos_vacios();
        base.tareas
            .push(tarea_base("t-1", "Inicial", "2026-11-28T23:59"));

        let mut pc = EstadoSync::nuevo("pc");
        registrar_cambios_locales(&base, &mut pc, 1_000);
        let mut cel = fusionar(&EstadoSync::nuevo("cel"), &pc.mensaje()).estado;
        cel.foto = Some(aplicar(&cel));

        // Caso A: PC edita en t=2_000; Cel borra después en t=3_000 -> queda borrada.
        let mut d_pc = aplicar(&pc);
        d_pc.tareas[0].titulo = "Editada antes".to_string();
        registrar_cambios_locales(&d_pc, &mut pc, 2_000);

        let mut d_cel = aplicar(&cel);
        d_cel.tareas.clear();
        registrar_cambios_locales(&d_cel, &mut cel, 3_000);

        let res_pc = fusionar(&pc, &cel.mensaje());
        let res_cel = fusionar(&cel, &pc.mensaje());
        assert_eq!(res_pc.estado.crdt, res_cel.estado.crdt);
        assert!(aplicar(&res_pc.estado).tareas.is_empty());

        // Caso B: Cel borró en t=3_000, pero PC edita después en t=4_000 -> reaparece con edición.
        let mut pc_resucita = res_pc.estado.clone();
        // Simulamos que otro dispositivo aún no había recibido la lápida y editó en t=4_000:
        let mut d_pc2 = datos_vacios();
        d_pc2.tareas.push(Tarea {
            titulo: "Editada después".to_string(),
            ..tarea_base("t-1", "Inicial", "2026-11-28T23:59")
        });
        let mut otro = pc.clone();
        registrar_cambios_locales(&d_pc2, &mut otro, 4_000);

        let m1 = fusionar(&pc_resucita, &otro.mensaje());
        let m2 = fusionar(&otro, &pc_resucita.mensaje());
        assert_eq!(m1.estado.crdt, m2.estado.crdt);
        let final_datos = aplicar(&m1.estado);
        assert_eq!(final_datos.tareas.len(), 1);
        assert_eq!(final_datos.tareas[0].titulo, "Editada después");

        // Purga de lápidas a los 60 días.
        pc_resucita = res_cel.estado;
        assert_eq!(pc_resucita.purgar_lapidas(3_000 + SESENTA_DIAS_MS), 0);
        assert_eq!(pc_resucita.purgar_lapidas(3_000 + SESENTA_DIAS_MS + 1), 1);
        assert!(pc_resucita.crdt.tareas.is_empty());
    }

    /// Punto de revisión 4:
    /// Edición a mano de `order.toml` en el PC -> detectada y aplicada al otro lado,
    /// conservando comentarios al reescribir con `Almacen::modificar`.
    #[test]
    fn edicion_a_mano_en_toml_se_propaga_y_conserva_comentarios() {
        let dir = tempfile::tempdir().unwrap();
        let ruta_pc = dir.path().join("pc.toml");
        std::fs::write(
            &ruta_pc,
            "\
# comentario importante
[ajustes]
aviso_dia_antes = \"20:00\"
aviso_horas_antes = 3 # mantener comentario

[[tarea]]
id = \"t-manual\"
titulo = \"Taller\"
vence = \"2026-11-28T23:59\"
hecha = false
",
        )
        .unwrap();

        let almacen_pc = Almacen {
            ruta: ruta_pc.clone(),
        };
        let datos_iniciales = almacen_pc.cargar().unwrap();
        let mut pc = EstadoSync::nuevo("pc");
        registrar_cambios_locales(&datos_iniciales, &mut pc, 1_000);

        let mut cel = fusionar(&EstadoSync::nuevo("cel"), &pc.mensaje()).estado;
        cel.foto = Some(aplicar(&cel));

        // El usuario edita a mano order.toml en el PC: cambia horas y marca la tarea hecha.
        let editado = std::fs::read_to_string(&ruta_pc)
            .unwrap()
            .replace("aviso_horas_antes = 3", "aviso_horas_antes = 6")
            .replace("hecha = false", "hecha = true");
        std::fs::write(&ruta_pc, editado).unwrap();

        let datos_leidos = almacen_pc.cargar().unwrap();
        assert!(registrar_cambios_locales(&datos_leidos, &mut pc, 2_000));

        // Cel recibe el cambio del PC, y además Cel añade un recordatorio.
        let mut datos_cel = aplicar(&cel);
        datos_cel.recordatorios.push(Recordatorio {
            id: "r-cel".to_string(),
            texto: "Revisar correo".to_string(),
            dias: vec![Dia::Lun, Dia::Vie],
            hora: hora("08:30"),
        });
        registrar_cambios_locales(&datos_cel, &mut cel, 2_500);

        let cel_final = fusionar(&cel, &pc.mensaje());
        let datos_cel_final = aplicar(&cel_final.estado);
        assert_eq!(datos_cel_final.ajustes.aviso_horas_antes, 6);
        assert!(datos_cel_final.tareas[0].hecha);
        assert_eq!(datos_cel_final.recordatorios.len(), 1);

        // PC recibe el recordatorio y lo escribe con Almacen::modificar sin perder comentarios.
        let pc_final = fusionar(&pc, &cel.mensaje());
        let nuevos_pc = aplicar(&pc_final.estado);
        almacen_pc
            .modificar(|doc| aplicar_en_doc(doc, &nuevos_pc))
            .unwrap();

        let texto_pc = std::fs::read_to_string(&ruta_pc).unwrap();
        assert!(texto_pc.contains("# comentario importante"));
        assert!(texto_pc.contains("# mantener comentario"));
        let reparado = modelo::parsear(&texto_pc).unwrap();
        assert!(datos_equivalentes(&reparado, &nuevos_pc));
    }

    /// Punto de revisión 5:
    /// Tareas de Moodle traídas en ambos -> no se duplican (mismo `origen`;
    /// si dos ids distintos tienen el mismo `origen`, se fusionan en uno).
    #[test]
    fn tareas_con_mismo_origen_se_fusionan_sin_duplicar() {
        let origen = Some("moodle:micampus.unal.edu.co:assign:42".to_string());

        let mut d_pc = datos_vacios();
        d_pc.tareas.push(Tarea {
            id: "t-20261122-ffff0001".to_string(),
            titulo: "Entrega 1".to_string(),
            materia: Some("Control".to_string()),
            vence: fecha_hora("2026-11-30T23:59"),
            hecha: false,
            hecha_en: None,
            origen: origen.clone(),
            enlace: Some("https://micampus/42".to_string()),
        });

        let mut d_cel = datos_vacios();
        d_cel.tareas.push(Tarea {
            id: "t-20261122-0000aaaa".to_string(),
            titulo: "Entrega 1".to_string(),
            materia: Some("Control".to_string()),
            vence: fecha_hora("2026-11-30T23:59"),
            hecha: true,
            hecha_en: Some(fecha("2026-11-22")),
            origen,
            enlace: Some("https://micampus/42".to_string()),
        });

        let mut pc = EstadoSync::nuevo("pc");
        registrar_cambios_locales(&d_pc, &mut pc, 1_000);

        let mut cel = EstadoSync::nuevo("cel");
        registrar_cambios_locales(&d_cel, &mut cel, 2_000);

        let r_pc = fusionar(&pc, &cel.mensaje());
        let r_cel = fusionar(&cel, &pc.mensaje());

        assert_eq!(r_pc.estado.crdt, r_cel.estado.crdt);
        let final_pc = aplicar(&r_pc.estado);
        assert_eq!(final_pc.tareas.len(), 1, "no debe duplicar la tarea de Moodle");
        assert_eq!(final_pc.tareas[0].id, "t-20261122-0000aaaa");
        assert!(final_pc.tareas[0].hecha);
        assert_eq!(final_pc.tareas[0].hecha_en, Some(fecha("2026-11-22")));

        // Idempotencia incluso al volver a recibir el mensaje del par con el id viejo.
        let r_otra = fusionar(&r_pc.estado, &pc.mensaje());
        assert!(!r_otra.cambio);
        assert_eq!(r_otra.estado.crdt, r_pc.estado.crdt);
    }

    #[test]
    fn conmutatividad_e_idempotencia_completas() {
        let mut d_a = datos_vacios();
        d_a.ajustes.aviso_horas_antes = 5;
        d_a.semestre = Some(Semestre {
            inicio: fecha("2026-08-03"),
            fin: fecha("2026-12-05"),
        });
        d_a.clases.push(Clase {
            id: "c-1".to_string(),
            materia: "Control".to_string(),
            dias: vec![Dia::Lun, Dia::Mie],
            inicio: hora("07:00"),
            fin: hora("09:00"),
            lugar: Some("453-201".to_string()),
        });
        d_a.tareas
            .push(tarea_base("t-1", "Tarea A", "2026-11-28T23:59"));

        let mut d_b = datos_vacios();
        d_b.ajustes.aviso_dia_antes = hora("19:00");
        d_b.semestre = Some(Semestre {
            inicio: fecha("2026-08-03"),
            fin: fecha("2026-12-12"),
        });
        d_b.clases.push(Clase {
            id: "c-1".to_string(),
            materia: "Control".to_string(),
            dias: vec![Dia::Lun, Dia::Mie],
            inicio: hora("07:00"),
            fin: hora("09:00"),
            lugar: Some("Lab 2".to_string()),
        });
        d_b.recordatorios.push(Recordatorio {
            id: "r-1".to_string(),
            texto: "Moodle".to_string(),
            dias: vec![Dia::Vie],
            hora: hora("18:00"),
        });

        let mut a = EstadoSync::nuevo("disp-a");
        registrar_cambios_locales(&d_a, &mut a, 1_000);

        let mut b = EstadoSync::nuevo("disp-b");
        registrar_cambios_locales(&d_b, &mut b, 2_000);

        let ab = fusionar(&a, &b.mensaje());
        let ba = fusionar(&b, &a.mensaje());
        assert_eq!(ab.estado.crdt, ba.estado.crdt);
        assert_eq!(aplicar(&ab.estado), aplicar(&ba.estado));

        // Idempotencia: fusionar dos veces equivale a una.
        let ab_b = fusionar(&ab.estado, &b.mensaje());
        assert!(!ab_b.cambio);
        assert_eq!(ab_b.estado.crdt, ab.estado.crdt);

        let ab_a = fusionar(&ab.estado, &a.mensaje());
        assert!(!ab_a.cambio);
        assert_eq!(ab_a.estado.crdt, ab.estado.crdt);
    }

    #[test]
    fn serializacion_y_permisos_600() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("sync-estado.json");

        let mut datos = datos_vacios();
        datos.semestre = Some(Semestre {
            inicio: fecha("2026-08-03"),
            fin: fecha("2026-12-05"),
        });
        datos.clases.push(Clase {
            id: "c-1".to_string(),
            materia: "Control".to_string(),
            dias: vec![Dia::Lun],
            inicio: hora("07:00"),
            fin: hora("09:00"),
            lugar: None,
        });
        datos.tareas.push(tarea_base("t-1", "Uno", "2026-11-28T23:59"));
        datos.tareas.push(Tarea {
            id: "t-2".to_string(),
            titulo: "Dos".to_string(),
            materia: None,
            vence: fecha_hora("2026-11-29T12:00"),
            hecha: true,
            hecha_en: Some(fecha("2026-11-22")),
            origen: Some("moodle:x:assign:1".to_string()),
            enlace: None,
        });
        let mut estado = EstadoSync::nuevo("disp-1");
        registrar_cambios_locales(&datos, &mut estado, 1_000);

        let bytes = estado.mensaje().a_json().unwrap();
        let dec = MensajeSync::desde_json(&bytes).unwrap();
        assert_eq!(dec, estado.mensaje());

        guardar_estado(&ruta, &estado).unwrap();
        let cargado = cargar_estado(&ruta, "disp-1");
        assert_eq!(cargado.crdt, estado.crdt);
        assert_eq!(cargado.foto, estado.foto);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(&ruta).unwrap().permissions().mode() & 0o777;
            assert_eq!(modo, 0o600);
        }
    }

    #[test]
    fn eventos_lww_crear_mover_renombrar_y_borrar() {
        let mut base = datos_vacios();
        base.tareas
            .push(tarea_base("t-1", "Taller 1", "2026-11-28T23:59"));
        base.eventos.push(Evento {
            id: "e-1".to_string(),
            titulo: "Estudiar Control".to_string(),
            fecha: fecha("2026-11-26"),
            inicio: hora("15:00"),
            fin: hora("16:00"),
            tarea: Some("t-1".to_string()),
            nota: None,
        });

        let mut pc = EstadoSync::nuevo("pc");
        registrar_cambios_locales(&base, &mut pc, 1_000);
        let mut cel = fusionar(&EstadoSync::nuevo("cel"), &pc.mensaje()).estado;
        cel.foto = Some(aplicar(&cel));

        // 1. Crear un evento en PC llega a Cel.
        let mut d_pc = aplicar(&pc);
        d_pc.eventos.push(Evento {
            id: "e-2".to_string(),
            titulo: "Repaso laboratorio".to_string(),
            fecha: fecha("2026-11-27"),
            inicio: hora("10:00"),
            fin: hora("11:00"),
            tarea: None,
            nota: Some("Traer guía".to_string()),
        });
        registrar_cambios_locales(&d_pc, &mut pc, 2_000);

        let cel_res = fusionar(&cel, &pc.mensaje());
        assert!(cel_res.cambio);
        cel = cel_res.estado;
        cel.foto = Some(aplicar(&cel));
        assert_eq!(aplicar(&cel).eventos.len(), 2);
        assert_eq!(aplicar(&cel).eventos[1].titulo, "Repaso laboratorio");

        // 2. Mover la hora en PC y cambiar el título en Cel conserva ambos cambios.
        let mut d_pc = aplicar(&pc);
        d_pc.eventos[0].fecha = fecha("2026-11-27");
        d_pc.eventos[0].inicio = hora("17:00");
        d_pc.eventos[0].fin = hora("18:30");
        registrar_cambios_locales(&d_pc, &mut pc, 3_000);

        let mut d_cel = aplicar(&cel);
        d_cel.eventos[0].titulo = "Estudiar Control II".to_string();
        registrar_cambios_locales(&d_cel, &mut cel, 3_100);

        let m_pc = fusionar(&pc, &cel.mensaje());
        let m_cel = fusionar(&cel, &pc.mensaje());
        assert_eq!(m_pc.estado.crdt, m_cel.estado.crdt);
        let fin_datos = aplicar(&m_pc.estado);
        assert_eq!(fin_datos.eventos[0].titulo, "Estudiar Control II");
        assert_eq!(fin_datos.eventos[0].fecha, fecha("2026-11-27"));
        assert_eq!(fin_datos.eventos[0].inicio, hora("17:00"));
        assert_eq!(fin_datos.eventos[0].fin, hora("18:30"));

        // 3. Borrar en uno propaga al otro.
        pc = m_pc.estado;
        pc.foto = Some(aplicar(&pc));
        cel = m_cel.estado;
        cel.foto = Some(aplicar(&cel));

        let mut d_cel_del = aplicar(&cel);
        d_cel_del.eventos.retain(|e| e.id != "e-2");
        registrar_cambios_locales(&d_cel_del, &mut cel, 4_000);

        let pc_tras = fusionar(&pc, &cel.mensaje());
        let datos_tras = aplicar(&pc_tras.estado);
        assert_eq!(datos_tras.eventos.len(), 1);
        assert_eq!(datos_tras.eventos[0].id, "e-1");
    }
}
