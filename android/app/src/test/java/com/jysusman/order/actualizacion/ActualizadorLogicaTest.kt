package com.jysusman.order.actualizacion

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ActualizadorLogicaTest {

    @Test
    fun comparaVersionesSemanticasIgnorandoV() {
        assertTrue(ActualizadorLogica.esVersionMasNueva("v0.1.1", "0.1.0"))
        assertTrue(ActualizadorLogica.esVersionMasNueva("0.2.0", "v0.1.9"))
        assertTrue(ActualizadorLogica.esVersionMasNueva("v1.0.0", "0.9.9"))
        assertTrue(ActualizadorLogica.esVersionMasNueva("v0.1.10", "0.1.2"))
        assertTrue(ActualizadorLogica.esVersionMasNueva("0.1.1", "0.1.1-rc.1"))

        assertFalse(ActualizadorLogica.esVersionMasNueva("v0.1.0", "0.1.0"))
        assertFalse(ActualizadorLogica.esVersionMasNueva("0.1.0", "v0.1.1"))
        assertFalse(ActualizadorLogica.esVersionMasNueva("v0.0.9", "0.1.0"))
        assertFalse(ActualizadorLogica.esVersionMasNueva("invalida", "0.1.0"))
        assertFalse(ActualizadorLogica.esVersionMasNueva("v0.1.1", ""))

        val v = ActualizadorLogica.parsearVersionSemantica("V1.2.3+build45")
        assertNotNull(v)
        assertEquals(VersionSemantica(1, 2, 3, null), v)
        assertNull(ActualizadorLogica.parsearVersionSemantica("v0.1.x"))
        assertNull(ActualizadorLogica.parsearVersionSemantica("v0.1.1-"))
    }

    @Test
    fun parseaJsonDeGitHubReleasesLatest() {
        val jsonEjemplo = """
            {
              "url": "https://api.github.com/repos/JysusCrack2321/order/releases/12345",
              "tag_name": "v0.1.1",
              "name": "ORDER v0.1.1",
              "draft": false,
              "prerelease": false,
              "body": "## Novedades\r\n- Actualizador automático desde GitHub.\r\n- Arreglo de \"avisos\" en Android.\r\n",
              "assets": [
                {
                  "name": "order_0.1.1_amd64.deb",
                  "size": 12345678,
                  "browser_download_url": "https://github.com/JysusCrack2321/order/releases/download/v0.1.1/order_0.1.1_amd64.deb"
                },
                {
                  "name": "ORDER-0.1.1.apk",
                  "size": 45678901,
                  "browser_download_url": "https://github.com/JysusCrack2321/order/releases/download/v0.1.1/ORDER-0.1.1.apk"
                },
                {
                  "name": "SHA256SUMS",
                  "size": 512,
                  "browser_download_url": "https://github.com/JysusCrack2321/order/releases/download/v0.1.1/SHA256SUMS"
                }
              ]
            }
        """.trimIndent()

        val resultado = ActualizadorLogica.parsearReleaseGitHub(jsonEjemplo)
        assertTrue(resultado.isSuccess)
        val release = resultado.getOrThrow()
        assertEquals("v0.1.1", release.tagName)
        assertEquals("0.1.1", release.versionLimpia)
        assertEquals("ORDER-0.1.1.apk", release.nombreAssetApk)
        assertEquals(
            "https://github.com/JysusCrack2321/order/releases/download/v0.1.1/ORDER-0.1.1.apk",
            release.urlDescargaApk,
        )
        assertEquals(45678901L, release.tamanoBytes)
        assertTrue(release.notas.contains("Actualizador automático desde GitHub."))
        assertTrue(release.notas.contains("\"avisos\""))
    }

    @Test
    fun fallaSiJsonNoTieneApkOResultadoInvalido() {
        val sinApk = """
            {
              "tag_name": "v0.1.1",
              "body": null,
              "assets": [
                {
                  "name": "SHA256SUMS",
                  "size": 100,
                  "browser_download_url": "https://example.com/SHA256SUMS"
                }
              ]
            }
        """.trimIndent()

        val resSinApk = ActualizadorLogica.parsearReleaseGitHub(sinApk)
        assertTrue(resSinApk.isFailure)

        val resRoto = ActualizadorLogica.parsearReleaseGitHub("{no valido")
        assertTrue(resRoto.isFailure)
    }

    @Test
    fun recortaNotasYControlaFrecuenciaYOmisionDiaria() {
        val largo = (1..20).joinToString("\n") { "Línea número $it con bastante texto descriptivo" }
        val recortado = ActualizadorLogica.recortarNotasRelease(largo, maxCaracteres = 120, maxLineas = 5)
        assertTrue(recortado.endsWith("…"))
        assertTrue(recortado.length <= 121)

        val seisHoras = 6L * 60L * 60L * 1000L
        assertTrue(ActualizadorLogica.debeComprobarAutomaticamente(0L, 10_000L))
        assertFalse(ActualizadorLogica.debeComprobarAutomaticamente(10_000L, 10_000L + seisHoras - 1L))
        assertTrue(ActualizadorLogica.debeComprobarAutomaticamente(10_000L, 10_000L + seisHoras))

        assertTrue(
            ActualizadorLogica.estaOmitidaParaHoy(
                versionOmitida = "0.1.1",
                fechaOmitidaIso = "2026-11-22",
                versionCandidata = "v0.1.1",
                fechaHoyIso = "2026-11-22",
            )
        )
        assertFalse(
            ActualizadorLogica.estaOmitidaParaHoy(
                versionOmitida = "0.1.1",
                fechaOmitidaIso = "2026-11-22",
                versionCandidata = "v0.1.1",
                fechaHoyIso = "2026-11-23",
            )
        )
        assertFalse(
            ActualizadorLogica.estaOmitidaParaHoy(
                versionOmitida = "0.1.1",
                fechaOmitidaIso = "2026-11-22",
                versionCandidata = "v0.1.2",
                fechaHoyIso = "2026-11-22",
            )
        )
    }
}
