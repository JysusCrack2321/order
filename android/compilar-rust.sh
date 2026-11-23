#!/usr/bin/env bash
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RAIZ"

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/30.0.16248370}"

JNI_OUT="android/app/src/main/jniLibs"
KT_OUT="android/app/src/main/java"

SOLO_ARM64=0
for arg in "$@"; do
  case "$arg" in
    --release-only-arm64) SOLO_ARM64=1 ;;
    *)
      echo "Uso: $0 [--release-only-arm64]" >&2
      exit 1
      ;;
  esac
done

mkdir -p "$JNI_OUT" "$KT_OUT"

TARGETS=(-t arm64-v8a)
if [[ "$SOLO_ARM64" -eq 0 ]]; then
  TARGETS+=(-t x86_64)
  echo "==> Compilando liborder_ffi.so para arm64-v8a y x86_64…"
else
  echo "==> Compilando liborder_ffi.so solo para arm64-v8a…"
fi

cargo ndk \
  "${TARGETS[@]}" \
  -o "$JNI_OUT" \
  build -p order-ffi --release --lib

echo "==> Generando bindings Kotlin con uniffi-bindgen…"
# [profile.release] tiene strip = true, lo que elimina los metadatos UniFFI de la .so release;
# compilamos la librería dev del host para extraer exactamente la misma interfaz sin símbolos faltantes.
cargo build -p order-ffi --lib
cargo run -p order-ffi --features cli --bin uniffi-bindgen -- \
  generate \
  --library "$RAIZ/target/debug/liborder_ffi.so" \
  --language kotlin \
  --no-format \
  --out-dir "$KT_OUT"

echo "==> Listo:"
find "$JNI_OUT" -name '*.so' -exec ls -lh {} +
find "$KT_OUT/com/jysusman/order/ffi" -name '*.kt' -exec ls -lh {} +
