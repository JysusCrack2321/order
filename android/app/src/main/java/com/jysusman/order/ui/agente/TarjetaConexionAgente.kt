package com.jysusman.order.ui.agente

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import com.jysusman.order.ffi.TextosConexionAgenteFfi
import com.jysusman.order.ui.comun.BotonBorde
import com.jysusman.order.ui.theme.OrderTema

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TarjetaConexionAgente(
    textos: TextosConexionAgenteFfi,
    clave: String,
    verClave: Boolean,
    conectando: Boolean,
    estadoTexto: String,
    estadoError: Boolean,
    ofertaPortapapeles: String?,
    alCambiarClave: (String) -> Unit,
    alAlternarVerClave: () -> Unit,
    alAbrirStudio: () -> Unit,
    alPegarClave: () -> Unit,
    alAceptarOferta: () -> Unit,
    alConectar: () -> Unit,
    alOtroProveedor: () -> Unit,
    alCerrar: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(OrderTema.FondoPanelAlto),
    ) {
        HorizontalDivider(color = OrderTema.Borde, thickness = OrderTema.Linea)
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = OrderTema.EspacioMd, vertical = OrderTema.EspacioSm),
            verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                Text(
                    text = textos.titulo,
                    fontFamily = OrderTema.FuenteTitulos,
                    fontWeight = FontWeight.Bold,
                    fontSize = OrderTema.TamanoSeccion,
                    letterSpacing = OrderTema.LetraNormal,
                    color = OrderTema.Acento,
                    modifier = Modifier.weight(1f),
                )
                BotonBorde(
                    texto = "Cerrar",
                    acento = false,
                    onClick = alCerrar,
                )
            }

            Column(verticalArrangement = Arrangement.spacedBy(OrderTema.Espacio2xs)) {
                Text(
                    text = textos.paso1,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.Texto,
                )
                Text(
                    text = textos.paso2,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.Texto,
                )
                Text(
                    text = textos.paso3,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.Texto,
                )
            }

            if (!ofertaPortapapeles.isNullOrBlank()) {
                Surface(
                    color = OrderTema.AcentoTenue,
                    shape = RoundedCornerShape(OrderTema.RadioPequeno),
                    border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                        .clickable(onClick = alAceptarOferta),
                ) {
                    Text(
                        text = textos.ofertaPortapapeles,
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.Acento,
                        modifier = Modifier.padding(
                            horizontal = OrderTema.EspacioSm,
                            vertical = OrderTema.EspacioXs,
                        ),
                    )
                }
            }

            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
            ) {
                BotonBorde(
                    texto = textos.botonAbrirStudio,
                    acento = true,
                    onClick = alAbrirStudio,
                )
                BotonBorde(
                    texto = textos.botonPegar,
                    acento = false,
                    onClick = alPegarClave,
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                OutlinedTextField(
                    value = clave,
                    onValueChange = alCambiarClave,
                    placeholder = {
                        Text(
                            text = textos.pistaClave,
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.TextoTenue,
                        )
                    },
                    singleLine = true,
                    visualTransformation = if (verClave) VisualTransformation.None else PasswordVisualTransformation(),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedContainerColor = OrderTema.FondoCampo,
                        unfocusedContainerColor = OrderTema.FondoCampo,
                        focusedBorderColor = if (estadoError) OrderTema.Vencida else OrderTema.AcentoSuave,
                        unfocusedBorderColor = if (estadoError) OrderTema.Vencida else OrderTema.BordeCampo,
                        focusedTextColor = OrderTema.Texto,
                        unfocusedTextColor = OrderTema.Texto,
                        cursorColor = OrderTema.Acento,
                    ),
                    modifier = Modifier.weight(1f),
                )
                BotonBorde(
                    texto = if (verClave) "Ocultar" else "Mostrar",
                    onClick = alAlternarVerClave,
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                BotonBorde(
                    texto = if (conectando) textos.conectando else textos.botonConectar,
                    acento = true,
                    onClick = { if (!conectando) alConectar() },
                )
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                        .clickable(onClick = alOtroProveedor)
                        .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioXs),
                ) {
                    Text(
                        text = textos.enlaceOtroProveedor,
                        fontSize = OrderTema.TamanoHora,
                        color = OrderTema.TextoSuave,
                    )
                }
            }

            if (estadoTexto.isNotBlank()) {
                Text(
                    text = estadoTexto,
                    fontSize = OrderTema.TamanoHora,
                    color = if (estadoError) OrderTema.TextoError else OrderTema.Acento,
                )
            }
        }
    }
}
