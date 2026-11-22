# ORDER — AppImage portable para Linux — Plan

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría). **Toda compilación, prueba o contenedor se ejecuta en el PC de escritorio con `./remoto.sh '<comando>'`** (ver el propio script); en el portátil solo se editan archivos. No abrir ventanas, no instalar en teléfonos.

**Objetivo:** un solo archivo `dist/ORDER-x86_64.AppImage` que funcione con doble clic en Ubuntu 22.04+ (en particular Ubuntu 24.04 de la novia), Mint, Debian 12, Fedora y Arch, sin instalar nada (ni `libfuse2`).

## Decisiones
- Compilar `order-escritorio` en modo release **dentro de un contenedor `ubuntu:22.04`** con podman (glibc 2.35) en el PC de escritorio. Rust con rustup dentro del contenedor (o imagen cacheada); paquetes de compilación: build-essential, cmake, clang, pkg-config, libfontconfig1-dev, libfreetype-dev, libasound2-dev, libxkbcommon-dev, libwayland-dev, libdbus-1-dev, curl, file. Caché de cargo y target en volúmenes para que la segunda vez sea rápida.
- whisper.cpp compilado **sin** instrucciones específicas de la CPU del PC de escritorio (`-march=x86-64-v2` como máximo / desactivar `GGML_NATIVE`), para que corra en cualquier PC x86-64.
- AppDir: binario, `order.desktop` (Categories=Office;Calendar;), icono desde `crates/order-escritorio/arte/order.svg` (+ PNG 256), `AppRun`. Empaquetar con `appimagetool` usando el **runtime estático tipo 2** (no necesita libfuse2). Bibliotecas: incluir solo lo que no trae toda distro de escritorio (revisar con `ldd`; fontconfig/freetype/alsa/libc **no** se incluyen). Si hace falta, usar `linuxdeploy` sin plugins Qt/GTK.
- Primera ejecución: si no existe `~/.config/order/order.toml`, la app arranca con el ejemplo como hoy. Inicio automático opcional: si el usuario lo activa en Ajustes (si no existe ya, añadir el interruptor "Abrir ORDER al iniciar sesión", que crea/borra `~/.config/autostart/order.desktop` apuntando a la ruta del AppImage — `$APPIMAGE`).
- `pdftoppm`/`tesseract` son externos: si faltan, la app ya muestra un mensaje; documentar en el README que en Ubuntu se instalan con `sudo apt install poppler-utils tesseract-ocr tesseract-ocr-spa` (solo hacen falta para PDFs escaneados con DeepSeek).
- Versión en el nombre: `ORDER-<versión de Cargo.toml>-x86_64.AppImage`, y copia `dist/ORDER-x86_64.AppImage`.

## Tarea 1: Script de empaquetado
- [x] `empaquetar/appimage.sh` (se corre allá: `./remoto.sh ./empaquetar/appimage.sh`): construye/reutiliza la imagen del contenedor, compila, arma el AppDir, descarga `appimagetool` y el runtime estático (fijar versión/URL), produce el AppImage en `dist/`. Idempotente. Commit `build: AppImage portable`.

## Tarea 2: Comprobación de compatibilidad
- [x] Con podman en el PC de escritorio, ejecutar el AppImage con `--appimage-extract-and-run --version` (o una bandera `--version` del binario; añadirla si no existe, sin abrir ventana) dentro de contenedores `ubuntu:22.04`, `ubuntu:24.04`, `debian:12` y `fedora:latest` con solo las bibliotecas de escritorio mínimas (fontconfig, alsa, libxkbcommon, wayland-client). Debe arrancar sin errores de glibc ni de bibliotecas faltantes (`ldd` del binario extraído sin "not found"). Anotar resultados. Commit `test: compatibilidad del AppImage`.

### Resultados de compatibilidad (`ORDER-0.1.0-x86_64.AppImage` / `ORDER-x86_64.AppImage`, 21 121 528 bytes / 20,1 MiB)
Ejecutado con `empaquetar/probar-appimage.sh` en contenedores limpios con `fontconfig`, `alsa`, `libxkbcommon` y `libwayland-client`:
- `ubuntu:22.04` (Ubuntu 22.04.5 LTS, glibc 2.35): `order 0.1.0`, `ldd` sin `not found`, `AppRun --version` correcto.
- `ubuntu:24.04` (Ubuntu 24.04.5 LTS, glibc 2.39): `order 0.1.0`, `ldd` sin `not found`, `AppRun --version` correcto.
- `debian:12` (Debian GNU/Linux 12 (bookworm), glibc 2.36): `order 0.1.0`, `ldd` sin `not found`, `AppRun --version` correcto.
- `fedora:latest` (Fedora Linux 45, glibc 2.44): `order 0.1.0`, `ldd` sin `not found`, `AppRun --version` correcto.
- Host CachyOS/Arch (ejecución directa FUSE y `--appimage-extract-and-run --version`): `order 0.1.0`.

## Tarea 3: Cierre
- [x] `./remoto.sh --traer` (ampliar `--traer` para traer también `dist/*.AppImage`). README: cómo descargar y abrir el AppImage (chmod +x o Propiedades → Permitir ejecutar). Commit `docs: AppImage`.
