package com.jysusman.order.voz

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class VozAndroidTest {

    @Test
    fun combinaTextoRespetandoLoPrevio() {
        assertEquals("taller de control", VozAndroid.combinarTexto("", "taller de control"))
        assertEquals("Por favor taller de control", VozAndroid.combinarTexto("Por favor", "taller de control"))
        assertEquals("Por favor", VozAndroid.combinarTexto("Por favor", "   "))
        assertTrue(VozAndroid.MSG_NO_DISPONIBLE.contains("Gboard"))
        assertEquals("es-CO", VozAndroid.IDIOMA_DEFECTO)
    }
}
