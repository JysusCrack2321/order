package com.jysusman.order.agente

import com.jysusman.order.datos.FakePuenteOrder
import com.jysusman.order.datos.Repositorio
import com.jysusman.order.ffi.TipoAdjuntoFfi
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class AdjuntosTest {

    @Test
    fun eligeVisionOCRSengunAgenteVeImagenes() = runTest {
        val fakePuente = FakePuenteOrder().apply {
            veImagenes = true
        }
        val fakeProc = FakeProcesadorPlataforma().apply {
            totalPaginasPdf = 22
        }
        val preparador = PreparadorAdjuntos(fakePuente, fakeProc)

        // Con visión: PDF > 20 páginas se recorta a 20, avisa y no ejecuta OCR.
        val resPdfVision = preparador.preparar(
            rutaConfig = "/tmp/agente.toml",
            fuente = FuenteAdjunto.Pdf("horario.pdf", byteArrayOf(1, 2, 3)),
            adjuntosActuales = 0,
        )
        assertEquals(TipoAdjuntoFfi.PDF, resPdfVision.adjunto.adjuntoFfi.tipo)
        assertEquals(20, resPdfVision.adjunto.adjuntoFfi.paginasJpeg.size)
        assertNull(resPdfVision.adjunto.adjuntoFfi.textoOcr)
        assertTrue(resPdfVision.aviso?.contains("20") == true)
        assertEquals(0, fakeProc.llamadasOcr)

        // Con visión: imagen no ejecuta OCR.
        val resImgVision = preparador.preparar(
            rutaConfig = "/tmp/agente.toml",
            fuente = FuenteAdjunto.Imagen("foto.jpg", byteArrayOf(9, 8, 7)),
            adjuntosActuales = 1,
        )
        assertEquals(TipoAdjuntoFfi.IMAGEN, resImgVision.adjunto.adjuntoFfi.tipo)
        assertNull(resImgVision.adjunto.adjuntoFfi.textoOcr)
        assertEquals(0, fakeProc.llamadasOcr)

        // Sin visión: PDF usa textoDePdf y solo hace OCR en páginas sin texto.
        fakePuente.veImagenes = false
        fakePuente.paginasPdfFake = listOf("Clase de Control", "", "Taller 2")
        fakeProc.ocrResultado = "Parcial viernes 8am"

        val resPdfTexto = preparador.preparar(
            rutaConfig = "/tmp/agente.toml",
            fuente = FuenteAdjunto.Pdf("mixto.pdf", byteArrayOf(1, 2, 3)),
            adjuntosActuales = 0,
        )
        assertTrue(resPdfTexto.adjunto.adjuntoFfi.paginasJpeg.isEmpty())
        assertEquals("Parcial viernes 8am", resPdfTexto.adjunto.adjuntoFfi.textoOcr)
        assertEquals(listOf(1), fakeProc.paginasRenderizadas.last())
        assertEquals(1, fakeProc.llamadasOcr)

        // Sin visión: imagen ejecuta OCR y lo adjunta.
        fakeProc.ocrResultado = "Entrega laboratorio"
        val resImgTexto = preparador.preparar(
            rutaConfig = "/tmp/agente.toml",
            fuente = FuenteAdjunto.Imagen("tablero.jpg", byteArrayOf(4, 5)),
            adjuntosActuales = 1,
        )
        assertEquals("Entrega laboratorio", resImgTexto.adjunto.adjuntoFfi.textoOcr)
        assertEquals(2, fakeProc.llamadasOcr)
    }

    @Test
    fun repositorioAdjuntaQuitaEnviaYLimpia() = runTest {
        val fakePuente = FakePuenteOrder().apply { veImagenes = true }
        val fakeProc = FakeProcesadorPlataforma().apply { totalPaginasPdf = 2 }
        val repo = Repositorio(
            puente = fakePuente,
            carpetaInicial = "/tmp/order-adj",
            dispatcherIo = UnconfinedTestDispatcher(testScheduler),
            asegurarCarpeta = {},
            rutaConfigAgente = "/tmp/agente.toml",
            procesadorAdjuntos = fakeProc,
        )
        repo.cargarInicial()

        repo.adjuntar(FuenteAdjunto.Imagen("apunte.jpg", byteArrayOf(1, 2, 3)))
        assertEquals(1, repo.estado.value.agenteAdjuntos.size)

        val id = repo.estado.value.agenteAdjuntos.first().id
        repo.quitarAdjunto(id)
        assertTrue(repo.estado.value.agenteAdjuntos.isEmpty())

        repo.adjuntar(FuenteAdjunto.Pdf("horario.pdf", byteArrayOf(4, 5, 6)))
        repo.enviarAlAgente()
        assertEquals(1, fakePuente.ultimosAdjuntosEnviados.size)
        assertTrue(repo.estado.value.agenteAdjuntos.isEmpty())
        assertTrue(repo.estado.value.agentePanelAbierto)
    }
}

private class FakeProcesadorPlataforma : ProcesadorPlataforma {
    var totalPaginasPdf: Int = 1
    var ocrResultado: String = ""
    var llamadasOcr: Int = 0
    val paginasRenderizadas = mutableListOf<List<Int>>()

    override fun contarPaginasPdf(bytes: ByteArray): Int = totalPaginasPdf

    override fun renderizarPaginasPdf(bytes: ByteArray, indicesBase0: List<Int>): List<ByteArray> {
        paginasRenderizadas.add(indicesBase0)
        return indicesBase0.map { byteArrayOf(it.toByte()) }
    }

    override suspend fun ocrImagen(jpeg: ByteArray): String {
        llamadasOcr++
        return ocrResultado
    }
}
