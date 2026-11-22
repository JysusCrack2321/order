//! Reloj lógico híbrido (HLC) para ordenar ediciones entre dispositivos.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Marca causal de un cambio: tiempo físico (ms desde epoch), contador lógico
/// para empates en el mismo milisegundo y el identificador del dispositivo para
/// un orden total determinista.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Marca {
    pub tiempo_ms: u64,
    pub contador: u32,
    pub dispositivo: String,
}

impl Marca {
    /// Marca mínima anterior a cualquier edición real.
    pub fn cero() -> Self {
        Marca {
            tiempo_ms: 0,
            contador: 0,
            dispositivo: String::new(),
        }
    }
}

/// Estado del reloj HLC local de un dispositivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelojHlc {
    pub dispositivo: String,
    pub ultima: Marca,
}

impl RelojHlc {
    pub fn nuevo(dispositivo: impl Into<String>) -> Self {
        let dispositivo = dispositivo.into();
        RelojHlc {
            ultima: Marca {
                tiempo_ms: 0,
                contador: 0,
                dispositivo: dispositivo.clone(),
            },
            dispositivo,
        }
    }

    /// Avanza el reloj ante una edición local en `ahora_ms`.
    pub fn tick(&mut self, ahora_ms: u64) -> Marca {
        if ahora_ms > self.ultima.tiempo_ms {
            self.ultima.tiempo_ms = ahora_ms;
            self.ultima.contador = 0;
        } else {
            self.ultima.contador = self.ultima.contador.saturating_add(1);
        }
        self.ultima.dispositivo = self.dispositivo.clone();
        self.ultima.clone()
    }

    /// Actualiza la cota del reloj al recibir una marca remota sin necesitar
    /// tiempo físico nuevo; el siguiente `tick` superará ambas marcas.
    pub fn observar(&mut self, remota: &Marca) {
        if (remota.tiempo_ms, remota.contador) > (self.ultima.tiempo_ms, self.ultima.contador) {
            self.ultima.tiempo_ms = remota.tiempo_ms;
            self.ultima.contador = remota.contador;
        }
    }

    /// Avanza el reloj ante la recepción de una marca remota en `ahora_ms`.
    pub fn recibir(&mut self, remota: &Marca, ahora_ms: u64) -> Marca {
        let max_t = ahora_ms
            .max(self.ultima.tiempo_ms)
            .max(remota.tiempo_ms);
        if max_t == self.ultima.tiempo_ms && max_t == remota.tiempo_ms {
            self.ultima.contador = self
                .ultima
                .contador
                .max(remota.contador)
                .saturating_add(1);
        } else if max_t == self.ultima.tiempo_ms {
            self.ultima.contador = self.ultima.contador.saturating_add(1);
        } else if max_t == remota.tiempo_ms {
            self.ultima.tiempo_ms = remota.tiempo_ms;
            self.ultima.contador = remota.contador.saturating_add(1);
        } else {
            self.ultima.tiempo_ms = ahora_ms;
            self.ultima.contador = 0;
        }
        self.ultima.dispositivo = self.dispositivo.clone();
        self.ultima.clone()
    }
}

/// Tiempo físico actual en milisegundos desde UNIX_EPOCH.
pub fn ahora_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}
