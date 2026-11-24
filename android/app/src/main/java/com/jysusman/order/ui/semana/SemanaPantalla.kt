package com.jysusman.order.ui.semana

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.jysusman.order.ffi.BloqueSemanaFfi
import com.jysusman.order.ffi.DiaSemanaFfi
import com.jysusman.order.ffi.EventoFfi
import com.jysusman.order.ffi.SemanaVistaFfi
import com.jysusman.order.ui.comun.BotonBorde
import com.jysusman.order.ui.comun.BotonPaso
import com.jysusman.order.ui.comun.InsigniaOrder
import com.jysusman.order.ui.theme.OrderTema

private val AnchoGutter = 52.dp
private val AnchoColumnaDia = 136.dp
private val AltoCabeceraDia = 48.dp
private val AltoMarcas = 64.dp
private val AltoHora = OrderTema.AltoHora

@Composable
fun SemanaPantalla(
    vista: SemanaVistaFfi?,
    onAnterior: () -> Unit,
    onSiguiente: () -> Unit,
    onIrHoy: () -> Unit,
    onNuevaClase: () -> Unit,
    onEditarClase: (String) -> Unit,
    onNuevoEvento: () -> Unit,
    onEditarEvento: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (vista == null) {
        Box(modifier = modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text(text = "Cargando…", color = OrderTema.TextoSuave)
        }
        return
    }

    val horasTotales = (vista.finHora - vista.baseHora).coerceAtLeast(1)
    val altoRejilla = AltoHora * horasTotales

    Column(
        modifier = modifier
            .fillMaxSize()
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
                if (vista.esActual) {
                    InsigniaOrder("SEMANA ACTUAL")
                }
                Spacer(modifier = Modifier.weight(1f))
                BotonBorde(texto = "+ Evento", onClick = onNuevoEvento)
                BotonBorde(texto = "+ Clase", onClick = onNuevaClase)
                BotonBorde(texto = "Hoy", onClick = onIrHoy, acento = true)
            }
        }

        // --- Rejilla horaria con columna fija de horas y scroll horizontal ---
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState()),
        ) {
            Row(modifier = Modifier.fillMaxWidth()) {
                // Columna fija de horas
                Column(modifier = Modifier.width(AnchoGutter)) {
                    Spacer(modifier = Modifier.height(AltoCabeceraDia))
                    Box(modifier = Modifier.height(altoRejilla).fillMaxWidth()) {
                        vista.etiquetasHora.forEachIndexed { idx, texto ->
                            val y = AltoHora * idx - 7.dp
                            Text(
                                text = texto,
                                fontSize = OrderTema.TamanoHora,
                                color = OrderTema.TextoTenue,
                                textAlign = TextAlign.End,
                                modifier = Modifier
                                    .offset(y = y.coerceAtLeast(0.dp))
                                    .fillMaxWidth()
                                    .padding(end = OrderTema.EspacioSm),
                            )
                        }
                    }
                    if (vista.hayTareas) {
                        Spacer(modifier = Modifier.height(AltoMarcas))
                    }
                }

                // Días desplazables horizontalmente
                Row(
                    modifier = Modifier
                        .weight(1f)
                        .horizontalScroll(rememberScrollState()),
                ) {
                    vista.dias.forEach { dia ->
                        ColumnaDiaSemana(
                            dia = dia,
                            baseHora = vista.baseHora,
                            altoRejilla = altoRejilla,
                            etiquetasCount = vista.etiquetasHora.size,
                            mostrarAhora = vista.mostrarAhora,
                            ahoraMin = vista.ahoraMin,
                            hayTareas = vista.hayTareas,
                            onEditarClase = onEditarClase,
                            onEditarEvento = onEditarEvento,
                        )
                    }
                }
            }
            Spacer(modifier = Modifier.height(84.dp))
        }
    }
}

