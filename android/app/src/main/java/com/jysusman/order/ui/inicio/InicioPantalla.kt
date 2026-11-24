package com.jysusman.order.ui.inicio

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.jysusman.order.ffi.ClaseInicioFfi
import com.jysusman.order.ffi.InicioVistaFfi
import com.jysusman.order.ui.comun.FilaClase
import com.jysusman.order.ui.comun.FilaEvento
import com.jysusman.order.ui.comun.FilaPorReagendar
import com.jysusman.order.ui.comun.FilaRecordatorio
import com.jysusman.order.ui.comun.FilaTarea
import com.jysusman.order.ui.comun.PildoraMateria
import com.jysusman.order.ui.comun.TarjetaSeccion
import com.jysusman.order.ui.comun.TextoVacio
import com.jysusman.order.ui.theme.OrderTema

@Composable
fun InicioPantalla(
    vista: InicioVistaFfi?,
    onEditarClase: (String) -> Unit,
    onEditarEvento: (String) -> Unit,
    onHechaEvento: (String) -> Unit,
    onReagendarEvento: (String) -> Unit,
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

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(
            start = OrderTema.EspacioMd,
            end = OrderTema.EspacioMd,
            top = OrderTema.EspacioMd,
            bottom = 84.dp,
        ),
        verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
    ) {
        item {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(OrderTema.RadioTarjeta))
                    .background(OrderTema.DegradadoInicio)
                    .padding(horizontal = OrderTema.EspacioLg, vertical = OrderTema.EspacioLg),
            ) {
                Column(verticalArrangement = Arrangement.spacedBy(OrderTema.Espacio2xs)) {
                    Text(
                        text = "INICIO",
                        fontFamily = OrderTema.FuenteTitulos,
                        fontWeight = FontWeight.Bold,
                        fontSize = OrderTema.TamanoSeccion,
                        letterSpacing = OrderTema.LetraTitulo,
                        color = OrderTema.Acento,
                    )
                    Text(
                        text = vista.fechaLarga,
                        fontFamily = OrderTema.FuenteTitulos,
                        fontWeight = FontWeight.Bold,
                        fontSize = OrderTema.TamanoSemana,
                        letterSpacing = OrderTema.LetraNormal,
                        color = OrderTema.Texto,
                    )
                }
            }
        }

        if (vista.porReagendar.isNotEmpty()) {
            item {
                TarjetaSeccion(
                    titulo = "¿TERMINASTE?",
                    detalle = "${vista.porReagendar.size}",
                    tono = OrderTema.Acento,
                ) {
                    vista.porReagendar.forEach { evento ->
                        FilaPorReagendar(
                            evento = evento,
                            onHecha = onHechaEvento,
                            onReagendar = onReagendarEvento,
                            onEditar = onEditarEvento,
                        )
                    }
                }
            }
        }

        item {
            val partes = buildList {
                when (vista.clases.size) {
                    0 -> {}
                    1 -> add("1 clase")
                    else -> add("${vista.clases.size} clases")
                }
                when (vista.eventos.size) {
                    0 -> {}
                    1 -> add("1 evento")
                    else -> add("${vista.eventos.size} eventos")
                }
            }
            val detalleHoy = partes.joinToString(" · ")
            TarjetaSeccion(
                titulo = "HOY",
                detalle = detalleHoy,
                tono = OrderTema.AcentoHoy,
            ) {
                if (vista.clases.isEmpty() && vista.eventos.isEmpty()) {
                    TextoVacio("sin clases ni eventos hoy")
                } else {
                    vista.clases.forEach { clase ->
                        FilaClase(clase = clase, onClick = { onEditarClase(clase.id) })
                    }
                    vista.eventos.forEach { evento ->
                        FilaEvento(evento = evento, onClick = { onEditarEvento(evento.id) })
                    }
                }
                vista.siguiente?.let { siguiente ->
                    TarjetaSiguienteClase(
                        clase = siguiente,
                        onClick = { onEditarClase(siguiente.id) },
                    )
                }
            }
        }

        item {
            TarjetaSeccion(
                titulo = "TAREAS DE HOY",
                detalle = if (vista.deHoy.isEmpty()) "" else "${vista.deHoy.size}",
                tono = OrderTema.AcentoHoyTareas,
            ) {
                if (vista.deHoy.isEmpty()) {
                    TextoVacio("nada vence hoy")
                } else {
                    vista.deHoy.forEach { tarea ->
                        FilaTarea(tarea = tarea, onClick = { onEditarTarea(tarea.id) })
                    }
                }
            }
        }

        item {
            TarjetaSeccion(
                titulo = "VENCIDAS",
                detalle = if (vista.vencidas.isEmpty()) "" else "${vista.vencidas.size}",
                tono = OrderTema.AcentoVencidas,
            ) {
                if (vista.vencidas.isEmpty()) {
                    TextoVacio("nada atrasado")
                } else {
                    vista.vencidas.forEach { tarea ->
                        FilaTarea(tarea = tarea, onClick = { onEditarTarea(tarea.id) })
                    }
                }
            }
        }

        item {
            TarjetaSeccion(
                titulo = "PRÓXIMOS 7 DÍAS",
                detalle = if (vista.proximas.isEmpty()) "" else "${vista.proximas.size}",
                tono = OrderTema.AcentoProximas,
            ) {
                if (vista.proximas.isEmpty()) {
                    TextoVacio("sin entregas a la vista")
                } else {
                    vista.proximas.forEach { tarea ->
                        FilaTarea(tarea = tarea, onClick = { onEditarTarea(tarea.id) })
                    }
                }
            }
        }

        item {
            TarjetaSeccion(
                titulo = "PRÓXIMOS RECORDATORIOS",
                tono = OrderTema.AcentoRecordatorios,
            ) {
                if (vista.recordatorios.isEmpty()) {
                    TextoVacio("sin recordatorios")
                } else {
                    vista.recordatorios.forEach { rec ->
                        FilaRecordatorio(
                            recordatorio = rec,
                            onClick = { onEditarRecordatorio(rec.id) },
                        )
                    }
                }
            }
        }

        item {
            TarjetaSeccion(
                titulo = "PENDIENTES POR MATERIA",
                tono = OrderTema.AcentoMaterias,
            ) {
                if (vista.porMateria.isEmpty()) {
                    TextoVacio("nada pendiente")
                } else {
                    vista.porMateria.forEach { item ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(vertical = OrderTema.EspacioXs),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            PildoraMateria(texto = item.materia, tono = item.tono)
                            Spacer(modifier = Modifier.weight(1f))
                            Text(
                                text = "${item.cuantas}",
                                fontFamily = OrderTema.FuenteTitulos,
                                fontSize = OrderTema.TamanoPequeno,
                                color = OrderTema.colorMateria(item.tono),
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun TarjetaSiguienteClase(
    clase: ClaseInicioFfi,
    onClick: () -> Unit,
) {
    Surface(
        color = OrderTema.AcentoTenue,
        shape = RoundedCornerShape(OrderTema.RadioPequeno),
        border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = OrderTema.EspacioMd, vertical = OrderTema.EspacioSm),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
        ) {
            Text(
                text = "SIGUIENTE",
                fontFamily = OrderTema.FuenteTitulos,
                fontSize = OrderTema.TamanoHora,
                letterSpacing = OrderTema.LetraNormal,
                color = OrderTema.Acento,
            )
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = clase.materia,
                    fontSize = OrderTema.TamanoTexto,
                    color = OrderTema.colorMateria(clase.tono),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                val detalle = if (clase.lugar.isEmpty()) clase.hora else "${clase.hora} · ${clase.lugar}"
                Text(
                    text = detalle,
                    fontSize = OrderTema.TamanoHora,
                    color = OrderTema.TextoSuave,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}
