# ORDER

Aplicación de escritorio personal para Linux (niri + barra DMS) que muestra el
horario de la semana, las tareas pendientes y los recordatorios, y avisa por
notificaciones de escritorio. Un solo binario Rust con Slint: sin servidor, sin
puerto, sin base de datos y sin navegador. Arranca oculto (solo el icono de la
bandeja) y la ventana se abre cuando se pide.

- Código: `~/apps/order`
- Diseño y plan: `docs/2026-11-21-order-diseño.md`, `docs/2026-11-21-order-plan.md`

## Datos

Todo vive en un único archivo TOML, legible y editable a mano o con Hermes:

| Archivo | Qué guarda |
|---|---|
| `~/.config/order/order.toml` | ajustes, semestre, clases, tareas y recordatorios |
| `~/.config/order/historial.toml` | tareas hechas con más de 30 días (las mueve la app) |
| `~/.local/state/order/enviados.txt` | claves de los avisos ya enviados (no repetir) |

Si `order.toml` no existe, la app lo crea a partir de `ejemplo.toml` (incluido
en el binario). La app **relee el archivo sola** cuando cambia: basta con
guardar el TOML y la ventana se actualiza en un par de segundos.

Reglas del archivo: horas y fechas siempre entre comillas (`"07:00"`,
`"2026-11-28T23:59"`); los días son `lun mar mie jue vie sab dom`; los `id` se
pueden omitir porque la app los asigna. Si el TOML tiene un error de sintaxis,
la app conserva el último estado válido, muestra el banner rojo con la línea y
**no** sobreescribe el archivo roto.

### Otro archivo (pruebas y copias)

La variable de entorno `ORDER_ARCHIVO` reemplaza la ruta del TOML; el registro
de avisos y el socket de instancia única se mueven con ella, así que una copia
de prueba puede correr al mismo tiempo que la app real:

```bash
ORDER_ARCHIVO=/tmp/copia.toml ~/.cargo/bin/order
```

## Colores, tamaños y tipografía

Todo el aspecto está en **`ui/tema.slint`**: la paleta (fondo, textos, oro de
acento, tonos de materia), los tamaños de letra, los espaciados, los radios y
las medidas de la ventana y de la rejilla. Ningún otro archivo `.slint` lleva un
color o un tamaño escrito a mano, así que para cambiar el look basta editar ese
archivo y reinstalar.

La tipografía es Cinzel (OFL, incrustada en el binario) para títulos y rótulos;
el texto pequeño usa la sans del sistema (Noto Sans) porque Cinzel a 11–14 px
pierde legibilidad.

## Instalar y reinstalar

```bash
cd ~/apps/order
cargo install --path . --locked     # queda en ~/.cargo/bin/order
```

Tras cambiar `ui/*.slint` o `src/*.rs` hay que repetir el `cargo install`
(el binario lleva los `.slint` y la fuente compilados dentro).

Comprobaciones sin abrir ventana:

```bash
cargo build          # sin warnings
cargo test
```

## Arranque

- **niri**: línea en `~/.config/niri/config.kdl`
  `spawn-at-startup "/home/jysusman/.cargo/bin/order"` (ruta absoluta: el PATH
  de niri al iniciar sesión no siempre incluye `~/.cargo/bin`). Validar con
  `niri validate`.
- **Lanzador**: `order.desktop` (en este repo) instalado en
  `~/.local/share/applications/`.
- **Autoarranque XDG** (GNOME, KDE, etc.): el mismo `order.desktop` en
  `~/.config/autostart/`.

Si se arranca dos veces (niri + autostart, o un clic en el lanzador con la app
ya abierta), la segunda instancia no crea otro proceso: habla por el socket
`$XDG_RUNTIME_DIR/order.sock` con la primera y solo muestra la ventana.

### GNOME

El icono de la bandeja es un StatusNotifierItem; en GNOME hace falta la
extensión **AppIndicator and KStatusNotifierItem Support** para verlo y para
tener el menú Abrir/Salir. En niri con DMS funciona sin nada extra.

## Avisos

- Tarea por vencer: un aviso el día antes a `aviso_dia_antes` y otro
  `aviso_horas_antes` antes de `vence` (ambos en `[ajustes]`).
- Recordatorios fijos: los `dias` indicados a la `hora` indicada.
- Las notificaciones no llevan botones. Un aviso ya mostrado no se repite, ni
  siquiera tras reiniciar la app; los que se perdieron con el equipo apagado no
  se disparan en ráfaga (como mucho un resumen al arrancar).

## Hermes

`hermes/order-tareas/SKILL.md` es la skill que enseña a Hermes el formato del
TOML y cómo agregar, editar, marcar y borrar sin romper el archivo. Vive también
en `~/.hermes/skills/order-tareas/`.
