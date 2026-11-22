# ORDER Android — Fase 2: app básica — Plan de implementación

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría). Leer este plan **y** `docs/2026-11-22-order-android-diseño.md` completo. NO hay teléfono conectado: verificar con `cargo test --workspace` y `./gradlew assembleDebug` (y pruebas unitarias de Kotlin donde se indique). No abrir emuladores ni ventanas.

**Objetivo:** APK de ORDER para Android con Inicio, Día, Semana, Pendientes (con filtros) y todos los formularios, leyendo y escribiendo el mismo `order.toml` a través de `order-core`.

**Arquitectura:** `crates/order-ffi` (UniFFI, proc-macros, `crate-type = ["cdylib", "lib"]`) expone `order-core` a Kotlin. `android/` es un proyecto Gradle con un módulo `app` (Kotlin + Jetpack Compose + Material 3 con tema propio). La `.so` se compila con `cargo ndk` para `arm64-v8a` y `x86_64` y se copia a `app/src/main/jniLibs/`; los bindings Kotlin se generan con `uniffi-bindgen` en modo librería.

**Stack:** Rust 1.100 + UniFFI (versión estable más reciente), cargo-ndk, NDK `~/Android/Sdk/ndk/30.0.16248370`, SDK en `~/Android/Sdk` (platform 36, build-tools 36.1.0), JDK 17, Gradle wrapper, AGP/Kotlin/Compose BOM compatibles con JDK 17 y compileSdk 36. `minSdk 31`, `targetSdk 36`. Paquete `com.jysusman.order`.

## Restricciones globales
- Toda la lógica, textos en español, colores por materia y validaciones vienen de `order-core` (vía FFI). Kotlin no recalcula ni redacta textos.
- La app de escritorio no cambia.
- Estilo: los valores de `crates/order-escritorio/ui/tema.slint` llevados a `ui/theme/Theme.kt` (colores, tamaños); Cinzel (`crates/order-escritorio/ui/fuentes/`) como fuente de títulos; Noto Sans/Roboto del sistema para texto pequeño. Icono: `crates/order-escritorio/arte/order.svg` convertido a icono adaptativo.
- Carpeta de datos por defecto `/storage/emulated/0/ORDER/` (`order.toml`, `historial.toml`, `respaldos/`); `agente.toml` y el registro de avisos en `filesDir` privado. Permiso `MANAGE_EXTERNAL_STORAGE` con pantalla explicativa que lleva a Ajustes del sistema.

## Puntos de revisión
1. **Sin permiso de archivos o carpeta inexistente** → pantalla explicativa con botón al ajuste del sistema; si la carpeta no existe se crea con `ejemplo.toml`. Nunca crashea.
2. **TOML roto o claves desconocidas** → banner rojo/amarillo como en escritorio, con el último estado válido.
3. **Archivo cambiado por Syncthing con la app abierta** → `FileObserver` recarga en < 2 s sin perder un formulario abierto.
4. **Copia en conflicto** → banner con *Quedarme con la actual* / *Usar la otra* (funciones de `order_core::conflictos`).
5. **Llamadas FFI en el hilo principal** → toda llamada que toque disco o red va en `Dispatchers.IO`; la UI no se congela.

---

### Tarea 1: `order-ffi`
**Archivos:** `crates/order-ffi/{Cargo.toml,src/lib.rs,uniffi.toml}`, `android/compilar-rust.sh`.
- [ ] Exponer con `#[uniffi::export]`/`#[derive(uniffi::Record)]` la superficie de §6.2 del diseño: cargar + advertencias + conflictos (detectar/resolver), todas las operaciones de `ops` (por ruta, vía `Almacen::modificar`), vistas listas para pintar (`inicio`, `dia`, `semana`, `pendientes(filtro)`, materias con tono, textos), validación de formularios con campo y mensaje, y `ruta_por_defecto`. (Agente, voz y avisos quedan para fases 3–4; no exponerlos aún.) Errores como `#[derive(uniffi::Error)]` con mensaje legible.
- [ ] Pruebas Rust en `order-ffi` que llamen a cada función expuesta sobre un temporal (mínimo: cargar ejemplo, crear y editar tarea, vista semana, filtro de pendientes, conflicto).
- [ ] `android/compilar-rust.sh`: `cargo ndk -t arm64-v8a -t x86_64 -o android/app/src/main/jniLibs build -p order-ffi --release` y generación de bindings Kotlin en `android/app/src/main/java/` con `uniffi-bindgen generate --library … --language kotlin`.
- [ ] `cargo test --workspace` sin warnings. Commit `feat: puente order-ffi con UniFFI`.

