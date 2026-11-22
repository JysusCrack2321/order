# ORDER — Atajos de teclado configurables (escritorio) — Plan

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría). Compilar y probar en el PC de compilación con `cargo test --workspace` (sin warnings). No abrir ventanas. Solo cambia la app de escritorio (Android no tiene teclado físico habitual).

**Objetivo:** manejar ORDER con el teclado, con atajos que la persona puede cambiar.

## Acciones y teclas por defecto
| Acción (id) | Por defecto |
|---|---|
| `cerrar` — cerrar la ventana/diálogo abierto (formulario, Ajustes, sub-paneles, ayuda) | Esc |
| `guardar` — guardar el formulario abierto | Ctrl+S (y Enter en campos de una línea) |
| `nueva_tarea` | Ctrl+N |
| `ir_inicio` / `ir_dia` / `ir_semana` / `ir_pendientes` | Ctrl+1 / Ctrl+2 / Ctrl+3 / Ctrl+4 |
| `anterior` / `siguiente` / `hoy` (día o semana según la vista) | ← / → / T |
| `agente` — enfocar el cuadro del agente | Ctrl+K |
| `voz` — empezar/detener grabación | Ctrl+M |
| `sincronizar` | Ctrl+R |
| `ajustes` | Ctrl+, |
| `ayuda_atajos` — mostrar la lista de atajos | ? |

## Decisiones
- Las teclas sin modificador (←, →, T, ?) **no** actúan mientras se escribe en un campo de texto; Esc sí (primero quita el foco del campo si no hay diálogo; si hay diálogo, lo cierra).
- **Cambios sin guardar:** hoy no existe confirmación. Al cerrar un formulario con cambios (por Esc, Cancelar o la ✕), mostrar "¿Descartar los cambios?" con **Seguir editando** / **Descartar**. Un formulario sin cambios se cierra directo. Pila de diálogos: Esc cierra solo el de arriba.
- Configuración en `atajos.toml` junto a `agente.toml` (privado, por dispositivo); solo se guardan los que difieren del valor por defecto. Lógica (ids, valores por defecto, parseo "Ctrl+Shift+K", detección de choques, coincidencia de una pulsación) en `order-core` con pruebas; Slint solo captura teclas (`FocusScope` / `key-pressed`) y llama al puente.
- **Ajustes → Atajos:** lista acción + tecla; clic en la tecla → "Pulsa la nueva combinación…" (Esc cancela); si choca con otra acción: "Ya la usa «X». ¿Cambiarla?" (sí intercambia). Botón **Restaurar valores por defecto**.
- **Ayuda (`?`):** ventana con la lista actual de atajos, estilo ORDER (tema.slint).

## Tarea 1: `order-core`
- [ ] Módulo `atajos`: tipos, valores por defecto, parseo/formato, choques, `accion_para(tecla, modificadores, escribiendo: bool)`, carga/guardado de `atajos.toml`; pruebas. Commit `feat: modelo de atajos de teclado`.

## Tarea 2: Escritorio
- [ ] Captura global de teclas, ejecución de cada acción, confirmación de cambios sin guardar, panel Ajustes → Atajos y ventana de ayuda. Commit `feat: atajos de teclado en escritorio`.

## Tarea 3: Cierre
- [ ] `cargo build --release -p order-escritorio`; README: tabla de atajos. Commit `docs: atajos de teclado`.
