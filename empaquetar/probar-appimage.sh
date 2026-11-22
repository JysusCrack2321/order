#!/usr/bin/env bash
# Comprueba que dist/ORDER-x86_64.AppImage arranca (--version) y resuelve todas
# sus bibliotecas dinámicas (ldd) en contenedores limpios con bibliotecas
# mínimas de escritorio.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPIMAGE="${1:-$RAIZ/dist/ORDER-x86_64.AppImage}"

[[ -x "$APPIMAGE" ]] || {
  echo "Error: no se encontró ejecutable AppImage en $APPIMAGE" >&2
  exit 1
}

probar_apt() {
  local imagen="$1"
  local paquete_alsa="$2"
  echo "==> Probando en $imagen"
  podman run --rm -v "$APPIMAGE:/ORDER.AppImage:ro" "$imagen" bash -euo pipefail -c "
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends libfontconfig1 $paquete_alsa libxkbcommon0 libwayland-client0 >/dev/null
    . /etc/os-release
    echo \"Distro: \$PRETTY_NAME (ldd \$(ldd --version | head -n 1))\"
    /ORDER.AppImage --appimage-extract-and-run --version
    cd /tmp
    /ORDER.AppImage --appimage-extract >/dev/null
    if ldd squashfs-root/usr/bin/order | grep -q 'not found'; then
      ldd squashfs-root/usr/bin/order
      exit 1
    fi
    squashfs-root/AppRun --version >/dev/null
    echo 'OK: sin bibliotecas faltantes'
  "
}

probar_dnf() {
  local imagen="$1"
  echo "==> Probando en $imagen"
  podman run --rm -v "$APPIMAGE:/ORDER.AppImage:ro" "$imagen" bash -euo pipefail -c '
    dnf install -y -q fontconfig alsa-lib libxkbcommon libwayland-client >/dev/null
    . /etc/os-release
    echo "Distro: $PRETTY_NAME (ldd $(ldd --version | head -n 1))"
    /ORDER.AppImage --appimage-extract-and-run --version
    cd /tmp
    /ORDER.AppImage --appimage-extract >/dev/null
    if ldd squashfs-root/usr/bin/order | grep -q "not found"; then
      ldd squashfs-root/usr/bin/order
      exit 1
    fi
    squashfs-root/AppRun --version >/dev/null
    echo "OK: sin bibliotecas faltantes"
  '
}

probar_apt "docker.io/library/ubuntu:22.04" "libasound2"
probar_apt "docker.io/library/ubuntu:24.04" "libasound2t64"
probar_apt "docker.io/library/debian:12" "libasound2"
probar_dnf "registry.fedoraproject.org/fedora:latest"
