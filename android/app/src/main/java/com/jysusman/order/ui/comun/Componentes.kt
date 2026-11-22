package com.jysusman.order.ui.comun

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.jysusman.order.ffi.ClaseInicioFfi
import com.jysusman.order.ffi.RecordatorioInicioFfi
import com.jysusman.order.ffi.TareaFfi
import com.jysusman.order.ui.theme.OrderTema

@Composable
fun TarjetaSeccion(
    titulo: String,
    detalle: String = "",
    tono: Color = OrderTema.Acento,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    Surface(
        color = OrderTema.Tarjeta,
        shape = RoundedCornerShape(OrderTema.RadioTarjeta),
        border = BorderStroke(OrderTema.Linea, OrderTema.TarjetaBorde),
        modifier = modifier.fillMaxWidth(),
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(OrderTema.Trazo)
                    .background(tono)
            )
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(OrderTema.RellenoTarjeta),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.Bottom,
                ) {
                    Text(
                        text = titulo,
                        fontFamily = OrderTema.FuenteTitulos,
                        fontWeight = FontWeight.Bold,
                        fontSize = OrderTema.TamanoSeccion,
                        letterSpacing = OrderTema.LetraSeccion,
                        color = tono,
                        modifier = Modifier.weight(1f),
                    )
                    if (detalle.isNotEmpty()) {
                        Text(
                            text = detalle,
                            fontSize = OrderTema.TamanoHora,
                            color = OrderTema.TextoTenue,
                        )
                    }
                }
                content()
            }
        }
    }
}

@Composable
fun SeccionTitulo(
    texto: String,
    detalle: String = "",
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.Bottom,
        horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
    ) {
        Text(
            text = texto,
            fontFamily = OrderTema.FuenteTitulos,
            fontWeight = FontWeight.Bold,
            fontSize = OrderTema.TamanoSeccion,
            letterSpacing = OrderTema.LetraSeccion,
            color = OrderTema.TextoSuave,
        )
        if (detalle.isNotEmpty()) {
            Text(
                text = detalle,
                fontSize = OrderTema.TamanoHora,
                color = OrderTema.TextoTenue,
            )
        }
    }
}

@Composable
fun TextoVacio(
    texto: String,
    modifier: Modifier = Modifier,
) {
    Text(
        text = texto,
        fontSize = OrderTema.TamanoPequeno,
        color = OrderTema.TextoTenue,
        modifier = modifier.padding(vertical = OrderTema.EspacioXs),
    )
}

