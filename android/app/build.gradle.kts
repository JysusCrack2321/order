import java.io.File
import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val rutaFirma = System.getenv("ORDER_FIRMA")?.takeIf { it.isNotBlank() }
    ?: "${System.getProperty("user.home")}/.config/order-firma/firma.properties"
val archivoFirma = file(rutaFirma)

fun resolverArchivoKeystore(ruta: String, baseDir: File): File {
    val expandida = if (ruta.startsWith("~/")) {
        "${System.getProperty("user.home")}${ruta.removePrefix("~")}"
    } else {
        ruta
    }
    val f = File(expandida)
    return if (f.isAbsolute) f else File(baseDir, expandida)
}

android {
    namespace = "com.jysusman.order"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.jysusman.order"
        minSdk = 31
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    signingConfigs {
        create("release") {
            if (archivoFirma.exists()) {
                val props = Properties()
                archivoFirma.inputStream().use { props.load(it) }
                val storeFileProp = props.getProperty("storeFile")?.trim().orEmpty()
                val storePassProp = props.getProperty("storePassword").orEmpty()
                val keyAliasProp = props.getProperty("keyAlias")?.trim().orEmpty()
                val keyPassProp = props.getProperty("keyPassword").orEmpty()
                if (storeFileProp.isNotEmpty()) {
                    storeFile = resolverArchivoKeystore(storeFileProp, archivoFirma.parentFile)
                }
                if (storePassProp.isNotEmpty()) {
                    storePassword = storePassProp
                }
                if (keyAliasProp.isNotEmpty()) {
                    keyAlias = keyAliasProp
                }
                if (keyPassProp.isNotEmpty()) {
                    keyPassword = keyPassProp
                }
            }
        }
    }

    buildTypes {
        debug {
            if (archivoFirma.exists()) {
                signingConfig = signingConfigs.getByName("release")
            }
            ndk {
                abiFilters += listOf("arm64-v8a", "x86_64")
            }
        }
        release {
            signingConfig = signingConfigs.getByName("release")
            // Se mantienen desactivados en v0.1.0: JNA/UniFFI requieren reglas R8 adicionales.
            isMinifyEnabled = false
            isShrinkResources = false
            ndk {
                abiFilters += listOf("arm64-v8a")
            }
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlin {
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
        }
    }

    buildFeatures {
        compose = true
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2026.06.01")
    implementation(composeBom)
    androidTestImplementation(composeBom)

    implementation("androidx.core:core-ktx:1.17.0")
    implementation("androidx.activity:activity-compose:1.12.4")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.9.4")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.4")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.9.4")

    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")

    implementation("net.java.dev.jna:jna:5.18.1@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
    implementation("androidx.work:work-runtime-ktx:2.10.0")
    implementation("androidx.exifinterface:exifinterface:1.4.1")
    implementation("com.google.android.gms:play-services-mlkit-text-recognition:19.0.1")
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")

    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.10.2")

    debugImplementation("androidx.compose.ui:ui-tooling")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}

tasks.matching { it.name.contains("Release") }.configureEach {
    doFirst {
        if (!archivoFirma.exists()) {
            throw GradleException(
                "No se encontró el archivo de propiedades de firma '$rutaFirma'. " +
                    "Define ORDER_FIRMA o crea ~/.config/order-firma/firma.properties para compilar en release."
            )
        }
        val props = Properties()
        archivoFirma.inputStream().use { props.load(it) }
        val faltantes = listOf("storeFile", "storePassword", "keyAlias", "keyPassword")
            .filter { props.getProperty(it).isNullOrBlank() }
        if (faltantes.isNotEmpty()) {
            throw GradleException(
                "Faltan propiedades obligatorias en '$rutaFirma': ${faltantes.joinToString(", ")}."
            )
        }
        val keystore = resolverArchivoKeystore(
            props.getProperty("storeFile").trim(),
            archivoFirma.parentFile
        )
        if (!keystore.isFile) {
            throw GradleException(
                "No existe el almacén de claves indicado en storeFile dentro de '$rutaFirma'."
            )
        }
    }
}
