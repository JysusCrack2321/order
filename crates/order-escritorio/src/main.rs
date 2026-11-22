//! ORDER — horario, tareas y recordatorios en un solo binario.

mod grabacion;
mod puente;
mod sistema;

use order_core::almacen::Almacen;
use order_core::{conflictos, modelo};

fn main() -> Result<(), slint::PlatformError> {
    // Instancia única: si ya corre ORDER, solo se le pide mostrar la ventana.
    // El socket lleva el nombre de ORDER_ARCHIVO, así una copia de prueba no
    // despierta a la app real.
    if let sistema::Arranque::Otra = sistema::reclamar_instancia() {
        return Ok(());
    }

    let almacen = Almacen::por_defecto();
    cargar(&almacen);

    // Hilos de escritorio: bandeja, observador del archivo y reloj de avisos.
    sistema::arrancar_bandeja();
    sistema::vigilar_archivo(almacen);
    sistema::arrancar_reloj();

    // Arranque oculto: sin ventana, solo el icono. El bucle se queda vivo
    // hasta que se pida "Salir" (continuar aunque no haya ventana visible).
    slint::run_event_loop_until_quit()
}

/// Primera lectura: archiva lo viejo, pinta la ventana (todavía oculta) y
/// publica los datos para el reloj de avisos.
fn cargar(almacen: &Almacen) {
    match almacen.cargar() {
        Ok(datos) => {
            // Una vez al abrir: lo hecho hace más de 30 días pasa a
            // historial.toml para que order.toml siga corto.
            let datos = almacen.archivar().unwrap_or_else(|error| {
                eprintln!("ORDER: no se pudo archivar: {error}");
                datos
            });
            sistema::publicar(&datos);
            puente::refrescar(&datos);
            puente::mostrar_advertencias(&advertencias(almacen));
            puente::mostrar_error(None);
        }
        Err(error) => {
            puente::mostrar_error(Some(&error));
        }
    }
    puente::mostrar_conflictos(conflictos::detectar(&almacen.ruta));
}

/// Las claves que el modelo no conoce; no bloquean la lectura (banner amarillo).
fn advertencias(almacen: &Almacen) -> Vec<modelo::ErrorDatos> {
    std::fs::read_to_string(&almacen.ruta)
        .map(|texto| modelo::advertencias(&texto))
        .unwrap_or_default()
}
