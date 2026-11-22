# ORDER — Sincronización P2P entre dispositivos — Plan de implementación

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría). Leer este plan y el código de `crates/order-core/src/{modelo.rs, ops.rs, almacen.rs, conflictos.rs}`, `crates/order-ffi/src/lib.rs`, `crates/order-escritorio/src/` + `ui/` y `android/`. Verificar con `cargo test --workspace` (sin warnings) y `JAVA_HOME=/usr/lib/jvm/java-17-openjdk ./gradlew assembleDebug testDebugUnitTest`. No abrir ventanas ni emuladores, no instalar en el teléfono. Las pruebas de red son **locales** (dos endpoints en el mismo proceso, sin relay).

**Objetivo:** que el PC y el celular de una misma persona tengan la misma agenda sin servidor propio, sin cuentas y sin que la persona entienda nada: se emparejan una vez con un QR y se sincronizan solos cuando los dos están encendidos.

## Decisiones
- **Transporte: `iroh` 1.x** (QUIC, conexión directa por clave pública, atraviesa NAT; si no hay conexión directa usa los relays públicos de n0, que solo reenvían tráfico **cifrado de extremo a extremo**). ALPN `order/sync/1`. Crate nuevo `crates/order-p2p` (tokio vive solo ahí). Clave secreta del nodo generada una vez y guardada en privado.
- **Fusión: registro LWW por campo** (es un CRDT; más simple que Automerge y `order.toml` sigue siendo legible y editable a mano o por Hermes):
  - Unidades: cada `clase`, `tarea`, `recordatorio` por `id`; `ajustes` y `semestre` como registros únicos. Cada campo lleva una marca **HLC** (reloj lógico híbrido: tiempo físico + contador + id de dispositivo para desempatar).
  - Dos cambios en **campos distintos** del mismo elemento → se conservan ambos. Mismo campo → gana la marca más nueva.
  - Borrado = **lápida** con su marca; un borrado vence a ediciones más viejas y pierde contra ediciones posteriores. Lápidas se purgan a los 60 días.
  - Los cambios locales se detectan comparando el `order.toml` actual con la **última foto** sincronizada (así cuentan también las ediciones a mano, de Hermes o del agente); cada campo distinto recibe marca nueva.
  - Resultado de la fusión se escribe con `Almacen::modificar` (respaldo + escritura atómica) solo si cambió algo; luego la UI recarga y Android reprograma avisos.
  - `nuevo_id` pasa de 16 a 32 bits aleatorios (los ids viejos siguen valiendo).
- **Estado privado** (junto a `agente.toml`, nunca en la carpeta de datos): `sync.toml` (clave del nodo, nombre del dispositivo, dispositivos emparejados: id público + nombre + fecha) y `sync-estado.json` (marcas HLC, lápidas, foto). Archivos en `0600`.
- **Emparejamiento:** el dispositivo A muestra un **QR** con `order://emparejar?n=<id del nodo>&s=<secreto de un solo uso>&d=<nombre>` (válido 10 min); B lo escanea, se conecta, presenta el secreto y ambos se guardan mutuamente. Alternativa sin cámara: el mismo enlace como **código de 8 palabras** para escribir. Solo se aceptan conexiones de nodos emparejados.
- **Protocolo:** al conectar, cada lado envía su estado completo (marcas + valores; la agenda pesa pocos KB), ambos fusionan y responden si cambiaron. Sin diffs ni optimizaciones.
- **Cuándo:** escritorio (vive en la bandeja) escucha siempre e intenta con cada emparejado al arrancar, cada 5 min y 2 s después de cada cambio local. Android: al abrir la app, tras cada cambio, y en `TrabajoRespaldo`; mientras la app está abierta acepta conexiones. Si el otro está apagado, se reintenta después sin mensajes.
- **Interfaz** (escritorio y Android, estilo ORDER): Ajustes → **Dispositivos**: "Este dispositivo: {nombre}" (editable), botón **Agregar dispositivo** → muestra QR + código de palabras y "Esperando…"; en Android además **Escanear QR** (Google Code Scanner `com.google.android.gms:play-services-code-scanner`, sin permiso de cámara) y "Escribir código"; lista de emparejados con "sincronizado hace X" / "sin conexión" y **Quitar**; botón **Sincronizar ahora**. Indicador pequeño en la barra/encabezado mientras sincroniza.
- Syncthing/conflictos actuales siguen funcionando igual (no se tocan).
- Compartir con otra persona (`compartido.toml`) **no** entra en este plan.

