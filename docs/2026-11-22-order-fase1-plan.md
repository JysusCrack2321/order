# ORDER Android — Fase 1: separar `order-core` — Plan de implementación

> **Para agentes:** cada tarea es un encargo independiente, en orden. Leer este plan **y** `docs/2026-11-22-order-android-diseño.md` (§6.1, §6.2, §9). Pasos con casillas (`- [ ]`). Commits sin líneas de coautoría. NO lanzar el binario ni abrir ventanas.

**Objetivo:** convertir `~/apps/order` en un workspace de Cargo con `order-core` (lógica pura, compilable para Android) y `order-escritorio` (la app actual), sin ningún cambio visible en la app de escritorio.

**Arquitectura:** `order-core` no depende de Slint, ksni, notify, notify-rust, cpal ni png. `order-escritorio` depende de `order-core` y conserva UI, bandeja, socket, observador, notificaciones y grabación de micrófono.

**Stack:** Rust 1.100 (rustup), workspace Cargo, `cargo ndk` (ya instalado, NDK en `~/Android/Sdk/ndk/30.0.16248370`).

**Diseño:** `docs/2026-11-22-order-android-diseño.md`

## Restricciones globales
- El binario sigue llamándose `order` y se instala con `cargo install --path crates/order-escritorio --locked`.
- Ningún cambio visible ni de comportamiento en escritorio; `ORDER_ARCHIVO` sigue funcionando igual (incluido `agente.toml` junto a ese archivo).
- El total de pruebas no baja (hoy: contar con `cargo test --workspace` antes de empezar y anotarlo en el primer commit).
- `order-core` compila para `aarch64-linux-android` con `cargo ndk -t arm64-v8a build -p order-core`.
- Mover archivos con `git mv` para conservar historia.

## Puntos de revisión
1. **`include_str!("../ejemplo.toml")` y rutas relativas** rotas al mover → el archivo de ejemplo vive en `crates/order-core/` y `Almacen` sigue creándolo. (Tarea 1: `crea_desde_ejemplo` sigue pasando)
2. **`ORDER_ARCHIVO`** deja de afectar `agente.toml` o el registro de avisos tras mover → prueba explícita. (Tarea 1: `agente_toml_junto_a_order_archivo`)
3. **Dependencias de escritorio colándose en el núcleo** (cpal, slint) → verificación con `cargo tree -p order-core`. (Tarea 5)
4. **Textos idénticos** tras mover funciones de `puente.rs` → las pruebas existentes de esas funciones se mueven con ellas y siguen pasando con los mismos valores. (Tarea 3)
5. **Copia en conflicto de Syncthing** nunca borra datos → prueba de que todo va a `respaldos/`. (Tarea 4)

---

### Tarea 1: Workspace y mudanza de módulos
**Archivos:** `Cargo.toml` raíz (workspace), `crates/order-core/{Cargo.toml,src/lib.rs,ejemplo.toml}`, `crates/order-escritorio/{Cargo.toml,build.rs,src/main.rs,ui/,arte/,order.desktop}`.
- [ ] Anotar el número de pruebas actual (`cargo test 2>&1 | grep "test result"`).
- [ ] `git mv` a `crates/order-core/src/`: `modelo.rs`, `almacen.rs`, `ops.rs`, `semana.rs`, `filtros.rs`, `avisos.rs`, `reloj.rs`, `agente/{mod,config,contexto,acciones,cliente}.rs`, y `ejemplo.toml`. `lib.rs` los declara `pub mod`.
- [ ] `git mv` a `crates/order-escritorio/`: `src/{main,sistema,puente}.rs`, `src/agente/voz.rs` (temporalmente; se parte en la Tarea 2), `ui/`, `arte/`, `build.rs`, `order.desktop`. Los `use crate::modelo` pasan a `use order_core::modelo`.
- [ ] Dependencias: `order-core` = serde, serde_json, toml, toml_edit, chrono, dirs, ureq; `order-escritorio` = order-core + slint, ksni, notify, notify-rust, png, cpal, hound. Quitar `#[allow(dead_code)]` donde ya no haga falta.
- [ ] Prueba nueva `agente_toml_junto_a_order_archivo` en `order-core` (con `ORDER_ARCHIVO` apuntando a un temporal, la ruta de `agente.toml` queda en el mismo directorio).
- [ ] `cargo build --workspace` y `cargo test --workspace` sin warnings, mismo número de pruebas + 1. Actualizar README (ruta de instalación).
- [ ] Commit `refactor: workspace con order-core y order-escritorio`.

