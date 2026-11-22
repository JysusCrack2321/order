package com.jysusman.order.voz

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import java.util.Locale

class VozAndroid(private val context: Context) {
    private var recognizer: SpeechRecognizer? = null
    private var escuchando = false
    private var prefijoTexto = ""
    private var idiomaActual = IDIOMA_DEFECTO
    private var yaReintentoIdioma = false

    fun estaEscuchando(): Boolean = escuchando

    fun empezar(
        textoActual: String,
        alActualizarTexto: (String) -> Unit,
        alTerminar: () -> Unit,
        alError: (String) -> Unit,
    ) {
        if (escuchando) return
        if (!SpeechRecognizer.isRecognitionAvailable(context)) {
            alError(MSG_NO_DISPONIBLE)
            return
        }

        prefijoTexto = textoActual.trim()
        idiomaActual = IDIOMA_DEFECTO
        yaReintentoIdioma = false
        iniciarConIdioma(idiomaActual, alActualizarTexto, alTerminar, alError)
    }

    private fun iniciarConIdioma(
        idioma: String,
        alActualizarTexto: (String) -> Unit,
        alTerminar: () -> Unit,
        alError: (String) -> Unit,
    ) {
        destruirRecognizer()
        val sr = runCatching { SpeechRecognizer.createSpeechRecognizer(context) }.getOrNull()
        if (sr == null) {
            escuchando = false
            alError(MSG_NO_DISPONIBLE)
            return
        }

        sr.setRecognitionListener(object : RecognitionListener {
            override fun onReadyForSpeech(params: Bundle?) {}
            override fun onBeginningOfSpeech() {}
            override fun onRmsChanged(rmsdB: Float) {}
            override fun onBufferReceived(buffer: ByteArray?) {}
            override fun onEndOfSpeech() {}

            override fun onPartialResults(partialResults: Bundle?) {
                val texto = extraerTexto(partialResults) ?: return
                alActualizarTexto(combinarTexto(prefijoTexto, texto))
            }

            override fun onResults(results: Bundle?) {
                val texto = extraerTexto(results)
                if (texto != null) {
                    alActualizarTexto(combinarTexto(prefijoTexto, texto))
                }
                escuchando = false
                destruirRecognizer()
                alTerminar()
            }

            override fun onError(error: Int) {
                val esErrorIdioma = error == SpeechRecognizer.ERROR_LANGUAGE_NOT_SUPPORTED ||
                    error == SpeechRecognizer.ERROR_LANGUAGE_UNAVAILABLE
                if (esErrorIdioma && !yaReintentoIdioma) {
                    val sistema = Locale.getDefault().toLanguageTag()
                    if (sistema.isNotBlank() && sistema != idiomaActual) {
                        yaReintentoIdioma = true
                        idiomaActual = sistema
                        iniciarConIdioma(sistema, alActualizarTexto, alTerminar, alError)
                        return
                    }
                }
                val estabaEscuchando = escuchando
                escuchando = false
                destruirRecognizer()
                alTerminar()
                if (estabaEscuchando || error != SpeechRecognizer.ERROR_CLIENT) {
                    alError(mensajeError(error))
                }
            }

            override fun onEvent(eventType: Int, params: Bundle?) {}
        })

        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            putExtra(RecognizerIntent.EXTRA_LANGUAGE, idioma)
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_PREFERENCE, idioma)
            putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
            putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
        }

        recognizer = sr
        escuchando = true
        runCatching { sr.startListening(intent) }.onFailure {
            escuchando = false
            destruirRecognizer()
            alTerminar()
            alError(MSG_NO_DISPONIBLE)
        }
    }

    fun detener() {
        if (!escuchando) return
        escuchando = false
        runCatching { recognizer?.stopListening() }
    }

    fun cancelar() {
        escuchando = false
        destruirRecognizer()
    }

    private fun destruirRecognizer() {
        val sr = recognizer
        recognizer = null
        runCatching { sr?.cancel() }
        runCatching { sr?.destroy() }
    }

    companion object {
        const val IDIOMA_DEFECTO = "es-CO"
        const val MSG_NO_DISPONIBLE =
            "Reconocimiento de voz no disponible; puedes usar el micrófono de Gboard."

        fun combinarTexto(prefijo: String, reconocido: String): String {
            val limpio = reconocido.trim()
            if (limpio.isEmpty()) return prefijo
            return if (prefijo.isBlank()) limpio else "${prefijo.trim()} $limpio"
        }

        private fun extraerTexto(bundle: Bundle?): String? =
            bundle?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                ?.firstOrNull()
                ?.trim()
                ?.takeIf { it.isNotEmpty() }

        private fun mensajeError(error: Int): String = when (error) {
            SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS ->
                "Permiso de micrófono denegado; puedes usar el micrófono de Gboard."
            SpeechRecognizer.ERROR_NO_MATCH,
            SpeechRecognizer.ERROR_SPEECH_TIMEOUT ->
                "No se detectó voz; intenta de nuevo o usa el micrófono de Gboard."
            SpeechRecognizer.ERROR_RECOGNIZER_BUSY ->
                "El micrófono está ocupado; intenta en un momento."
            SpeechRecognizer.ERROR_NETWORK,
            SpeechRecognizer.ERROR_NETWORK_TIMEOUT ->
                "Sin conexión para dictado; intenta con el micrófono de Gboard."
            SpeechRecognizer.ERROR_AUDIO ->
                "No se pudo usar el micrófono; puedes usar el micrófono de Gboard."
            else ->
                "No se pudo reconocer la voz; puedes usar el micrófono de Gboard."
        }
    }
}
