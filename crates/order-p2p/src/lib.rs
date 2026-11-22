//! Transporte P2P cifrado de extremo a extremo con `iroh` (`order/sync/1`).
//!
//! Expone una API bloqueante (`ServicioSync`) respaldada por un runtime `tokio`
//! interno para que ni `order-core`, ni `order-ffi`, ni `order-escritorio`
//! tengan que lidiar con async.

pub mod config;
pub mod emparejamiento;
pub mod protocolo;

pub use config::{
    DispositivoEmparejado, InfoDispositivo, SyncConfig, ruta_por_defecto, ruta_sync_estado,
    ruta_sync_toml,
};
pub use emparejamiento::{
    CodigoEmparejamiento, ERROR_CODIGO_INVALIDO, TTL_EMPAREJAMIENTO, codificar_palabras,
    decodificar_palabras, generar_qr_matriz, generar_qr_svg, parsear_enlace,
};
pub use protocolo::ALPN;

use config::{ahora_local, cargar_o_crear, guardar};
use emparejamiento::{InvitacionActiva, coincide_prefijo, generar_codigo, hex_4};
use iroh::address_lookup::MemoryLookup;
use iroh::endpoint::{Connection, RelayMode, presets};
use iroh::{Endpoint, EndpointAddr, EndpointId};
use iroh_mdns_address_lookup::{DiscoveryEvent, MdnsAddressLookup};
use order_core::almacen::Almacen;
use order_core::sincronizacion::{
    EstadoSync, MensajeSync, ahora_ms, aplicar, aplicar_en_doc, cargar_estado, datos_equivalentes,
    fusionar, guardar_estado, registrar_cambios_locales,
};
use protocolo::{Peticion, Respuesta, enviar_peticion, enviar_respuesta, leer_peticion, leer_respuesta};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

const TIEMPO_CONEXION: Duration = Duration::from_secs(12);
const TIEMPO_SESION: Duration = Duration::from_secs(20);

/// Cómo configurar el endpoint `iroh`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpcionesRed {
    /// Uso normal: relays públicos de n0 (para conectar entre redes) + mDNS local.
    Publica,
    /// Pruebas locales: sin relays, sin DNS externo.
    SoloLocal,
}

/// Resultado de una ronda de sincronización con todos los pares emparejados.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResumenP2p {
    pub intentados: u32,
    pub exitosos: u32,
    pub cambios_locales: bool,
    pub errores: Vec<String>,
}

/// Estado visible del servicio de sincronización.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstadoServicio {
    pub nodo_id: String,
    pub nombre: String,
    pub sincronizando: bool,
    pub secuencia_cambio: u64,
    pub total_emparejados: usize,
    pub texto_estado: String,
}

struct Contexto {
    ruta_datos: PathBuf,
    ruta_config: PathBuf,
    ruta_estado: PathBuf,
    config: Mutex<SyncConfig>,
    disco_lock: Mutex<()>,
    invitacion: Mutex<Option<InvitacionActiva>>,
    ttl_emparejamiento: Duration,
    sincronizando: AtomicBool,
    secuencia_cambio: AtomicU64,
    al_cambiar: Option<Arc<dyn Fn() + Send + Sync>>,
    pares_mdns: RwLock<HashMap<EndpointId, EndpointAddr>>,
    memoria_lookup: MemoryLookup,
}

/// Servicio P2P activo con runtime Tokio interno y API bloqueante.
pub struct ServicioSync {
    runtime: Option<Runtime>,
    endpoint: Endpoint,
    ctx: Arc<Contexto>,
    aceptador: Mutex<Option<JoinHandle<()>>>,
    mdns_tarea: Mutex<Option<JoinHandle<()>>>,
}

