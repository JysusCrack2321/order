package com.jysusman.order.ui.dia

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.jysusman.order.ffi.ClaseInicioFfi
import com.jysusman.order.ffi.DiaVistaFfi
import com.jysusman.order.ui.comun.BotonBorde
import com.jysusman.order.ui.comun.BotonPaso
import com.jysusman.order.ui.comun.FilaRecordatorio
import com.jysusman.order.ui.comun.FilaTarea
import com.jysusman.order.ui.comun.InsigniaOrder
import com.jysusman.order.ui.comun.SeccionTitulo
import com.jysusman.order.ui.comun.TextoVacio
import com.jysusman.order.ui.theme.OrderTema

@Composable
fun DiaPantalla(
    vista: DiaVistaFfi?,
    onAnterior: () -> Unit,
    onSiguiente: () -> Unit,
    onIrHoy: () -> Unit,
    onNuevaClase: () -> Unit,
    onEditarClase: (String) -> Unit,
    onEditarTarea: (String) -> Unit,
    onEditarRecordatorio: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (vista == null) {
        Box(modifier = modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text(text = "Cargando…", color = OrderTema.TextoSuave)
        }
        return
    }

    val umbralPx = with(LocalDensity.current) { 56.dp.toPx() }
    var arrastreAcumulado by remember(vista.fechaIso) { mutableFloatStateOf(0f) }

    Column(
        modifier = modifier
            .fillMaxSize()
            .pointerInput(vista.fechaIso) {
                detectHorizontalDragGestures(
                    onDragStart = { arrastreAcumulado = 0f },
                    onHorizontalDrag = { _, dragAmount -> arrastreAcumulado += dragAmount },
                    onDragEnd = {
                        if (arrastreAcumulado > umbralPx) {
                            onAnterior()
                        } else if (arrastreAcumulado < -umbralPx) {
                            onSiguiente()
                        }
                        arrastreAcumulado = 0f
                    },
                )
            }
            .padding(horizontal = OrderTema.EspacioMd, vertical = OrderTema.EspacioMd),
        verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
    ) {
        // --- Navegación ----------------------------------------------------
        Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                BotonPaso(glifo = "‹", onClick = onAnterior)
                BotonPaso(glifo = "›", onClick = onSiguiente)
                Text(
                    text = vista.etiqueta,
                    fontFamily = OrderTema.FuenteTitulos,
                    fontWeight = FontWeight.Bold,
                    fontSize = OrderTema.TamanoSemana,
                    letterSpacing = OrderTema.LetraNormal,
                    color = OrderTema.Texto,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                if (vista.esHoy) {
                    InsigniaOrder("HOY")
                }
                Spacer(modifier = Modifier.weight(1f))
                BotonBorde(texto = "+  Clase", onClick = onNuevaClase)
                BotonBorde(texto = "Hoy", onClick = onIrHoy, acento = true)
            }
        }

        // --- Contenido -----------------------------------------------------
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = 84.dp),
            verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
        ) {
            item {
                val detalleClases = when (vista.clases.size) {
                    0 -> ""
                    1 -> "1 clase"
                    else -> "${vista.clases.size} clases"
                }
                SeccionTitulo(texto = "CLASES", detalle = detalleClases)
            }

            if (vista.clases.isEmpty()) {
                item { TextoVacio("sin clases este día") }
            } else {
                items(vista.clases, key = { it.id }) { clase ->
                    FilaTiempo(clase = clase, onClick = { onEditarClase(clase.id) })
                }
            }

            item {
                Spacer(modifier = Modifier.height(OrderTema.EspacioSm))
                SeccionTitulo(
                    texto = "TAREAS QUE VENCEN",
                    detalle = if (vista.tareas.isEmpty()) "" else "${vista.tareas.size}",
                )
            }

            if (vista.tareas.isEmpty()) {
                item { TextoVacio("ninguna tarea vence este día") }
            } else {
                items(vista.tareas, key = { it.id }) { tarea ->
                    FilaTarea(tarea = tarea, onClick = { onEditarTarea(tarea.id) })
                }
            }

            item {
                Spacer(modifier = Modifier.height(OrderTema.EspacioSm))
                SeccionTitulo(texto = "RECORDATORIOS")
            }

            if (vista.recordatorios.isEmpty()) {
                item { TextoVacio("sin recordatorios este día") }
            } else {
                items(vista.recordatorios, key = { it.id }) { rec ->
                    FilaRecordatorio(
                        recordatorio = rec,
                        onClick = { onEditarRecordatorio(rec.id) },
                    )
                }
            }
        }
    }
}

@Composable
private fun FilaTiempo(
    clase: ClaseInicioFfi,
    onClick: () -> Unit,
) {
    val color = OrderTema.colorMateria(clase.tono)
    val fondo = OrderTema.fondoMateria(clase.tono)

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(IntrinsicSize.Min)
            .clickable(onClick = onClick),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = clase.hora,
            fontSize = OrderTema.TamanoPequeno,
            color = OrderTema.TextoSuave,
            textAlign = TextAlign.End,
            modifier = Modifier
                .width(90.dp)
                .padding(end = OrderTema.EspacioSm),
        )
        Box(
            modifier = Modifier
                .width(16.dp)
                .fillMaxHeight(),
            contentAlignment = Alignment.Center,
        ) {
            Box(
                modifier = Modifier
                    .width(OrderTema.Linea)
                    .fillMaxHeight()
                    .background(OrderTema.Borde)
            )
            Box(
                modifier = Modifier
                    .size(9.dp)
                    .clip(CircleShape)
                    .background(color)
            )
        }
        Box(
            modifier = Modifier
                .weight(1f)
                .padding(vertical = OrderTema.EspacioXs)
                .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                .background(fondo)
                .border(OrderTema.Linea, OrderTema.BordeClase, RoundedCornerShape(OrderTema.RadioPequeno))
                .padding(horizontal = OrderTema.EspacioMd, vertical = OrderTema.EspacioMd),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                Text(
                    text = clase.materia,
                    fontSize = OrderTema.TamanoTexto,
                    fontWeight = FontWeight.SemiBold,
                    color = color,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                if (clase.lugar.isNotEmpty()) {
                    Text(
                        text = clase.lugar,
                        fontSize = OrderTema.TamanoHora,
                        color = OrderTema.TextoTenue,
                        maxLines = 1,
                    )
                }
            }
        }
    }
}
