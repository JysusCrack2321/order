#!/usr/bin/env bash
# Construye dist/ORDER-<version>-x86_64.AppImage y dist/ORDER-x86_64.AppImage
# dentro de un contenedor ubuntu:22.04 (glibc 2.35) usando podman.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$RAIZ/empaquetar/comun.sh"

APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
RUNTIME_URL="https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"

VERSION="$(leer_version_escritorio "$RAIZ")"
preparar_imagen_linux "$RAIZ"
mkdir -p "$RAIZ/dist"

echo "==> Compilando y empaquetando ORDER $VERSION (x86_64)"
ejecutar_en_contenedor_linux "$RAIZ" '
  cargo build --release -p order-escritorio --locked

  BIN="/cache/target/release/order"
  echo "==> Dependencias dinámicas del binario:"
  ldd "$BIN"

  # Descargar y cachear appimagetool (extraído para no requerir FUSE en contenedor) y runtime estático.
  mkdir -p /cache/tools
  if [ ! -x /cache/tools/appimagetool-dir/AppRun ]; then
    curl -fsSL -o /cache/tools/appimagetool.AppImage "$APPIMAGETOOL_URL"
    chmod +x /cache/tools/appimagetool.AppImage
    (
      cd /cache/tools
      rm -rf squashfs-root appimagetool-dir
      ./appimagetool.AppImage --appimage-extract >/dev/null
      mv squashfs-root appimagetool-dir
    )
  fi
  if [ ! -f /cache/tools/runtime-x86_64 ]; then
    curl -fsSL -o /cache/tools/runtime-x86_64 "$RUNTIME_URL"
    chmod +x /cache/tools/runtime-x86_64
  fi

  APPDIR="/tmp/ORDER.AppDir"
  rm -rf "$APPDIR"
  mkdir -p \
    "$APPDIR/usr/bin" \
    "$APPDIR/usr/lib" \
    "$APPDIR/usr/share/applications" \
    "$APPDIR/usr/share/icons/hicolor/256x256/apps" \
    "$APPDIR/usr/share/icons/hicolor/scalable/apps"

  cp "$BIN" "$APPDIR/usr/bin/order"
  strip --strip-unneeded "$APPDIR/usr/bin/order"
  chmod 0755 "$APPDIR/usr/bin/order"

  cp /repo/empaquetar/order.desktop "$APPDIR/order.desktop"
  cp "$APPDIR/order.desktop" "$APPDIR/usr/share/applications/order.desktop"

  cp /repo/crates/order-escritorio/arte/order.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/order.svg"
  rsvg-convert -w 256 -h 256 /repo/crates/order-escritorio/arte/order.svg -o "$APPDIR/order.png"
  cp "$APPDIR/order.png" "$APPDIR/.DirIcon"
  cp "$APPDIR/order.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/order.png"

  cat > "$APPDIR/AppRun" <<\EOF
#!/bin/sh
set -eu
HERE="$(dirname "$(readlink -f "$0")")"
if [ -d "$HERE/usr/lib" ]; then
  export LD_LIBRARY_PATH="$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
exec "$HERE/usr/bin/order" "$@"
EOF
  chmod 0755 "$APPDIR/AppRun"

  SALIDA="/repo/dist/ORDER-${VERSION}-x86_64.AppImage"
  rm -f "$SALIDA" "/repo/dist/ORDER-x86_64.AppImage"
  ARCH=x86_64 /cache/tools/appimagetool-dir/AppRun \
    --no-appstream \
    --runtime-file /cache/tools/runtime-x86_64 \
    "$APPDIR" \
    "$SALIDA"
  chmod 0755 "$SALIDA"
  cp -f "$SALIDA" "/repo/dist/ORDER-x86_64.AppImage"
  chmod 0755 "/repo/dist/ORDER-x86_64.AppImage"
  ls -lh "$SALIDA" "/repo/dist/ORDER-x86_64.AppImage"
' -e VERSION="$VERSION" -e APPIMAGETOOL_URL="$APPIMAGETOOL_URL" -e RUNTIME_URL="$RUNTIME_URL"