impl ServicioSync {
    /// Arranca el nodo P2P con la configuración normal (relays n0 + mDNS local).
    pub fn iniciar(
        ruta_datos: PathBuf,
        ruta_config: PathBuf,
        nombre_inicial: Option<String>,
        al_cambiar: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, String> {
        Self::iniciar_con_opciones(
            ruta_datos,
            ruta_config,
            nombre_inicial,
            al_cambiar,
            OpcionesRed::Publica,
            TTL_EMPAREJAMIENTO,
        )
    }

    /// Arranca el nodo P2P con opciones explícitas (útil para pruebas locales).
    pub fn iniciar_con_opciones(
        ruta_datos: PathBuf,
        ruta_config: PathBuf,
        nombre_inicial: Option<String>,
        al_cambiar: Option<Arc<dyn Fn() + Send + Sync>>,
        opciones: OpcionesRed,
        ttl_emparejamiento: Duration,
    ) -> Result<Self, String> {
        let ruta_cfg = config::ruta_sync_toml(&ruta_config);
        let ruta_est = config::ruta_sync_estado(&ruta_config);
        let cfg = cargar_o_crear(&ruta_cfg, nombre_inicial.as_deref())?;
        let secret_key = cfg.secret_key()?;

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .thread_name("order-p2p")
            .build()
            .map_err(|e| format!("no se pudo iniciar el motor de red P2P: {e}"))?;

        let memoria_lookup = MemoryLookup::new();
        let (endpoint, mdns) = runtime.block_on(async {
            let mut builder = match opciones {
                OpcionesRed::Publica => Endpoint::builder(presets::N0),
                OpcionesRed::SoloLocal => {
                    Endpoint::builder(presets::Minimal).relay_mode(RelayMode::Disabled)
                }
            };
            builder = builder
                .secret_key(secret_key)
                .alpns(vec![ALPN.to_vec()])
                .address_lookup(memoria_lookup.clone());

            let ep = builder
                .bind()
                .await
                .map_err(|e| format!("no se pudo abrir el puerto P2P: {e}"))?;

            let mdns = if opciones == OpcionesRed::Publica {
                match MdnsAddressLookup::builder().build(ep.id()) {
                    Ok(m) => {
                        if let Ok(al) = ep.address_lookup() {
                            al.add(m.clone());
                        }
                        Some(m)
                    }
                    Err(_) => None,
                }
            } else {
                None
            };

            Ok::<_, String>((ep, mdns))
        })?;

        let ctx = Arc::new(Contexto {
            ruta_datos,
            ruta_config: ruta_cfg,
            ruta_estado: ruta_est,
            config: Mutex::new(cfg),
            disco_lock: Mutex::new(()),
            invitacion: Mutex::new(None),
            ttl_emparejamiento,
            sincronizando: AtomicBool::new(false),
            secuencia_cambio: AtomicU64::new(0),
            al_cambiar,
            pares_mdns: RwLock::new(HashMap::new()),
            memoria_lookup,
        });

        let mdns_tarea = mdns.map(|m| {
            let ctx_m = Arc::clone(&ctx);
            runtime.spawn(async move {
                let mut eventos = m.subscribe().await;
                while let Some(ev) = recv_discovery(&mut eventos).await {
                    match ev {
                        DiscoveryEvent::Discovered { endpoint_info, .. } => {
                            let id = endpoint_info.endpoint_id;
                            let addr = endpoint_info.to_endpoint_addr();
                            ctx_m.memoria_lookup.add_endpoint_info(endpoint_info);
                            if let Ok(mut mapa) = ctx_m.pares_mdns.write() {
                                mapa.insert(id, addr);
                            }
                        }
                        DiscoveryEvent::Expired { endpoint_id } => {
                            if let Ok(mut mapa) = ctx_m.pares_mdns.write() {
                                mapa.remove(&endpoint_id);
                            }
                        }
                        _ => {}
                    }
                }
            })
        });

        let ep_accept = endpoint.clone();
        let ctx_accept = Arc::clone(&ctx);
        let aceptador = runtime.spawn(async move {
            while let Some(incoming) = ep_accept.accept().await {
                let ctx_c = Arc::clone(&ctx_accept);
                tokio::spawn(async move {
                    if let Ok(conn) = incoming.await {
                        manejar_conexion_entrante(ctx_c, conn).await;
                    }
                });
            }
        });

        Ok(ServicioSync {
            runtime: Some(runtime),
            endpoint,
            ctx,
            aceptador: Mutex::new(Some(aceptador)),
            mdns_tarea: Mutex::new(mdns_tarea),
        })
    }

    pub fn nodo_id(&self) -> String {
        self.endpoint.id().to_string()
    }

    pub fn direccion_local(&self) -> EndpointAddr {
        self.endpoint.addr()
    }

    /// Registra manualmente la dirección de otro nodo (útil en pruebas locales sin DNS/relay).
    pub fn registrar_direccion(&self, addr: EndpointAddr) {
        self.ctx.memoria_lookup.add_endpoint_info(addr);
    }

    pub fn nombre(&self) -> String {
        self.ctx
            .config
            .lock()
            .map(|c| c.nombre_dispositivo.clone())
            .unwrap_or_default()
    }

    pub fn cambiar_nombre(&self, nombre: &str) -> Result<(), String> {
        let limpio = nombre.trim();
        if limpio.is_empty() {
            return Err("El nombre del dispositivo no puede estar vacío.".to_string());
        }
        let mut cfg = self
            .ctx
            .config
            .lock()
            .map_err(|_| "estado de configuración bloqueado".to_string())?;
        cfg.nombre_dispositivo = limpio.to_string();
        guardar(&self.ctx.ruta_config, &cfg)
    }

    /// Genera un nuevo código/QR de emparejamiento de un solo uso válido 10 minutos.
    pub fn codigo_emparejamiento(&self) -> Result<CodigoEmparejamiento, String> {
        let invitacion = InvitacionActiva::nueva();
        let secreto = invitacion.secreto;
        if let Ok(mut guard) = self.ctx.invitacion.lock() {
            *guard = Some(invitacion);
        }
        let nombre = self.nombre();
        generar_codigo(&self.endpoint.id(), &nombre, secreto)
    }

    /// Empareja este dispositivo con otro usando un enlace `order://emparejar?...`
    /// o un código de 8 palabras (en la misma red Wi-Fi).
    pub fn emparejar(&self, enlace_o_palabras: &str) -> Result<DispositivoEmparejado, String> {
        let entrada = enlace_o_palabras.trim();
        if entrada.is_empty() {
            return Err("Escribe el código de 8 palabras o pega el enlace de emparejamiento.".to_string());
        }

        let (nodo_id, secreto_bytes, nombre_pista) = if entrada.starts_with("order://") {
            let datos = parsear_enlace(entrada)?;
            (datos.nodo, datos.secreto, datos.nombre)
        } else {
            let (prefijo, secreto) = decodificar_palabras(entrada)?;
            let nodo = self.buscar_nodo_local_por_prefijo(prefijo)?;
            (nodo, secreto, None)
        };

        if nodo_id == self.endpoint.id() {
            return Err("No puedes emparejar este dispositivo consigo mismo.".to_string());
        }

        let nombre_local = self.nombre();
        let secreto_hex = hex_4(secreto_bytes);
        let endpoint = self.endpoint.clone();
        let ctx = Arc::clone(&self.ctx);
        let rt = self
            .runtime
            .as_ref()
            .ok_or_else(|| "servicio P2P detenido".to_string())?;

        let emparejado = rt.block_on(async move {
            let conn = tokio::time::timeout(TIEMPO_CONEXION, endpoint.connect(nodo_id, ALPN))
                .await
                .map_err(|_| "No se pudo conectar con el otro dispositivo a tiempo.".to_string())?
                .map_err(|e| format!("No se pudo conectar con el otro dispositivo: {e}"))?;

            let (mut send, mut recv) = conn
                .open_bi()
                .await
                .map_err(|e| format!("No se pudo abrir canal de emparejamiento: {e}"))?;

            enviar_peticion(
                &mut send,
                &Peticion::Emparejar {
                    secreto: secreto_hex,
                    nombre: nombre_local,
                },
            )
            .await?;

    // Si el servidor cerró la conexión durante el emparejamiento (p. ej. no había
    // invitación vigente o caducó), informamos el mensaje claro del plan.
    let resp = tokio::time::timeout(TIEMPO_SESION, leer_respuesta(&mut recv))
        .await
        .map_err(|_| "El otro dispositivo no respondió al emparejamiento.".to_string())?
        .map_err(|_| ERROR_CODIGO_INVALIDO.to_string())?;

            conn.close(0u32.into(), b"ok");

            match resp {
                Respuesta::Emparejado { nombre } => {
                    let nombre_final = if !nombre.trim().is_empty() {
                        nombre
                    } else {
                        nombre_pista.unwrap_or_else(|| "Dispositivo".to_string())
                    };
                    let mut cfg = ctx
                        .config
                        .lock()
                        .map_err(|_| "estado de configuración bloqueado".to_string())?;
                    cfg.registrar_emparejado(nodo_id.to_string(), nombre_final, ahora_local());
                    guardar(&ctx.ruta_config, &cfg)?;
                    let registro = cfg
                        .emparejados
                        .iter()
                        .find(|d| d.id == nodo_id.to_string())
                        .cloned()
                        .ok_or_else(|| "no se pudo guardar el emparejamiento".to_string())?;
                    Ok(registro)
                }
                Respuesta::Rechazado { motivo } => Err(motivo),
                Respuesta::Sincronizado { .. } => Err(ERROR_CODIGO_INVALIDO.to_string()),
            }
        })?;

        // Primera sincronización inmediata tras emparejar (si falla no revierte el emparejamiento).
        let _ = self.sincronizar_uno(&emparejado.id);
        Ok(emparejado)
    }

    fn buscar_nodo_local_por_prefijo(&self, prefijo: [u8; 4]) -> Result<EndpointId, String> {
        let rt = self
            .runtime
            .as_ref()
            .ok_or_else(|| "servicio P2P detenido".to_string())?;
        // Si mDNS acaba de arrancar, esperamos brevemente a que lleguen anuncios locales.
        rt.block_on(async {
            for _ in 0..8 {
                if let Ok(mapa) = self.ctx.pares_mdns.read() {
                    if let Some(id) = mapa.keys().find(|id| coincide_prefijo(id, prefijo)) {
                        return Ok(*id);
                    }
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err("No se encontró ese dispositivo en la red local. Si están en redes distintas, usa el QR o el enlace completo.".to_string())
        })
    }

    pub fn dispositivos(&self) -> Vec<InfoDispositivo> {
        let ahora = ahora_local();
        self.ctx
            .config
            .lock()
            .map(|cfg| cfg.emparejados.iter().map(|d| d.a_info(ahora)).collect())
            .unwrap_or_default()
    }

    pub fn quitar(&self, id: &str) -> Result<(), String> {
        let mut cfg = self
            .ctx
            .config
            .lock()
            .map_err(|_| "estado de configuración bloqueado".to_string())?;
        let antes = cfg.emparejados.len();
        cfg.emparejados.retain(|d| d.id != id);
        if cfg.emparejados.len() != antes {
            guardar(&self.ctx.ruta_config, &cfg)?;
        }
        Ok(())
    }

    /// Registra cualquier cambio pendiente de `order.toml` en `sync-estado.json`.
    pub fn capturar_cambios_locales(&self) -> Result<bool, String> {
        let _guard = self
            .ctx
            .disco_lock
            .lock()
            .map_err(|_| "estado de disco bloqueado".to_string())?;
        let almacen = Almacen {
            ruta: self.ctx.ruta_datos.clone(),
        };
        let datos = almacen.cargar().map_err(|e| e.to_string())?;
        let nodo_id = self.endpoint.id().to_string();
        let mut estado = cargar_estado(&self.ctx.ruta_estado, &nodo_id);
        let hubo = registrar_cambios_locales(&datos, &mut estado, ahora_ms());
        estado.purgar_lapidas(ahora_ms());
        if hubo || !self.ctx.ruta_estado.exists() {
            guardar_estado(&self.ctx.ruta_estado, &estado)?;
        }
        Ok(hubo)
    }

    /// Sincroniza con todos los dispositivos emparejados.
    pub fn sincronizar_ahora(&self) -> ResumenP2p {
        let ids: Vec<String> = self
            .ctx
            .config
            .lock()
            .map(|c| c.emparejados.iter().map(|d| d.id.clone()).collect())
            .unwrap_or_default();

        if ids.is_empty() {
            return ResumenP2p::default();
        }

        self.ctx.sincronizando.store(true, Ordering::SeqCst);
        let _ = self.capturar_cambios_locales();

        let mut resumen = ResumenP2p::default();
        for id in ids {
            resumen.intentados += 1;
            match self.sincronizar_uno(&id) {
                Ok(cambio) => {
                    resumen.exitosos += 1;
                    resumen.cambios_locales |= cambio;
                }
                Err(err) => {
                    resumen.errores.push(err);
                }
            }
        }
        self.ctx.sincronizando.store(false, Ordering::SeqCst);
        resumen
    }

    fn sincronizar_uno(&self, id_str: &str) -> Result<bool, String> {
        let peer_id = EndpointId::from_str(id_str)
            .map_err(|_| format!("identificador de dispositivo inválido: {id_str}"))?;
        let rt = self
            .runtime
            .as_ref()
            .ok_or_else(|| "servicio P2P detenido".to_string())?;

        let mensaje_local = preparar_mensaje_local(&self.ctx, &self.endpoint.id().to_string())?;
        let nombre_local = self.nombre();
        let endpoint = self.endpoint.clone();

        let resultado = rt.block_on(async move {
            let conn = tokio::time::timeout(TIEMPO_CONEXION, endpoint.connect(peer_id, ALPN))
                .await
                .map_err(|_| "sin conexión".to_string())?
                .map_err(|_| "sin conexión".to_string())?;

            let (mut send, mut recv) = conn
                .open_bi()
                .await
                .map_err(|e| format!("error de canal P2P: {e}"))?;

            enviar_peticion(
                &mut send,
                &Peticion::Sincronizar {
                    nombre: nombre_local,
                    mensaje: mensaje_local,
                },
            )
            .await?;

            let resp = tokio::time::timeout(TIEMPO_SESION, leer_respuesta(&mut recv))
                .await
                .map_err(|_| "sin respuesta del otro dispositivo".to_string())??;

            conn.close(0u32.into(), b"ok");
            Ok::<_, String>(resp)
        });

        match resultado {
            Ok(Respuesta::Sincronizado {
                nombre, mensaje, ..
            }) => {
                let cambio_disco =
                    aplicar_mensaje_remoto(&self.ctx, &self.endpoint.id().to_string(), &mensaje)?;
                if let Ok(mut cfg) = self.ctx.config.lock() {
                    cfg.anotar_exito(id_str, Some(&nombre), ahora_local());
                    let _ = guardar(&self.ctx.ruta_config, &cfg);
                }
                if cambio_disco {
                    self.ctx.secuencia_cambio.fetch_add(1, Ordering::SeqCst);
                    if let Some(cb) = &self.ctx.al_cambiar {
                        cb();
                    }
                }
                Ok(cambio_disco)
            }
            Ok(Respuesta::Rechazado { motivo }) => {
                if let Ok(mut cfg) = self.ctx.config.lock() {
                    cfg.anotar_error(id_str, &motivo);
                    let _ = guardar(&self.ctx.ruta_config, &cfg);
                }
                Err(motivo)
            }
            Ok(Respuesta::Emparejado { .. }) => Err("respuesta inesperada del par".to_string()),
            Err(err) => {
                if let Ok(mut cfg) = self.ctx.config.lock() {
                    cfg.anotar_error(id_str, &err);
                    let _ = guardar(&self.ctx.ruta_config, &cfg);
                }
                Err(err)
            }
        }
    }

    pub fn estado(&self) -> EstadoServicio {
        let sincronizando = self.ctx.sincronizando.load(Ordering::SeqCst);
        let secuencia_cambio = self.ctx.secuencia_cambio.load(Ordering::SeqCst);
        let (nombre, total_emparejados) = self
            .ctx
            .config
            .lock()
            .map(|c| (c.nombre_dispositivo.clone(), c.emparejados.len()))
            .unwrap_or_default();

        let texto_estado = if sincronizando {
            "Sincronizando…".to_string()
        } else if total_emparejados == 0 {
            "Sin dispositivos emparejados".to_string()
        } else {
            let lista = self.dispositivos();
            let en_linea = lista.iter().filter(|d| d.en_linea).count();
            if en_linea > 0 {
                format!("{total_emparejados} emparejado(s)")
            } else {
                "Sin conexión con los otros dispositivos".to_string()
            }
        };

        EstadoServicio {
            nodo_id: self.nodo_id(),
            nombre,
            sincronizando,
            secuencia_cambio,
            total_emparejados,
            texto_estado,
        }
    }

    pub fn secuencia_cambio(&self) -> u64 {
        self.ctx.secuencia_cambio.load(Ordering::SeqCst)
    }

    pub fn detener(mut self) {
        self.detener_interno();
    }

    fn detener_interno(&mut self) {
        if let Ok(mut guard) = self.aceptador.lock() {
            if let Some(h) = guard.take() {
                h.abort();
            }
        }
        if let Ok(mut guard) = self.mdns_tarea.lock() {
            if let Some(h) = guard.take() {
                h.abort();
            }
        }
        if let Some(rt) = self.runtime.take() {
            let ep = self.endpoint.clone();
            rt.block_on(async move {
                ep.close().await;
            });
            rt.shutdown_timeout(Duration::from_secs(2));
        }
    }
}

impl Drop for ServicioSync {
    fn drop(&mut self) {
        self.detener_interno();
    }
}

async fn recv_discovery(
    stream: &mut (impl futures_core::Stream<Item = DiscoveryEvent> + Unpin),
) -> Option<DiscoveryEvent> {
    use std::pin::Pin;
    std::future::poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx)).await
}

async fn manejar_conexion_entrante(ctx: Arc<Contexto>, conn: Connection) {
    let remote_id = conn.remote_id();
    let remote_str = remote_id.to_string();
    let emparejado = ctx
        .config
        .lock()
        .map(|c| c.esta_emparejado(&remote_str))
        .unwrap_or(false);
    let hay_invitacion = ctx
        .invitacion
        .lock()
        .map(|inv| inv.is_some())
        .unwrap_or(false);

    // Solo aceptamos nodos emparejados (o intentos de emparejamiento cuando
    // alguna vez se abrió una invitación, para poder responder el rechazo claro).
    if !emparejado && !hay_invitacion {
        conn.close(403u32.into(), b"no emparejado");
        return;
    }

    let Ok((mut send, mut recv)) = conn.accept_bi().await else {
        return;
    };
    let Ok(Ok(peticion)) = tokio::time::timeout(TIEMPO_SESION, leer_peticion(&mut recv)).await else {
        conn.close(400u32.into(), b"peticion invalida");
        return;
    };

    match peticion {
        Peticion::Emparejar { secreto, nombre } => {
            let valido = {
                let mut guard = match ctx.invitacion.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                match guard.as_mut() {
                    Some(inv)
                        if inv.vigente(ctx.ttl_emparejamiento)
                            && inv.secreto_hex() == secreto.trim().to_lowercase() =>
                    {
                        inv.usada = true;
                        true
                    }
                    _ => false,
                }
            };

            if !valido {
                let _ = enviar_respuesta(
                    &mut send,
                    &Respuesta::Rechazado {
                        motivo: ERROR_CODIGO_INVALIDO.to_string(),
                    },
                )
                .await;
                let _ = conn.closed().await;
                return;
            }

            let nombre_local = match ctx.config.lock() {
                Ok(mut cfg) => {
                    cfg.registrar_emparejado(remote_str, nombre, ahora_local());
                    let _ = guardar(&ctx.ruta_config, &cfg);
                    cfg.nombre_dispositivo.clone()
                }
                Err(_) => return,
            };

            let _ = enviar_respuesta(
                &mut send,
                &Respuesta::Emparejado {
                    nombre: nombre_local,
                },
            )
            .await;
            let _ = conn.closed().await;
        }
        Peticion::Sincronizar { nombre, mensaje } => {
            if !emparejado {
                let _ = enviar_respuesta(
                    &mut send,
                    &Respuesta::Rechazado {
                        motivo: "Dispositivo no emparejado.".to_string(),
                    },
                )
                .await;
                let _ = conn.closed().await;
                return;
            }

            let local_id = match ctx.config.lock() {
                Ok(c) => c
                    .secret_key()
                    .map(|k| k.public().to_string())
                    .unwrap_or_default(),
                Err(_) => return,
            };

            let respuesta = {
                ctx.sincronizando.store(true, Ordering::SeqCst);
                let res = procesar_sync_entrante(&ctx, &local_id, &mensaje);
                ctx.sincronizando.store(false, Ordering::SeqCst);
                res
            };

            match respuesta {
                Ok((mi_mensaje, cambio_disco)) => {
                    let nombre_local = if let Ok(mut cfg) = ctx.config.lock() {
                        cfg.anotar_exito(&remote_str, Some(&nombre), ahora_local());
                        let _ = guardar(&ctx.ruta_config, &cfg);
                        cfg.nombre_dispositivo.clone()
                    } else {
                        "Este dispositivo".to_string()
                    };
                    let _ = enviar_respuesta(
                        &mut send,
                        &Respuesta::Sincronizado {
                            nombre: nombre_local,
                            mensaje: mi_mensaje,
                            cambio_remoto: cambio_disco,
                        },
                    )
                    .await;
                    let _ = conn.closed().await;
                    if cambio_disco {
                        ctx.secuencia_cambio.fetch_add(1, Ordering::SeqCst);
                        if let Some(cb) = &ctx.al_cambiar {
                            cb();
                        }
                    }
                }
                Err(err) => {
                    let _ = enviar_respuesta(&mut send, &Respuesta::Rechazado { motivo: err }).await;
                    let _ = conn.closed().await;
                }
            }
        }
    }
}

fn preparar_mensaje_local(ctx: &Contexto, nodo_id: &str) -> Result<MensajeSync, String> {
    let _guard = ctx
        .disco_lock
        .lock()
        .map_err(|_| "estado de disco bloqueado".to_string())?;
    let almacen = Almacen {
        ruta: ctx.ruta_datos.clone(),
    };
    let datos = almacen.cargar().map_err(|e| e.to_string())?;
    let mut estado = cargar_estado(&ctx.ruta_estado, nodo_id);
    registrar_cambios_locales(&datos, &mut estado, ahora_ms());
    estado.purgar_lapidas(ahora_ms());
    guardar_estado(&ctx.ruta_estado, &estado)?;
    Ok(estado.mensaje())
}

fn aplicar_mensaje_remoto(ctx: &Contexto, nodo_id: &str, remoto: &MensajeSync) -> Result<bool, String> {
    let _guard = ctx
        .disco_lock
        .lock()
        .map_err(|_| "estado de disco bloqueado".to_string())?;
    let almacen = Almacen {
        ruta: ctx.ruta_datos.clone(),
    };
    let datos_actuales = almacen.cargar().map_err(|e| e.to_string())?;
    let mut estado: EstadoSync = cargar_estado(&ctx.ruta_estado, nodo_id);
    registrar_cambios_locales(&datos_actuales, &mut estado, ahora_ms());

    let mut res = fusionar(&estado, remoto);
    let mut cambio_disco = false;
    if res.cambio {
        let nuevos = aplicar(&res.estado);
        if !datos_equivalentes(&datos_actuales, &nuevos) {
            let escritos = almacen
                .modificar(|doc| aplicar_en_doc(doc, &nuevos))
                .map_err(|e| e.to_string())?;
            res.estado.foto = Some(escritos);
            cambio_disco = true;
        }
    }
    res.estado.purgar_lapidas(ahora_ms());
    guardar_estado(&ctx.ruta_estado, &res.estado)?;
    Ok(cambio_disco)
}

fn procesar_sync_entrante(
    ctx: &Contexto,
    nodo_id: &str,
    remoto: &MensajeSync,
) -> Result<(MensajeSync, bool), String> {
    let _guard = ctx
        .disco_lock
        .lock()
        .map_err(|_| "estado de disco bloqueado".to_string())?;
    let almacen = Almacen {
        ruta: ctx.ruta_datos.clone(),
    };
    let datos_actuales = almacen.cargar().map_err(|e| e.to_string())?;
    let mut estado = cargar_estado(&ctx.ruta_estado, nodo_id);
    registrar_cambios_locales(&datos_actuales, &mut estado, ahora_ms());

    let mut res = fusionar(&estado, remoto);
    let mut cambio_disco = false;
    if res.cambio {
        let nuevos = aplicar(&res.estado);
        if !datos_equivalentes(&datos_actuales, &nuevos) {
            let escritos = almacen
                .modificar(|doc| aplicar_en_doc(doc, &nuevos))
                .map_err(|e| e.to_string())?;
            res.estado.foto = Some(escritos);
            cambio_disco = true;
        }
    }
    res.estado.purgar_lapidas(ahora_ms());
    guardar_estado(&ctx.ruta_estado, &res.estado)?;
    Ok((res.estado.mensaje(), cambio_disco))
}

/// Comprueba si la ruta de datos parece pertenecer al ejemplo inicial (útil solo
/// internamente si alguna vez se necesita distinguir).
#[allow(dead_code)]
fn _archivo_existe(ruta: &Path) -> bool {
    ruta.exists()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use order_core::modelo::{self, FORMATO_FECHA_HORA, Tarea};
    use order_core::ops::{self, NuevaTarea};
    use chrono::NaiveDateTime;
    use std::sync::atomic::AtomicUsize;

    fn crear_servicio(
        dir: &Path,
        nombre: &str,
        ttl: Duration,
        cb: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> ServicioSync {
        let datos = dir.join("order.toml");
        let cfg = dir.join("sync.toml");
        ServicioSync::iniciar_con_opciones(
            datos,
            cfg,
            Some(nombre.to_string()),
            cb,
            OpcionesRed::SoloLocal,
            ttl,
        )
        .expect("servicio local debe arrancar")
    }

    #[test]
    fn emparejar_sincronizar_y_rechazar_no_emparejados() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let dir_intruso = tempfile::tempdir().unwrap();

        let cambios_a = Arc::new(AtomicUsize::new(0));
        let cambios_b = Arc::new(AtomicUsize::new(0));
        let ca = Arc::clone(&cambios_a);
        let cb = Arc::clone(&cambios_b);

        let a = crear_servicio(
            dir_a.path(),
            "PC",
            TTL_EMPAREJAMIENTO,
            Some(Arc::new(move || {
                ca.fetch_add(1, Ordering::SeqCst);
            })),
        );
        let b = crear_servicio(
            dir_b.path(),
            "Celular",
            TTL_EMPAREJAMIENTO,
            Some(Arc::new(move || {
                cb.fetch_add(1, Ordering::SeqCst);
            })),
        );
        let intruso = crear_servicio(dir_intruso.path(), "Intruso", TTL_EMPAREJAMIENTO, None);

        // En pruebas locales sin DNS/relay, cada nodo conoce la dirección local del otro.
        b.registrar_direccion(a.direccion_local());
        a.registrar_direccion(b.direccion_local());
        intruso.registrar_direccion(a.direccion_local());

        // Intruso no emparejado intenta sincronizar sin haber código activo -> rechazado.
        let mut cfg_intruso = cargar_o_crear(&dir_intruso.path().join("sync.toml"), None).unwrap();
        cfg_intruso.registrar_emparejado(a.nodo_id(), "PC".to_string(), ahora_local());
        guardar(&dir_intruso.path().join("sync.toml"), &cfg_intruso).unwrap();
        // Reiniciamos intruso para que cargue ese sync.toml falso en memoria.
        drop(intruso);
        let intruso = crear_servicio(dir_intruso.path(), "Intruso", TTL_EMPAREJAMIENTO, None);
        intruso.registrar_direccion(a.direccion_local());
        let res_intruso = intruso.sincronizar_ahora();
        assert_eq!(res_intruso.exitosos, 0, "el nodo no emparejado debe ser rechazado");

        // Preparamos tareas distintas en A y B antes de emparejar.
        let alm_a = Almacen {
            ruta: dir_a.path().join("order.toml"),
        };
        let alm_b = Almacen {
            ruta: dir_b.path().join("order.toml"),
        };
        alm_a.cargar().unwrap();
        alm_b.cargar().unwrap();
        alm_a
            .modificar(|doc| {
                doc.remove("tarea");
                doc.remove("clase");
                doc.remove("recordatorio");
                ops::agregar_tarea(
                    doc,
                    &NuevaTarea {
                        titulo: "Tarea PC".to_string(),
                        materia: Some("Control".to_string()),
                        vence: NaiveDateTime::parse_from_str("2026-11-28T23:59", FORMATO_FECHA_HORA)
                            .unwrap(),
                    },
                )?;
                Ok(())
            })
            .unwrap();
        alm_b
            .modificar(|doc| {
                doc.remove("tarea");
                doc.remove("clase");
                doc.remove("recordatorio");
                ops::agregar_tarea(
                    doc,
                    &NuevaTarea {
                        titulo: "Tarea Cel".to_string(),
                        materia: Some("Robótica".to_string()),
                        vence: NaiveDateTime::parse_from_str("2026-11-29T18:00", FORMATO_FECHA_HORA)
                            .unwrap(),
                    },
                )?;
                Ok(())
            })
            .unwrap();

        // A genera código y B se empareja.
        let codigo = a.codigo_emparejamiento().unwrap();
        assert!(codigo.enlace.starts_with("order://emparejar?n="));
        assert!(codigo.qr_svg.contains("<svg"));
        assert_eq!(codigo.palabras.split_whitespace().count(), 8);

        // Un secreto equivocado es rechazado con el mensaje esperado.
        let enlace_malo = format!(
            "order://emparejar?n={}&s=00000000&d=PC",
            a.nodo_id()
        );
        let err = b.emparejar(&enlace_malo);
        // En el rarísimo 1/2^32 caso de colisión exacta no fallaría, probamos solo si difiere.
        if !codigo.enlace.contains("s=00000000") {
            assert_eq!(err.unwrap_err(), ERROR_CODIGO_INVALIDO);
        }

        // Con el enlace válido, emparejan y sincronizan de una vez.
        let disp = b.emparejar(&codigo.enlace).expect("emparejamiento válido");
        assert_eq!(disp.nombre, "PC");
        assert_eq!(a.dispositivos().len(), 1);
        assert_eq!(b.dispositivos().len(), 1);

        // El secreto es de un solo uso: reutilizarlo falla.
        let err_reuso = intruso.emparejar(&codigo.enlace).unwrap_err();
        assert_eq!(err_reuso, ERROR_CODIGO_INVALIDO);

        // Ambos deben tener las dos tareas.
        let datos_a = modelo::parsear(&std::fs::read_to_string(&alm_a.ruta).unwrap()).unwrap();
        let datos_b = modelo::parsear(&std::fs::read_to_string(&alm_b.ruta).unwrap()).unwrap();
        assert!(
            datos_a.tareas.iter().any(|t: &Tarea| t.titulo == "Tarea Cel"),
            "A debe recibir Tarea Cel: {:?}",
            datos_a.tareas
        );
        assert!(
            datos_b.tareas.iter().any(|t: &Tarea| t.titulo == "Tarea PC"),
            "B debe recibir Tarea PC: {:?}",
            datos_b.tareas
        );

        // Edición posterior en A -> B sincroniza y la recibe.
        alm_a
            .modificar(|doc| {
                let id_cel = datos_a
                    .tareas
                    .iter()
                    .find(|t| t.titulo == "Tarea Cel")
                    .unwrap()
                    .id
                    .clone();
                ops::editar_tarea(
                    doc,
                    &id_cel,
                    &NuevaTarea {
                        titulo: "Tarea Cel editada en PC".to_string(),
                        materia: Some("Robótica".to_string()),
                        vence: NaiveDateTime::parse_from_str("2026-11-29T18:00", FORMATO_FECHA_HORA)
                            .unwrap(),
                    },
                )
            })
            .unwrap();

        let resumen = b.sincronizar_ahora();
        assert_eq!(resumen.exitosos, 1);
        assert!(resumen.cambios_locales);
        let datos_b2 = modelo::parsear(&std::fs::read_to_string(&alm_b.ruta).unwrap()).unwrap();
        assert!(
            datos_b2
                .tareas
                .iter()
                .any(|t| t.titulo == "Tarea Cel editada en PC")
        );
        assert!(cambios_a.load(Ordering::SeqCst) >= 1);
        assert!(cambios_b.load(Ordering::SeqCst) >= 1);

        // Quitar dispositivo en A -> B ya no puede sincronizar con A.
        a.quitar(&b.nodo_id()).unwrap();
        assert!(a.dispositivos().is_empty());
        let res_tras_quitar = b.sincronizar_ahora();
        assert_eq!(res_tras_quitar.exitosos, 0);

        a.detener();
        b.detener();
        intruso.detener();
    }

    #[test]
    fn secreto_vencido_es_rechazado() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();

        // TTL muy corto (1 ms) para probar expiración sin dormir 10 minutos.
        let a = crear_servicio(dir_a.path(), "PC", Duration::from_millis(1), None);
        let b = crear_servicio(dir_b.path(), "Celular", TTL_EMPAREJAMIENTO, None);
        b.registrar_direccion(a.direccion_local());

        let codigo = a.codigo_emparejamiento().unwrap();
        std::thread::sleep(Duration::from_millis(20));

        let err = b.emparejar(&codigo.enlace).unwrap_err();
        assert_eq!(err, ERROR_CODIGO_INVALIDO);

        a.detener();
        b.detener();
    }

