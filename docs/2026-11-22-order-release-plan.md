# ORDER — APK release firmado y publicación v0.1.0 — Plan

> **Para agentes:** encargos en orden, cada uno con su commit (sin coautoría). Compilar aquí (PC de compilación). No instalar en teléfonos. **Nunca** copiar al repositorio la clave ni sus contraseñas, ni imprimirlas.

## Firma
- La clave ya existe fuera del repo: `~/.config/order-firma/order-release.jks` y `~/.config/order-firma/firma.properties` (storeFile, storePassword, keyAlias, keyPassword).
- `android/app/build.gradle.kts`: `signingConfigs.release` que lee esas propiedades desde la ruta de la variable `ORDER_FIRMA` o, por defecto, `~/.config/order-firma/firma.properties`. Si el archivo no existe, la compilación release falla con un mensaje claro (debug sigue igual, con la clave de depuración).
- `.gitignore`: `*.jks`, `*.keystore`, `firma.properties`.

## Build release
- `buildTypes.release`: firmado con `release`, `isMinifyEnabled = false` en esta versión (JNA/UniFFI necesitan reglas de R8 que no vamos a arriesgar ahora; anotarlo), `isShrinkResources = false`.
- Solo ABI `arm64-v8a` en release (los teléfonos actuales); debug conserva `x86_64` para emuladores. `compilar-rust.sh` acepta `--release-only-arm64` o similar para no compilar x86_64 cuando no hace falta.
- Comprobar que `liborder_ffi.so` sale con símbolos quitados (`strip = true` / `debug = false` en el perfil release de Cargo para order-ffi) y `opt-level` adecuado; reportar tamaño del APK antes/después.
- `versionName` = versión de `Cargo.toml` (0.1.0), `versionCode` = 1.
- **Importante:** el APK release lleva otra firma que el debug instalado en los teléfonos del usuario: para pasar de debug a release hay que desinstalar (se pierden datos privados de la app: agente.toml, conversación, emparejamientos; el `order.toml` de `/storage/emulated/0/ORDER` no se pierde). Escribirlo en el README.

## Tarea 1: Firma y build release
- [x] Lo de arriba; `./android/gradlew -p android assembleRelease` produce `android/app/build/outputs/apk/release/app-release.apk`; verificar firma con `apksigner verify --print-certs` (huella SHA-256 debe ser `80:A5:E7:90:3C:F3:0A:DE:B6:C9:1E:7C:EF:6B:8A:0D:8C:C5:95:95:3B:8B:37:60:87:3A:42:86:D0:A9:A8:D8`). Copiar a `dist/ORDER-0.1.0.apk`. Commit `build: APK release firmado`.

## Tarea 2: Paquete .deb
- [ ] `cargo-deb` (metadatos en `crates/order-escritorio/Cargo.toml`: nombre `order`, descripción en español, sección `utils`, mantenedor = autor del repo, licencia). Compilado **dentro del mismo contenedor `ubuntu:22.04`** que el AppImage (reutilizar `empaquetar/`), sin optimizaciones de CPU específicas (igual que el AppImage).
- [ ] Contenido: `/usr/bin/order`, `/usr/share/applications/order.desktop`, iconos en `/usr/share/icons/hicolor/{scalable,256x256}/apps/order.*`. `Depends` calculadas (`$auto`) y comprobadas: deben aceptar los nombres `t64` de Ubuntu 24.04 (si una dependencia sale con nombre viejo sin `Provides` en 24.04, usar alternativa `libasound2 | libasound2t64`).
- [ ] Prueba: instalar con `apt install ./order_0.1.0_amd64.deb` en contenedores `ubuntu:22.04` y `ubuntu:24.04` y ejecutar `order --version`. Salida en `dist/order_0.1.0_amd64.deb`. Commit `build: paquete .deb para Ubuntu`.

## Tarea 3: Script de publicación
- [ ] `empaquetar/publicar.sh <versión>`: compila AppImage, .deb y APK release, genera `dist/SHA256SUMS`, y deja listos los archivos para `gh release create v<versión>` (NO ejecutar gh ni hacer push; solo imprimir el comando). README: sección "Descargar" (.deb para Ubuntu/Debian/Mint, AppImage para cualquier Linux, APK para Android, cómo verificar SHA256). Commit `build: script de publicación`.
