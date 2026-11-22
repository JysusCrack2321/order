//! Protocolo de sincronización y emparejamiento sobre QUIC (`order/sync/1`).

use iroh::endpoint::{RecvStream, SendStream};
use order_core::sincronizacion::MensajeSync;
use serde::{Deserialize, Serialize};

/// Identificador ALPN para conexiones P2P de ORDER.
pub const ALPN: &[u8] = b"order/sync/1";

/// Límite máximo por mensaje en la red (16 MiB).
const MAX_MENSAJE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Peticion {
    Emparejar {
        secreto: String,
        nombre: String,
    },
    Sincronizar {
        nombre: String,
        mensaje: MensajeSync,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Respuesta {
    Emparejado {
        nombre: String,
    },
    Sincronizado {
        nombre: String,
        mensaje: MensajeSync,
        cambio_remoto: bool,
    },
    Rechazado {
        motivo: String,
    },
}

pub async fn enviar_peticion(send: &mut SendStream, peticion: &Peticion) -> Result<(), String> {
    let bytes = serde_json::to_vec(peticion)
        .map_err(|e| format!("no se pudo serializar la petición P2P: {e}"))?;
    send.write_all(&bytes)
        .await
        .map_err(|e| format!("no se pudo enviar la petición P2P: {e}"))?;
    send.finish()
        .map_err(|e| format!("no se pudo cerrar el envío P2P: {e}"))?;
    Ok(())
}

pub async fn leer_peticion(recv: &mut RecvStream) -> Result<Peticion, String> {
    let bytes = recv
        .read_to_end(MAX_MENSAJE_BYTES)
        .await
        .map_err(|e| format!("no se pudo leer la petición P2P: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("petición P2P inválida: {e}"))
}

pub async fn enviar_respuesta(send: &mut SendStream, respuesta: &Respuesta) -> Result<(), String> {
    let bytes = serde_json::to_vec(respuesta)
        .map_err(|e| format!("no se pudo serializar la respuesta P2P: {e}"))?;
    send.write_all(&bytes)
        .await
        .map_err(|e| format!("no se pudo enviar la respuesta P2P: {e}"))?;
    send.finish()
        .map_err(|e| format!("no se pudo cerrar la respuesta P2P: {e}"))?;
    Ok(())
}

pub async fn leer_respuesta(recv: &mut RecvStream) -> Result<Respuesta, String> {
    let bytes = recv
        .read_to_end(MAX_MENSAJE_BYTES)
        .await
        .map_err(|e| format!("no se pudo leer la respuesta P2P: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("respuesta P2P inválida: {e}"))
}
