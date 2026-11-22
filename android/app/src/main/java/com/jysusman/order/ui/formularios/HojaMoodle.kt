package com.jysusman.order.ui.formularios

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.jysusman.order.datos.Repositorio
import com.jysusman.order.ui.comun.BotonBorde
import com.jysusman.order.ui.theme.OrderTema
import kotlinx.coroutines.launch

@Composable
fun HojaMoodle(
    repositorio: Repositorio,
    alVolver: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val estado by repositorio.estado.collectAsStateWithLifecycle()
    val moodle = estado.moodleEstado

    var sitio by remember { mutableStateOf("micampus.unal.edu.co") }
    var usuario by remember { mutableStateOf("") }
    var contrasena by remember { mutableStateOf("") }
    var verContrasena by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        repositorio.cargarEstadoMoodle()
    }

    LaunchedEffect(moodle?.sitio) {
        val actual = moodle?.sitio.orEmpty()
        if (actual.isNotBlank()) {
            sitio = actual
        }
    }

    val conectado = moodle?.conectado == true
    val requiereReingreso = moodle?.requiereReingreso == true

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = OrderTema.EspacioLg, vertical = OrderTema.EspacioMd),
        verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(OrderTema.Espacio2xs)) {
            Text(
                text = "Moodle",
                fontFamily = OrderTema.FuenteTitulos,
                fontWeight = FontWeight.Bold,
                fontSize = OrderTema.TamanoSemana,
                letterSpacing = OrderTema.LetraNormal,
                color = OrderTema.Texto,
            )
            Text(
                text = "Sincroniza entregas automáticamente. Solo guarda el token.",
                fontSize = OrderTema.TamanoHora,
                color = OrderTema.TextoTenue,
            )
        }

        if (estado.moodleError.isNotEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                    .background(OrderTema.FondoError)
                    .border(OrderTema.Linea, OrderTema.BordeError, RoundedCornerShape(OrderTema.RadioPequeno))
                    .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioSm),
            ) {
                Text(
                    text = estado.moodleError,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoError,
                )
            }
        }

        if (moodle != null && moodle.conectado) {
            Text(
                text = moodle.textoEstado,
                fontSize = OrderTema.TamanoPequeno,
                color = if (requiereReingreso) OrderTema.TextoAviso else OrderTema.Texto,
            )
        }

        if (!conectado || requiereReingreso) {
            CampoMoodleTexto(
                etiqueta = "Sitio de Moodle",
                valor = sitio,
                alCambiar = { sitio = it },
                pista = "micampus.unal.edu.co",
                habilitado = !estado.moodleOcupado,
            )
            CampoMoodleTexto(
                etiqueta = "Usuario",
                valor = usuario,
                alCambiar = { usuario = it },
                pista = "usuario",
                habilitado = !estado.moodleOcupado,
            )
            Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs)) {
                Text(
                    text = "Contraseña",
                    fontSize = OrderTema.TamanoPequeno,
                    letterSpacing = OrderTema.LetraNormal,
                    color = OrderTema.TextoSuave,
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                ) {
                    OutlinedTextField(
                        value = contrasena,
                        onValueChange = { contrasena = it },
                        enabled = !estado.moodleOcupado,
                        placeholder = {
                            Text(
                                text = "••••••••",
                                fontSize = OrderTema.TamanoPequeno,
                                color = OrderTema.TextoTenue,
                            )
                        },
                        singleLine = true,
                        visualTransformation = if (verContrasena) {
                            VisualTransformation.None
                        } else {
                            PasswordVisualTransformation()
                        },
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedContainerColor = OrderTema.FondoCampo,
                            unfocusedContainerColor = OrderTema.FondoCampo,
                            focusedBorderColor = OrderTema.AcentoSuave,
                            unfocusedBorderColor = OrderTema.BordeCampo,
                            focusedTextColor = OrderTema.Texto,
                            unfocusedTextColor = OrderTema.Texto,
                            cursorColor = OrderTema.Acento,
                        ),
                        modifier = Modifier.weight(1f),
                    )
                    BotonBorde(
                        texto = if (verContrasena) "Ocultar" else "Mostrar",
                        onClick = { verContrasena = !verContrasena },
                    )
                }
            }
        }

        if (estado.moodleAviso.isNotEmpty()) {
            Text(
                text = estado.moodleAviso,
                fontSize = OrderTema.TamanoPequeno,
                color = OrderTema.TextoSuave,
            )
        }

        Spacer(modifier = Modifier.height(OrderTema.EspacioXs))

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
        ) {
            if (conectado) {
                OutlinedButton(
                    onClick = {
                        if (!estado.moodleOcupado) {
                            scope.launch { repositorio.desconectarMoodle() }
                        }
                    },
                    enabled = !estado.moodleOcupado,
                    shape = RoundedCornerShape(OrderTema.Radio),
                    border = BorderStroke(OrderTema.Linea, OrderTema.BordeError),
                ) {
                    Text(
                        text = "Desconectar",
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.VencidaTexto,
                    )
                }
            }
            Spacer(modifier = Modifier.weight(1f))
            OutlinedButton(
                onClick = alVolver,
                shape = RoundedCornerShape(OrderTema.Radio),
                border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
            ) {
                Text(
                    text = "Volver",
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoSuave,
                )
            }
            Button(
                onClick = {
                    if (estado.moodleOcupado) return@Button
                    scope.launch {
                        if (!conectado || requiereReingreso) {
                            val ok = repositorio.conectarMoodle(sitio, usuario, contrasena)
                            if (ok) {
                                contrasena = ""
                                verContrasena = false
                            }
                        } else {
                            repositorio.sincronizarMoodle(silencioso = false)
                        }
                    }
                },
                enabled = !estado.moodleOcupado,
                shape = RoundedCornerShape(OrderTema.Radio),
                colors = ButtonDefaults.buttonColors(
                    containerColor = OrderTema.AcentoTenue,
                    contentColor = OrderTema.Acento,
                ),
                border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
            ) {
                val textoBoton = if (!conectado || requiereReingreso) {
                    if (estado.moodleOcupado) "Conectando…" else "Conectar"
                } else {
                    if (estado.moodleOcupado) "Sincronizando…" else "Sincronizar ahora"
                }
                Text(
                    text = textoBoton,
                    fontFamily = OrderTema.FuenteTitulos,
                    fontSize = OrderTema.TamanoPequeno,
                )
            }
        }

        Spacer(modifier = Modifier.height(OrderTema.EspacioMd))
    }
}

@Composable
private fun CampoMoodleTexto(
    etiqueta: String,
    valor: String,
    alCambiar: (String) -> Unit,
    pista: String,
    habilitado: Boolean,
) {
    Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs)) {
        Text(
            text = etiqueta,
            fontSize = OrderTema.TamanoPequeno,
            letterSpacing = OrderTema.LetraNormal,
            color = OrderTema.TextoSuave,
        )
        OutlinedTextField(
            value = valor,
            onValueChange = alCambiar,
            enabled = habilitado,
            placeholder = {
                Text(
                    text = pista,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoTenue,
                )
            },
            singleLine = true,
            colors = OutlinedTextFieldDefaults.colors(
                focusedContainerColor = OrderTema.FondoCampo,
                unfocusedContainerColor = OrderTema.FondoCampo,
                focusedBorderColor = OrderTema.AcentoSuave,
                unfocusedBorderColor = OrderTema.BordeCampo,
                focusedTextColor = OrderTema.Texto,
                unfocusedTextColor = OrderTema.Texto,
                cursorColor = OrderTema.Acento,
            ),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}
