#!/usr/bin/env bash
# Construye dist/order_<version>_amd64.deb dentro del contenedor ubuntu:22.04.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$RAIZ/empaquetar/comun.sh"

VERSION="$(leer_version_escritorio "$RAIZ")"
preparar_imagen_linux "$RAIZ"
mkdir -p "$RAIZ/dist"

echo "==> Compilando y empaquetando order_${VERSION}_amd64.deb"
ejecutar_en_contenedor_linux "$RAIZ" '
  SALIDA="/repo/dist/order_${VERSION}_amd64.deb"
  rm -f "$SALIDA"
  cargo deb -p order-escritorio --locked -o "$SALIDA"
  chmod 0644 "$SALIDA"
  dpkg-deb -I "$SALIDA"
  dpkg-deb -c "$SALIDA"
  ls -lh "$SALIDA"
' -e VERSION="$VERSION"
