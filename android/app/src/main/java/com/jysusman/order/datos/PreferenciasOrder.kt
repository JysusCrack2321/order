package com.jysusman.order.datos

import android.content.Context
import java.io.File

private const val PREFS_ORDER = "order_prefs"
private const val CLAVE_CARPETA = "carpeta_datos"
private const val CLAVE_AVISOS_ACTIVOS = "avisos_activos"
private const val CLAVE_AVISOS_PROGRAMADOS = "avisos_programados"
private const val CLAVE_VOZ_ANDROID = "voz_android"

object PreferenciasOrder {
    fun carpetaDatos(context: Context): String? =
        prefs(context).getString(CLAVE_CARPETA, null)

    fun guardarCarpetaDatos(context: Context, carpeta: String) {
        prefs(context).edit().putString(CLAVE_CARPETA, carpeta).apply()
    }

    fun avisosActivos(context: Context): Boolean =
        prefs(context).getBoolean(CLAVE_AVISOS_ACTIVOS, true)

    fun guardarAvisosActivos(context: Context, activos: Boolean) {
        prefs(context).edit().putBoolean(CLAVE_AVISOS_ACTIVOS, activos).apply()
    }

    fun clavesProgramadas(context: Context): Set<String> =
        prefs(context).getStringSet(CLAVE_AVISOS_PROGRAMADOS, emptySet())?.toSet() ?: emptySet()

    fun guardarClavesProgramadas(context: Context, claves: Set<String>) {
        prefs(context).edit().putStringSet(CLAVE_AVISOS_PROGRAMADOS, claves.toSet()).apply()
    }

    fun usarVozAndroid(context: Context): Boolean =
        prefs(context).getBoolean(CLAVE_VOZ_ANDROID, true)

    fun guardarUsarVozAndroid(context: Context, usar: Boolean) {
        prefs(context).edit().putBoolean(CLAVE_VOZ_ANDROID, usar).apply()
    }

    private fun prefs(context: Context) =
        context.getSharedPreferences(PREFS_ORDER, Context.MODE_PRIVATE)
}

object RutasPrivadas {
    fun archivoAgenteConfig(context: Context): String =
        File(context.filesDir, "agente.toml").absolutePath

    fun archivoMoodleConfig(context: Context): String =
        File(context.filesDir, "moodle.toml").absolutePath

    fun registroAvisos(context: Context): String =
        File(context.filesDir, "enviados.txt").absolutePath
}
