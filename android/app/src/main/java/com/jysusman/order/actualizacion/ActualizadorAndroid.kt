package com.jysusman.order.actualizacion

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import kotlin.coroutines.coroutineContext

object ActualizadorAndroid {
    const val URL_RELEASES_LATEST =
        "https://api.github.com/repos/JysusCrack2321/order/releases/latest"
    const val USER_AGENT = "ORDER-app"

    fun versionInstalada(context: Context): String = runCatching {
        val pm = context.packageManager
        val info = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            pm.getPackageInfo(context.packageName, PackageManager.PackageInfoFlags.of(0))
        } else {
            @Suppress("DEPRECATION")
            pm.getPackageInfo(context.packageName, 0)
        }
        info.versionName?.takeIf { it.isNotBlank() } ?: "0.0.0"
    }.getOrDefault("0.0.0")

    fun puedeInstalarPaquetes(context: Context): Boolean =
        context.packageManager.canRequestPackageInstalls()

    fun intentPermisoInstalacion(context: Context): Intent = Intent(
        Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
        Uri.parse("package:${context.packageName}"),
    )

    suspend fun buscarUltimaVersion(
        urlApi: String = URL_RELEASES_LATEST,
    ): Result<ReleaseGitHub> = withContext(Dispatchers.IO) {
        runCatching {
            val conexion = (URL(urlApi).openConnection() as HttpURLConnection).apply {
                requestMethod = "GET"
                connectTimeout = 10_000
                readTimeout = 15_000
                setRequestProperty("User-Agent", USER_AGENT)
                setRequestProperty("Accept", "application/vnd.github+json")
            }
            try {
                val codigo = conexion.responseCode
                if (codigo !in 200..299) {
                    error(mensajeErrorHttp(codigo))
                }
                val cuerpo = conexion.inputStream.bufferedReader().use { it.readText() }
                ActualizadorLogica.parsearReleaseGitHub(cuerpo).getOrThrow()
            } finally {
                conexion.disconnect()
            }
        }.recoverCatching { err ->
            throw IllegalStateException(mensajeCortoError(err), err)
        }
    }

    suspend fun descargarApk(
        context: Context,
        release: ReleaseGitHub,
        alProgreso: suspend (Float?) -> Unit,
    ): Result<File> = withContext(Dispatchers.IO) {
        val carpeta = File(context.cacheDir, "actualizaciones").apply { mkdirs() }
        // Limpia descargas anteriores para no acumular APKs en caché.
        carpeta.listFiles()?.forEach { it.delete() }

        val nombreSeguro = release.nombreAssetApk
            .replace(Regex("[^A-Za-z0-9._-]"), "_")
            .ifBlank { "ORDER-${release.versionLimpia}.apk" }
        val archivoFinal = File(carpeta, nombreSeguro)
        val archivoTemp = File(carpeta, "$nombreSeguro.tmp")

        runCatching {
            val conexion = abrirConexionConRedirecciones(release.urlDescargaApk)
            try {
                val codigo = conexion.responseCode
                if (codigo !in 200..299) {
                    error("No se pudo descargar el APK (HTTP $codigo).")
                }
                val total = conexion.contentLengthLong.takeIf { it > 0L }
                    ?: release.tamanoBytes.takeIf { it > 0L }
                    ?: -1L

                var descargado = 0L
                val buffer = ByteArray(16 * 1024)
                var ultimoPorcentaje = -1

                alProgreso(if (total > 0L) 0f else null)
                conexion.inputStream.use { entrada ->
                    FileOutputStream(archivoTemp).use { salida ->
                        while (true) {
                            coroutineContext.ensureActive()
                            val leidos = entrada.read(buffer)
                            if (leidos < 0) break
                            salida.write(buffer, 0, leidos)
                            descargado += leidos
                            if (total > 0L) {
                                val porcentaje = ((descargado * 100L) / total).toInt().coerceIn(0, 100)
                                if (porcentaje != ultimoPorcentaje) {
                                    ultimoPorcentaje = porcentaje
                                    alProgreso((descargado.toFloat() / total.toFloat()).coerceIn(0f, 1f))
                                }
                            }
                        }
                        salida.fd.sync()
                    }
                }

                if (release.tamanoBytes > 0L && archivoTemp.length() != release.tamanoBytes) {
                    error("Descarga incompleta (${archivoTemp.length()} de ${release.tamanoBytes} bytes).")
                }
                if (archivoTemp.length() <= 0L) {
                    error("El archivo descargado está vacío.")
                }

                if (archivoFinal.exists()) archivoFinal.delete()
                if (!archivoTemp.renameTo(archivoFinal)) {
                    archivoTemp.copyTo(archivoFinal, overwrite = true)
                    archivoTemp.delete()
                }
                alProgreso(1f)
                archivoFinal
            } finally {
                conexion.disconnect()
                if (archivoTemp.exists()) archivoTemp.delete()
            }
        }.recoverCatching { err ->
            if (err is kotlinx.coroutines.CancellationException) throw err
            throw IllegalStateException(mensajeCortoError(err), err)
        }
    }

    fun instalarApk(context: Context, archivoApk: File): Result<Unit> = runCatching {
        val uri = FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            archivoApk,
        )
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, "application/vnd.android.package-archive")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        context.startActivity(intent)
    }.recoverCatching { err ->
        throw IllegalStateException("No se pudo iniciar la instalación: ${err.message ?: "error"}", err)
    }

    private fun abrirConexionConRedirecciones(urlInicial: String): HttpURLConnection {
        var actual = urlInicial
        repeat(6) {
            val conexion = (URL(actual).openConnection() as HttpURLConnection).apply {
                requestMethod = "GET"
                connectTimeout = 15_000
                readTimeout = 30_000
                instanceFollowRedirects = false
                setRequestProperty("User-Agent", USER_AGENT)
            }
            val codigo = conexion.responseCode
            if (codigo in 300..399) {
                val destino = conexion.getHeaderField("Location")?.trim().orEmpty()
                conexion.disconnect()
                if (destino.isEmpty()) error("Redirección sin destino al descargar.")
                actual = URL(URL(actual), destino).toString()
            } else {
                return conexion
            }
        }
        error("Demasiadas redirecciones al descargar.")
    }

    private fun mensajeErrorHttp(codigo: Int): String = when (codigo) {
        403, 429 -> "GitHub limitó las consultas; intenta más tarde."
        404 -> "Aún no hay versiones publicadas en GitHub."
        else -> "No se pudo comprobar actualizaciones (HTTP $codigo)."
    }

    private fun mensajeCortoError(err: Throwable): String {
        val msg = err.message.orEmpty()
        return when {
            err is java.net.UnknownHostException ||
                err is java.net.ConnectException ||
                err is java.net.SocketTimeoutException -> "Sin conexión para comprobar actualizaciones."
            msg.isNotBlank() -> msg
            else -> "No se pudo completar la actualización."
        }
    }
}
