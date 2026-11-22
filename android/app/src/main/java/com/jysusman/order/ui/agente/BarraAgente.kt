package com.jysusman.order.ui.agente

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.jysusman.order.ui.theme.OrderTema

@Composable
fun BarraAgente(
    texto: String,
    pensando: Boolean,
    aviso: String,
    alCambiarTexto: (String) -> Unit,
    alEnviar: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(OrderTema.FondoPanel),
    ) {
        HorizontalDivider(color = OrderTema.Borde, thickness = OrderTema.Linea)
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
                        text = aviso.ifBlank { "Pídele algo al agente…" },
                        fontSize = OrderTema.TamanoPequeno,
                        color = if (aviso.isNotBlank()) OrderTema.TextoError else OrderTema.TextoTenue,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                BasicTextField(
                    value = texto,
                    onValueChange = alCambiarTexto,
                    singleLine = true,
                    enabled = !pensando,
                    textStyle = TextStyle(
                        fontFamily = OrderTema.FuenteTexto,
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.Texto,
                    ),
                    cursorBrush = SolidColor(OrderTema.Acento),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                    keyboardActions = KeyboardActions(onSend = { alEnviar() }),
                    modifier = Modifier.fillMaxWidth(),
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
                        .clickable(enabled = texto.isNotBlank(), onClick = alEnviar)
                        .padding(horizontal = OrderTema.EspacioMd),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = "Enviar",
                        fontFamily = OrderTema.FuenteTitulos,
                        fontSize = OrderTema.TamanoPequeno,
                        letterSpacing = OrderTema.LetraNormal,
                        color = if (texto.isNotBlank()) OrderTema.Acento else OrderTema.TextoTenue,
                    )
                }
            }
        }
    }
}
