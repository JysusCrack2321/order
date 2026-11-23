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
| `~/.config/order/moodle.toml` | conexión con Moodle (solo token, permisos `600`, nunca se sincroniza) |
| `~/.config/order/calendarios.toml` | suscripciones iCal (permisos `600`, nunca se sincroniza) |
| `~/.config/order/atajos.toml` | atajos de teclado personalizados en escritorio (permisos `600`, nunca se sincroniza) |
| `~/.config/order/sync.toml` | clave privada del nodo P2P y dispositivos emparejados (permisos `600`, nunca se sincroniza) |
| `~/.config/order/sync-estado.json` | marcas HLC por campo, lápidas y foto de sincronización (permisos `600`, nunca se sincroniza) |
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

## Icono

La imagen de la app es el reloj de arena de **`arte/order.svg`**. De ahí salen
dos cosas distintas:

- **Ventana y bandeja**: `ui/app.slint` la usa como `icon` de la ventana y
  `src/sistema.rs` incrusta `arte/bandeja-{22,32,48}.png` para el
  `icon_pixmap` (ARGB32) del StatusNotifierItem. Con los píxeles dentro del
  binario el icono se ve igual aunque el tema no tenga nada parecido;
  `icon_name = "order"` queda solo de respaldo.
- **Escritorio**: se instala en el tema hicolor para el `.desktop`.

Paso de instalación (una vez, y de nuevo solo si cambia el SVG):

```bash
cd ~/apps/order
rsvg-convert -w 22 -h 22 arte/order.svg -o arte/bandeja-22.png   # ídem 32 y 48
for n in 16 22 24 32 48 64 128 256; do
  mkdir -p ~/.local/share/icons/hicolor/${n}x${n}/apps
  rsvg-convert -w $n -h $n arte/order.svg \
    -o ~/.local/share/icons/hicolor/${n}x${n}/apps/order.png
done
cp arte/order.svg ~/.local/share/icons/hicolor/scalable/apps/order.svg
gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor   # si existe
```

Los `.desktop` (el del repo, `~/.local/share/applications/` y
`~/.config/autostart/`) llevan `Icon=order`, que apunta a esos archivos.

## Descargar

En la página de lanzamientos (Releases) encontrarás los instaladores listos para usar sin compilar:

### Ubuntu, Debian y Linux Mint (`.deb`, x86_64)

Descarga `order_<versión>_amd64.deb` e instálalo con `apt` (compatible con Ubuntu 22.04+ y 24.04+):

```bash
sudo apt install ./order_0.1.0_amd64.deb
```

Instala el binario en `/usr/bin/order`, el lanzador en el menú de aplicaciones y los iconos.

### Cualquier distribución Linux (`AppImage`, x86_64)

Descarga `ORDER-x86_64.AppImage` (o `ORDER-<versión>-x86_64.AppImage`; no requiere instalar `libfuse2`):

1. Dale permiso de ejecución:
   - En terminal: `chmod +x ORDER-x86_64.AppImage`
   - En gestor de archivos (Ubuntu/GNOME, Mint, etc.): clic derecho → **Propiedades** → **Permisos** → **Permitir ejecutar el archivo como un programa**.
2. Ábrelo con doble clic o ejecutando `./ORDER-x86_64.AppImage` (arranca en la bandeja; en **Ajustes** puedes activar **Abrir ORDER al iniciar sesión**).

### Android (`.apk`, arm64-v8a)

Descarga `ORDER-<versión>.apk` e instálalo en el teléfono (permitiendo instalar apps de ese origen).

> **Importante si vienes de una compilación debug:** el APK `release` lleva otra firma que el `debug`, así que Android no permite actualizarlo encima. Para pasar de `debug` a `release` hay que **desinstalar** primero la versión anterior (se pierden los datos privados de la app: `agente.toml`, `conversacion.json`, `moodle.toml`, `calendarios.toml`, `sync.toml` y `sync-estado.json`; el archivo `order.toml` en `/storage/emulated/0/ORDER` **no** se pierde).

### Verificar integridad (`SHA256SUMS`)

Descarga `SHA256SUMS` en la misma carpeta que el instalador y comprueba su firma hash:

```bash
sha256sum -c --ignore-missing SHA256SUMS
```

## Instalar y compilar desde código fuente

### Paquetes de escritorio y publicación (requiere `podman`)

```bash
./empaquetar/appimage.sh           # deja dist/ORDER-<versión>-x86_64.AppImage y dist/ORDER-x86_64.AppImage
./empaquetar/deb.sh                # deja dist/order_<versión>_amd64.deb
./empaquetar/publicar.sh 0.1.0     # compila AppImage, .deb y APK release y genera dist/SHA256SUMS
```

### Binario local en Linux

```bash
cd ~/apps/order
cargo install --path crates/order-escritorio --locked     # queda en ~/.cargo/bin/order
```

Tras cambiar `crates/order-escritorio/ui/*.slint` o el código Rust hay que repetir el `cargo install`
(el binario lleva los `.slint` y la fuente compilados dentro).

