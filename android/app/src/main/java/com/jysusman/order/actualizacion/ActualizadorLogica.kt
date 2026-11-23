package com.jysusman.order.actualizacion

/**
 * Lógica pura de actualización desde GitHub Releases (comparación semántica,
 * parseo de JSON y reglas de frecuencia/silenciado).
 */
data class VersionSemantica(
    val mayor: Int,
    val menor: Int,
    val parche: Int,
    val preLanzamiento: String? = null,
) : Comparable<VersionSemantica> {
    override fun compareTo(other: VersionSemantica): Int {
        if (mayor != other.mayor) return mayor.compareTo(other.mayor)
        if (menor != other.menor) return menor.compareTo(other.menor)
        if (parche != other.parche) return parche.compareTo(other.parche)
        return when {
            preLanzamiento == null && other.preLanzamiento == null -> 0
            preLanzamiento == null -> 1 // estable > pre-lanzamiento
            other.preLanzamiento == null -> -1
            else -> compararPreLanzamiento(preLanzamiento, other.preLanzamiento)
        }
    }
}

private fun compararPreLanzamiento(a: String, b: String): Int {
    val partesA = a.split('.')
    val partesB = b.split('.')
    val limite = minOf(partesA.size, partesB.size)
    for (i in 0 until limite) {
        val pa = partesA[i]
        val pb = partesB[i]
        val na = pa.toIntOrNull()
        val nb = pb.toIntOrNull()
        val cmp = when {
            na != null && nb != null -> na.compareTo(nb)
            na != null -> -1 // numérico < alfanumérico según SemVer
            nb != null -> 1
            else -> pa.compareTo(pb)
        }
        if (cmp != 0) return cmp
    }
    return partesA.size.compareTo(partesB.size)
}

data class ReleaseGitHub(
    val tagName: String,
    val versionLimpia: String,
    val notas: String,
    val nombreAssetApk: String,
    val urlDescargaApk: String,
    val tamanoBytes: Long,
)

object ActualizadorLogica {
    const val INTERVALO_COMPROBACION_MS: Long = 6L * 60L * 60L * 1000L

    fun parsearVersionSemantica(texto: String): VersionSemantica? {
        val limpia = texto.trim()
            .removePrefix("v")
            .removePrefix("V")
            .substringBefore('+')
        if (limpia.isEmpty()) return null

        val indiceGuion = limpia.indexOf('-')
        val nucleo = if (indiceGuion >= 0) limpia.substring(0, indiceGuion) else limpia
        val pre = if (indiceGuion >= 0) limpia.substring(indiceGuion + 1).takeIf { it.isNotEmpty() } else null
        if (indiceGuion >= 0 && pre == null) return null

        val partes = nucleo.split('.')
        if (partes.isEmpty() || partes.size > 3) return null

        val numeros = partes.map { parte ->
            if (parte.isEmpty() || !parte.all { it.isDigit() }) return null
            parte.toIntOrNull() ?: return null
        }

        return VersionSemantica(
            mayor = numeros[0],
            menor = numeros.getOrElse(1) { 0 },
            parche = numeros.getOrElse(2) { 0 },
            preLanzamiento = pre,
        )
    }

    fun esVersionMasNueva(versionRemota: String, versionInstalada: String): Boolean {
        val remota = parsearVersionSemantica(versionRemota) ?: return false
        val instalada = parsearVersionSemantica(versionInstalada) ?: return false
        return remota > instalada
    }

    fun recortarNotasRelease(
        body: String,
        maxCaracteres: Int = 500,
        maxLineas: Int = 10,
    ): String {
        val normalizado = body.replace("\r\n", "\n").replace('\r', '\n').trim()
        if (normalizado.isEmpty()) return ""

        val lineas = normalizado.lines()
        val recortadoLineas = if (lineas.size > maxLineas) {
            lineas.take(maxLineas).joinToString("\n").trimEnd() + "…"
        } else {
            normalizado
        }

        return if (recortadoLineas.length > maxCaracteres) {
            recortadoLineas.take(maxCaracteres).trimEnd() + "…"
        } else {
            recortadoLineas
        }
    }

