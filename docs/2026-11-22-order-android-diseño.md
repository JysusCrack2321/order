# ORDER para Android — documento de diseño

Fecha: 2026-11-22 · Estado: borrador para aprobación · Dispositivo objetivo: Motorola Edge 50 Fusion (Android 14+)

## 1. Intención
Tener ORDER en el celular con **las mismas funciones que la app de escritorio** (Inicio, Día, Semana, Pendientes con filtros, formularios completos, avisos, agente y voz), sobre **los mismos datos** (`order.toml`) sincronizados con Syncthing a través de la Orange Pi. Sigue valiendo todo lo del diseño de escritorio (`2026-11-21-order-diseño.md`): simple, sin servidor propio, sin cuentas, datos en un TOML legible.

## 2. Objetivos
1. App Android instalable por `adb` (no Play Store) con el mismo estilo: carbón, oro viejo, marfil, Cinzel en títulos, reloj de arena como icono.
2. Paridad funcional con el escritorio (lista arriba), adaptada a pantalla vertical y táctil.
3. Avisos que llegan a su hora aunque la app esté cerrada o el teléfono reiniciado.
4. **Una sola lógica**: lo que define cómo se leen/escriben los datos, cómo se calcula la semana, qué se avisa y cómo habla el agente vive en un solo lugar compartido por las dos apps.

## 3. No-objetivos (esta versión)
- Widget de pantalla de inicio (queda previsto; Compose/Glance lo hace fácil después).
- Publicación en Play Store, iOS, tablets.
- Sincronización propia: la hace Syncthing.

## 4. Anti-objetivos
- Un servidor, base de datos o cuenta para que funcione.
- Dos lógicas que se comporten distinto entre celular y escritorio.
- Que las claves de API viajen por la sincronización.

## 5. Enfoques considerados (elegido por el usuario: B)
- **A. Todo Rust + Slint en Android**: una sola interfaz, pero Slint en Android es menos maduro y alarmas, permisos y archivos pelean con el sistema.
- **B. Núcleo Rust compartido + interfaz nativa Kotlin/Jetpack Compose** — *elegido*. Lo nativo (avisos, permisos, micrófono, archivos) va por Android; la lógica es una sola. Costo: dos interfaces que mantener.
- **C. Reescribir en Kotlin**: duplica la lógica; descartado.

## 6. Diseño

### 6.1 Estructura del repositorio (*decisión de Claude*: un solo repo `~/apps/order`)
- Workspace de Cargo:
  - `crates/order-core` (librería): `modelo`, `almacen`, `ops`, `semana`, `filtros`, `avisos`, `reloj`, `agente::{config, contexto, acciones, cliente}` y la transcripción de voz por API. Sin Slint, sin bandeja, sin hilos de UI.
  - `crates/order-escritorio`: la app actual (Slint, bandeja, notificaciones, socket) usando `order-core`. **No cambia nada visible.**
  - `crates/order-ffi`: puente a Kotlin con **UniFFI** (genera los bindings Kotlin automáticamente).
- `android/`: proyecto Gradle (Kotlin, Jetpack Compose, Material 3 con tema propio), que empaqueta `liborder_ffi.so` compilada con `cargo ndk` para `arm64-v8a` (y `x86_64` para emulador).

### 6.2 Superficie del puente (orientativa; el builder fija nombres)
Funciones que reciben/devuelven registros simples (o JSON si un tipo no encaja en UniFFI):
- `cargar(ruta) -> Resultado<Datos, ErrorDatos>` y `advertencias(ruta)`.
- Todas las operaciones de `ops` (crear/editar/marcar/borrar tarea, clase, recordatorio; semestre; ajustes; archivar), cada una vía `Almacen::modificar` sobre la ruta dada.
- Vistas ya calculadas: `inicio(ahora)`, `dia(fecha)`, `semana(lunes)`, `pendientes(filtro)`, materias con su tono de color.
- Avisos: `proximos_avisos(desde, hasta) -> lista {clave, momento, titulo, cuerpo}` para que Android programe alarmas.
- Agente: `enviar_al_agente(texto, config) -> {respuesta, acciones}` y `aplicar_accion(accion)`; voz: `transcribir(wav_bytes, config_voz) -> texto`. HTTP dentro de Rust (ureq + rustls), llamado desde un hilo de fondo en Kotlin.

### 6.3 Dónde viven los archivos en el teléfono (*decisión de Claude*)
- `order.toml` e `historial.toml`: en una carpeta visible, por defecto `/storage/emulated/0/ORDER/`, que Syncthing-Fork sincroniza. La app pide el permiso **"Acceso a todos los archivos"** (`MANAGE_EXTERNAL_STORAGE`) para leer/escribir ahí con rutas normales desde Rust. Aceptable porque es una app personal fuera de Play Store. La carpeta se puede cambiar en Ajustes.
- `agente.toml` (claves) y el registro de avisos enviados: en el almacenamiento **privado** de la app, nunca en la carpeta sincronizada. Las claves se escriben en el teléfono.
- Recarga: `FileObserver` sobre la carpeta + recarga al volver a primer plano, para ver cambios que llegan por Syncthing.
- Conflictos de Syncthing (`order.sync-conflict-*.toml`): la app los detecta y muestra un aviso para resolver (abrir uno u otro); no los fusiona sola.

