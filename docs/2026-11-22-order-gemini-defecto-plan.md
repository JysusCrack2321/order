# ORDER — Gemini gratis por defecto con conexión guiada — Plan de implementación

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría). Leer este plan y el código de `crates/order-core/src/agente/config.rs`, `crates/order-ffi/src/lib.rs`, `crates/order-escritorio/{src/puente.rs, ui/agente.slint, ui/app.slint}` y `android/.../ui/agente/{BarraAgente.kt, AjustesAgente.kt}`. Verificar con `cargo test --workspace` (sin warnings) y `JAVA_HOME=/usr/lib/jvm/java-17-openjdk ./gradlew assembleDebug testDebugUnitTest`. No abrir ventanas ni emuladores, no instalar en el teléfono, no hacer llamadas reales a APIs.

**Objetivo:** que alguien que instala ORDER conecte el agente gratis en un minuto sin saber qué es una API: Gemini es el proveedor por defecto y la app lo guía paso a paso.

## Decisiones
- `Proveedor::default()` pasa a **Gemini**; fichas en orden **Gemini · DeepSeek · Claude · Otro** (cuidar `codigo`/`de_codigo` y que los `agente.toml` existentes, que guardan `proveedor` por nombre, no cambien). La ficha Gemini muestra la etiqueta "Gratis".
- **Tarjeta "Conecta el agente gratis"**: aparece en el panel del agente cuando no hay key (al abrirlo o al intentar enviar; el mensaje escrito no se pierde y se envía al conectar). Texto corto y en pasos numerados:
  1. "Toca **Abrir Google AI Studio** y entra con tu cuenta de Google."
  2. "Toca **Create API key** y copia la clave."
  3. "Vuelve aquí y toca **Pegar clave**."
  Botones: **Abrir Google AI Studio** (abre `https://aistudio.google.com/apikey`), **Pegar clave** (lee el portapapeles; si el texto empieza por `AIza` lo pone en el campo), campo de clave oculto con mostrar, y **Conectar**.
- **Conectar** guarda la key en `agente.toml` (nunca en `order.toml`, nunca se imprime) y la valida con la función de validación existente; mensajes: "Listo, el agente está conectado." / "Esa clave no funciona; cópiala otra vez." / "Sin internet; inténtalo luego." Si la validación falla por red, la key se guarda igual.
- Android: al volver a la app (`onResume`) con la tarjeta abierta y una key `AIza…` en el portapapeles, ofrecer "¿Usar la clave que copiaste?" (un toque).
- Un enlace pequeño "Usar otro proveedor" lleva a Ajustes → Agente.
- Textos de la tarjeta en `order-core` (como el resto de textos) para que escritorio y Android digan lo mismo.

## Puntos de revisión
1. Instalación nueva → proveedor Gemini, la tarjeta aparece al primer intento de usar el agente.
2. `agente.toml` existente con DeepSeek → sigue en DeepSeek, no aparece la tarjeta.
3. Portapapeles con texto que no es key → "Pegar clave" no lo mete y dice "Lo que copiaste no parece una clave de Google."
4. Key inválida → mensaje claro, se puede reintentar.

---

### Tarea 1: `order-core` + FFI
- [ ] Gemini por defecto y orden de fichas; textos de la tarjeta; `parece_clave_gemini(texto) -> bool`; pruebas (config nueva = Gemini, toml viejo con deepseek se respeta, códigos de ficha ida y vuelta). Exponer lo necesario en la FFI. Commit `feat: Gemini gratis por defecto`.

### Tarea 2: Escritorio
- [ ] Tarjeta en `ui/agente.slint` con el estilo de `tema.slint`; botón abre el navegador (xdg-open) y "Pegar clave" lee el portapapeles. Commit `feat: conexión guiada del agente en escritorio`.

### Tarea 3: Android
- [ ] Tarjeta en el panel del agente (Compose, Theme.kt), Intent al navegador, `ClipboardManager`, oferta en `onResume`. Prueba unitaria Kotlin con el fake del puente. Commit `feat: conexión guiada del agente en Android`.

### Tarea 4: Cierre
- [ ] `./android/compilar-rust.sh` + `assembleDebug` desde cero; `cargo install --path crates/order-escritorio --locked`; APK a `dist/ORDER.apk`; README: una línea. Commit `docs: Gemini por defecto`.