## Puntos de revisión
1. PC y celular editan **la misma tarea**: uno el título, otro la fecha, sin conexión → al conectar quedan los dos cambios.
2. Mismo campo editado en ambos → queda el más reciente, en los dos.
3. Borrar en uno y editar antes en el otro → queda borrada; editar después del borrado → reaparece con la edición.
4. Edición a mano de `order.toml` en el PC → llega al celular.
5. Tareas de Moodle traídas en ambos → no se duplican (mismo `origen`; si dos ids distintos tienen el mismo `origen`, fusionar en uno).
6. Un nodo no emparejado intenta conectar → rechazado.
7. QR vencido o secreto errado → "Este código ya no sirve; genera otro."
8. Sin internet pero misma wifi → se sincronizan por la red local.

---

### Tarea 1: Fusión en `order-core` (sin red)
**Archivos:** `crates/order-core/src/sincronizacion/{mod.rs, hlc.rs, estado.rs, fusion.rs}`, `almacen.rs` (`nuevo_id` 32 bits).
- [ ] HLC, estado (marcas, lápidas, foto), `registrar_cambios_locales(datos, &mut estado, ahora)`, `fusionar(local, remoto) -> Resultado {estado, cambio: bool}`, `aplicar(estado) -> Datos`, serialización del estado y del mensaje. Pruebas de todos los puntos de revisión 1–5 como casos puros, más conmutatividad/idempotencia (fusionar A con B = B con A; fusionar dos veces = una). Commit `feat: fusión por campos para sincronizar`.

### Tarea 2: `crates/order-p2p` con iroh
- [ ] Endpoint con clave persistente, ALPN, servidor que acepta solo emparejados, emparejamiento con secreto de un solo uso (10 min), código QR (crate `qrcode`, salida SVG o matriz) y código de palabras, sesión de sincronización que usa la Tarea 1 y escribe con `Almacen::modificar`. API bloqueante sencilla para FFI/escritorio (runtime tokio interno). Pruebas: dos endpoints locales con relays desactivados → emparejar, sincronizar, rechazar no emparejado, secreto vencido. Commit `feat: sincronización P2P con iroh`.

### Tarea 3: FFI
- [ ] `sync_iniciar(ruta_datos, ruta_config)`, `sync_detener()`, `sync_codigo_emparejamiento() -> {enlace, palabras, qr_svg}`, `sync_emparejar(enlace_o_palabras)`, `sync_dispositivos()`, `sync_quitar(id)`, `sync_ahora() -> Resumen`, `sync_estado()`, `sync_nombre(nombre)`; callback o sondeo para avisar a Kotlin que el archivo cambió. Verificar que `cargo ndk` compila iroh para `arm64-v8a` y `x86_64`. Pruebas Rust. Commit `feat: sincronización en la ffi`.

### Tarea 4: Escritorio
- [ ] Servicio de sincronización en hilo aparte al arrancar (sin congelar la UI), Ajustes → Dispositivos con QR (imagen generada) y código, lista, quitar, sincronizar ahora, indicador. Commit `feat: sincronización en escritorio`.

### Tarea 5: Android
- [ ] Ajustes → Dispositivos con mostrar QR/código, **Escanear QR** (Code Scanner), escribir código, lista, quitar, sincronizar ahora; sincronizar al abrir, tras cambios y en `TrabajoRespaldo`; recargar y reprogramar avisos tras recibir cambios; permiso `INTERNET` (y `ACCESS_NETWORK_STATE` si hace falta). Prueba unitaria Kotlin con el fake del puente. Commit `feat: sincronización en Android`.

### Tarea 6: Cierre
- [ ] `./android/compilar-rust.sh` + `assembleDebug` desde cero; `cargo install --path crates/order-escritorio --locked`; APK a `dist/ORDER.apk`. README: cómo emparejar, qué viaja (cifrado de extremo a extremo, relays de n0 solo como paso), dónde vive el estado, que ambos deben estar encendidos a la vez. Anotar en la respuesta final los pasos de prueba manual para el usuario (emparejar PC ↔ Motorola, editar en uno, ver en el otro). Commit `docs: sincronización P2P`.