### 6.4 Avisos
- Android programa alarmas exactas (`AlarmManager.setExactAndAllowWhileIdle`, permiso `SCHEDULE_EXACT_ALARM`/`USE_EXACT_ALARM`) para los próximos avisos que calcula `order-core` (ventana de 48 h), y reprograma: al abrir la app, al cambiar el archivo, al arrancar el teléfono (`BOOT_COMPLETED`) y en un trabajo periódico (`WorkManager`, cada ~6 h) como respaldo.
- Al disparar: notificación con título y cuerpo de `order-core`; canal "Recordatorios" y canal "Tareas". Permiso `POST_NOTIFICATIONS` pedido al primer uso.
- Mismo criterio que escritorio (día antes, N horas antes, recordatorios, resumen al abrir). *Decisión de Claude:* en Ajustes del teléfono un interruptor "Avisos en este dispositivo" (encendido por defecto), por si no quieres que suenen el portátil y el celular a la vez.

### 6.5 Interfaz (Compose)
- Barra inferior con Inicio · Día · Semana · Pendientes; botón flotante "+ Tarea"; Ajustes desde la cabecera.
- Semana: rejilla con desplazamiento horizontal entre días (o vista de 3 días con deslizamiento); el builder elige la que se lea mejor en vertical.
- Formularios como hojas inferiores; selectores de fecha y hora en ruedas como en escritorio (12 h con a. m./p. m.); materias como píldoras con su color.
- Barra del agente abajo con campo de texto y micrófono (graba con `AudioRecord` a WAV 16 kHz mono, permiso `RECORD_AUDIO`); acciones propuestas como tarjetas Aplicar/Descartar.
- Tema: los mismos valores de `ui/tema.slint` llevados a un `Theme.kt` (colores, tipografía Cinzel incluida como recurso).

### 6.6 Sincronización (fase final)
- Orange Pi con Syncthing siempre encendida; carpeta `order` compartida con portátil y celular.
- En el portátil se sincroniza `~/.config/order/` **con `.stignore`** que excluye `agente.toml`, el registro de avisos y temporales; solo viajan `order.toml` e `historial.toml`.
- En el celular, Syncthing-Fork apunta a `/storage/emulated/0/ORDER/`.

### 6.7 Compilación e instalación
- Herramientas ya instaladas: rustup (stable 1.100) con `aarch64-linux-android` y `x86_64-linux-android`, `cargo-ndk`, Android SDK en `~/Android/Sdk` (platform 36, build-tools 36.1.0, NDK 30), JDK 17, Gradle.
- `minSdk` 31, `targetSdk`/`compileSdk` 36. Gradle wrapper en el repo. Instalación por `adb install` con depuración USB.

## 7. Fases
1. Extraer `order-core` (y `order-escritorio`) sin cambios visibles; las 77+ pruebas siguen pasando.
2. `order-ffi` + app Android básica: vistas y formularios leyendo/escribiendo `order.toml` en la carpeta elegida.
3. Avisos con alarmas.
4. Agente y voz en Android.
5. Syncthing: Orange Pi, portátil (con `.stignore`) y celular.

## 8. Queda a criterio del builder
Nombres exactos de la superficie UniFFI, versión de AGP/Kotlin/Compose compatible con el JDK 17 y el SDK 36, organización de paquetes Kotlin, forma concreta de la vista Semana en vertical, iconos de la barra inferior.

## 9. Precisiones tras la revisión
- **Quién construye** (ya decidido por el usuario): el código lo escriben agentes DeepSeek vía Hermes en encargos pequeños; Claude orquesta y revisa. Cada encargo tiene un criterio de "listo" comprobable sin teléfono (`cargo test`, `./gradlew assembleDebug`, pruebas instrumentadas o unitarias de Kotlin donde aplique); la prueba en el teléfono la hace el usuario al final de cada fase.
- **Qué pasa a `order-core`** (*decisión de Claude*): además de los módulos existentes, toda la lógica de `src/puente.rs` que no es Slint: tono de color por materia y normalización de materias, textos en español (fechas largas, etiqueta de semana, resumen, pie, próximo recordatorio, horas 12 h), lectura y validación de campos de formulario con sus mensajes, y "aplicar todas" del agente. La FFI devuelve vistas casi listas para pintar; Kotlin no recalcula ni redacta textos. La fase 1 es por tanto "mover y reexportar", con la app de escritorio sin cambios visibles.
- **Conflictos de Syncthing** (*decisión de Claude*): detección y resolución en `order-core`, con aviso en **las dos** apps. Si existe `order.sync-conflict-*.toml` junto al archivo, se muestra un banner "Hay una copia en conflicto (fecha)" con dos opciones: *Quedarme con la actual* (mueve la copia a `respaldos/`) o *Usar la otra* (mueve la actual a `respaldos/` y pone la copia como `order.toml`). Nunca se borra nada: todo va a `respaldos/` junto al archivo (excluida de Syncthing).
- **Resumen al abrir en el teléfono** (*decisión de Claude*): se muestra al abrir la app la primera vez del día (misma clave `resumen:{fecha}` del registro), no al encender el teléfono. Avisos perdidos con el teléfono apagado no se disparan tarde (igual que escritorio); al arrancar el teléfono solo se reprograman las alarmas futuras.
