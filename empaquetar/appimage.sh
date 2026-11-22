#!/usr/bin/env bash
# Construye dist/ORDER-<version>-x86_64.AppImage y dist/ORDER-x86_64.AppImage
# dentro de un contenedor ubuntu:22.04 (glibc 2.35) usando podman.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGEN="localhost/order-appimage-builder:ubuntu22.04"
VOL_CARGO="order-appimage-cargo"
VOL_TARGET="order-appimage-target"
VOL_TOOLS="order-appimage-tools"

APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
RUNTIME_URL="https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"

VERSION="$(awk -F'"' '/^version[[:space:]]*=/{print $2; exit}' "$RAIZ/crates/order-escritorio/Cargo.toml")"
[[ -n "$VERSION" ]] || { echo "Error: no se pudo leer la versión de crates/order-escritorio/Cargo.toml" >&2; exit 1; }

echo "==> Preparando imagen $IMAGEN"
podman build -t "$IMAGEN" -f "$RAIZ/empaquetar/Containerfile" "$RAIZ/empaquetar"

podman volume exists "$VOL_CARGO" || podman volume create "$VOL_CARGO" >/dev/null
podman volume exists "$VOL_TARGET" || podman volume create "$VOL_TARGET" >/dev/null
podman volume exists "$VOL_TOOLS" || podman volume create "$VOL_TOOLS" >/dev/null

mkdir -p "$RAIZ/dist"

echo "==> Compilando y empaquetando ORDER $VERSION (x86_64)"
podman run --rm \
  -v "$RAIZ:/repo" \
  -v "$VOL_CARGO:/cache/cargo" \
  -v "$VOL_TARGET:/cache/target" \
  -v "$VOL_TOOLS:/cache/tools" \
  -e VERSION="$VERSION" \
  -e APPIMAGETOOL_URL="$APPIMAGETOOL_URL" \
  -e RUNTIME_URL="$RUNTIME_URL" \
  "$IMAGEN" \
  bash -euo pipefail -c '
    export CARGO_HOME=/cache/cargo
    export CARGO_TARGET_DIR=/cache/target
    export HILOS=$(( $(nproc) > 1 ? $(nproc) - 1 : 1 ))
    export CARGO_BUILD_JOBS="$HILOS"
    export CMAKE_BUILD_PARALLEL_LEVEL="$HILOS"

    # whisper.cpp / ggml sin instrucciones específicas de la CPU del host (x86-64-v2 máx).
    export GGML_NATIVE=OFF
    export GGML_SSE42=ON
    export GGML_AVX=OFF
    export GGML_AVX2=OFF
    export GGML_AVX_VNNI=OFF
    export GGML_AVX512=OFF
    export GGML_AVX512_VBMI=OFF
    export GGML_AVX512_VNNI=OFF
    export GGML_AVX512_BF16=OFF
    export GGML_FMA=OFF
    export GGML_F16C=OFF
    export GGML_BMI2=OFF
    export GGML_AMX_TILE=OFF
    export GGML_AMX_INT8=OFF
    export GGML_AMX_BF16=OFF
    export CMAKE_C_FLAGS="-march=x86-64-v2"
    export CMAKE_CXX_FLAGS="-march=x86-64-v2"
    export RUSTFLAGS="-C target-cpu=x86-64-v2"

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

    cat > "$APPDIR/order.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=ORDER
Comment=Horario, tareas y recordatorios para organizarte
Exec=order
Icon=order
Terminal=false
Categories=Office;Calendar;
StartupNotify=false
X-GNOME-Autostart-enabled=true
NoDisplay=false
EOF
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
  '
