package com.jysusman.order.actualizacion

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import com.jysusman.order.ui.theme.OrderTema
import kotlin.math.roundToInt

sealed interface EstadoActualizacion {
    data object Oculto : EstadoActualizacion
    data class Disponible(
        val release: ReleaseGitHub,
        val descargando: Boolean = false,
        val progreso: Float? = null,
        val avisoPermiso: String? = null,
        val error: String? = null,
    ) : EstadoActualizacion
}

@Composable
fun DialogoActualizacion(
    estado: EstadoActualizacion.Disponible,
    alActualizar: () -> Unit,
    alAhoraNo: () -> Unit,
) {
    val release = estado.release

    Dialog(onDismissRequest = alAhoraNo) {
        Surface(
            color = OrderTema.FondoPanel,
            shape = RoundedCornerShape(OrderTema.RadioTarjeta),
            border = BorderStroke(OrderTema.Linea, OrderTema.BordeFuerte),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(OrderTema.EspacioLg),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
            ) {
                Text(
                    text = "ACTUALIZACIÓN",
                    fontFamily = OrderTema.FuenteTitulos,
                    fontWeight = FontWeight.Bold,
                    fontSize = OrderTema.TamanoSeccion,
                    letterSpacing = OrderTema.LetraSeccion,
                    color = OrderTema.Acento,
                )

                Text(
                    text = "Hay una versión nueva (${release.versionLimpia}). ¿Actualizar?",
                    fontSize = OrderTema.TamanoTexto,
                    fontWeight = FontWeight.SemiBold,
                    color = OrderTema.Texto,
                )

                if (release.notas.isNotEmpty()) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(max = 180.dp)
                            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                            .background(OrderTema.FondoCampo)
                            .border(
                                OrderTema.Linea,
                                OrderTema.BordeCampo,
                                RoundedCornerShape(OrderTema.RadioPequeno),
                            )
                            .verticalScroll(rememberScrollState())
                            .padding(OrderTema.EspacioSm),
                    ) {
                        Text(
                            text = release.notas,
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.TextoSuave,
                        )
                    }
                }

                if (!estado.avisoPermiso.isNullOrBlank()) {
                    Text(
                        text = estado.avisoPermiso,
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.TextoAviso,
                    )
                }

                if (!estado.error.isNullOrBlank()) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                            .background(OrderTema.FondoError)
                            .border(
                                OrderTema.Linea,
                                OrderTema.BordeError,
                                RoundedCornerShape(OrderTema.RadioPequeno),
                            )
                            .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioXs),
                    ) {
                        Text(
                            text = estado.error,
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.TextoError,
                        )
                    }
                }

                if (estado.descargando) {
                    Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs)) {
                        val p = estado.progreso
                        if (p != null) {
                            LinearProgressIndicator(
                                progress = { p },
                                color = OrderTema.Acento,
                                trackColor = OrderTema.FondoCampo,
                                modifier = Modifier.fillMaxWidth(),
                            )
                            Text(
                                text = "Descargando… ${(p * 100f).roundToInt()}%",
                                fontSize = OrderTema.TamanoHora,
                                color = OrderTema.TextoSuave,
                            )
                        } else {
                            LinearProgressIndicator(
                                color = OrderTema.Acento,
                                trackColor = OrderTema.FondoCampo,
                                modifier = Modifier.fillMaxWidth(),
                            )
                            Text(
                                text = "Descargando…",
                                fontSize = OrderTema.TamanoHora,
                                color = OrderTema.TextoSuave,
                            )
                        }
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                ) {
                    Spacer(modifier = Modifier.weight(1f))
                    OutlinedButton(
                        onClick = alAhoraNo,
                        shape = RoundedCornerShape(OrderTema.Radio),
                        border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
                    ) {
                        Text(
                            text = if (estado.descargando) "Cancelar" else "Ahora no",
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.TextoSuave,
                        )
                    }
                    Button(
                        onClick = alActualizar,
                        enabled = !estado.descargando,
                        shape = RoundedCornerShape(OrderTema.Radio),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = OrderTema.AcentoTenue,
                            contentColor = OrderTema.Acento,
                            disabledContainerColor = OrderTema.FondoCampo,
                            disabledContentColor = OrderTema.TextoTenue,
                        ),
                        border = BorderStroke(
                            OrderTema.Linea,
                            if (estado.descargando) OrderTema.Borde else OrderTema.AcentoSuave,
                        ),
                    ) {
                        Text(
                            text = "Actualizar",
                            fontFamily = OrderTema.FuenteTitulos,
                            fontSize = OrderTema.TamanoPequeno,
                        )
                    }
                }
            }
        }
    }
}