@Composable
private fun ColumnaDiaSemana(
    dia: DiaSemanaFfi,
    baseHora: Int,
    altoRejilla: Dp,
    etiquetasCount: Int,
    mostrarAhora: Boolean,
    ahoraMin: Int,
    hayTareas: Boolean,
    onEditarClase: (String) -> Unit,
    onEditarEvento: (String) -> Unit,
) {
    Row(
        modifier = Modifier
            .width(AnchoColumnaDia)
            .background(if (dia.esHoy) OrderTema.FondoHoy else Color.Transparent),
    ) {
        Box(
            modifier = Modifier
                .width(OrderTema.Linea)
                .height(AltoCabeceraDia + altoRejilla + if (hayTareas) AltoMarcas else 0.dp)
                .background(OrderTema.BordeSuave)
        )
        Column(modifier = Modifier.weight(1f)) {
            // Cabecera del día
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(AltoCabeceraDia),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                Text(
                    text = dia.nombre,
                    fontFamily = OrderTema.FuenteTitulos,
                    fontWeight = if (dia.esHoy) FontWeight.Bold else FontWeight.Normal,
                    fontSize = OrderTema.TamanoDia,
                    letterSpacing = OrderTema.LetraNormal,
                    color = if (dia.esHoy) OrderTema.Acento else OrderTema.TextoSuave,
                )
                Text(
                    text = dia.fechaCorta,
                    fontSize = OrderTema.TamanoHora,
                    color = if (dia.esHoy) OrderTema.AcentoSuave else OrderTema.TextoTenue,
                )
            }
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(OrderTema.Linea)
                    .background(OrderTema.BordeSuave)
            )

            // Cuerpo con líneas horarias y bloques
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(altoRejilla)
            ) {
                repeat(etiquetasCount) { i ->
                    Box(
                        modifier = Modifier
                            .offset(y = AltoHora * i)
                            .fillMaxWidth()
                            .height(OrderTema.Linea)
                            .background(OrderTema.LineaHora)
                    )
                }

                dia.bloques.forEach { bloque ->
                    BloqueClaseRejilla(
                        bloque = bloque,
                        baseHora = baseHora,
                        onClick = { onEditarClase(bloque.id) },
                    )
                }

                dia.eventos.forEach { evento ->
                    BloqueEventoRejilla(
                        evento = evento,
                        baseHora = baseHora,
                        onClick = { onEditarEvento(evento.id) },
                    )
                }

                if (mostrarAhora && dia.esHoy) {
                    val yAhora = AltoHora * ((ahoraMin - baseHora * 60) / 60f)
                    Box(
                        modifier = Modifier
                            .offset(y = yAhora)
                            .fillMaxWidth()
                            .height(OrderTema.Trazo)
                            .background(OrderTema.Acento)
                    )
                }
            }

            // Marcas de tareas que vencen ese día
            if (hayTareas) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(OrderTema.Linea)
                        .background(OrderTema.BordeSuave)
                )
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(AltoMarcas)
                        .padding(OrderTema.EspacioXs),
                    verticalArrangement = Arrangement.spacedBy(OrderTema.Espacio2xs),
                ) {
                    dia.tareas.forEach { tarea ->
                        val marca = if (tarea.hecha) "✓ " else "• "
                        Text(
                            text = "$marca${tarea.titulo}  ${tarea.hora}",
                            fontSize = OrderTema.TamanoHora,
                            color = if (tarea.hecha) OrderTema.TextoTenue else OrderTema.Acento,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    if (dia.extraTareas.isNotEmpty()) {
                        Text(
                            text = dia.extraTareas,
                            fontSize = OrderTema.TamanoHora,
                            color = OrderTema.TextoTenue,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun BloqueClaseRejilla(
    bloque: BloqueSemanaFfi,
    baseHora: Int,
    onClick: () -> Unit,
) {
    val inicioHoras = (bloque.inicioMin - baseHora * 60) / 60f
    val duracionHoras = ((bloque.finMin - bloque.inicioMin) / 60f).coerceAtLeast(0.4f)
    val yInicio = AltoHora * inicioHoras + OrderTema.Linea
    val alto = (AltoHora * duracionHoras - OrderTema.Trazo).coerceAtLeast(24.dp)
    val color = OrderTema.colorMateria(bloque.tono)
    val fondo = OrderTema.fondoMateria(bloque.tono)

    Row(
        modifier = Modifier
            .offset(y = yInicio)
            .padding(horizontal = OrderTema.EspacioXs)
            .fillMaxWidth()
            .height(alto)
            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
            .background(fondo)
            .border(OrderTema.Linea, OrderTema.BordeClase, RoundedCornerShape(OrderTema.RadioPequeno))
            .clickable(onClick = onClick),
    ) {
        Box(
            modifier = Modifier
                .width(OrderTema.AnchoMarca)
                .fillMaxHeight()
                .background(color)
        )
        Column(
            modifier = Modifier
                .weight(1f)
                .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioXs),
        ) {
            Text(
                text = bloque.materia,
                fontSize = OrderTema.TamanoBloque,
                fontWeight = FontWeight.SemiBold,
                color = color,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = bloque.hora,
                fontSize = OrderTema.TamanoHora,
                color = OrderTema.TextoSuave,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (bloque.lugar.isNotEmpty() && alto >= 44.dp) {
                Text(
                    text = bloque.lugar,
                    fontSize = OrderTema.TamanoHora,
                    color = OrderTema.TextoTenue,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

@Composable
private fun BloqueEventoRejilla(
    evento: EventoFfi,
    baseHora: Int,
    onClick: () -> Unit,
) {
    val inicioHoras = (evento.inicioMin - baseHora * 60) / 60f
    val duracionHoras = ((evento.finMin - evento.inicioMin) / 60f).coerceAtLeast(0.4f)
    val yInicio = AltoHora * inicioHoras + OrderTema.Linea
    val alto = (AltoHora * duracionHoras - OrderTema.Trazo).coerceAtLeast(24.dp)
    val color = if (evento.tareaId.isNotBlank()) OrderTema.colorMateria(evento.tono) else OrderTema.Acento
    val fondo = if (evento.tareaHecha) OrderTema.FondoCampo else OrderTema.Tarjeta
    val borde = if (evento.tareaHecha) OrderTema.BordeSuave else color.copy(alpha = 0.6f)

    Column(
        modifier = Modifier
            .offset(y = yInicio)
            .padding(horizontal = OrderTema.EspacioXs)
            .fillMaxWidth()
            .height(alto)
            .clip(RoundedCornerShape(OrderTema.RadioPequeno))
            .background(fondo)
            .border(OrderTema.Linea, borde, RoundedCornerShape(OrderTema.RadioPequeno))
            .clickable(onClick = onClick)
            .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioXs),
    ) {
        Text(
            text = "◷ ${evento.titulo}",
            fontSize = OrderTema.TamanoBloque,
            fontWeight = FontWeight.Medium,
            color = if (evento.tareaHecha) OrderTema.TextoTenue else color,
            textDecoration = if (evento.tareaHecha) TextDecoration.LineThrough else TextDecoration.None,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (alto >= 36.dp) {
            Text(
                text = evento.rangoHora,
                fontSize = OrderTema.TamanoHora,
                color = OrderTema.TextoSuave,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}
