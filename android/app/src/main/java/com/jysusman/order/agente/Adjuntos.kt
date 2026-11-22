package com.jysusman.order.agente

import android.content.ContentResolver
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Matrix
import android.graphics.pdf.PdfRenderer
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.OpenableColumns
import androidx.exifinterface.media.ExifInterface
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import com.jysusman.order.datos.PuenteOrder
import com.jysusman.order.ffi.AdjuntoFfi
import com.jysusman.order.ffi.TipoAdjuntoFfi
import kotlinx.coroutines.suspendCancellableCoroutine
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.UUID
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.math.roundToInt

data class AdjuntoPendiente(
    val id: String = UUID.randomUUID().toString(),
    val nombre: String,
    val esPdf: Boolean,
    val adjuntoFfi: AdjuntoFfi,
)

data class ResultadoAdjunto(
    val adjunto: AdjuntoPendiente,
    val aviso: String? = null,
)

sealed interface FuenteAdjunto {
    val nombre: String

    data class Pdf(override val nombre: String, val bytes: ByteArray) : FuenteAdjunto
    data class Imagen(override val nombre: String, val jpeg: ByteArray) : FuenteAdjunto
}

interface ProcesadorPlataforma {
    fun contarPaginasPdf(bytes: ByteArray): Int
    fun renderizarPaginasPdf(bytes: ByteArray, indicesBase0: List<Int>): List<ByteArray>
    suspend fun ocrImagen(jpeg: ByteArray): String
}

/**
 * Prepara PDFs e imágenes en `AdjuntoFfi` según si el modelo configurado
 * acepta imágenes (`agente_ve_imagenes`) o necesita texto/OCR.
 */
class PreparadorAdjuntos(
    private val puente: PuenteOrder,
    private val procesador: ProcesadorPlataforma,
) {
    suspend fun preparar(
        rutaConfig: String,
        fuente: FuenteAdjunto,
        adjuntosActuales: Int,
    ): ResultadoAdjunto {
        val limites = puente.limitesAdjuntos()
        val maxAdjuntos = limites.maxAdjuntos.toInt()
        if (adjuntosActuales >= maxAdjuntos) {
            throw IllegalStateException(
                "Puedes adjuntar como máximo $maxAdjuntos archivos por mensaje."
            )
        }

        val veImagenes = puente.agenteVeImagenes(rutaConfig)
        val maxPaginas = limites.maxPaginasPdf.toInt()

        return when (fuente) {
            is FuenteAdjunto.Pdf -> prepararPdf(fuente, veImagenes, maxPaginas)
            is FuenteAdjunto.Imagen -> prepararImagen(fuente, veImagenes)
        }
    }

    private suspend fun prepararPdf(
        fuente: FuenteAdjunto.Pdf,
        veImagenes: Boolean,
        maxPaginas: Int,
    ): ResultadoAdjunto {
        if (fuente.bytes.isEmpty()) {
            throw IllegalStateException("El archivo «${fuente.nombre}» está vacío.")
        }

        if (veImagenes) {
            val totalPaginas = try {
                procesador.contarPaginasPdf(fuente.bytes)
            } catch (e: SecurityException) {
                throw IllegalStateException(
                    "El PDF «${fuente.nombre}» está protegido con contraseña; quítala para poder leerlo."
                )
            } catch (e: Exception) {
                // Si PdfRenderer falla (p. ej. PDF dañado), delegamos el mensaje legible a order-core.
                runCatching { puente.textoDePdf(fuente.bytes) }.onFailure { throw it }
                throw IllegalStateException("No se pudo abrir el PDF «${fuente.nombre}».")
            }

            if (totalPaginas <= 0) {
                throw IllegalStateException("El PDF «${fuente.nombre}» no tiene páginas.")
            }

            val aviso = if (totalPaginas > maxPaginas) {
                "«${fuente.nombre}» tiene $totalPaginas páginas; se enviarán solo las primeras $maxPaginas."
            } else {
                null
            }
            val indices = (0 until minOf(totalPaginas, maxPaginas)).toList()
            val paginasJpeg = procesador.renderizarPaginasPdf(fuente.bytes, indices)
            val ffi = AdjuntoFfi(
                nombre = fuente.nombre,
                tipo = TipoAdjuntoFfi.PDF,
                bytes = fuente.bytes,
                paginasJpeg = paginasJpeg,
                textoOcr = null,
            )
            return ResultadoAdjunto(
                adjunto = AdjuntoPendiente(
                    nombre = fuente.nombre,
                    esPdf = true,
                    adjuntoFfi = ffi,
                ),
                aviso = aviso,
            )
        }

        val paginasTexto = puente.textoDePdf(fuente.bytes)
        if (paginasTexto.isEmpty()) {
            throw IllegalStateException("El PDF «${fuente.nombre}» no tiene páginas.")
        }

        val aviso = if (paginasTexto.size > maxPaginas) {
            "«${fuente.nombre}» tiene ${paginasTexto.size} páginas; se leerán solo las primeras $maxPaginas."
        } else {
            null
        }
        val recortadas = paginasTexto.take(maxPaginas)
        val indicesSinTexto = puente.paginasSinTexto(recortadas).map { it.toInt() }

        var textoOcr: String? = null
        if (indicesSinTexto.isNotEmpty()) {
            val imagenes = try {
                procesador.renderizarPaginasPdf(fuente.bytes, indicesSinTexto)
            } catch (e: SecurityException) {
                throw IllegalStateException(
                    "El PDF «${fuente.nombre}» está protegido con contraseña; quítala para poder leerlo."
                )
            }
            val partes = ArrayList<String>(imagenes.size)
            for (jpeg in imagenes) {
                partes.add(procesador.ocrImagen(jpeg).trim())
            }
            val unido = partes.joinToString("\u000C")
            if (unido.replace("\u000C", "").isNotBlank()) {
                textoOcr = unido
            } else if (indicesSinTexto.size == recortadas.size) {
                throw IllegalStateException(
                    "No pude leer texto en este archivo; prueba con Gemini o Claude."
                )
            }
        }

        val ffi = AdjuntoFfi(
            nombre = fuente.nombre,
            tipo = TipoAdjuntoFfi.PDF,
            bytes = fuente.bytes,
            paginasJpeg = emptyList(),
            textoOcr = textoOcr,
        )
        return ResultadoAdjunto(
            adjunto = AdjuntoPendiente(
                nombre = fuente.nombre,
                esPdf = true,
                adjuntoFfi = ffi,
            ),
            aviso = aviso,
        )
    }

    private suspend fun prepararImagen(
        fuente: FuenteAdjunto.Imagen,
        veImagenes: Boolean,
    ): ResultadoAdjunto {
        if (fuente.jpeg.isEmpty()) {
            throw IllegalStateException("La imagen «${fuente.nombre}» está vacía.")
        }

        val textoOcr = if (veImagenes) {
            null
        } else {
            val ocr = procesador.ocrImagen(fuente.jpeg).trim()
            if (ocr.isBlank()) {
                throw IllegalStateException(
                    "No pude leer texto en este archivo; prueba con Gemini o Claude."
                )
            }
            ocr
        }

        val ffi = AdjuntoFfi(
            nombre = fuente.nombre,
            tipo = TipoAdjuntoFfi.IMAGEN,
            bytes = fuente.jpeg,
            paginasJpeg = emptyList(),
            textoOcr = textoOcr,
        )
        return ResultadoAdjunto(
            adjunto = AdjuntoPendiente(
                nombre = fuente.nombre,
                esPdf = false,
                adjuntoFfi = ffi,
            ),
            aviso = null,
        )
    }
}

