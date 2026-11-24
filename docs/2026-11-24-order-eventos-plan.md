# ORDER — Eventos de un solo día — Plan de implementación

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría, mismo autor del último commit, sin push). Leer este plan y el código de `crates/order-core/src/{modelo.rs, ops.rs, avisos.rs, semana.rs, presentacion/, sincronizacion/, agente/}`, `crates/order-ffi/src/lib.rs`, `crates/order-escritorio/src/` + `ui/{dia,semana,inicio,formularios,pendientes}.slint` y `android/.../{ui/dia, ui/semana, ui/inicio, ui/formularios, ui/pendientes, avisos/, datos/Repositorio.kt}`. Verificar con `cargo test --workspace` (sin warnings) y `./android/compilar-rust.sh && ./android/gradlew -p android assembleDebug testDebugUnitTest`. No abrir ventanas ni emuladores, no instalar en teléfonos, no publicar.

**Objetivo:** además de las clases (se repiten cada semana del semestre), poder poner cosas que pasan **una sola vez**: "estudiar control el jueves de 3 a 4", o reservar un rato para hacer una tarea. Si ese rato pasa y la tarea no quedó hecha, ORDER pregunta y deja reagendarla.

## Decisiones
- **Modelo:** `[[evento]]` en `order.toml`: `id` (`e-...`, como los demás), `titulo`, `fecha` (YYYY-MM-DD), `inicio` y `fin` (HH:MM, fin > inicio), `tarea` opcional (id de la tarea a la que pertenece), `nota` opcional. Se serializa solo lo que existe; los TOML viejos siguen válidos (prueba ida y vuelta).
- **Sincronización P2P:** `evento` es una unidad más del LWW por campo (por `id`, con lápidas), igual que tarea/recordatorio. Pruebas: crear en uno llega al otro; mover la hora en uno y cambiar el título en el otro conserva ambos; borrar propaga.
- **Si se borra la tarea**, sus eventos quedan como eventos sueltos (se quita el enlace `tarea`), no se borran.
- **Vista día y semana** (escritorio y Android): los eventos se dibujan en la misma línea de tiempo que las clases, con estilo distinto (por ejemplo borde o fondo con trama, ícono de reloj), y con el color de la materia de la tarea si tiene. Si la tarea ya está hecha, el evento se ve atenuado/tachado. Inicio/"hoy" los lista junto a las clases del día.
- **Crear:** botón "Nuevo evento" (en el día/semana) con título, fecha, inicio, fin. En el detalle de una tarea, botón **Agendar** que crea un evento ligado con el título de la tarea (solo se elige día y hora; duración por defecto 1 h). Una tarea puede tener varios eventos. En la tarea se muestran sus eventos próximos.
- **Editar/borrar:** tocar el evento abre su formulario (mover día/hora, borrar). Atajos y Esc del escritorio funcionan igual que en los demás formularios.
- **Avisos:** al empezar el evento (y, si la persona tiene configurado un aviso previo para clases, reutilizar esa misma antelación). En escritorio por el reloj de avisos; en Android entra en `proximosAvisos` y se programa como los demás. Clave de aviso estable por id+fecha+inicio, para que mover el evento genere un aviso nuevo.
- **Reagendar:** cuando termina un evento ligado a una tarea **no hecha**, aparece (una sola vez por evento):
  - Escritorio: notificación "¿Terminaste {tarea}?" y, en la app, una tarjeta en Inicio/Pendientes con **Hecha** y **Reagendar**.
  - Android: notificación con acciones **Hecha** y **Reagendar** (Reagendar abre la app en el selector de día/hora del evento) y la misma tarjeta en Inicio.
  - Hecha → marca la tarea hecha. Reagendar → mueve ese evento a la nueva fecha/hora (no crea otro). Si se ignora, la tarjeta sigue hasta que se resuelva o se borre el evento.
- **Agente:** herramientas para crear, mover y borrar eventos ("mañana a las 3 estudio control una hora", "agenda la tarea 3 de control el jueves a las 10"). Actualizar el prompt de sistema con la diferencia entre clase (semanal), recordatorio (semanal), tarea (con fecha de entrega) y evento (una vez).
- **Fuera de este plan:** convertir eventos de iCal en eventos (siguen entrando como tareas); eventos de varios días o repetidos.

## Puntos de revisión
1. Evento suelto mañana 15:00–16:00 → aparece en día y semana, avisa a las 15:00 (y antes si hay antelación configurada).
2. "Agendar" desde una tarea → evento ligado con su título y color de materia.
3. Pasa el evento con la tarea sin hacer → pregunta; Reagendar lo mueve; Hecha marca la tarea.
4. Tarea hecha antes del evento → evento atenuado, sin pregunta al terminar.
5. Crear en el PC → llega al celular por sync, y viceversa; mover en uno se refleja en el otro.
6. `order.toml` viejo sin eventos sigue cargando; uno con eventos se lee a mano sin sorpresas.

---

### Tarea 1: núcleo (`order-core`)
- [ ] Modelo `Evento` + validación (fin > inicio, tarea existente o se quita), ops crear/editar/mover/borrar, borrado de tarea suelta sus eventos, presentación para día/semana/inicio, avisos (inicio y "¿terminaste?"), unidad de sincronización LWW. Pruebas de todo lo anterior. Commit `feat: eventos de un solo día en order-core`.

### Tarea 2: agente
- [ ] Herramientas de eventos y prompt de sistema. Pruebas con el proveedor simulado. Commit `feat: el agente agenda eventos`.

### Tarea 3: FFI
- [ ] Exponer eventos (listar por rango, crear, editar, borrar, reagendar) y que `proximosAvisos` incluya eventos y preguntas de reagendar. Pruebas Rust. Commit `feat: eventos en la ffi`.

### Tarea 4: escritorio
- [ ] Eventos en día/semana/inicio, formulario, botón Agendar en tareas, tarjeta y notificación de reagendar. Commit `feat: eventos en escritorio`.

### Tarea 5: Android
- [ ] Lo mismo en Compose, notificación con acciones Hecha/Reagendar, reprogramar avisos tras cambios. Prueba Kotlin con el fake del puente. Commit `feat: eventos en Android`.

### Tarea 6: cierre
- [ ] README (qué es un evento frente a clase/tarea/recordatorio, Agendar y Reagendar). Versión 0.2.0 / versionCode 4 (Android y los cuatro Cargo.toml). `cargo build --release -p order-escritorio` y APK debug a `dist/ORDER.apk`. Commits `docs: eventos` y `chore: versión 0.2.0`. Al final, resumen corto y pasos de prueba manual.
