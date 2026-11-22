# ORDER — Moodle y calendarios iCal sincronizados solos — Plan de implementación

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría). Leer este plan y el código de `crates/order-core/src/{modelo.rs, ops.rs, almacen.rs, agente/config.rs}`, `crates/order-ffi/src/lib.rs`, `crates/order-escritorio/src/puente.rs` + `ui/` (ajustes) y `android/.../{avisos/TrabajoRespaldo.kt, datos/Repositorio.kt, ui/}`. Verificar con `cargo test --workspace` (sin warnings) y `JAVA_HOME=/usr/lib/jvm/java-17-openjdk ./gradlew assembleDebug testDebugUnitTest`. No abrir ventanas ni emuladores, no instalar en el teléfono, **no hacer llamadas reales** a Moodle ni a APIs (servidor simulado en las pruebas).

**Objetivo:** la persona inicia sesión una vez con su usuario de Moodle dentro de ORDER y sus entregas (tareas, cuestionarios, foros con fecha) aparecen como tareas de ORDER y se mantienen al día solas.

**Lo que ya se comprobó del Moodle de la UNAL (`https://micampus.unal.edu.co`):** Moodle 4.5 (`version=2024100708`), `enablewebservices=1`, `enablemobilewebservice=1`, `typeoflogin=1` (usuario y contraseña directos, sin SSO). `login/token.php?service=moodle_mobile_app` responde (con credenciales falsas: `invalidlogin`).

## Decisiones
- **Inicio de sesión:** `POST {sitio}/login/token.php` con `username`, `password`, `service=moodle_mobile_app` → `token`. **La contraseña nunca se guarda**; solo el token. Antes, consultar `lib/ajax/service-nologin.php` `tool_mobile_get_public_config`: si `typeoflogin != 1` o el servicio móvil está apagado, decir "Este Moodle no permite iniciar sesión desde otras apps." (otros Moodle con SSO quedan fuera por ahora).
- **Dónde se guarda:** `moodle.toml` junto a `agente.toml` (privado, `config::junto_a`; Android `filesDir`), con `sitio`, `token`, `usuario` (nombre visible), `ultima_sincronizacion`. Nunca en la carpeta de datos ni en `order.toml`; el token nunca se imprime ni va a logs.
- **Qué se trae** (REST `webservice/rest/server.php?wstoken=…&moodlewsrestformat=json`):
  - `core_webservice_get_site_info` (nombre, userid).
  - `core_calendar_get_action_events_by_timesort` con `timesortfrom` = hoy − 14 días, `limitnum` 50 (paginar con `aftereventid` si llegan 50): nombre de la actividad, curso, fecha (`timesort`), `url`, `modulename`, `instance`.
  - Estado de entrega: para `assign`, `mod_assign_get_submission_status` (`submitted` → hecha); para el resto, `core_completion_get_activities_completion_status` del curso (completa → hecha). Si una función no está permitida, se ignora ese dato (no rompe la sincronización).
- **Modelo:** `Tarea` gana dos campos opcionales: `origen: Option<String>` (`"moodle:{sitio}:{modulename}:{instance}"`) y `enlace: Option<String>` (URL de la actividad). Se serializan solo si existen; los TOML viejos siguen válidos.
- **Reglas de sincronización** (función pura en `order-core/src/moodle/sincronizar.rs`, `aplicar(datos, eventos, hoy) -> Cambios`, probada sin red):
  - Evento sin tarea con ese `origen` → crear tarea (`titulo` = nombre de la actividad, `vence` = fecha en hora local, `materia` = ver abajo).
  - Tarea existente con ese `origen` → actualizar `titulo`, `vence` y `enlace` si cambiaron en Moodle.
  - Moodle dice entregada/completa → `hecha = true`, `hecha_en = hoy`. Nunca se desmarca una tarea que la persona marcó hecha a mano.
  - Una tarea de Moodle que ya no aparece **no se borra** (puede ser vieja o ya entregada).
  - Tareas sin `origen` (las creadas a mano) nunca se tocan.
  - Se escribe con `Almacen::modificar` (respaldo, escritura atómica) y solo si hubo cambios.
- **Materia:** el nombre del curso de Moodle se compara con las materias de ORDER sin mayúsculas ni acentos, por código (`[2022888]`, `2022888`) o porque un nombre contiene al otro. Si coincide, se usa la materia de ORDER; si no, el nombre del curso sin el código ni el grupo.
- **Cuándo se sincroniza:** al abrir la app, cada 3 horas mientras está abierta (escritorio) y en el `TrabajoRespaldo` periódico de Android; botón "Sincronizar ahora". Sin red → se intenta la próxima vez, sin mensajes molestos. Token rechazado (`invalidtoken`) → aviso "Vuelve a iniciar sesión en Moodle" en Ajustes y banner suave.
- **Varios dispositivos:** como el `origen` es único, si el PC y el celular sincronizan el mismo archivo no se duplican tareas.
- **Interfaz:** Ajustes → **Moodle** (escritorio y Android, estilo ORDER): campo Sitio (por defecto `micampus.unal.edu.co`), Usuario, Contraseña (oculta), **Conectar**; ya conectado: "Conectado como {nombre} · última sincronización hace X", **Sincronizar ahora**, **Desconectar** (borra `moodle.toml`, no borra tareas). En la edición de una tarea con `enlace`, botón **Abrir en Moodle**.
- Avisos: las tareas de Moodle avisan igual que las demás (ya lo hacen por ser tareas).

