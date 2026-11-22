# ORDER — Historial completo con compactación y lectura de tablas en PDF — Plan de implementación

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría). Leer este plan y el código actual de `crates/order-core/src/agente/` (sobre todo `conversacion.rs`, `adjuntos.rs`, `cliente.rs`, `config.rs`), `crates/order-ffi/src/lib.rs`, `crates/order-escritorio/src/puente.rs` y `android/.../ui/agente/`. Verificar con `cargo test --workspace` (sin warnings) y `JAVA_HOME=/usr/lib/jvm/java-17-openjdk ./gradlew assembleDebug testDebugUnitTest` (desde `android/`). No abrir ventanas ni emuladores, no instalar en el teléfono, no hacer llamadas reales a APIs.

**Objetivo:** que el agente recuerde toda la conversación (incluido el contenido de los PDFs que se le pasaron) y que entienda tablas como el horario de la UNAL.

**Problemas reales que resuelve:**
1. `pdf_extract::extract_text_from_mem_by_pages` saca el horario **columna por columna** (todos los días, luego todas las materias, luego profesores, luego horas), así que el modelo no puede asociar hora con materia. `pdftotext -layout` sí conserva las filas (`LUNES [2022888] Técnicas de control … 11:00 - 13:00`).
2. La conversación actual guarda solo 3 intercambios recortados a 500 caracteres y de los adjuntos solo el nombre: a las 3 preguntas se olvida del horario.

## Parte A — Texto de PDF con filas (layout)
- Nuevo extractor en `order-core` (`agente/maquetado.rs` o dentro de `adjuntos.rs`) que use las **posiciones** de cada fragmento de texto: implementar el trait `pdf_extract::OutputDev` (o leer con `lopdf` directamente) para obtener (x, y, tamaño, texto) por página.
- Reconstrucción: agrupar fragmentos en líneas por `y` (tolerancia ≈ mitad del tamaño de fuente), ordenar cada línea por `x`, y convertir los huecos horizontales en espacios proporcionales (≈ ancho medio de carácter) para que las columnas queden alineadas como `pdftotext -layout`. Quitar líneas vacías repetidas.
- `extraer_texto_pdf` pasa a devolver este texto maquetado (misma firma). Si el maquetado falla o sale vacío, se cae al `extract_text_from_mem_by_pages` actual.
- Funciona igual en escritorio y Android (Rust puro, sin `pdftotext`).
- Prueba: PDF de prueba generado en la prueba con una tabla de 3 columnas × 3 filas escritas en orden de columnas en el stream → el texto sale por filas (`LUNES  Control  11:00 - 13:00`). Si es posible copiar una versión anonimizada del horario a `crates/order-core/tests/datos/` (cambiar nombre, documento y matrícula por valores ficticios), mejor; si no, la sintética basta.

## Parte B — Historial completo persistente con compactación

### Qué se guarda
`Conversacion` (reemplaza la actual, misma ubicación `agente/conversacion.rs`):
- `resumen: String` — bloque de compactación (vacío hasta la primera compactación).
- `documentos: Vec<Documento { nombre, texto }>` — **texto completo** de cada adjunto (PDF maquetado, o OCR/descripción de foto). Se guarda una sola vez al adjuntar y **nunca se compacta ni se recorta** salvo el tope de abajo. Para modelos con visión, el archivo se manda como hoy solo en el turno en que se adjunta, y además se guarda su texto extraído aquí (para Claude/Gemini extraer texto con el mismo extractor; si el PDF no tiene texto, guardar lo que devuelva el OCR de la plataforma si ya existe, o solo el nombre).
- `mensajes: Vec<Mensaje { rol: Usuario|Agente, texto, adjuntos: Vec<String> }>` — literales, sin recortar.
- `resultados: Vec<String>` — aplicadas/descartadas pendientes de reportar (como hoy).
- Tope de documentos: 200 000 caracteres en total; si se pasa, el documento más viejo se reemplaza por "[documento retirado: nombre]" y se avisa en la respuesta.

