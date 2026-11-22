//! ORDER — horario, tareas y recordatorios en un solo binario.

#[allow(dead_code)] // el almacén se consume en las tareas siguientes del plan
mod almacen;
#[allow(dead_code)] // el modelo se consume en las tareas siguientes del plan
mod modelo;
#[allow(dead_code)] // las operaciones se usan desde los formularios (Tarea 7)
mod ops;
#[allow(dead_code)] // la semana la dibuja la ventana (Tarea 6)
mod semana;

fn main() {
    println!("ORDER");
}
