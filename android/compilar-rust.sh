#!/usr/bin/env bash
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RAIZ"

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/30.0.16248370}"

JNI_OUT="android/app/src/main/jniLibs"
KT_OUT="android/app/src/main/java"

mkdir -p "$JNI_OUT" "$KT_OUT"

echo "==> Compilando liborder_ffi.so para arm64-v8a y x86_64…"
cargo ndk \
  -t arm64-v8a \
  -t x86_64 \
  -o "$JNI_OUT" \
  build -p order-ffi --release --lib

echo "==> Generando bindings Kotlin con uniffi-bindgen…"
cargo run -p order-ffi --features cli --bin uniffi-bindgen -- \
  generate \
  --library "$RAIZ/target/aarch64-linux-android/release/liborder_ffi.so" \
  --language kotlin \
  --no-format \
  --out-dir "$KT_OUT"

echo "==> Listo:"
find "$JNI_OUT" -name '*.so' -exec ls -lh {} +
find "$KT_OUT/com/jysusman/order/ffi" -name '*.kt' -exec ls -lh {} +