### Persistencia
- Archivo `conversacion.json` (serde) junto a `agente.toml` (`config::junto_a`), privado, **nunca** en la carpeta de datos ni en `order.toml`. Escritorio: carpeta de config de ORDER; Android: `filesDir`.
- Se carga al arrancar y se guarda (escritura atómica: temporal + rename) después de cada turno, cada compactación, cada aplicar/descartar y en "Nueva conversación" (que lo borra).
- JSON corrupto → se ignora, se empieza vacío, sin crash.

### Cómo se arma cada petición (mensaje del usuario, en este orden)
1. Agenda actual (como hoy).
2. `## Documentos de la conversación` — cada documento con `--- archivo: nombre ---` y su texto.
3. `## Resumen de lo hablado antes` — si hay resumen.
4. `## Conversación` — todos los mensajes literales (`Usuario:` / `ORDER:`), con `[adjuntó: nombre]`.
5. `## Qué pasó con tus propuestas` — resultados (se vacían tras enviarse).
6. Petición nueva (+ adjuntos binarios si el modelo ve imágenes, como hoy).

Se elimina el campo `resumen` del esquema de respuesta y su línea del system prompt (ya no hace falta). Verificar que el parseo siga aceptando respuestas que todavía lo traigan.

### Compactación (inspirada en la de Claude Code)
- Tamaño estimado = caracteres de secciones 2–5 / 4 (tokens aprox.). Umbral configurable `limite_contexto_tokens` en `agente.toml` (por defecto **60 000**), editable en Ajustes → Agente → Avanzado en escritorio y Android.
- **Antes** de enviar la petición, si se pasa el umbral: llamada aparte `compactar(config, conversacion)` en `cliente.rs`, mismo proveedor y modelo, sin esquema JSON de acciones (respuesta en texto libre).
- Se compactan todos los mensajes **menos los últimos 10**, más el resumen anterior si existe. Los documentos no entran en la compactación (siguen enteros).
- Prompt de compactación (en español, constante en `conversacion.rs`):
  > Resume esta conversación entre una persona y ORDER (su agenda de clases, tareas y recordatorios) para poder seguirla sin el texto original. Escribe en español, con estas secciones y nada más:
  > 1. **Qué quiere la persona**: objetivos y peticiones, en sus palabras cuando importe.
  > 2. **Preferencias y datos personales que dio**: cómo le gusta que se hagan las cosas, nombres, materias, horarios, fechas.
  > 3. **Lo que ya se hizo**: acciones aplicadas (qué tarea/clase/recordatorio, qué día) y las descartadas.
  > 4. **Decisiones y correcciones**: lo que cambió de opinión ("no, mejor el viernes") y lo que dijo que estaba mal.
  > 5. **Pendiente**: lo que pidió y aún no se hizo.
  > 6. **Tema actual**: de qué se hablaba al final.
  > Incluye el resumen anterior si lo hay; no pierdas ningún dato concreto (fechas, horas, nombres). Máximo 4 000 caracteres.
- Resultado: `resumen` = texto devuelto (recortado a 6 000 caracteres), `mensajes` = últimos 10. Si la compactación falla (red, 429), se envía la petición sin compactar y se reintenta en el turno siguiente; si la petición misma da error de contexto demasiado largo, compactar y reintentar una vez.
- La UI muestra "Compactando conversación…" mientras dura (escritorio y Android), reutilizando el indicador "pensando…".

### Puntos de revisión
1. Pasar el horario, luego 5 preguntas más → la sexta petición todavía lleva el texto completo del horario en "Documentos".
2. Horario PDF con DeepSeek → el texto enviado tiene filas: día, materia, grupo, profesor, aula y hora en la misma línea.
3. Conversación que pasa el umbral → una llamada de compactación con el prompt de arriba, luego la petición con resumen + 10 mensajes + documentos.
4. Cerrar y abrir la app → la conversación sigue (escritorio y Android).
5. "Nueva conversación" → borra `conversacion.json`; la siguiente petición va sin documentos, resumen ni mensajes.
6. `conversacion.json` corrupto → arranque limpio.
7. La API key nunca aparece en `conversacion.json` ni en logs.