class ProcesadorAndroid(
    private val context: Context,
    private val ladoLargo: Int = 1568,
    private val calidadJpeg: Int = 85,
) : ProcesadorPlataforma {

    override fun contarPaginasPdf(bytes: ByteArray): Int =
        conPdfRenderer(bytes) { renderer -> renderer.pageCount }

    override fun renderizarPaginasPdf(bytes: ByteArray, indicesBase0: List<Int>): List<ByteArray> =
        conPdfRenderer(bytes) { renderer ->
            indicesBase0.mapNotNull { idx ->
                if (idx < 0 || idx >= renderer.pageCount) return@mapNotNull null
                renderer.openPage(idx).use { pagina ->
                    val maxDim = maxOf(pagina.width, pagina.height).coerceAtLeast(1)
                    val escala = minOf(ladoLargo.toFloat() / maxDim.toFloat(), 2f)
                    val ancho = (pagina.width * escala).roundToInt().coerceAtLeast(1)
                    val alto = (pagina.height * escala).roundToInt().coerceAtLeast(1)
                    val bitmap = Bitmap.createBitmap(ancho, alto, Bitmap.Config.ARGB_8888)
                    try {
                        Canvas(bitmap).drawColor(Color.WHITE)
                        pagina.render(bitmap, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY)
                        comprimirJpeg(bitmap, calidadJpeg)
                    } finally {
                        bitmap.recycle()
                    }
                }
            }
        }

    override suspend fun ocrImagen(jpeg: ByteArray): String {
        val bitmap = BitmapFactory.decodeByteArray(jpeg, 0, jpeg.size)
            ?: return ""
        return try {
            val imagen = InputImage.fromBitmap(bitmap, 0)
            val reconocedor = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
            suspendCancellableCoroutine { cont ->
                reconocedor.process(imagen)
                    .addOnSuccessListener { resultado ->
                        if (cont.isActive) cont.resume(resultado.text)
                    }
                    .addOnFailureListener { err ->
                        if (cont.isActive) cont.resumeWithException(err)
                    }
            }
        } finally {
            bitmap.recycle()
        }
    }

    private fun <T> conPdfRenderer(bytes: ByteArray, bloque: (PdfRenderer) -> T): T {
        val temp = File.createTempFile("order-pdf-", ".pdf", context.cacheDir)
        try {
            temp.writeBytes(bytes)
            ParcelFileDescriptor.open(temp, ParcelFileDescriptor.MODE_READ_ONLY).use { pfd ->
                PdfRenderer(pfd).use { renderer ->
                    return bloque(renderer)
                }
            }
        } finally {
            temp.delete()
        }
    }

    companion object {
        fun leerFuenteDesdeUri(
            context: Context,
            uri: Uri,
            ladoLargo: Int = 1568,
            calidadJpeg: Int = 85,
            nombreForzado: String? = null,
        ): FuenteAdjunto {
            val resolver = context.contentResolver
            val nombre = nombreForzado ?: obtenerNombre(resolver, uri)
            val mime = resolver.getType(uri).orEmpty().lowercase()
            val esPdf = mime == "application/pdf" || nombre.lowercase().endsWith(".pdf")

            return if (esPdf) {
                val bytes = resolver.openInputStream(uri)?.use { it.readBytes() }
                    ?: throw IllegalStateException("No se pudo abrir «$nombre».")
                FuenteAdjunto.Pdf(nombre = nombre, bytes = bytes)
            } else {
                val jpeg = normalizarImagenUri(resolver, uri, ladoLargo, calidadJpeg, nombre)
                FuenteAdjunto.Imagen(nombre = nombre, jpeg = jpeg)
            }
        }

        private fun obtenerNombre(resolver: ContentResolver, uri: Uri): String {
            resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) {
                    val idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (idx >= 0) {
                        val nombre = cursor.getString(idx)?.trim()
                        if (!nombre.isNullOrEmpty()) return nombre
                    }
                }
            }
            return uri.lastPathSegment?.substringAfterLast('/')?.takeIf { it.isNotBlank() }
                ?: "archivo"
        }

        private fun normalizarImagenUri(
            resolver: ContentResolver,
            uri: Uri,
            ladoLargo: Int,
            calidadJpeg: Int,
            nombre: String,
        ): ByteArray {
            val orientacion = resolver.openInputStream(uri)?.use { input ->
                runCatching {
                    ExifInterface(input).getAttributeInt(
                        ExifInterface.TAG_ORIENTATION,
                        ExifInterface.ORIENTATION_NORMAL,
                    )
                }.getOrDefault(ExifInterface.ORIENTATION_NORMAL)
            } ?: ExifInterface.ORIENTATION_NORMAL

            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            resolver.openInputStream(uri)?.use { input ->
                BitmapFactory.decodeStream(input, null, bounds)
            }
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) {
                throw IllegalStateException("No se pudo leer la imagen «$nombre».")
            }

            var inSampleSize = 1
            val maxOriginal = maxOf(bounds.outWidth, bounds.outHeight)
            while (maxOriginal / (inSampleSize * 2) >= ladoLargo) {
                inSampleSize *= 2
            }

            val decodeOpts = BitmapFactory.Options().apply { this.inSampleSize = inSampleSize }
            val decodificado = resolver.openInputStream(uri)?.use { input ->
                BitmapFactory.decodeStream(input, null, decodeOpts)
            } ?: throw IllegalStateException("No se pudo decodificar la imagen «$nombre».")

            val rotado = aplicarExif(decodificado, orientacion)
            if (rotado !== decodificado) {
                decodificado.recycle()
            }

            val maxRotado = maxOf(rotado.width, rotado.height)
            val finalBitmap = if (maxRotado > ladoLargo) {
                val factor = ladoLargo.toFloat() / maxRotado.toFloat()
                val nuevoAncho = (rotado.width * factor).roundToInt().coerceAtLeast(1)
                val nuevoAlto = (rotado.height * factor).roundToInt().coerceAtLeast(1)
                Bitmap.createScaledBitmap(rotado, nuevoAncho, nuevoAlto, true).also {
                    if (it !== rotado) rotado.recycle()
                }
            } else {
                rotado
            }

            return try {
                comprimirJpeg(finalBitmap, calidadJpeg)
            } finally {
                finalBitmap.recycle()
            }
        }

        private fun aplicarExif(bitmap: Bitmap, orientacion: Int): Bitmap {
            val matrix = Matrix()
            when (orientacion) {
                ExifInterface.ORIENTATION_ROTATE_90 -> matrix.postRotate(90f)
                ExifInterface.ORIENTATION_ROTATE_180 -> matrix.postRotate(180f)
                ExifInterface.ORIENTATION_ROTATE_270 -> matrix.postRotate(270f)
                ExifInterface.ORIENTATION_FLIP_HORIZONTAL -> matrix.preScale(-1f, 1f)
                ExifInterface.ORIENTATION_FLIP_VERTICAL -> matrix.preScale(1f, -1f)
                ExifInterface.ORIENTATION_TRANSPOSE -> {
                    matrix.postRotate(90f)
                    matrix.preScale(-1f, 1f)
                }
                ExifInterface.ORIENTATION_TRANSVERSE -> {
                    matrix.postRotate(270f)
                    matrix.preScale(-1f, 1f)
                }
                else -> return bitmap
            }
            return Bitmap.createBitmap(bitmap, 0, 0, bitmap.width, bitmap.height, matrix, true)
        }

        private fun comprimirJpeg(bitmap: Bitmap, calidad: Int): ByteArray {
            val salida = ByteArrayOutputStream()
            bitmap.compress(Bitmap.CompressFormat.JPEG, calidad, salida)
            return salida.toByteArray()
        }
    }
}