Comprobaciones sin abrir ventana:

```bash
cargo build --workspace   # sin warnings
cargo test --workspace
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

## Atajos de teclado (escritorio)

Todos los atajos se pueden personalizar desde **Ajustes → Atajos** (se guardan solo los cambios en `atajos.toml`) y consultar en cualquier momento pulsando `?`. Las teclas sin modificador (`←`, `→`, `T`, `?`) no actúan mientras escribes en un campo de texto; `Esc` quita primero el foco del campo o cierra la capa superior abierta (preguntando si hay cambios sin guardar).

| Acción | Atajo por defecto |
|---|---|
| Cerrar diálogo, sub-panel, ayuda o quitar foco | `Esc` |
| Guardar el formulario abierto | `Ctrl+S` (`Enter` en campos de una línea) |
| Nueva tarea | `Ctrl+N` |
| Ir a Inicio / Día / Semana / Pendientes | `Ctrl+1` / `Ctrl+2` / `Ctrl+3` / `Ctrl+4` |
| Día o semana anterior / siguiente / hoy | `←` / `→` / `T` |
| Enfocar el cuadro del agente | `Ctrl+K` |
| Empezar o detener grabación de voz | `Ctrl+M` |
| Sincronizar (Moodle, calendarios iCal y P2P) | `Ctrl+R` |
| Abrir Ajustes | `Ctrl+,` |
| Mostrar la lista de atajos | `?` |

## Avisos

- Tarea por vencer: un aviso el día antes a `aviso_dia_antes` y otro
  `aviso_horas_antes` antes de `vence` (ambos en `[ajustes]`).
- Recordatorios fijos: los `dias` indicados a la `hora` indicada.
- Las notificaciones no llevan botones. Un aviso ya mostrado no se repite, ni
  siquiera tras reiniciar la app; los que se perdieron con el equipo apagado no
  se disparan en ráfaga (como mucho un resumen al arrancar).

## Agente y adjuntos

El agente usa **Gemini** por defecto (gratis con clave de Google AI Studio) y abre una guía paso a paso para conectarlo al primer uso si aún no hay clave.

### Voz

- **Escritorio**: usa **«Este PC (gratis)»** por defecto, que transcribe en español en local con `whisper.cpp` (`whisper-rs`) y el modelo `ggml-small.bin` en `~/.local/share/order/modelos/ggml-small.bin` (se descarga la primera vez con barra de progreso y se libera de memoria tras 10 minutos sin uso). También se pueden elegir **OpenAI** o **Groq** en **Ajustes → Agente**.
- **Android**: usa **«Android (gratis)»** (reconocimiento del sistema `es-CO`) por defecto, o OpenAI/Groq con clave.

Cada petición al agente se arma en este orden:

1. **Agenda actual** (`Datos actuales de ORDER`).
2. **`## Documentos de la conversación`**: texto completo extraído de los adjuntos (los PDF se leen conservando sus filas y columnas como `pdftotext -layout` en Rust puro; en fotos se guarda el OCR si lo hay). Tope combinado de **200 000 caracteres**: si se supera, el documento más antiguo se reemplaza por `[documento retirado: nombre]`.
3. **`## Resumen de lo hablado antes`**: presente tras una compactación.
4. **`## Conversación`**: todos los intercambios literales (`Usuario:` / `ORDER:`).
5. **`## Qué pasó con tus propuestas`**: acciones aplicadas o descartadas desde el turno anterior.
6. **Petición nueva** (+ adjuntos binarios solo en el turno en que se adjuntan).

### Persistencia (`conversacion.json`) y compactación

- El historial vive en **`conversacion.json`** junto a `agente.toml`, con permisos `600` y nunca en la carpeta sincronizada ni en `order.toml`:
  - **Escritorio**: `~/.config/order/conversacion.json` (o junto a `ORDER_ARCHIVO` si está definida).
  - **Android**: almacenamiento privado de la app (`filesDir`).
- Se guarda de forma atómica tras cada turno, compactación y acción aplicada o descartada. **«Nueva conversación»** borra `conversacion.json`.
- **Compactación**: antes de enviar, si las secciones 2–5 superan el umbral configurable `limite_contexto_tokens` (**60 000** tokens aprox. = caracteres / 4, editable en **Ajustes → Agente → Avanzado**) y hay más de 10 mensajes, el modelo resume todos los mensajes anteriores y conserva intactos los últimos **10** mensajes literales y todos los documentos. Si la API rechaza una petición por longitud de contexto, compacta y reintenta una vez.

Desde la barra inferior del agente se pueden adjuntar hasta **5 archivos** por mensaje (PDF o imágenes; máximo **20 páginas** por PDF, imágenes reducidas a **1568 px** en el lado largo en JPEG 85 respetando EXIF, total **≤ 20 MB**; los binarios no se guardan en disco). Lo que se envía a cada proveedor depende de sus capacidades:

