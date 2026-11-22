#!/usr/bin/env bash
# Compila y prueba ORDER en el PC de escritorio (más potente) en vez del portátil.
#
# Uso:
#   ./remoto.sh <comando>       copia el código allá y corre <comando> en la raíz del repo
#   ./remoto.sh --traer         trae dist/ORDER.apk y el binario de escritorio
#
# Ejemplos:
#   ./remoto.sh cargo test --workspace
#   ./remoto.sh ./android/compilar-rust.sh
#   ./remoto.sh ./android/gradlew -p android assembleDebug testDebugUnitTest
#   ./remoto.sh 'cargo build --release -p order-escritorio && mkdir -p dist && cp android/app/build/outputs/apk/debug/app-debug.apk dist/ORDER.apk'
#
# Variables: ORDER_REMOTO (por defecto jesuspc@192.168.2.32), ORDER_REMOTO_DIR (apps/order).
set -euo pipefail

REMOTO="${ORDER_REMOTO:-jesuspc@192.168.2.32}"
DIR="${ORDER_REMOTO_DIR:-apps/order}"
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ "${1:-}" == "--traer" ]]; then
  mkdir -p "$RAIZ/dist"
  rsync -a "$REMOTO:$DIR/dist/ORDER.apk" "$RAIZ/dist/ORDER.apk" 2>/dev/null \
    && echo "==> dist/ORDER.apk traído" || echo "==> no hay dist/ORDER.apk allá"
  if rsync -a "$REMOTO:$DIR/target/release/order" "$RAIZ/dist/order" 2>/dev/null; then
    echo "==> binario de escritorio traído a dist/order"
  fi
  exit 0
fi

[[ $# -gt 0 ]] || { sed -n '2,13p' "$0"; exit 1; }

echo "==> Copiando código a $REMOTO:$DIR"
ssh -o BatchMode=yes "$REMOTO" "mkdir -p $DIR"
rsync -a --delete \
  --exclude '/target/' --exclude '/dist/' --exclude '.gradle/' \
  --exclude 'android/app/build/' --exclude 'android/build/' \
  --exclude 'android/local.properties' --exclude 'android/app/src/main/jniLibs/' \
  --exclude 'android/app/src/main/java/com/jysusman/order/ffi/' \
  "$RAIZ/" "$REMOTO:$DIR/"

echo "==> Ejecutando allá: $*"
ssh -o BatchMode=yes "$REMOTO" "cd $DIR && \
  export ANDROID_HOME=\$HOME/Android/Sdk ANDROID_NDK_HOME=\$HOME/Android/Sdk/ndk/30.0.16248370 \
         JAVA_HOME=/usr/lib/jvm/java-17-openjdk && \
  HILOS=\$(( \$(nproc) - 1 )) && \
  export CARGO_BUILD_JOBS=\$HILOS CMAKE_BUILD_PARALLEL_LEVEL=\$HILOS \
         GRADLE_OPTS=\"-Dorg.gradle.workers.max=\$HILOS\" && \
  echo sdk.dir=\$HOME/Android/Sdk > android/local.properties && \
  bash -c $(printf '%q' "$*")"