@Composable
fun PildoraMateria(
    texto: String,
    tono: Int,
    modifier: Modifier = Modifier,
) {
    val tinta = OrderTema.colorMateria(tono)
    val fondo = OrderTema.fondoMateria(tono)
    Box(
        modifier = modifier
            .height(OrderTema.AltoPildora)
            .widthIn(max = 200.dp)
            .clip(RoundedCornerShape(OrderTema.AltoPildora / 2))
            .background(fondo)
            .border(OrderTema.Linea, tinta, RoundedCornerShape(OrderTema.AltoPildora / 2))
            .padding(horizontal = OrderTema.EspacioSm),
        contentAlignment = Alignment.CenterStart,
    ) {
        Text(
            text = texto,
            fontSize = OrderTema.TamanoHora,
            color = tinta,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
fun CasillaOrder(
    marcada: Boolean,
    onToggle: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .size(OrderTema.AltoCheck)
            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
            .background(if (marcada) OrderTema.AcentoTenue else OrderTema.FondoCampo)
            .border(
                width = OrderTema.Linea,
                color = if (marcada) OrderTema.Acento else OrderTema.BordeFuerte,
                shape = RoundedCornerShape(OrderTema.RadioPequeno),
            )
            .clickable(onClick = onToggle),
        contentAlignment = Alignment.Center,
    ) {
        if (marcada) {
            Text(
                text = "✓",
                fontSize = OrderTema.TamanoPequeno,
                color = OrderTema.Acento,
            )
        }
    }
}

@Composable
fun FilaClase(
    clase: ClaseInicioFfi,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val color = OrderTema.colorMateria(clase.tono)
    Row(
        modifier = modifier
            .fillMaxWidth()
            .height(IntrinsicSize.Min)
            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
            .clickable(onClick = onClick)
            .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioSm),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
    ) {
        Box(
            modifier = Modifier
                .width(OrderTema.AnchoMarca)
                .fillMaxHeight()
                .background(color)
        )
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = clase.materia,
                fontSize = OrderTema.TamanoTexto,
                fontWeight = FontWeight.SemiBold,
                color = color,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = clase.hora,
                fontSize = OrderTema.TamanoPequeno,
                color = OrderTema.TextoSuave,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        if (clase.lugar.isNotEmpty()) {
            Text(
                text = clase.lugar,
                fontSize = OrderTema.TamanoHora,
                color = OrderTema.TextoTenue,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
fun FilaTarea(
    tarea: TareaFfi,
    onClick: () -> Unit,
    onToggleHecha: ((Boolean) -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    val colorBarra = when {
        tarea.hecha -> OrderTema.BordeSuave
        tarea.vencida -> OrderTema.Vencida
        else -> OrderTema.AcentoSuave
    }
    val colorTitulo = when {
        tarea.hecha -> OrderTema.TextoTenue
        tarea.vencida -> OrderTema.VencidaTexto
        else -> OrderTema.Texto
    }
    val colorFecha = if (tarea.vencida && !tarea.hecha) OrderTema.Vencida else OrderTema.TextoTenue

    Row(
        modifier = modifier
            .fillMaxWidth()
            .height(IntrinsicSize.Min)
            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
            .clickable(onClick = onClick)
            .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioSm),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
    ) {
        Box(
            modifier = Modifier
                .width(OrderTema.AnchoMarca)
                .fillMaxHeight()
                .background(colorBarra)
        )
        if (onToggleHecha != null) {
            CasillaOrder(
                marcada = tarea.hecha,
                onToggle = { onToggleHecha(!tarea.hecha) },
            )
        }
        Column(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(OrderTema.Espacio2xs),
        ) {
            Text(
                text = tarea.titulo,
                fontSize = OrderTema.TamanoTexto,
                color = colorTitulo,
                textDecoration = if (tarea.hecha) TextDecoration.LineThrough else TextDecoration.None,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                if (tarea.materia.isNotEmpty()) {
                    PildoraMateria(texto = tarea.materia, tono = tarea.tono)
                }
                Text(
                    text = tarea.fechaTexto,
                    fontSize = OrderTema.TamanoHora,
                    color = colorFecha,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        if (tarea.vencida && !tarea.hecha) {
            Text(
                text = "VENCIDA",
                fontFamily = OrderTema.FuenteTitulos,
                fontSize = OrderTema.TamanoHora,
                letterSpacing = OrderTema.LetraNormal,
                color = OrderTema.Vencida,
            )
        } else if (tarea.hecha) {
            Text(
                text = "HECHA",
                fontFamily = OrderTema.FuenteTitulos,
                fontSize = OrderTema.TamanoHora,
                letterSpacing = OrderTema.LetraNormal,
                color = OrderTema.TextoTenue,
            )
        }
    }
}

@Composable
fun FilaRecordatorio(
    recordatorio: RecordatorioInicioFfi,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
            .clickable(onClick = onClick)
            .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioSm),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
    ) {
        Text(
            text = recordatorio.texto,
            fontSize = OrderTema.TamanoPequeno,
            color = OrderTema.Texto,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        Text(
            text = recordatorio.cuando,
            fontSize = OrderTema.TamanoHora,
            color = OrderTema.TextoSuave,
            maxLines = 1,
        )
    }
}

@Composable
fun BotonPaso(
    glifo: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .size(34.dp)
            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
            .border(OrderTema.Linea, OrderTema.Borde, RoundedCornerShape(OrderTema.RadioPequeno))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = glifo,
            fontFamily = OrderTema.FuenteTitulos,
            fontSize = OrderTema.TamanoPestana,
            color = OrderTema.TextoSuave,
        )
    }
}

@Composable
fun BotonBorde(
    texto: String,
    onClick: () -> Unit,
    acento: Boolean = false,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .height(34.dp)
            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
            .background(if (acento) OrderTema.AcentoTenue else Color.Transparent)
            .border(
                OrderTema.Linea,
                if (acento) OrderTema.AcentoSuave else OrderTema.Borde,
                RoundedCornerShape(OrderTema.RadioPequeno),
            )
            .clickable(onClick = onClick)
            .padding(horizontal = OrderTema.EspacioMd),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = texto,
            fontFamily = OrderTema.FuenteTitulos,
            fontSize = OrderTema.TamanoPequeno,
            letterSpacing = OrderTema.LetraNormal,
            color = if (acento) OrderTema.Acento else OrderTema.TextoSuave,
        )
    }
}

@Composable
fun ChipOrder(
    texto: String,
    activo: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .height(30.dp)
            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
            .background(if (activo) OrderTema.AcentoTenue else OrderTema.FondoCampo)
            .border(
                OrderTema.Linea,
                if (activo) OrderTema.Acento else OrderTema.Borde,
                RoundedCornerShape(OrderTema.RadioPequeno),
            )
            .clickable(onClick = onClick)
            .padding(horizontal = OrderTema.EspacioSm),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = texto,
            fontFamily = OrderTema.FuenteTitulos,
            fontSize = OrderTema.TamanoHora,
            letterSpacing = OrderTema.LetraNormal,
            color = if (activo) OrderTema.Acento else OrderTema.TextoSuave,
            maxLines = 1,
        )
    }
}

@Composable
fun InsigniaOrder(
    texto: String,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
            .background(OrderTema.AcentoTenue)
            .border(OrderTema.Linea, OrderTema.AcentoSuave, RoundedCornerShape(OrderTema.RadioPequeno))
            .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.Espacio2xs),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = texto,
            fontFamily = OrderTema.FuenteTitulos,
            fontSize = OrderTema.TamanoHora,
            letterSpacing = OrderTema.LetraNormal,
            color = OrderTema.Acento,
        )
    }
}
