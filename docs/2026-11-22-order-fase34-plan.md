# ORDER Android — Fases 3 y 4: avisos, agente y voz — Plan de implementación

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría). Leer este plan **y** `docs/2026-11-22-order-android-diseño.md` (§6.2, §6.4, §6.5, §9) y el código actual de `crates/order-ffi` y `android/`. Verificar con `cargo test --workspace` (sin warnings) y `JAVA_HOME=/usr/lib/jvm/java-17-openjdk ./gradlew assembleDebug testDebugUnitTest`. No abrir emuladores, no instalar en el teléfono, no hacer llamadas reales a APIs.

**Objetivo:** paridad con escritorio en avisos (a su hora aunque la app esté cerrada), agente (cuadro + tarjetas Aplicar/Descartar + ajustes simplificados) y voz (micrófono → Whisper por API).

## Restricciones globales
- Lógica y textos desde `order-core` por la FFI; Kotlin solo pinta, programa alarmas y graba audio.
- `agente.toml` y el registro de avisos enviados viven en `filesDir` (privado), nunca en la carpeta sincronizada.
- Mismo criterio de avisos que escritorio: día antes a `aviso_dia_antes`, N horas antes, recordatorios en sus días/hora, sin repetir (registro), sin ráfagas de avisos perdidos; resumen la primera vez que se abre la app cada día (clave `resumen:{fecha}`).
- Interruptor "Avisos en este dispositivo" en Ajustes (encendido por defecto), guardado en preferencias locales.
- Estilo ORDER (Theme.kt), igual que las pantallas actuales.

## Puntos de revisión
1. **Teléfono reiniciado** → `BOOT_COMPLETED` reprograma; no dispara avisos atrasados.
2. **Archivo cambiado por Syncthing** → se reprograman las alarmas (cancelar las viejas por clave).
3. **Permiso de notificaciones o de alarma exacta denegado** → banner explicativo con botón a ajustes; la app no crashea; si no hay alarma exacta, usar `setAndAllowWhileIdle` como respaldo.
4. **Sin red / sin key / 401 / 429** → mensaje legible en el panel del agente.
5. **Micrófono denegado o grabación vacía** → mensaje legible; no se envía nada.

---

### Tarea 1: FFI de avisos, agente y voz
**Archivos:** `crates/order-ffi/src/lib.rs` (+ pruebas).
- [ ] `proximos_avisos(ruta, desde_iso, hasta_iso, registro_ruta) -> Vec<AvisoProgramable {clave, momento_iso, titulo, cuerpo}>` usando `order_core::avisos` (todos los momentos futuros en la ventana, no solo los "debidos"); `anotar_aviso(registro_ruta, clave)`; `resumen_del_dia(ruta, ahora_iso, registro_ruta) -> Option<Aviso>` que anota su clave.
- [ ] Agente: leer/guardar config en una ruta dada (`filesDir/agente.toml`), fichas de proveedor con URL de "Conseguir mi API key", `validar_agente`, `enviar_al_agente(ruta_datos, ruta_config, texto) -> RespuestaAgente {respuesta, acciones: Vec<AccionVista {id, descripcion}>}` (las acciones quedan guardadas del lado Rust por id o se devuelven serializadas: el builder elige), `aplicar_accion`, `aplicar_todas`.
- [ ] Voz: `transcribir(ruta_config, wav: Vec<u8>) -> String`, y `wav_de_pcm16(muestras: Vec<i16>, frecuencia: u32) -> Vec<u8>` en `order-core` (mono 16 kHz) para que Kotlin no arme WAV a mano.
- [ ] Pruebas Rust de cada función sin red (respuestas simuladas como en `order-core`). Commit `feat: ffi de avisos, agente y voz`.

### Tarea 2: Avisos en Android
**Archivos:** `.../avisos/{Programador.kt, ReceptorAlarma.kt, ReceptorArranque.kt, TrabajoRespaldo.kt, Canales.kt}`, `AndroidManifest.xml`.
- [ ] Canales "Tareas" y "Recordatorios"; pedir `POST_NOTIFICATIONS`; `SCHEDULE_EXACT_ALARM`/`USE_EXACT_ALARM` con respaldo inexacto.
- [ ] `Programador.reprogramar()`: pide `proximos_avisos` (ventana 48 h), cancela las alarmas anteriores y programa una por clave con `setExactAndAllowWhileIdle`. Se llama al abrir la app, al recargar el archivo, en `BOOT_COMPLETED` y en un `PeriodicWorkRequest` de ~6 h.
- [ ] `ReceptorAlarma`: muestra la notificación (icono del reloj de arena, toca → abre la app) y `anotar_aviso`. No notifica si el interruptor está apagado.
- [ ] Resumen del día al abrir.
- [ ] Prueba unitaria Kotlin del `Programador` con un fake del puente y del AlarmManager (cancela y programa por clave). Commit `feat: avisos con alarmas en Android`.

### Tarea 3: Agente en Android
**Archivos:** `.../ui/agente/{BarraAgente.kt, PanelRespuesta.kt, AjustesAgente.kt}`.
- [ ] Barra fija sobre la barra inferior: campo de texto, enviar, indicador "pensando…"; panel con la respuesta y tarjetas Aplicar / Descartar / Aplicar todas; tras aplicar, recargar y reprogramar avisos.
- [ ] Ajustes → Agente igual que escritorio simplificado: fichas DeepSeek/Gemini/Claude/Otro, botón "Conseguir mi API key" (Intent al navegador), campo de key (oculto con botón mostrar), Voz con fichas OpenAI/Groq y su botón, "Avanzado" plegado con URL base y modelo.
- [ ] Commit `feat: agente en Android`.

### Tarea 4: Voz en Android
**Archivos:** `.../voz/Grabadora.kt`, botón de micrófono en `BarraAgente.kt`.
- [ ] Permiso `RECORD_AUDIO`; `AudioRecord` a 16 kHz mono PCM16; clic empieza (indicador rojo + tiempo), clic detiene; `wav_de_pcm16` → `transcribir` en `Dispatchers.IO` → texto al campo para revisar.
- [ ] Commit `feat: voz en Android`.

### Tarea 5: Cierre
- [ ] `./compilar-rust.sh` + `assembleDebug` desde cero producen el APK; actualizar `android/README.md` (permisos que pedirá la app). Commit `docs: avisos, agente y voz en Android`.
