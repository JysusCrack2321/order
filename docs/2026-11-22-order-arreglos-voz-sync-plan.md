# ORDER — Voz local en escritorio, sync tras borrar en Android y ventana del Agente — Plan

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría). Verificar con `cargo test --workspace` (sin warnings) y `JAVA_HOME=/usr/lib/jvm/java-17-openjdk ./gradlew assembleDebug testDebugUnitTest`. No abrir ventanas ni emuladores, no instalar en el teléfono, no hacer llamadas reales a APIs (la descarga del modelo tampoco en pruebas).

## Tarea 1: Voz "Este PC (gratis)" en escritorio
**Problema:** en el PC la voz está en OpenAI sin clave; al grabar y finalizar no pasa nada y no hay ningún mensaje.
- [x] Nueva ficha de voz **"Este PC (gratis)"** (primera y por defecto en instalaciones nuevas de escritorio; en Android no aparece, allí sigue "Android (gratis)"). Transcribe en local con whisper.cpp vía la crate `whisper-rs` (versión estable más reciente), modelo `ggml-small.bin` (multilingüe), idioma `es`. Solo en `order-escritorio` (o tras una feature que `order-ffi` no active) para no tocar la compilación de Android.
- [x] Modelo en `~/.local/share/order/modelos/ggml-small.bin`; si falta, al elegir la ficha o al primer uso: diálogo "Descargar modelo de voz (≈470 MB)" con barra de progreso, descarga desde `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin` a archivo temporal + rename, verificar tamaño/SHA si está publicado. Transcripción en hilo aparte (indicador "transcribiendo…"); el modelo se descarga de memoria tras 10 min sin uso.
- [x] **Errores visibles siempre:** sin clave → "Falta la clave de voz: ponla en Ajustes → Agente o usa «Este PC (gratis)»"; error de red/401/429 → mensaje legible en el panel; grabación vacía → "No se oyó nada". Nunca quedarse en silencio.
- [x] Pruebas: elección de ficha/defecto, mensajes de error sin red, ruta del modelo; la transcripción real no se prueba en CI (marcar `#[ignore]` una prueba con un WAV corto). Commit `feat: voz local gratis en escritorio`.

## Tarea 2: Android sincroniza después de borrar
**Problema observado en el Motorola:** tras borrar 5 clases en el celular, no hubo sincronización (`ultima_sincronizacion` quedó en 17:38:10 y `sync-estado.json` sin cambios) hasta tocar "Sincronizar ahora". El plan pedía sincronizar tras **cada** cambio local.
- [x] Revisar en `Repositorio.kt` que **toda** operación que escribe (crear/editar/borrar tarea, clase, recordatorio, semestre, ajustes, aplicar acción del agente, Moodle/iCal) dispare la sincronización ~2 s después (con rebote para varias seguidas), igual que escritorio. Revisar también escritorio por si le falta algún caso.
- [x] Prueba unitaria Kotlin: borrar clase con el fake del puente → se llama `syncAhora` una vez tras el rebote. Commit `fix: sincronizar tras cualquier cambio en Android`.

## Tarea 3: Ventana Ajustes → Agente en escritorio
**Problema (captura del usuario):** el área con scroll del contenido es diminuta (se corta en "Avanzado ▸") y hay un hueco enorme vacío encima de Cancelar/Guardar.
- [x] En `ui/app.slint` (o donde viva el diálogo), que el contenido con scroll ocupe todo el alto disponible (`vertical-stretch: 1` / `ScrollView` con altura flexible) y la fila de botones solo su altura natural. Revisar los demás diálogos de Ajustes (Moodle, Calendarios, Dispositivos) por el mismo error. Commit `fix: diálogos de ajustes usan todo el alto`.

## Tarea 4: Cierre
- [x] `./android/compilar-rust.sh` + `assembleDebug` desde cero; `cargo install --path crates/order-escritorio --locked`; APK a `dist/ORDER.apk`. Commit `docs: voz local y arreglos`.
