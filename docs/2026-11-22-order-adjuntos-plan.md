# ORDER — Adjuntos (PDF y fotos) para el agente y voz de Android — Plan de implementación

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría). Leer este plan, `docs/2026-11-22-order-android-diseño.md` y el código actual de `crates/order-core/src/agente/`, `crates/order-ffi`, `crates/order-escritorio` y `android/`. Verificar con `cargo test --workspace` (sin warnings) y `JAVA_HOME=/usr/lib/jvm/java-17-openjdk ./gradlew assembleDebug testDebugUnitTest`. No abrir ventanas ni emuladores, no instalar en el teléfono, no hacer llamadas reales a APIs.

**Objetivo:** que el agente pueda recibir PDFs y fotos (horarios, enunciados, capturas, tablero) con cualquier proveedor, como hace Hermes: si el modelo ve imágenes se le manda el archivo; si no, ORDER lo convierte a texto en el propio dispositivo. Además, en Android el micrófono usa el reconocimiento de voz del sistema (gratis, sin key).

**Por qué así:** Hermes no depende de que la API lea archivos: extrae el texto localmente (pdftotext/anydoc) y transcribe con Whisper local. En el celular, Whisper local es lento y pesado; el reconocimiento de voz de Android y ML Kit hacen lo mismo en el teléfono, gratis y sin red.

## Decisiones

| Proveedor | ¿Ve imágenes? | PDF | Foto |
|---|---|---|---|
| Claude | sí | bloque `document` base64 (`application/pdf`), antes del texto | bloque `image` base64 |
| Gemini (OpenAI-compatible) | sí | páginas renderizadas a JPEG → `image_url` con `data:` URL | `image_url` con `data:` URL |
| DeepSeek | no | texto extraído (+ OCR si la página no tiene texto) | OCR |
| Otro | según casilla "El modelo ve imágenes" en Avanzado (apagada por defecto) | como Gemini o como DeepSeek | ídem |

- Extracción de texto de PDF en `order-core` con una crate Rust pura (p. ej. `pdf-extract`), que compile para Android. Si una página sale vacía o casi vacía se marca "necesita OCR".
- Renderizado de páginas y OCR son de cada plataforma:
  - Android: `PdfRenderer` para páginas; **ML Kit Text Recognition** (variante de Play Services, `com.google.android.gms:play-services-mlkit-text-recognition`) para OCR, en el teléfono.
  - Escritorio: `pdftoppm` y `tesseract -l spa` por subproceso (están instalados); si faltan, mensaje claro y se sigue con lo que haya.
- Límites: máx. 5 adjuntos por mensaje, máx. 20 páginas por PDF (si hay más: aviso y solo las 20 primeras), imágenes reducidas a 1568 px en el lado largo y JPEG calidad 85, total ≤ 20 MB por petición. Mensajes de error en español.
- Los adjuntos no se guardan: viven solo mientras se envía el mensaje. Nada de esto toca `order.toml`.
- Las tarjetas Aplicar/Descartar siguen igual: el agente propone, el usuario aprueba.
- Voz en Android: Ajustes → Voz con fichas **Android (gratis)** (por defecto) · OpenAI · Groq. Escritorio sin cambios en voz.

## Puntos de revisión
1. **PDF escaneado** (sin texto) con DeepSeek → OCR en el dispositivo; si el OCR tampoco saca texto, mensaje "No pude leer texto en este archivo; prueba con Gemini o Claude."
2. **PDF con contraseña o dañado** → mensaje legible, no se envía nada, la app no se cae.
3. **Archivo grande** (> 20 páginas o > 20 MB) → aviso y recorte, nunca un 413 sin explicar.
4. **Foto de cámara girada** → se respeta la orientación EXIF antes de reducir.
5. **Sin permiso de micrófono / reconocimiento de voz no disponible** → mensaje y sugerencia de usar el micrófono de Gboard.

---

