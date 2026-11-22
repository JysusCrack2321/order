package com.jysusman.order.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import com.jysusman.order.ffi.ConflictoFfi
import com.jysusman.order.ui.theme.OrderTema

@Composable
fun BannersEstado(
    error: String?,
    advertencias: List<String>,
    conflicto: ConflictoFfi?,
    onQuedarActual: () -> Unit,
    onUsarOtra: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier.fillMaxWidth()) {
        if (!error.isNullOrBlank()) {
            BannerTexto(
                titulo = error,
                lineas = emptyList(),
                fondo = OrderTema.FondoError,
                filo = OrderTema.BordeError,
                tinta = OrderTema.TextoError,
            )
        }
        if (advertencias.isNotEmpty()) {
            BannerTexto(
                titulo = "Claves que ORDER no conoce (se ignoran):",
                lineas = advertencias,
                fondo = OrderTema.FondoAviso,
                filo = OrderTema.BordeAviso,
                tinta = OrderTema.TextoAviso,
            )
        }
        if (conflicto != null) {
            BannerConflicto(
                mensaje = conflicto.mensaje,
                onQuedarActual = onQuedarActual,
                onUsarOtra = onUsarOtra,
            )
        }
    }
}

@Composable
private fun BannerTexto(
    titulo: String,
    lineas: List<String>,
    fondo: Color,
    filo: Color,
    tinta: Color,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(IntrinsicSize.Min)
            .background(fondo),
    ) {
        Box(
            modifier = Modifier
                .width(OrderTema.AnchoMarca)
                .fillMaxHeight()
                .background(filo)
        )
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = OrderTema.EspacioMd, vertical = OrderTema.EspacioSm),
            verticalArrangement = Arrangement.spacedBy(OrderTema.Espacio2xs),
        ) {
            Text(
                text = titulo,
                fontSize = OrderTema.TamanoPequeno,
                fontWeight = FontWeight.SemiBold,
                color = tinta,
            )
            lineas.forEach { linea ->
                Text(
                    text = linea,
                    fontSize = OrderTema.TamanoHora,
                    color = tinta,
                )
            }
        }
    }
}

@Composable
private fun BannerConflicto(
    mensaje: String,
    onQuedarActual: () -> Unit,
    onUsarOtra: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(IntrinsicSize.Min)
            .background(OrderTema.FondoAviso),
    ) {
        Box(
            modifier = Modifier
                .width(OrderTema.AnchoMarca)
                .fillMaxHeight()
                .background(OrderTema.BordeAviso)
        )
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = OrderTema.EspacioMd, vertical = OrderTema.EspacioSm),
            verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
        ) {
            Text(
                text = mensaje,
                fontSize = OrderTema.TamanoPequeno,
                fontWeight = FontWeight.SemiBold,
                color = OrderTema.TextoAviso,
            )
            Row(
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Button(
                    onClick = onQuedarActual,
                    shape = RoundedCornerShape(OrderTema.Radio),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = OrderTema.AcentoTenue,
                        contentColor = OrderTema.Acento,
                    ),
                    border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
                ) {
                    Text(
                        text = "Quedarme con la actual",
                        fontFamily = OrderTema.FuenteTitulos,
                        fontSize = OrderTema.TamanoHora,
                    )
                }
                OutlinedButton(
                    onClick = onUsarOtra,
                    shape = RoundedCornerShape(OrderTema.Radio),
                    border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
                ) {
                    Text(
                        text = "Usar la otra",
                        fontSize = OrderTema.TamanoHora,
                        color = OrderTema.TextoSuave,
                    )
                }
            }
        }
    }
}
