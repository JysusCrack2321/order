//! ORDER — horario, tareas y recordatorios en un solo binario.

mod almacen;
#[allow(dead_code)] // los avisos los envía el reloj (Tarea 8)
mod avisos;
#[allow(dead_code)] // las constantes de formato también las usan las pruebas
mod modelo;
#[allow(dead_code)] // las operaciones se usan desde los formularios (Tarea 7)
mod ops;
mod puente;
mod semana;

use almacen::Almacen;

fn main() -> Result<(), slint::PlatformError> {
    let almacen = Almacen::por_defecto();

    match almacen.cargar() {
        Ok(datos) => {
            // Una vez al abrir: lo hecho hace más de 30 días pasa a
            // historial.toml para que order.toml siga corto.
            let datos = almacen.archivar().unwrap_or_else(|error| {
                eprintln!("ORDER: no se pudo archivar: {error}");
                datos
            });
            // Las claves desconocidas no bloquean: el archivo se carga y solo
            // se avisa en amarillo. El texto se relee aparte porque `cargar`
            // devuelve los datos ya interpretados.
            let advertencias = std::fs::read_to_string(&almacen.ruta)
                .map(|texto| modelo::advertencias(&texto))
                .unwrap_or_default();
            puente::refrescar(&datos);
            puente::mostrar_advertencias(&advertencias);
            puente::mostrar_error(None);
        }
        Err(error) => {
            puente::mostrar_error(Some(&error));
        }
    }

    // En esta tarea la ventana se abre al arrancar; el arranque oculto con
    // bandeja llega en la Tarea 8.
    puente::mostrar_ventana();
    slint::run_event_loop()
}