### Tarea 2: Partir la voz
**Archivos:** `crates/order-core/src/agente/voz.rs` (transcripción), `crates/order-escritorio/src/grabacion.rs` (micrófono).
**Interfaces producidas:** `order_core::agente::voz::transcribir(wav: &[u8], config: &ConfigVoz) -> Result<String, String>` y el armado del multipart (con su prueba existente); la grabación con cpal/hound queda en escritorio y entrega `Vec<u8>` WAV.
- [ ] Mover la prueba del multipart al núcleo; `cargo test --workspace` pasa.
- [ ] Commit `refactor: transcripción en el núcleo, grabación en escritorio`.

### Tarea 3: Presentación al núcleo
**Archivos:** `crates/order-core/src/presentacion/{mod,materias,textos,formularios}.rs`; modificar `crates/order-escritorio/src/puente.rs`.
**Interfaces producidas** (mismos nombres que hoy en `puente.rs`, ahora `pub`):
- `materias`: `canonica`, `clave_materia`, `tono_materia`, y la lista de materias existentes sin duplicados.
- `textos`: `fecha_larga`, `etiqueta_semana`, `resumen`, `pie`, `proximo_recordatorio`, `formato_tiempo` y demás funciones de texto puro.
- `formularios`: `interpretar_fecha`, `interpretar_hora`, `opcional` y las validaciones con sus mensajes (devuelven el campo inválido y el mensaje, sin tipos de Slint).
- `agente`: la lógica de "aplicar todas" (aplicar en orden, parar o seguir ante error según el comportamiento actual, devolver resultado por acción).
- [ ] Mover cada función **con sus pruebas**; `puente.rs` solo convierte a/desde tipos Slint y conecta callbacks.
- [ ] `cargo test --workspace` pasa con el mismo total; sin warnings.
- [ ] Commit `refactor: presentación y validaciones en order-core`.

### Tarea 4: Conflictos de Syncthing
**Archivos:** `crates/order-core/src/conflictos.rs`; banner en `crates/order-escritorio` (`ui/app.slint`, `puente.rs`, `sistema.rs` para que el observador también reaccione a archivos `*.sync-conflict-*`).
**Interfaces producidas:**
```rust
pub struct Conflicto { pub ruta: PathBuf, pub fecha: Option<NaiveDateTime> }
pub fn detectar(ruta_order: &Path) -> Vec<Conflicto>            // order.sync-conflict-*.toml en el mismo dir
pub fn quedarme_con_actual(ruta_order: &Path, c: &Conflicto) -> Result<(), String>  // mueve la copia a respaldos/
pub fn usar_la_otra(ruta_order: &Path, c: &Conflicto) -> Result<(), String>         // mueve la actual a respaldos/ y renombra la copia a order.toml
```
`respaldos/` junto a `order.toml`, nombres con marca de tiempo; nunca se borra nada.
- [ ] Pruebas: `detecta_copias_en_conflicto`, `quedarme_mueve_copia_a_respaldos`, `usar_la_otra_conserva_ambas` (ambas versiones existen al final, una en `respaldos/`).
- [ ] Banner en escritorio con el estilo actual (Tema): "Hay una copia en conflicto (fecha)" + botones *Quedarme con la actual* / *Usar la otra*.
- [ ] Commit `feat: detectar y resolver conflictos de Syncthing`.

### Tarea 5: Verificación de núcleo portable e instalación
- [ ] `cargo tree -p order-core` no contiene slint, ksni, notify, notify-rust, cpal, png.
- [ ] `ANDROID_NDK_HOME=~/Android/Sdk/ndk/30.0.16248370 cargo ndk -t arm64-v8a build -p order-core` compila (ajustar features de ureq a rustls si hace falta).
- [ ] `cargo install --path crates/order-escritorio --locked`.
- [ ] Commit `chore: order-core compila para Android` (solo si hubo cambios).