## Puntos de revisión
1. Primera conexión con 5 entregas futuras → 5 tareas con su materia correcta; segunda sincronización sin cambios → archivo intacto.
2. El profesor cambia la fecha → la tarea se actualiza; la persona había editado el título → se respeta la regla (Moodle manda en título/fecha de tareas con `origen`).
3. Entregar en Moodle → la tarea queda hecha en la siguiente sincronización.
4. Contraseña errada → "Usuario o contraseña incorrectos." Sitio que no es Moodle o sin servicio móvil → mensaje claro.
5. Token vencido → aviso de volver a iniciar sesión; nada se borra.
6. `moodle.toml` nunca contiene la contraseña.

## Parte B — Calendarios iCal (Google Calendar u otros)
- Ajustes → **Calendarios**: lista de calendarios agregados (nombre, color de materia opcional, "última actualización"), botón **Agregar calendario** con campo para pegar la dirección iCal (`https://…/basic.ics`, también `webcal://`) y materia opcional (por defecto se intenta emparejar por nombre como en Moodle), y **Quitar** (no borra lo ya creado).
- Comprobado: `https://calendar.google.com/calendar/ical/{id}/public/basic.ics` de un calendario público de la UNAL devuelve VEVENT con `SUMMARY`/`DTSTART` (UTC y `VALUE=DATE`).
- Se guardan en `calendarios.toml` junto a `agente.toml` (privado; las direcciones secretas son como contraseñas: nunca en `order.toml` ni en logs).
- Parser ICS en Rust puro dentro de `order-core` (crate `icalendar` o parser propio pequeño): líneas plegadas, `TZID`, UTC (`Z`), fechas de todo el día (vencen a las 23:59), `RRULE` solo si es sencillo (si no, solo la primera ocurrencia futura), `STATUS:CANCELLED` se ignora.
- Solo eventos de hoy − 1 día a hoy + 120 días. `origen = "ical:{hash corto de la URL}:{UID}"` (+ fecha si es recurrencia). Mismas reglas que Moodle: crear, actualizar título/fecha, no borrar, no tocar lo hecho a mano. Como el iCal no dice si se entregó, nunca se marca hecha sola.
- Los eventos entran siempre como **tareas** (el modelo no tiene recordatorios de un solo día).
- Atajo: al abrir ORDER (o volver a ella) con una dirección iCal en el portapapeles (`.ics` o `webcal://` o `calendar.google.com/calendar/ical/`), ofrecer "¿Agregar este calendario?" con un toque (escritorio y Android).
- Misma frecuencia que Moodle (al abrir, cada 3 h en escritorio, `TrabajoRespaldo` en Android) y mismo botón "Sincronizar ahora" (sincroniza Moodle y calendarios).
- Todo opcional: sin Moodle ni calendarios, ORDER no muestra nada de esto fuera de Ajustes.

### Tarea extra (después de la Tarea 5 y antes del Cierre): Calendarios iCal
- [ ] `order-core`: parser ICS + reglas + `calendarios.toml` + pruebas (ICS de ejemplo con UTC, TZID, todo el día, cancelado, líneas plegadas; segunda sincronización sin cambios no escribe). FFI y UI en escritorio y Android con el atajo del portapapeles. Commit `feat: calendarios iCal`.

---

### Tarea 1: Modelo y reglas en `order-core`
- [ ] Campos `origen` y `enlace` en `Tarea` (serde opcional, TOML viejo válido, prueba ida y vuelta). Módulo `moodle/` con tipos de evento, emparejado de materia y `sincronizar::aplicar` con todas las reglas de arriba y sus pruebas. Commit `feat: reglas de sincronización con Moodle`.

### Tarea 2: Cliente Moodle en `order-core`
- [ ] `moodle/cliente.rs`: configuración pública, login (token), site info, eventos con paginación, estado de entrega/compleción; errores en español (`invalidlogin`, `invalidtoken`, sin red, no es Moodle). `moodle/config.rs` para `moodle.toml`. `sincronizar_ahora(ruta_datos, ruta_moodle, hoy)` que junta cliente + reglas + `Almacen::modificar`. Pruebas con servidor HTTP simulado (como las del agente). Commit `feat: cliente de Moodle en order-core`.

### Tarea 3: FFI
- [ ] `moodle_conectar(ruta_config, sitio, usuario, contrasena)`, `moodle_estado(ruta_config)`, `moodle_sincronizar(ruta_datos, ruta_config) -> ResumenSync {creadas, actualizadas, hechas}`, `moodle_desconectar`. Pruebas con servidor simulado. Commit `feat: Moodle en la ffi`.

### Tarea 4: Escritorio
- [ ] Sección Moodle en Ajustes, sincronización al abrir + temporizador de 3 h (en hilo aparte, sin congelar la UI), botón "Abrir en Moodle" (xdg-open). Commit `feat: Moodle en escritorio`.

### Tarea 5: Android
- [ ] Sección Moodle en Ajustes, sincronización al abrir y en `TrabajoRespaldo` (luego reprogramar avisos), botón "Abrir en Moodle" (Intent). Prueba unitaria Kotlin con el fake del puente. Commit `feat: Moodle en Android`.

### Tarea 6: Cierre
- [ ] `./android/compilar-rust.sh` + `assembleDebug` desde cero; `cargo install --path crates/order-escritorio --locked`; APK a `dist/ORDER.apk`; README: qué se trae de Moodle, qué se guarda (token, nunca la contraseña) y cada cuánto se sincroniza. Commit `docs: Moodle`.