---

### Tarea 1: PDF con filas en `order-core`
**Archivos:** `crates/order-core/src/agente/adjuntos.rs` (+ módulo nuevo si conviene), `Cargo.toml` si hace falta `lopdf` directo.
- [ ] Extractor maquetado + respaldo + pruebas de la Parte A. Commit `feat: texto de PDF por filas`.

### Tarea 2: Conversación persistente y compactación en `order-core`
**Archivos:** `agente/conversacion.rs`, `cliente.rs`, `acciones.rs` (quitar `resumen` del esquema), `config.rs` (`limite_contexto_tokens`).
- [ ] Nuevo `Conversacion` con `documentos`, `mensajes`, `resumen`, `resultados`; `cargar_de(ruta)`, `guardar_en(ruta)`, `agregar_documento`, `registrar_turno(peticion, adjuntos, respuesta)`, `anotar_resultado`, `bloque_contexto()`, `tokens_estimados()`, `necesita_compactar(limite)`, `aplicar_compactacion(resumen)`.
- [ ] `cliente::compactar(config, &Conversacion) -> Result<String, _>` con el prompt de arriba (cuerpo Claude y OpenAI-compatible) y `consultar` usando el nuevo bloque.
- [ ] Pruebas sin red: documentos sobreviven a muchos turnos y a la compactación; compactación deja 10 mensajes; orden de secciones; JSON ida y vuelta; JSON corrupto; tope de documentos; cuerpo de compactación correcto. Commit `feat: historial completo con compactación en order-core`.

### Tarea 3: FFI
**Archivos:** `crates/order-ffi/src/lib.rs`.
- [ ] La `CONVERSACION` global se carga de `conversacion.json` junto a `ruta_config` la primera vez y se guarda tras cada cambio. `enviar_al_agente` compacta si hace falta antes de consultar; nueva función `necesita_compactar(ruta_config) -> bool` para que Android muestre el aviso. `nueva_conversacion()` borra el archivo. Adjuntos: guardar su texto como documento.
- [ ] Pruebas con servidor simulado: segunda petición lleva el documento del primer turno; con umbral bajo hay una llamada de compactación antes de la petición; persistencia entre "reinicios" (recargar desde archivo). Commit `feat: historial persistente en la ffi`.

### Tarea 4: Escritorio
**Archivos:** `crates/order-escritorio/src/puente.rs`, `ui/agente.slint`, `ui/ajustes*.slint` (Avanzado), `ui/tema.slint` si hace falta.
- [ ] Cargar/guardar la conversación junto a `agente.toml`; compactar antes de enviar con aviso "Compactando conversación…"; campo "Límite de contexto (tokens)" en Avanzado. Commit `feat: historial completo en escritorio`.

### Tarea 5: Android
**Archivos:** `.../ui/agente/{BarraAgente.kt, AjustesAgente.kt}`, `.../datos/Repositorio.kt`.
- [ ] Aviso "Compactando conversación…" cuando `necesita_compactar` es true; campo de límite en Avanzado; "Nueva conversación" visible si hay conversación guardada (también tras reiniciar la app).
- [ ] Prueba unitaria Kotlin con el fake del puente. Commit `feat: historial completo en Android`.

### Tarea 6: Cierre
- [ ] `./android/compilar-rust.sh` + `assembleDebug` desde cero; `cargo install --path crates/order-escritorio --locked`; copiar el APK a `dist/ORDER.apk`. README: cómo se arma el contexto, dónde vive `conversacion.json` y qué es la compactación. Commit `docs: historial y compactación del agente`.
