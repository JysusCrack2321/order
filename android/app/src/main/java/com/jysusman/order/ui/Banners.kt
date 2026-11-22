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
    faltaPermisoNotificaciones: Boolean = false,
    onAbrirAjustesNotificaciones: () -> Unit = {},
    faltaPermisoAlarmaExacta: Boolean = false,
    onAbrirAjustesAlarmaExacta: () -> Unit = {},
    moodleRequiereReingreso: Boolean = false,
    onAbrirMoodle: () -> Unit = {},
    icalOferta: String? = null,
    onAceptarIcal: () -> Unit = {},
    onRechazarIcal: () -> Unit = {},
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
        if (faltaPermisoNotificaciones) {
            BannerAccionAviso(
                mensaje = "Los avisos están desactivados sin permiso de notificaciones.",
                textoBoton = "Activar notificaciones",
                onClick = onAbrirAjustesNotificaciones,
            )
        }
        if (faltaPermisoAlarmaExacta) {
            BannerAccionAviso(
                mensaje = "Sin permiso de alarmas exactas los avisos pueden llegar con retraso.",
                textoBoton = "Permitir alarmas",
                onClick = onAbrirAjustesAlarmaExacta,
            )
        }
        if (moodleRequiereReingreso) {
            BannerAccionAviso(
                mensaje = "Vuelve a iniciar sesión en Moodle.",
                textoBoton = "Moodle",
                onClick = onAbrirMoodle,
            )
        }
        if (!icalOferta.isNullOrBlank()) {
            BannerConflicto(
                mensaje = "¿Agregar este calendario? $icalOferta",
                textoPrincipal = "Agregar",
                textoSecundario = "No",
                onQuedarActual = onAceptarIcal,
                onUsarOtra = onRechazarIcal,
            )
        }
    }
}

@Composable
private fun BannerAccionAviso(
    mensaje: String,
    textoBoton: String,
    onClick: () -> Unit,
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
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = OrderTema.EspacioMd, vertical = OrderTema.EspacioSm),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
        ) {
            Text(
                text = mensaje,
                fontSize = OrderTema.TamanoPequeno,
                color = OrderTema.TextoAviso,
                modifier = Modifier.weight(1f),
            )
            OutlinedButton(
                onClick = onClick,
                shape = RoundedCornerShape(OrderTema.Radio),
                border = BorderStroke(OrderTema.Linea, OrderTema.BordeAviso),
            ) {
                Text(
                    text = textoBoton,
                    fontSize = OrderTema.TamanoHora,
                    color = OrderTema.TextoAviso,
                )
            }
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
    textoPrincipal: String = "Quedarme con la actual",
    textoSecundario: String = "Usar la otra",
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
                        text = textoPrincipal,
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
                        text = textoSecundario,
                        fontSize = OrderTema.TamanoHora,
                        color = OrderTema.TextoSuave,
                    )
                }
            }
        }
    }
}
