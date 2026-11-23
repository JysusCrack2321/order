#!/usr/bin/env bash
# Prueba la instalación del paquete .deb con apt install en ubuntu:22.04 y ubuntu:24.04
# y verifica que `order --version` funcione.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(awk -F'"' '/^version[[:space:]]*=/{print $2; exit}' "$RAIZ/crates/order-escritorio/Cargo.toml")"
DEB="${1:-$RAIZ/dist/order_${VERSION}_amd64.deb}"

[[ -f "$DEB" ]] || {
  echo "Error: no se encontró el paquete .deb en $DEB" >&2
  exit 1
}

probar_ubuntu() {
  local imagen="$1"
  echo "==> Probando .deb en $imagen"
  podman run --rm -v "$DEB:/tmp/order.deb:ro" "$imagen" bash -euo pipefail -c '
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq /tmp/order.deb >/dev/null
    . /etc/os-release
    echo "Distro: $PRETTY_NAME"
    order --version
    test -f /usr/share/applications/order.desktop
    test -f /usr/share/icons/hicolor/scalable/apps/order.svg
    test -f /usr/share/icons/hicolor/256x256/apps/order.png
    echo "OK: paquete .deb instalado y verificado"
  '
}

probar_ubuntu "docker.io/library/ubuntu:22.04"
probar_ubuntu "docker.io/library/ubuntu:24.04"