    #[test]
    fn codigo_palabras_y_qr_ida_y_vuelta() {
        let clave = iroh::SecretKey::generate();
        let id = clave.public();
        let secreto = [0x12, 0x34, 0xab, 0xcd];
        let codigo = generar_codigo(&id, "Mi Laptop", secreto).unwrap();

        let parsed = parsear_enlace(&codigo.enlace).unwrap();
        assert_eq!(parsed.nodo, id);
        assert_eq!(parsed.secreto, secreto);
        assert_eq!(parsed.nombre.as_deref(), Some("Mi Laptop"));

        let (prefijo, dec_secreto) = decodificar_palabras(&codigo.palabras).unwrap();
        assert!(coincide_prefijo(&id, prefijo));
        assert_eq!(dec_secreto, secreto);
        assert!(codigo.qr_tamano >= 21);
        assert_eq!(
            codigo.qr_modulos.len(),
            (codigo.qr_tamano * codigo.qr_tamano) as usize
        );
    }

    #[test]
    fn palabras_sin_duplicados() {
        let mut vistas = std::collections::HashSet::new();
        for p in emparejamiento::palabras_lista() {
            assert!(!p.is_empty());
            assert_eq!(*p, p.to_lowercase());
            assert!(vistas.insert(p), "palabra repetida: {p}");
        }
        assert_eq!(vistas.len(), 256);
    }
}