### Tarea 1: Adjuntos en `order-core`
**Archivos:** `crates/order-core/src/agente/adjuntos.rs` (nuevo), `cliente.rs`, `config.rs`, `mod.rs`.
- [ ] Tipos: `Adjunto { nombre: String, contenido: ContenidoAdjunto }`, `ContenidoAdjunto::{Pdf { bytes, paginas_jpeg: Vec<Vec<u8>>, texto_ocr: Option<String> }, Imagen { jpeg: Vec<u8>, texto_ocr: Option<String> }}`. `extraer_texto_pdf(&[u8]) -> Result<Vec<String>, ErrorAdjunto>` (una entrada por página) y `paginas_sin_texto(&[String]) -> Vec<usize>`.
- [ ] `AgenteConfig::ve_imagenes()` (Claude y Gemini sí, DeepSeek no, Otro según nuevo campo `ve_imagenes: bool` en `agente.toml`, por defecto false).
- [ ] `consultar(config, contexto, pregunta, adjuntos: &[Adjunto])`: arma el contenido según la tabla de Decisiones. Para modelos sin visión, el texto va en el mensaje como bloques `--- archivo: nombre (página N) ---`.
- [ ] Validación de límites con `ErrorAdjunto` legible.
- [ ] Pruebas sin red: cuerpo Claude con `document` antes del texto; cuerpo Gemini con `image_url` data URL; cuerpo DeepSeek solo texto; PDF de prueba pequeño (generado en la prueba o en `tests/datos/`) con texto extraído; límites. Escritorio y FFI siguen compilando (pasar `&[]` donde aún no hay adjuntos). Commit `feat: adjuntos para el agente en order-core`.

### Tarea 2: FFI
**Archivos:** `crates/order-ffi/src/lib.rs`.
- [ ] `AdjuntoFfi` (Record) y `enviar_al_agente(ruta_datos, ruta_config, texto, adjuntos: Vec<AdjuntoFfi>)`; `texto_de_pdf(bytes) -> Vec<String>` para que Android sepa qué páginas necesitan OCR; `agente_ve_imagenes(ruta_config) -> bool`.
- [ ] Pruebas con servidor simulado como las actuales. Commit `feat: adjuntos en la ffi`.

### Tarea 3: Android — adjuntar
**Archivos:** `.../ui/agente/BarraAgente.kt`, `.../agente/Adjuntos.kt` (nuevo: PdfRenderer, reducción + EXIF, ML Kit), `AndroidManifest.xml`, `FileProvider`.
- [ ] Botón 📎 en la barra con menú: **Archivo (PDF o imagen)** (`OpenDocument`, `application/pdf` e `image/*`) y **Tomar foto** (`TakePicture` a caché vía `FileProvider`; permiso `CAMERA` solo si hace falta).
- [ ] Píldoras sobre el campo con nombre del archivo y ✕; indicador "leyendo archivo…" mientras se prepara.
- [ ] Preparación en `Dispatchers.IO`: si el modelo ve imágenes → páginas JPEG; si no → `texto_de_pdf` + ML Kit en las páginas sin texto / en fotos.
- [ ] Prueba unitaria Kotlin del armado de adjuntos con fakes (elige visión u OCR según `agente_ve_imagenes`). Commit `feat: adjuntar PDF y fotos al agente en Android`.

### Tarea 4: Android — voz del sistema
**Archivos:** `.../voz/VozAndroid.kt` (nuevo), `BarraAgente.kt`, `AjustesAgente.kt`, preferencias locales.
- [ ] `SpeechRecognizer` en `es-CO` (o idioma del sistema si no está), resultados parciales en el campo, el texto queda para revisar antes de enviar.
- [ ] Ficha **Android (gratis)** por defecto en Ajustes → Voz; OpenAI y Groq siguen como opción con su flujo actual. Commit `feat: voz del sistema en Android`.

### Tarea 5: Escritorio — adjuntar
**Archivos:** `crates/order-escritorio/src/{adjuntos.rs (nuevo), puente.rs}`, `ui/agente.slint`.
- [ ] Botón 📎 en el cuadro del agente que abre el selector de archivos del sistema (crate `rfd` con portal XDG; filtros PDF e imágenes). Píldoras con ✕ como en Android, mismo estilo (`tema.slint`).
- [ ] `pdftoppm -jpeg -r 110` para páginas cuando el modelo ve imágenes; `tesseract - - -l spa` para OCR; si no están instalados, mensaje claro.
- [ ] Commit `feat: adjuntar PDF y fotos al agente en escritorio`.

### Tarea 6: Cierre
- [ ] `./compilar-rust.sh` + `assembleDebug` desde cero; `cargo install --path crates/order-escritorio --locked`. Actualizar `android/README.md` y `README.md` (qué se manda a cada proveedor). Commit `docs: adjuntos y voz del sistema`.
