//! Agente dentro de ORDER: configuración, contexto, acciones y cliente HTTP.
//!
//! El agente nunca toca el archivo por su cuenta: propone acciones, la ventana
//! las muestra como tarjetas y el usuario decide. Todo lo que se aplica pasa
//! por `Almacen::modificar` y las funciones de `ops`.

pub mod acciones;
pub mod cliente;
pub mod config;
pub mod contexto;
pub mod voz;
