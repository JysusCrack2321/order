package com.jysusman.order.datos

import android.os.FileObserver
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.io.File

/**
 * Observa la carpeta de datos para recargar automáticamente cuando Syncthing
 * escribe o renombra `order.toml` o crea una copia en conflicto.
 */
class ObservadorCarpeta(
    carpeta: File,
    private val scope: CoroutineScope,
    private val alCambiar: suspend () -> Unit,
) : FileObserver(
    carpeta,
    CLOSE_WRITE or MOVED_TO or CREATE or DELETE,
) {
    private var trabajoDebounce: Job? = null

    override fun onEvent(event: Int, path: String?) {
        val nombre = path ?: return
        // Ignoramos temporales atómicos (`.order.toml.*.tmp`) y respaldos.
        if (nombre.startsWith(".") && nombre.endsWith(".tmp")) return
        if (nombre == "order.toml" || nombre.contains(".sync-conflict-")) {
            trabajoDebounce?.cancel()
            trabajoDebounce = scope.launch {
                delay(350L)
                alCambiar()
            }
        }
    }

    fun detener() {
        trabajoDebounce?.cancel()
        stopWatching()
    }
}