    fun debeComprobarAutomaticamente(
        ultimaComprobacionMs: Long,
        ahoraMs: Long,
        intervaloMs: Long = INTERVALO_COMPROBACION_MS,
    ): Boolean {
        if (ultimaComprobacionMs <= 0L) return true
        if (ahoraMs < ultimaComprobacionMs) return true
        return (ahoraMs - ultimaComprobacionMs) >= intervaloMs
    }

    fun estaOmitidaParaHoy(
        versionOmitida: String?,
        fechaOmitidaIso: String?,
        versionCandidata: String,
        fechaHoyIso: String,
    ): Boolean {
        if (versionOmitida.isNullOrBlank() || fechaOmitidaIso.isNullOrBlank()) return false
        val vOmitida = versionOmitida.trim().removePrefix("v").removePrefix("V")
        val vCandidata = versionCandidata.trim().removePrefix("v").removePrefix("V")
        return vOmitida == vCandidata && fechaOmitidaIso == fechaHoyIso
    }

    fun parsearReleaseGitHub(jsonTexto: String): Result<ReleaseGitHub> = runCatching {
        val raiz = JsonLector(jsonTexto).leerValor() as? Map<*, *>
            ?: error("Respuesta de GitHub no válida.")

        val tagName = (raiz["tag_name"] as? String)?.trim().orEmpty()
        if (tagName.isEmpty()) error("El lanzamiento no tiene etiqueta de versión.")
        val versionLimpia = tagName.removePrefix("v").removePrefix("V")
        if (parsearVersionSemantica(versionLimpia) == null) {
            error("Versión no válida en GitHub: $tagName")
        }

        val cuerpo = (raiz["body"] as? String).orEmpty()
        val notas = recortarNotasRelease(cuerpo)

        val assets = raiz["assets"] as? List<*> ?: emptyList<Any?>()
        val assetApk = assets.asSequence()
            .mapNotNull { it as? Map<*, *> }
            .firstOrNull { asset ->
                val nombre = (asset["name"] as? String)?.trim().orEmpty()
                val url = (asset["browser_download_url"] as? String)?.trim().orEmpty()
                nombre.lowercase().endsWith(".apk") && url.isNotEmpty()
            } ?: error("El lanzamiento no incluye un archivo APK.")

        val nombreApk = (assetApk["name"] as String).trim()
        val urlApk = (assetApk["browser_download_url"] as String).trim()
        val tamano = (assetApk["size"] as? Number)?.toLong() ?: -1L

        ReleaseGitHub(
            tagName = tagName,
            versionLimpia = versionLimpia,
            notas = notas,
            nombreAssetApk = nombreApk,
            urlDescargaApk = urlApk,
            tamanoBytes = tamano,
        )
    }
}

/**
 * Lector JSON minimalista compatible con JVM y Android sin depender de stubs de org.json.
 */
internal class JsonLector(private val entrada: String) {
    private var pos = 0

    fun leerValor(): Any? {
        saltarEspacios()
        val valor = leerSiguiente()
        saltarEspacios()
        if (pos != entrada.length) {
            error("Contenido inesperado tras el JSON en posición $pos.")
        }
        return valor
    }

    private fun leerSiguiente(): Any? {
        saltarEspacios()
        if (pos >= entrada.length) error("JSON incompleto.")
        return when (val c = entrada[pos]) {
            '{' -> leerObjeto()
            '[' -> leerArreglo()
            '"' -> leerCadena()
            't' -> leerLiteral("true", true)
            'f' -> leerLiteral("false", false)
            'n' -> leerLiteral("null", null)
            '-', in '0'..'9' -> leerNumero()
            else -> error("Carácter no esperado '$c' en posición $pos.")
        }
    }

