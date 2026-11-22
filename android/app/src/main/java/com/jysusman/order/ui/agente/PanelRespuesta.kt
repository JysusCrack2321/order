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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import com.jysusman.order.ffi.AccionVistaFfi
import com.jysusman.order.ui.comun.BotonBorde
import com.jysusman.order.ui.theme.OrderTema

@Composable
fun PanelRespuesta(
    respuesta: String,
    nota: String,
    acciones: List<AccionVistaFfi>,
    alAplicar: (String) -> Unit,
    alDescartar: (String) -> Unit,
    alAplicarTodas: () -> Unit,
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
                    text = "RESPUESTA DEL AGENTE",
                    fontFamily = OrderTema.FuenteTitulos,
                    fontSize = OrderTema.TamanoHora,
                    letterSpacing = OrderTema.LetraSeccion,
                    color = OrderTema.TextoSuave,
                    modifier = Modifier.weight(1f),
                )
                if (acciones.size > 1) {
                    BotonBorde(
                        texto = "Aplicar todas",
                        acento = true,
                        onClick = alAplicarTodas,
                    )
                }
                BotonBorde(
                    texto = "Cerrar",
                    acento = false,
                    onClick = alCerrar,
                )
            }

            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(max = 220.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                if (respuesta.isNotBlank()) {
                    Text(
                        text = respuesta,
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.Texto,
                    )
                }

                if (nota.isNotBlank()) {
                    Text(
                        text = nota,
                        fontSize = OrderTema.TamanoHora,
                        color = OrderTema.TextoAviso,
                    )
                }

                acciones.forEach { accion ->
                    TarjetaAccion(
                        descripcion = accion.descripcion,
                        alAplicar = { alAplicar(accion.id) },
                        alDescartar = { alDescartar(accion.id) },
                    )
                }
            }
        }
    }
}

@Composable
private fun TarjetaAccion(
    descripcion: String,
    alAplicar: () -> Unit,
    alDescartar: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
            .background(OrderTema.FondoPanel)
            .border(OrderTema.Linea, OrderTema.Borde, RoundedCornerShape(OrderTema.RadioPequeno))
            .padding(OrderTema.EspacioSm),
        verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
    ) {
        Text(
            text = descripcion,
            fontSize = OrderTema.TamanoPequeno,
            color = OrderTema.Texto,
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.End,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                modifier = Modifier
                    .height(30.dp)
                    .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                    .border(OrderTema.Linea, OrderTema.Borde, RoundedCornerShape(OrderTema.RadioPequeno))
                    .clickable(onClick = alDescartar)
                    .padding(horizontal = OrderTema.EspacioMd),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = "Descartar",
                    fontSize = OrderTema.TamanoHora,
                    color = OrderTema.TextoSuave,
                )
            }
            Box(modifier = Modifier.padding(start = OrderTema.EspacioSm)) {
                Box(
                    modifier = Modifier
                        .height(30.dp)
                        .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                        .background(OrderTema.AcentoTenue)
                        .border(OrderTema.Linea, OrderTema.AcentoSuave, RoundedCornerShape(OrderTema.RadioPequeno))
                        .clickable(onClick = alAplicar)
                        .padding(horizontal = OrderTema.EspacioMd),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = "Aplicar",
                        fontFamily = OrderTema.FuenteTitulos,
                        fontSize = OrderTema.TamanoHora,
                        letterSpacing = OrderTema.LetraNormal,
                        color = OrderTema.Acento,
                    )
                }
            }
        }
    }
}