### Tarea 2: Proyecto Android y tema
**Archivos:** `android/{settings.gradle.kts,build.gradle.kts,gradle/,gradlew}`, `android/app/{build.gradle.kts,src/main/AndroidManifest.xml,src/main/java/com/jysusman/order/...,src/main/res/...}`.
- [ ] Gradle wrapper; módulo `app` con Compose, Material 3, dependencia `net.java.dev.jna:jna` (aar) que necesitan los bindings UniFFI.
- [ ] `Theme.kt` con la paleta y tamaños de `tema.slint`; Cinzel en `res/font`; icono adaptativo con el reloj de arena; nombre "ORDER".
- [ ] `MainActivity` con barra inferior Inicio · Día · Semana · Pendientes (pantallas vacías por ahora) que ya llama a `cargar` y muestra el número de tareas.
- [ ] `./gradlew assembleDebug` pasa. Añadir `android/local.properties` a `.gitignore` y las `.so`/bindings generados también (se regeneran con el script). Commit `feat: proyecto Android con tema ORDER`.

### Tarea 3: Archivos, permisos y recarga
**Archivos:** `.../datos/Repositorio.kt` (envuelve FFI en `Dispatchers.IO` y expone `StateFlow`), `.../ui/Permiso.kt`, `.../ui/Banners.kt`.
- [ ] Pantalla de permiso `MANAGE_EXTERNAL_STORAGE`; crear carpeta por defecto si falta; carpeta configurable en Ajustes (texto de ruta).
- [ ] `FileObserver` sobre la carpeta + recarga en `onResume`; banners de error, advertencias y conflicto con sus botones.
- [ ] Prueba unitaria Kotlin del `Repositorio` con un fake del puente (recarga y estado de error). `./gradlew assembleDebug testDebugUnitTest` pasa. Commit `feat: datos, permisos y recarga en Android`.

### Tarea 4: Vistas
**Archivos:** `.../ui/inicio/`, `.../ui/dia/`, `.../ui/semana/`, `.../ui/pendientes/`, componentes comunes (tarjeta, píldora de materia, bloque de clase).
- [ ] Inicio con tarjetas y acentos por sección y franja con degradado, como escritorio.
- [ ] Día: línea de tiempo con clases, tareas, recordatorios; ‹ › y Hoy; deslizar para cambiar de día.
- [ ] Semana: la forma que mejor se lea en vertical (rejilla con scroll horizontal o 3 días con deslizamiento); ‹ › y Hoy.
- [ ] Pendientes: lista con casilla hecha/no hecha, hechas tachadas, "Mostrar hechas", filtros (materia, fecha, hora), "Quitar filtros".
- [ ] `./gradlew assembleDebug` pasa. Commit `feat: vistas Inicio, Día, Semana y Pendientes en Android`.

### Tarea 5: Formularios
**Archivos:** `.../ui/formularios/` (hojas inferiores), `.../ui/selectores/` (ruedas de hora 1–12 + a. m./p. m., minutos 00–59 circulares; selector de fecha; píldoras de materias con "Escribir una nueva").
- [ ] Tarea, clase (varios días), recordatorio, semestre y ajustes: crear, editar, borrar con confirmación; validación con el campo marcado y el mensaje de `order-core`; botón flotante "+ Tarea"; tocar un elemento abre su edición.
- [ ] `./gradlew assembleDebug` pasa. Commit `feat: formularios en Android`.

### Tarea 6: Construcción reproducible
- [ ] `android/README.md`: requisitos, `./compilar-rust.sh`, `./gradlew assembleDebug`, instalación con `adb install -r app/build/outputs/apk/debug/app-debug.apk`, activar depuración USB.
- [ ] Desde cero (borrando jniLibs y bindings generados) el script + `assembleDebug` producen el APK. Commit `docs: cómo compilar e instalar ORDER Android`.
