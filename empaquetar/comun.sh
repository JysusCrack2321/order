#!/usr/bin/env bash
# Utilidades compartidas para compilar ORDER para Linux x86_64 dentro del
# contenedor ubuntu:22.04 (glibc 2.35) sin instrucciones específicas de CPU.

ORDER_IMAGEN="localhost/order-appimage-builder:ubuntu22.04"
ORDER_VOL_CARGO="order-appimage-cargo"
ORDER_VOL_TARGET="order-appimage-target"
ORDER_VOL_TOOLS="order-appimage-tools"

leer_version_escritorio() {
  local raiz="$1"
  local version
  version="$(awk -F'"' '/^version[[:space:]]*=/{print $2; exit}' "$raiz/crates/order-escritorio/Cargo.toml")"
  [[ -n "$version" ]] || {
    echo "Error: no se pudo leer la versión de crates/order-escritorio/Cargo.toml" >&2
    return 1
  }
  printf '%s\n' "$version"
}

preparar_imagen_linux() {
  local raiz="$1"
  echo "==> Preparando imagen $ORDER_IMAGEN"
  podman build -t "$ORDER_IMAGEN" -f "$raiz/empaquetar/Containerfile" "$raiz/empaquetar"
  podman volume exists "$ORDER_VOL_CARGO" || podman volume create "$ORDER_VOL_CARGO" >/dev/null
  podman volume exists "$ORDER_VOL_TARGET" || podman volume create "$ORDER_VOL_TARGET" >/dev/null
  podman volume exists "$ORDER_VOL_TOOLS" || podman volume create "$ORDER_VOL_TOOLS" >/dev/null
}

ejecutar_en_contenedor_linux() {
  local raiz="$1"
  local script="$2"
  shift 2

  podman run --rm \
    -v "$raiz:/repo" \
    -v "$ORDER_VOL_CARGO:/cache/cargo" \
    -v "$ORDER_VOL_TARGET:/cache/target" \
    -v "$ORDER_VOL_TOOLS:/cache/tools" \
    "$@" \
    "$ORDER_IMAGEN" \
    bash -euo pipefail -c "
      export CARGO_HOME=/cache/cargo
      export CARGO_TARGET_DIR=/cache/target
      export HILOS=\$(( \$(nproc) > 1 ? \$(nproc) - 1 : 1 ))
      export CARGO_BUILD_JOBS=\"\$HILOS\"
      export CMAKE_BUILD_PARALLEL_LEVEL=\"\$HILOS\"

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
      export CMAKE_C_FLAGS=\"-march=x86-64-v2\"
      export CMAKE_CXX_FLAGS=\"-march=x86-64-v2\"
      export RUSTFLAGS=\"-C target-cpu=x86-64-v2\"

      $script
    "
}
