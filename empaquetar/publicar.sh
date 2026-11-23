#!/usr/bin/env bash
# Compila todos los artefactos de publicación de ORDER (AppImage, .deb y APK release),
# genera dist/SHA256SUMS e imprime el comando `gh release create` sin ejecutarlo.
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Uso: $0 <versión>   (ejemplo: $0 0.1.0)" >&2
  exit 1
fi

VERSION="${1#v}"
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RAIZ"

VER_ESCRITORIO="$(awk -F'"' '/^version[[:space:]]*=/{print $2; exit}' crates/order-escritorio/Cargo.toml)"
VER_FFI="$(awk -F'"' '/^version[[:space:]]*=/{print $2; exit}' crates/order-ffi/Cargo.toml)"
VER_ANDROID="$(awk -F'"' '/versionName[[:space:]]*=/{print $2; exit}' android/app/build.gradle.kts)"
VER_CODE_ANDROID="$(awk -F'=' '/versionCode[[:space:]]*=/{gsub(/[^0-9]/, "", $2); print $2; exit}' android/app/build.gradle.kts)"

if [[ "$VERSION" != "$VER_ESCRITORIO" || "$VERSION" != "$VER_FFI" || "$VERSION" != "$VER_ANDROID" ]]; then
  echo "Error: la versión pedida ($VERSION) no coincide con los manifiestos:" >&2
  echo "  order-escritorio: $VER_ESCRITORIO" >&2
  echo "  order-ffi:        $VER_FFI" >&2
  echo "  android:          $VER_ANDROID" >&2
  exit 1
fi

comprobar_version_code_publicado() {
  local codigo_actual="$1"
  local tag_ultimo=""
  if command -v gh >/dev/null 2>&1; then
    tag_ultimo="$(gh release view --repo JysusCrack2321/order --json tagName -q .tagName 2>/dev/null || true)"
  fi
  if [[ -z "$tag_ultimo" ]] && command -v curl >/dev/null 2>&1; then
    tag_ultimo="$(curl -fsSL --max-time 5 -A "ORDER-publicar" \
      "https://api.github.com/repos/JysusCrack2321/order/releases/latest" 2>/dev/null \
      | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1 || true)"
  fi
  if [[ -z "$tag_ultimo" ]]; then
    tag_ultimo="$(git tag --sort=-v:refname | head -n 1 || true)"
  fi
  [[ -n "$tag_ultimo" ]] || return 0

  local gradle_anterior=""
  gradle_anterior="$(git show "${tag_ultimo}:android/app/build.gradle.kts" 2>/dev/null || true)"
  if [[ -z "$gradle_anterior" ]] && command -v curl >/dev/null 2>&1; then
    gradle_anterior="$(curl -fsSL --max-time 5 -A "ORDER-publicar" \
      "https://raw.githubusercontent.com/JysusCrack2321/order/${tag_ultimo}/android/app/build.gradle.kts" 2>/dev/null || true)"
  fi
  [[ -n "$gradle_anterior" ]] || return 0

  local codigo_anterior
  codigo_anterior="$(printf '%s\n' "$gradle_anterior" | awk -F'=' '/versionCode[[:space:]]*=/{gsub(/[^0-9]/, "", $2); print $2; exit}')"
  if [[ -n "$codigo_anterior" && -n "$codigo_actual" ]] && (( codigo_actual <= codigo_anterior )); then
    echo "AVISO: versionCode ($codigo_actual) no subió respecto al último release publicado ($tag_ultimo: versionCode=$codigo_anterior)." >&2
    echo "       Android exige un versionCode mayor para actualizar una app ya instalada." >&2
  fi
}

comprobar_version_code_publicado "$VER_CODE_ANDROID"

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/30.0.16248370}"
export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk}"
HILOS=$(( $(nproc) > 1 ? $(nproc) - 1 : 1 ))
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$HILOS}"
export CMAKE_BUILD_PARALLEL_LEVEL="${CMAKE_BUILD_PARALLEL_LEVEL:-$HILOS}"
export GRADLE_OPTS="${GRADLE_OPTS:--Dorg.gradle.workers.max=$HILOS}"

if [[ ! -f "$RAIZ/android/local.properties" ]]; then
  echo "sdk.dir=$ANDROID_HOME" > "$RAIZ/android/local.properties"
fi

echo "==> [1/3] Generando AppImage..."
"$RAIZ/empaquetar/appimage.sh"

echo "==> [2/3] Generando paquete .deb..."
"$RAIZ/empaquetar/deb.sh"

echo "==> [3/3] Compilando APK release firmado (arm64-v8a)..."
"$RAIZ/android/compilar-rust.sh" --release-only-arm64
"$RAIZ/android/gradlew" -p "$RAIZ/android" assembleRelease

APKSIGNER="$(find "$ANDROID_HOME/build-tools" -maxdepth 2 -name apksigner 2>/dev/null | sort -V | tail -n 1 || true)"
if [[ -z "$APKSIGNER" || ! -x "$APKSIGNER" ]]; then
  echo "Error: no se encontró apksigner en $ANDROID_HOME/build-tools" >&2
  exit 1
fi

APK_SRC="$RAIZ/android/app/build/outputs/apk/release/app-release.apk"
APK_DST="$RAIZ/dist/ORDER-${VERSION}.apk"
mkdir -p "$RAIZ/dist"
"$APKSIGNER" verify --print-certs "$APK_SRC"
cp -f "$APK_SRC" "$APK_DST"

ARCHIVOS=(
  "order_${VERSION}_amd64.deb"
  "ORDER-${VERSION}-x86_64.AppImage"
  "ORDER-x86_64.AppImage"
  "ORDER-${VERSION}.apk"
)

echo "==> Generando dist/SHA256SUMS..."
(
  cd "$RAIZ/dist"
  sha256sum "${ARCHIVOS[@]}" > SHA256SUMS
  sha256sum -c SHA256SUMS
)

echo ""
echo "==> Artefactos listos en dist/:"
ls -lh "$RAIZ/dist/${ARCHIVOS[0]}" "$RAIZ/dist/${ARCHIVOS[1]}" "$RAIZ/dist/${ARCHIVOS[2]}" "$RAIZ/dist/${ARCHIVOS[3]}" "$RAIZ/dist/SHA256SUMS"

echo ""
echo "==> Comando para publicar en GitHub Releases (ejecútalo manualmente tras git push):"
cat <<EOF
gh release create "v${VERSION}" \\
  dist/order_${VERSION}_amd64.deb \\
  dist/ORDER-${VERSION}-x86_64.AppImage \\
  dist/ORDER-x86_64.AppImage \\
  dist/ORDER-${VERSION}.apk \\
  dist/SHA256SUMS \\
  --title "ORDER v${VERSION}" \\
  --generate-notes
EOF
