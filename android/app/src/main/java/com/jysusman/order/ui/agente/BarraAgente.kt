package com.jysusman.order.ui.agente

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.AttachFile
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.Image
import androidx.compose.material.icons.outlined.Mic
import androidx.compose.material.icons.outlined.PhotoCamera
import androidx.compose.material.icons.outlined.Stop
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.jysusman.order.agente.AdjuntoPendiente
import com.jysusman.order.ui.theme.OrderTema

@Composable
fun BarraAgente(
    texto: String,
    pensando: Boolean,
    transcribiendo: Boolean = false,
    leyendoArchivo: Boolean = false,
    grabando: Boolean = false,
    tiempoGrabacion: String = "",
    aviso: String,
    avisoEsError: Boolean = false,
    adjuntos: List<AdjuntoPendiente> = emptyList(),
    hayConversacion: Boolean = false,
    alCambiarTexto: (String) -> Unit,
    alGrabar: () -> Unit = {},
    alElegirArchivo: () -> Unit = {},
    alTomarFoto: () -> Unit = {},
    alQuitarAdjunto: (String) -> Unit = {},
    alNuevaConversacion: () -> Unit = {},
    alEnviar: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val ocupado = pensando || transcribiendo || leyendoArchivo
    val puedeEnviar = !ocupado && (texto.isNotBlank() || adjuntos.isNotEmpty())
    var menuAdjuntoAbierto by remember { mutableStateOf(false) }

    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(OrderTema.FondoPanel),
    ) {
        HorizontalDivider(color = OrderTema.Borde, thickness = OrderTema.Linea)
        if (hayConversacion) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(
                        start = OrderTema.EspacioMd,
                        end = OrderTema.EspacioMd,
                        top = OrderTema.EspacioXs,
                    ),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    text = "Conversación activa",
                    fontSize = OrderTema.TamanoHora,
                    color = OrderTema.TextoTenue,
                )
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                        .border(OrderTema.Linea, OrderTema.Borde, RoundedCornerShape(OrderTema.RadioPequeno))
                        .clickable(enabled = !ocupado, onClick = alNuevaConversacion)
                        .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioXs),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = "Nueva conversación",
                        fontSize = OrderTema.TamanoHora,
                        color = if (ocupado) OrderTema.TextoTenue else OrderTema.TextoSuave,
                    )
                }
            }
        }
        if (leyendoArchivo) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(
                        start = OrderTema.EspacioMd,
                        end = OrderTema.EspacioMd,
                        top = OrderTema.EspacioXs,
                    ),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
            ) {
                CircularProgressIndicator(
                    modifier = Modifier.size(12.dp),
                    strokeWidth = 1.5.dp,
                    color = OrderTema.Acento,
                )
                Text(
                    text = "leyendo archivo…",
                    fontSize = OrderTema.TamanoHora,
                    color = OrderTema.TextoSuave,
                )
            }
        } else if (aviso.isNotBlank()) {
            Text(
                text = aviso,
                fontSize = OrderTema.TamanoHora,
                color = if (avisoEsError) OrderTema.TextoError else OrderTema.TextoSuave,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(
                        start = OrderTema.EspacioMd,
                        end = OrderTema.EspacioMd,
                        top = OrderTema.EspacioXs,
                    ),
            )
        }

        if (adjuntos.isNotEmpty()) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .padding(
                        start = OrderTema.EspacioMd,
                        end = OrderTema.EspacioMd,
                        top = OrderTema.EspacioXs,
                    ),
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                adjuntos.forEach { adj ->
                    PildoraAdjunto(
                        adjunto = adj,
                        habilitado = !ocupado,
                        alQuitar = { alQuitarAdjunto(adj.id) },
                    )
                }
            }
        }

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = OrderTema.EspacioMd, vertical = OrderTema.EspacioSm),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
        ) {
            Text(
                text = "AGENTE",
                fontFamily = OrderTema.FuenteTitulos,
                fontSize = OrderTema.TamanoHora,
                letterSpacing = OrderTema.LetraSeccion,
                color = OrderTema.Acento,
            )

            Box {
                Box(
                    modifier = Modifier
                        .size(36.dp)
                        .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                        .background(OrderTema.FondoCampo)
                        .border(
                            OrderTema.Linea,
                            if (adjuntos.isNotEmpty()) OrderTema.AcentoSuave else OrderTema.BordeCampo,
                            RoundedCornerShape(OrderTema.RadioPequeno),
                        )
                        .clickable(enabled = !ocupado && !grabando) { menuAdjuntoAbierto = true },
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = Icons.Outlined.AttachFile,
                        contentDescription = "Adjuntar PDF o foto",
                        tint = if (adjuntos.isNotEmpty()) OrderTema.Acento else OrderTema.TextoSuave,
                        modifier = Modifier.size(18.dp),
                    )
                }

                DropdownMenu(
                    expanded = menuAdjuntoAbierto,
                    onDismissRequest = { menuAdjuntoAbierto = false },
                    containerColor = OrderTema.FondoPanelAlto,
                ) {
                    DropdownMenuItem(
                        text = {
                            Text(
                                text = "Archivo (PDF o imagen)",
                                fontSize = OrderTema.TamanoPequeno,
                                color = OrderTema.Texto,
                            )
                        },
                        leadingIcon = {
                            Icon(
                                imageVector = Icons.Outlined.Description,
                                contentDescription = null,
                                tint = OrderTema.Acento,
                            )
                        },
                        onClick = {
                            menuAdjuntoAbierto = false
                            alElegirArchivo()
                        },
                    )
                    DropdownMenuItem(
                        text = {
                            Text(
                                text = "Tomar foto",
                                fontSize = OrderTema.TamanoPequeno,
                                color = OrderTema.Texto,
                            )
                        },
                        leadingIcon = {
                            Icon(
                                imageVector = Icons.Outlined.PhotoCamera,
                                contentDescription = null,
                                tint = OrderTema.Acento,
                            )
                        },
                        onClick = {
                            menuAdjuntoAbierto = false
                            alTomarFoto()
                        },
                    )
                }
            }

            Box(
                modifier = Modifier
                    .weight(1f)
                    .height(36.dp)
                    .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                    .background(OrderTema.FondoCampo)
                    .border(OrderTema.Linea, OrderTema.BordeCampo, RoundedCornerShape(OrderTema.RadioPequeno))
                    .padding(horizontal = OrderTema.EspacioSm),
                contentAlignment = Alignment.CenterStart,
            ) {
                if (texto.isEmpty()) {
                    Text(
                        text = "Pídele algo al agente…",
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.TextoTenue,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                BasicTextField(
                    value = texto,
                    onValueChange = alCambiarTexto,
                    singleLine = true,
                    enabled = !ocupado,
                    textStyle = TextStyle(
                        fontFamily = OrderTema.FuenteTexto,
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.Texto,
                    ),
                    cursorBrush = SolidColor(OrderTema.Acento),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                    keyboardActions = KeyboardActions(onSend = { if (puedeEnviar) alEnviar() }),
                    modifier = Modifier.fillMaxWidth(),
                )
            }

            Box(
                modifier = Modifier
                    .size(36.dp)
                    .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                    .background(if (grabando) OrderTema.FondoError else OrderTema.FondoCampo)
                    .border(
                        OrderTema.Linea,
                        if (grabando) OrderTema.Vencida else OrderTema.BordeCampo,
                        RoundedCornerShape(OrderTema.RadioPequeno),
                    )
                    .clickable(enabled = !ocupado, onClick = alGrabar),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = if (grabando) Icons.Outlined.Stop else Icons.Outlined.Mic,
                    contentDescription = if (grabando) "Detener grabación" else "Grabar voz",
                    tint = if (grabando) OrderTema.VencidaTexto else OrderTema.TextoSuave,
                    modifier = Modifier.size(18.dp),
                )
            }

            if (grabando && tiempoGrabacion.isNotBlank()) {
                Text(
                    text = tiempoGrabacion,
                    fontSize = OrderTema.TamanoHora,
                    color = OrderTema.VencidaTexto,
                )
            }

            if (pensando) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
                ) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(14.dp),
                        strokeWidth = 1.5.dp,
                        color = OrderTema.Acento,
                    )
                    Text(
                        text = "pensando…",
                        fontSize = OrderTema.TamanoHora,
                        color = OrderTema.TextoTenue,
                    )
                }
            } else {
                Box(
                    modifier = Modifier
                        .height(36.dp)
                        .clip(RoundedCornerShape(OrderTema.Radio))
                        .background(OrderTema.AcentoTenue)
                        .border(OrderTema.Linea, OrderTema.AcentoSuave, RoundedCornerShape(OrderTema.Radio))
                        .clickable(enabled = puedeEnviar, onClick = alEnviar)
                        .padding(horizontal = OrderTema.EspacioMd),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = "Enviar",
                        fontFamily = OrderTema.FuenteTitulos,
                        fontSize = OrderTema.TamanoPequeno,
                        letterSpacing = OrderTema.LetraNormal,
                        color = if (puedeEnviar) OrderTema.Acento else OrderTema.TextoTenue,
                    )
                }
            }
        }
    }
}

@Composable
private fun PildoraAdjunto(
    adjunto: AdjuntoPendiente,
    habilitado: Boolean,
    alQuitar: () -> Unit,
) {
    Row(
        modifier = Modifier
            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
            .background(OrderTema.AcentoTenue)
            .border(OrderTema.Linea, OrderTema.AcentoSuave, RoundedCornerShape(OrderTema.RadioPequeno))
            .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioXs),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
    ) {
        Icon(
            imageVector = if (adjunto.esPdf) Icons.Outlined.Description else Icons.Outlined.Image,
            contentDescription = null,
            tint = OrderTema.Acento,
            modifier = Modifier.size(14.dp),
        )
        Text(
            text = adjunto.nombre,
            fontSize = OrderTema.TamanoHora,
            color = OrderTema.Texto,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.widthIn(max = 180.dp),
        )
        Icon(
            imageVector = Icons.Outlined.Close,
            contentDescription = "Quitar ${adjunto.nombre}",
            tint = OrderTema.TextoSuave,
            modifier = Modifier
                .size(14.dp)
                .clickable(enabled = habilitado, onClick = alQuitar),
        )
    }
}
