# ORDER para Android

Aplicación Android nativa (Kotlin + Jetpack Compose) respaldada por el núcleo compartido en Rust (`order-core` vía `order-ffi` con UniFFI).

## Requisitos

- **Rust** (`rustup` stable 1.100+) con los targets Android:
  ```bash
  rustup target add aarch64-linux-android x86_64-linux-android
  cargo install cargo-ndk
  ```
- **JDK 17** (`java` en `PATH` o `JAVA_HOME` apuntando al JDK 17).
- **Android SDK** (`platform-36`, `build-tools 36.1.0`) y **NDK** `30.0.16248370`.
- Archivo `android/local.properties` (ignorado por git) apuntando al SDK:
  ```properties
  sdk.dir=/home/tu_usuario/Android/Sdk
  ```

## Compilar desde cero

1. **Compilar Rust y generar los bindings Kotlin** (desde la raíz del repositorio o desde `android/`):
   ```bash
   ./android/compilar-rust.sh
   ```
   Esto genera `android/app/src/main/jniLibs/{arm64-v8a,x86_64}/liborder_ffi.so` y `android/app/src/main/java/com/jysusman/order/ffi/order_ffi.kt`.

2. **Compilar el APK de depuración**:
   ```bash
   cd android
   ./gradlew assembleDebug
   ```
   El APK queda en:
   `android/app/build/outputs/apk/debug/app-debug.apk`

3. **Ejecutar pruebas**:
   ```bash
   cargo test --workspace
   cd android && ./gradlew testDebugUnitTest
   ```

## Instalar en el teléfono por USB (`adb`)

1. En el teléfono Android, activa **Opciones para desarrolladores**:
   - **Ajustes → Acerca del teléfono → Información del software** (o **Número de compilación**) y toca 7 veces sobre **Número de compilación**.
2. En **Ajustes → Sistema → Opciones para desarrolladores**, activa **Depuración USB** y conecta el cable al computador (acepta la huella RSA en la pantalla del teléfono).
3. Verifica que el dispositivo aparezca y luego instala el APK:
   ```bash
   adb devices
   cd android
   adb install -r app/build/outputs/apk/debug/app-debug.apk
   ```

## Datos y permisos

- Al abrir la app por primera vez solicita **Acceso a todos los archivos** (`MANAGE_EXTERNAL_STORAGE`) para leer y escribir `/storage/emulated/0/ORDER/order.toml` (compatible con Syncthing-Fork).
- Si la carpeta o el archivo no existen, se crean automáticamente con el contenido de ejemplo.
- La carpeta de datos se puede cambiar desde **Ajustes** dentro de la app.
- **Notificaciones y alarmas exactas** (`POST_NOTIFICATIONS`, `SCHEDULE_EXACT_ALARM` / `USE_EXACT_ALARM`, `RECEIVE_BOOT_COMPLETED`): para mostrar avisos de tareas y recordatorios a su hora aunque la app esté cerrada o el teléfono se reinicie (si faltan permisos, la app muestra un banner para activarlos y usa alarmas inexactas de respaldo). Se pueden desactivar en **Ajustes → Avisos en este dispositivo**.
- **Red y micrófono** (`INTERNET`, `RECORD_AUDIO`): para enviar peticiones al agente o dictar por voz. Por defecto en **Ajustes → Agente → Voz** se usa **Android (gratis)** (`SpeechRecognizer` en `es-CO`), sin clave ni API externa; OpenAI y Groq siguen disponibles como alternativa.
- **Adjuntos y cámara** (`CAMERA`, solo si tomas una foto): puedes adjuntar hasta 5 PDFs o fotos por mensaje (máximo 20 páginas por PDF, imágenes reducidas a 1568 px respetando EXIF en JPEG 85, total ≤ 20 MB; no se guardan). Según el proveedor configurado en el agente:
  - **Claude**: envía el PDF en base64 (`application/pdf`) y las fotos en base64 antes del texto.
  - **Gemini**: renderiza las páginas del PDF en el teléfono (`PdfRenderer`) y las envía junto a las fotos como `image_url` (`data:image/jpeg;base64,…`).
  - **DeepSeek**: extrae localmente el texto del PDF y usa OCR en el dispositivo (**ML Kit**) para fotos o páginas escaneadas sin texto.
  - **Otro**: sigue el comportamiento de texto/OCR por defecto o envía imágenes si activas **El modelo ve imágenes** en **Avanzado**.
- Las claves de API (`agente.toml`) y el registro de avisos enviados (`enviados.txt`) viven en el almacenamiento privado de la app (`filesDir`), nunca en la carpeta sincronizada.
