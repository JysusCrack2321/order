# ORDER — Conversación con contexto para el agente — Plan de implementación

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría). Leer este plan y el código actual de `crates/order-core/src/agente/`, `crates/order-ffi`, `crates/order-escritorio` (puente.rs, ui/agente.slint, ui/app.slint) y `android/.../ui/agente/`. Verificar con `cargo test --workspace` (sin warnings) y `JAVA_HOME=/usr/lib/jvm/java-17-openjdk ./gradlew assembleDebug testDebugUnitTest`. No abrir ventanas ni emuladores, no instalar en el teléfono, no hacer llamadas reales a APIs.

**Objetivo:** que el agente entienda la conversación ("no, mejor el viernes") sin reenviar todo el historial: el contexto se construye compacto en cada petición.

**Por qué así:** los modelos no recuerdan nada; la app debe reenviarles lo hablado. Reenviar la conversación completa crece sin límite y es ineficiente, así que se manda un resumen que el propio modelo mantiene, más lo último literal.

## Cómo se arma cada petición (en este orden, dentro del mensaje del usuario)
1. **Agenda actual** (lo que ya se manda hoy, fresca en cada petición; el historial nunca la repite).
2. **Resumen de la conversación**: texto corto que el modelo devolvió en su respuesta anterior (vacío al empezar).
3. **Últimos intercambios**: los 3 más recientes, literales (petición del usuario y `respuesta` del agente), cada texto recortado a 500 caracteres. Los adjuntos aparecen solo como `[adjuntó: horario.pdf]`, nunca su contenido.
4. **Qué pasó con tus propuestas**: una línea por acción desde la última petición: `aplicada: Crear tarea "Informe de Control" (miércoles)` / `descartada: …`.
5. **Petición nueva** (+ adjuntos, como hoy).

## Decisiones
- El esquema de respuesta (`acciones::esquema()`) gana un campo **`resumen`** (string): "Resumen actualizado de toda la conversación, incluida esta petición y tu respuesta; máximo 5 líneas; solo lo que sirva para seguirla (de qué se habla, qué quiere el usuario, qué decidió)". El system prompt lo explica en una línea. Sin llamada extra.
- Si el modelo no devuelve `resumen` (o viene vacío), se conserva el anterior. Se recorta a 800 caracteres.
- Estado en memoria, nunca en disco ni en `order.toml`: `Conversacion { resumen, turnos: VecDeque<Turno> (máx. 3), resultados: Vec<String> }`. `resultados` se vacía después de enviarse.
- **"Nueva conversación"** borra todo. Cerrar la app también.
- Las tarjetas Aplicar/Descartar no cambian; solo se anota el resultado.

## Puntos de revisión
1. Conversación de 20 mensajes → la petición lleva resumen + 3 intercambios, no 20.
2. El modelo omite `resumen` → no se pierde el anterior, no hay error.
3. Adjunto en un turno viejo → no se reenvía el archivo, solo su nombre.
4. Acción aplicada y luego "deshazlo" → el modelo ve en la sección 4 que se aplicó.
5. "Nueva conversación" → la siguiente petición va sin resumen ni intercambios.

---

### Tarea 1: `order-core`
**Archivos:** `crates/order-core/src/agente/conversacion.rs` (nuevo), `cliente.rs`, `acciones.rs`, `mod.rs`.
- [ ] `Conversacion` con `nueva()`, `registrar_turno(peticion, nombres_adjuntos, respuesta, resumen: Option<String>)`, `anotar_resultado(descripcion, aplicada: bool)`, `bloque_contexto() -> String` (secciones 2–4 con títulos en español; vacío si no hay nada) y `tomar_resultados()`.
- [ ] `consultar(config, contexto, pregunta, adjuntos, conversacion: &Conversacion)`; el parseo de la respuesta expone `resumen: Option<String>`. Campo `resumen` en el esquema y en el system prompt.
- [ ] Pruebas: solo 3 turnos en el bloque; recortes de 500/800; adjuntos solo por nombre; resultados se vacían tras enviarse; `resumen` ausente conserva el anterior; esquema y cuerpo Claude/OpenAI incluyen el bloque en el orden de arriba; `nueva()` vacía. Commit `feat: contexto de conversación en order-core`.

### Tarea 2: FFI
**Archivos:** `crates/order-ffi/src/lib.rs`.
- [ ] Una `Conversacion` global (`Mutex`, como `ACCIONES_PENDIENTES`). `enviar_al_agente` la usa y la actualiza; `aplicar_accion`, `aplicar_todas` y `descartar_accion` anotan el resultado con la descripción de la acción; nueva `nueva_conversacion()`.
- [ ] Pruebas con servidor simulado: segunda petición lleva el resumen devuelto por la primera; descartar se refleja en la siguiente. Commit `feat: conversación del agente en la ffi`.

### Tarea 3: Escritorio
**Archivos:** `crates/order-escritorio/src/puente.rs`, `ui/agente.slint` (y `app.slint` si hace falta), `ui/tema.slint` para cualquier tamaño o color nuevo.
- [ ] `Conversacion` en el estado del puente; mismas anotaciones al aplicar/descartar.
- [ ] Botón **"Nueva conversación"** en el panel del agente (visible solo si hay conversación), mismo estilo. Commit `feat: conversación del agente en escritorio`.

### Tarea 4: Android
**Archivos:** `.../ui/agente/{BarraAgente.kt, PanelRespuesta.kt}`, `.../datos/Repositorio.kt`.
- [ ] Botón **"Nueva conversación"** (visible solo si hay conversación) que llama a `nueva_conversacion()`; el resto vive en Rust.
- [ ] Prueba unitaria Kotlin con el fake del puente. Commit `feat: conversación del agente en Android`.

### Tarea 5: Cierre
- [ ] `./android/compilar-rust.sh` + `assembleDebug` desde cero; `cargo install --path crates/order-escritorio --locked`; copiar el APK a `dist/ORDER.apk`. README: una línea sobre cómo se arma el contexto. Commit `docs: conversación del agente`.