    private fun leerObjeto(): Map<String, Any?> {
        pos++ // '{'
        saltarEspacios()
        val mapa = linkedMapOf<String, Any?>()
        if (pos < entrada.length && entrada[pos] == '}') {
            pos++
            return mapa
        }
        while (true) {
            saltarEspacios()
            if (pos >= entrada.length || entrada[pos] != '"') {
                error("Se esperaba una clave en posición $pos.")
            }
            val clave = leerCadena()
            saltarEspacios()
            if (pos >= entrada.length || entrada[pos] != ':') {
                error("Se esperaba ':' en posición $pos.")
            }
            pos++
            mapa[clave] = leerSiguiente()
            saltarEspacios()
            if (pos >= entrada.length) error("Objeto JSON sin cerrar.")
            when (entrada[pos]) {
                ',' -> pos++
                '}' -> {
                    pos++
                    return mapa
                }
                else -> error("Se esperaba ',' o '}' en posición $pos.")
            }
        }
    }

    private fun leerArreglo(): List<Any?> {
        pos++ // '['
        saltarEspacios()
        val lista = mutableListOf<Any?>()
        if (pos < entrada.length && entrada[pos] == ']') {
            pos++
            return lista
        }
        while (true) {
            lista.add(leerSiguiente())
            saltarEspacios()
            if (pos >= entrada.length) error("Arreglo JSON sin cerrar.")
            when (entrada[pos]) {
                ',' -> pos++
                ']' -> {
                    pos++
                    return lista
                }
                else -> error("Se esperaba ',' o ']' en posición $pos.")
            }
        }
    }

    private fun leerCadena(): String {
        pos++ // '"'
        val sb = StringBuilder()
        while (pos < entrada.length) {
            val c = entrada[pos++]
            when (c) {
                '"' -> return sb.toString()
                '\\' -> {
                    if (pos >= entrada.length) error("Escape incompleto en cadena JSON.")
                    when (val esc = entrada[pos++]) {
                        '"', '\\', '/' -> sb.append(esc)
                        'b' -> sb.append('\b')
                        'f' -> sb.append('\u000C')
                        'n' -> sb.append('\n')
                        'r' -> sb.append('\r')
                        't' -> sb.append('\t')
                        'u' -> {
                            if (pos + 4 > entrada.length) error("Escape unicode incompleto.")
                            val hex = entrada.substring(pos, pos + 4)
                            sb.append(hex.toInt(16).toChar())
                            pos += 4
                        }
                        else -> error("Escape JSON desconocido '\\$esc'.")
                    }
                }
                else -> sb.append(c)
            }
        }
        error("Cadena JSON sin cerrar.")
    }

    private fun leerNumero(): Number {
        val inicio = pos
        if (entrada[pos] == '-') pos++
        while (pos < entrada.length && entrada[pos].isDigit()) pos++
        var esDecimal = false
        if (pos < entrada.length && entrada[pos] == '.') {
            esDecimal = true
            pos++
            while (pos < entrada.length && entrada[pos].isDigit()) pos++
        }
        if (pos < entrada.length && (entrada[pos] == 'e' || entrada[pos] == 'E')) {
            esDecimal = true
            pos++
            if (pos < entrada.length && (entrada[pos] == '+' || entrada[pos] == '-')) pos++
            while (pos < entrada.length && entrada[pos].isDigit()) pos++
        }
        val bruto = entrada.substring(inicio, pos)
        return if (esDecimal) bruto.toDouble() else bruto.toLong()
    }

    private fun <T> leerLiteral(literal: String, valor: T): T {
        if (!entrada.startsWith(literal, pos)) {
            error("Literal JSON no válido en posición $pos.")
        }
        pos += literal.length
        return valor
    }

    private fun saltarEspacios() {
        while (pos < entrada.length && entrada[pos].isWhitespace()) {
            pos++
        }
    }
}