| Proveedor | PDF | Imagen / foto |
|---|---|---|
| **Claude** | bloque `document` base64 (`application/pdf`), antes del texto | bloque `image` base64 (`image/jpeg`) |
| **Gemini** | páginas renderizadas a JPEG → `image_url` (`data:`) | `image_url` (`data:`) |
| **DeepSeek** | texto extraído localmente (+ OCR si una página no tiene texto) | texto extraído por OCR |
| **Otro** | texto/OCR por defecto, o imágenes si se activa *El modelo ve imágenes* en **Avanzado** | ídem |

En escritorio, el renderizado de páginas usa `pdftoppm -jpeg -r 110` (Poppler) y el OCR usa `tesseract - - -l spa`; si alguno falta, la app muestra un aviso claro. En Ubuntu/Debian se instalan con `sudo apt install poppler-utils tesseract-ocr tesseract-ocr-spa` (solo hacen falta para PDFs escaneados con DeepSeek).

## Moodle y calendarios iCal

Desde **Ajustes → Moodle** puedes conectar tu cuenta (por ejemplo `micampus.unal.edu.co`) para traer automáticamente tus entregas como tareas de ORDER, y desde **Ajustes → Calendarios** puedes suscribirte a direcciones iCal (`.ics` o `webcal://`, con atajo automático desde el portapapeles).

- **Qué se trae de Moodle**: nombre visible y usuario (`core_webservice_get_site_info`), eventos de actividades desde hace 14 días (`core_calendar_get_action_events_by_timesort`: título, curso, fecha de entrega y enlace) y estado de entrega/compleción (`mod_assign_get_submission_status` y `core_completion_get_activities_completion_status`). Las tareas creadas a mano nunca se tocan ni se borran tareas previas.
- **Qué se guarda**:
  - **`moodle.toml`** (junto a `agente.toml`: `~/.config/order/` en escritorio o `filesDir` en Android, con permisos `600`): `sitio`, `token`, nombre visible, `userid` y fecha de última sincronización. **La contraseña nunca se guarda** en disco ni en logs.
  - **`calendarios.toml`** (misma ubicación privada con permisos `600`): nombre, dirección iCal, materia opcional y última actualización. Ninguno de los dos archivos se guarda dentro de `order.toml`.
- **Cada cuánto se sincroniza**: al abrir la aplicación, cada **3 horas** mientras la app de escritorio está abierta, de forma periódica en `TrabajoRespaldo` en Android, y en cualquier momento con **Sincronizar ahora**.

## Hermes

`hermes/order-tareas/SKILL.md` es la skill que enseña a Hermes el formato del
TOML y cómo agregar, editar, marcar y borrar sin romper el archivo. Vive también
en `~/.hermes/skills/order-tareas/`.

## Sincronización entre dispositivos (P2P)

ORDER sincroniza `order.toml` directamente entre tu PC y tu celular sin cuentas ni servidor propio, fusionando cambios por campo (CRDT LWW con marcas HLC y lápidas que se purgan a los 60 días; los cambios hechos a mano o por Hermes también se detectan y viajan).

- **Cómo emparejar**:
  1. En un dispositivo (por ejemplo el PC), abre **Ajustes → Dispositivos** y toca **Agregar dispositivo**. Aparece un código QR (válido 10 minutos, de un solo uso), un código de **8 palabras** (para la misma red Wi-Fi) y un enlace `order://emparejar?…`.
  2. En el celular, abre **Ajustes → Dispositivos** y toca **Escanear QR** (o **Escribir código** para introducir las 8 palabras en la misma Wi-Fi o pegar el enlace `order://emparejar?…`).
  3. Solo se aceptan conexiones de nodos previamente emparejados.
- **Qué viaja**: exclusivamente los campos de tu agenda (`ajustes`, `semestre`, `clase`, `tarea`, `recordatorio`) con sus marcas lógicas, sobre QUIC (`iroh`, ALPN `order/sync/1`) con **cifrado de extremo a extremo**. En la misma red Wi-Fi se descubren localmente por mDNS; cuando están en redes distintas, los relays públicos de n0 actúan solo como paso de tráfico cifrado (nunca ven tu agenda ni tus claves).
- **Dónde vive el estado**: en **`sync.toml`** (clave privada del nodo, nombre del dispositivo y lista de emparejados) y **`sync-estado.json`** (marcas HLC, lápidas y última foto sincronizada), siempre con permisos **`600`** en el almacenamiento privado junto a `agente.toml` (`~/.config/order/` en escritorio o `filesDir` en Android, nunca dentro de `order.toml` ni de la carpeta compartida).
- **Cuándo sincroniza**: como no hay servidor intermedio, **ambos dispositivos deben estar encendidos a la vez** para intercambiar cambios (en escritorio basta con que ORDER esté vivo en la bandeja). El escritorio escucha siempre y sincroniza al arrancar, cada 5 minutos y 2 segundos después de cada cambio local; Android sincroniza al abrir o volver a primer plano, tras cada cambio y periódicamente en `TrabajoRespaldo`, y escucha mientras la app está abierta.
