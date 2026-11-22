package com.jysusman.order.ui.selectores

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.snapping.rememberSnapFlingBehavior
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import com.jysusman.order.datos.Repositorio
import com.jysusman.order.ffi.CalendarioMesFfi
import com.jysusman.order.ffi.MateriaConTonoFfi
import com.jysusman.order.ui.comun.BotonPaso
import com.jysusman.order.ui.comun.ChipOrder
import com.jysusman.order.ui.theme.OrderTema
import kotlinx.coroutines.launch
import kotlin.math.abs

@Composable
fun SelectorFechaDialog(
    fechaInicialIso: String,
    repositorio: Repositorio,
    onAceptar: (String) -> Unit,
    onCerrar: () -> Unit,
    onLimpiar: (() -> Unit)? = null,
) {
    val scope = rememberCoroutineScope()
    val partesInicial = remember(fechaInicialIso) {
        val trozos = fechaInicialIso.trim().split("-")
        if (trozos.size == 3) {
            Triple(
                trozos[0].toIntOrNull() ?: 2026,
                (trozos[1].toIntOrNull() ?: 11).coerceIn(1, 12),
                (trozos[2].toIntOrNull() ?: 1).coerceIn(1, 31),
            )
        } else {
            Triple(2026, 11, 22)
        }
    }

    var anio by remember { mutableIntStateOf(partesInicial.first) }
    var mes by remember { mutableIntStateOf(partesInicial.second) }
    var dia by remember { mutableIntStateOf(partesInicial.third) }
    var calendario by remember { mutableStateOf<CalendarioMesFfi?>(null) }

    LaunchedEffect(Unit) {
        if (fechaInicialIso.isBlank()) {
            val hoy = repositorio.estado.value.diaIso
            val trozos = hoy.split("-")
            if (trozos.size == 3) {
                anio = trozos[0].toIntOrNull() ?: anio
                mes = (trozos[1].toIntOrNull() ?: mes).coerceIn(1, 12)
                dia = (trozos[2].toIntOrNull() ?: dia).coerceIn(1, 31)
            }
        }
    }

    LaunchedEffect(anio, mes, dia) {
        calendario = repositorio.calendarioMes(anio, mes, dia)
    }

    Dialog(onDismissRequest = onCerrar) {
        Surface(
            color = OrderTema.FondoPanel,
            shape = RoundedCornerShape(OrderTema.RadioTarjeta),
            border = BorderStroke(OrderTema.Linea, OrderTema.BordeFuerte),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(OrderTema.EspacioLg),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
            ) {
                Text(
                    text = "FECHA",
                    fontFamily = OrderTema.FuenteTitulos,
                    fontWeight = FontWeight.Bold,
                    fontSize = OrderTema.TamanoSeccion,
                    letterSpacing = OrderTema.LetraSeccion,
                    color = OrderTema.Texto,
                )

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                ) {
                    BotonPaso(
                        glifo = "‹",
                        onClick = {
                            scope.launch {
                                val vecino = repositorio.mesVecino(anio, mes, -1)
                                val maxDia = repositorio.diasDelMes(vecino.anio, vecino.mes.toInt())
                                anio = vecino.anio
                                mes = vecino.mes.toInt()
                                dia = dia.coerceIn(1, maxDia)
                            }
                        },
                    )
                    Text(
                        text = calendario?.etiquetaMes ?: "",
                        fontSize = OrderTema.TamanoTexto,
                        color = OrderTema.Texto,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.weight(1f),
                    )
                    BotonPaso(
                        glifo = "›",
                        onClick = {
                            scope.launch {
                                val vecino = repositorio.mesVecino(anio, mes, 1)
                                val maxDia = repositorio.diasDelMes(vecino.anio, vecino.mes.toInt())
                                anio = vecino.anio
                                mes = vecino.mes.toInt()
                                dia = dia.coerceIn(1, maxDia)
                            }
                        },
                    )
                }

                // Cabecera L M X J V S D
                Row(modifier = Modifier.fillMaxWidth()) {
                    listOf("L", "M", "X", "J", "V", "S", "D").forEach { letra ->
                        Text(
                            text = letra,
                            fontFamily = OrderTema.FuenteTitulos,
                            fontSize = OrderTema.TamanoHora,
                            color = OrderTema.TextoTenue,
                            textAlign = TextAlign.Center,
                            modifier = Modifier.weight(1f),
                        )
                    }
                }

                // 6 semanas x 7 días
                val celdas = calendario?.celdas.orEmpty()
                if (celdas.isNotEmpty()) {
                    Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs)) {
                        for (fila in 0..5) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
                            ) {
                                for (col in 0..6) {
                                    val celda = celdas.getOrNull(fila * 7 + col)
                                    if (celda == null || celda.dia <= 0) {
                                        Spacer(
                                            modifier = Modifier
                                                .weight(1f)
                                                .height(36.dp)
                                        )
                                    } else {
                                        val bg = if (celda.elegida) OrderTema.AcentoTenue else Color.Transparent
                                        val borde = when {
                                            celda.elegida -> OrderTema.Acento
                                            celda.hoy -> OrderTema.AcentoSuave
                                            else -> Color.Transparent
                                        }
                                        val tinta = when {
                                            celda.elegida -> OrderTema.Acento
                                            celda.hoy -> OrderTema.Texto
                                            else -> OrderTema.TextoSuave
                                        }
                                        Box(
                                            modifier = Modifier
                                                .weight(1f)
                                                .height(36.dp)
                                                .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                                                .background(bg)
                                                .border(
                                                    OrderTema.Linea,
                                                    borde,
                                                    RoundedCornerShape(OrderTema.RadioPequeno),
                                                )
                                                .clickable { dia = celda.dia },
                                            contentAlignment = Alignment.Center,
                                        ) {
                                            Text(
                                                text = "${celda.dia}",
                                                fontSize = OrderTema.TamanoPequeno,
                                                color = tinta,
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                ) {
                    if (onLimpiar != null) {
                        TextButton(onClick = {
                            onLimpiar()
                            onCerrar()
                        }) {
                            Text(
                                text = "Limpiar",
                                fontSize = OrderTema.TamanoPequeno,
                                color = OrderTema.TextoSuave,
                            )
                        }
                    }
                    Spacer(modifier = Modifier.weight(1f))
                    OutlinedButton(
                        onClick = onCerrar,
                        shape = RoundedCornerShape(OrderTema.Radio),
                        border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
                    ) {
                        Text(
                            text = "Cancelar",
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.TextoSuave,
                        )
                    }
                    Button(
                        onClick = {
                            onAceptar("%04d-%02d-%02d".format(anio, mes, dia))
                            onCerrar()
                        },
                        shape = RoundedCornerShape(OrderTema.Radio),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = OrderTema.AcentoTenue,
                            contentColor = OrderTema.Acento,
                        ),
                        border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
                    ) {
                        Text(
                            text = "Aceptar",
                            fontFamily = OrderTema.FuenteTitulos,
                            fontSize = OrderTema.TamanoPequeno,
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun SelectorHoraDialog(
    horaInicialTexto: String,
    repositorio: Repositorio,
    onAceptar: (String) -> Unit,
    onCerrar: () -> Unit,
    onLimpiar: (() -> Unit)? = null,
) {
    var horasTextos by remember { mutableStateOf<List<String>>(emptyList()) }
    var minutosTextos by remember { mutableStateOf<List<String>>(emptyList()) }
    var listo by remember { mutableStateOf(false) }

    var hora12 by remember { mutableIntStateOf(11) }
    var minuto by remember { mutableIntStateOf(59) }
    var tarde by remember { mutableStateOf(true) }
    var vistaPrevia by remember { mutableStateOf("") }

    LaunchedEffect(horaInicialTexto) {
        horasTextos = repositorio.horas12Texto()
        minutosTextos = repositorio.minutos60Texto()
        val parsed = repositorio.parsearHora12(horaInicialTexto)
        hora12 = parsed.hora12.toInt().coerceIn(1, 12)
        minuto = parsed.minuto.toInt().coerceIn(0, 59)
        tarde = parsed.tarde
        vistaPrevia = parsed.texto
        listo = true
    }

    LaunchedEffect(listo, hora12, minuto, tarde) {
        if (listo) {
            vistaPrevia = repositorio.formatearHora12(hora12, minuto, tarde)
        }
    }

    Dialog(onDismissRequest = onCerrar) {
        Surface(
            color = OrderTema.FondoPanel,
            shape = RoundedCornerShape(OrderTema.RadioTarjeta),
            border = BorderStroke(OrderTema.Linea, OrderTema.BordeFuerte),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(OrderTema.EspacioLg),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(
                    text = "HORA",
                    fontFamily = OrderTema.FuenteTitulos,
                    fontWeight = FontWeight.Bold,
                    fontSize = OrderTema.TamanoSeccion,
                    letterSpacing = OrderTema.LetraSeccion,
                    color = OrderTema.Texto,
                    modifier = Modifier.fillMaxWidth(),
                )

                Text(
                    text = vistaPrevia,
                    fontFamily = OrderTema.FuenteTitulos,
                    fontWeight = FontWeight.Bold,
                    fontSize = OrderTema.TamanoTitulo,
                    letterSpacing = OrderTema.LetraTitulo,
                    color = OrderTema.Acento,
                )

                if (listo && horasTextos.isNotEmpty() && minutosTextos.isNotEmpty()) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
                    ) {
                        RuedaCircular(
                            valores = horasTextos,
                            indiceInicial = (hora12 - 1).coerceIn(0, 11),
                            alElegirIndice = { idx -> hora12 = idx + 1 },
                            modifier = Modifier.width(88.dp),
                        )
                        Text(
                            text = ":",
                            fontFamily = OrderTema.FuenteTitulos,
                            fontSize = OrderTema.TamanoTitulo,
                            color = OrderTema.TextoSuave,
                        )
                        RuedaCircular(
                            valores = minutosTextos,
                            indiceInicial = minuto.coerceIn(0, 59),
                            alElegirIndice = { idx -> minuto = idx },
                            modifier = Modifier.width(88.dp),
                        )
                    }
                }

                Row(horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm)) {
                    ChipOrder(
                        texto = "a. m.",
                        activo = !tarde,
                        onClick = { tarde = false },
                        modifier = Modifier.width(76.dp),
                    )
                    ChipOrder(
                        texto = "p. m.",
                        activo = tarde,
                        onClick = { tarde = true },
                        modifier = Modifier.width(76.dp),
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                ) {
                    if (onLimpiar != null) {
                        TextButton(onClick = {
                            onLimpiar()
                            onCerrar()
                        }) {
                            Text(
                                text = "Limpiar",
                                fontSize = OrderTema.TamanoPequeno,
                                color = OrderTema.TextoSuave,
                            )
                        }
                    }
                    Spacer(modifier = Modifier.weight(1f))
                    OutlinedButton(
                        onClick = onCerrar,
                        shape = RoundedCornerShape(OrderTema.Radio),
                        border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
                    ) {
                        Text(
                            text = "Cancelar",
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.TextoSuave,
                        )
                    }
                    Button(
                        onClick = {
                            val texto = vistaPrevia
                            if (texto.isNotEmpty()) {
                                onAceptar(texto)
                            }
                            onCerrar()
                        },
                        shape = RoundedCornerShape(OrderTema.Radio),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = OrderTema.AcentoTenue,
                            contentColor = OrderTema.Acento,
                        ),
                        border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
                    ) {
                        Text(
                            text = "Aceptar",
                            fontFamily = OrderTema.FuenteTitulos,
                            fontSize = OrderTema.TamanoPequeno,
                        )
                    }
                }
            }
        }
    }
}

/**
 * Rueda circular sin topes: repite `valores` en un bucle largo y encaja al centro.
 */
@Composable
private fun RuedaCircular(
    valores: List<String>,
    indiceInicial: Int,
    alElegirIndice: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val cuenta = valores.size
    val ciclos = 1000
    val totalVirtual = cuenta * ciclos
    val indiceCentroInicial = remember(cuenta, indiceInicial) {
        (ciclos / 2) * cuenta + indiceInicial.coerceIn(0, (cuenta - 1).coerceAtLeast(0))
    }

    val listState = rememberLazyListState(initialFirstVisibleItemIndex = indiceCentroInicial)
    val flingBehavior = rememberSnapFlingBehavior(lazyListState = listState)
    val scope = rememberCoroutineScope()

    val altoItem = 36.dp
    val altoRueda = altoItem * 5

    val indiceCentralVirtual by remember {
        derivedStateOf {
            val info = listState.layoutInfo
            val centroViewport = (info.viewportStartOffset + info.viewportEndOffset) / 2
            info.visibleItemsInfo.minByOrNull { item ->
                abs((item.offset + item.size / 2) - centroViewport)
            }?.index ?: indiceCentroInicial
        }
    }

    LaunchedEffect(indiceCentralVirtual) {
        if (cuenta > 0) {
            alElegirIndice(indiceCentralVirtual % cuenta)
        }
    }

    Box(
        modifier = modifier.height(altoRueda),
        contentAlignment = Alignment.Center,
    ) {
        // Banda central
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(altoItem)
                .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                .background(OrderTema.AcentoTenue)
                .border(OrderTema.Linea, OrderTema.AcentoSuave, RoundedCornerShape(OrderTema.RadioPequeno))
        )

        LazyColumn(
            state = listState,
            flingBehavior = flingBehavior,
            contentPadding = PaddingValues(vertical = altoItem * 2),
            modifier = Modifier
                .fillMaxWidth()
                .height(altoRueda),
        ) {
            items(count = totalVirtual) { virtualIdx ->
                val realIdx = virtualIdx % cuenta
                val distancia = abs(virtualIdx - indiceCentralVirtual)
                val alpha = when (distancia) {
                    0 -> 1f
                    1 -> 0.72f
                    else -> 0.35f
                }
                val color = when {
                    distancia == 0 -> OrderTema.Acento
                    distancia == 1 -> OrderTema.TextoSuave
                    else -> OrderTema.TextoTenue
                }
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(altoItem)
                        .alpha(alpha)
                        .clickable {
                            scope.launch { listState.animateScrollToItem(virtualIdx) }
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = valores[realIdx],
                        fontFamily = OrderTema.FuenteTitulos,
                        fontSize = if (distancia == 0) OrderTema.TamanoTexto else OrderTema.TamanoPequeno,
                        fontWeight = if (distancia == 0) FontWeight.Bold else FontWeight.Normal,
                        color = color,
                    )
                }
            }
        }
    }
}

@Composable
fun SelectorMateriaDialog(
    materias: List<MateriaConTonoFfi>,
    repositorio: Repositorio,
    tituloSinMateria: String = "Sin materia",
    onElegir: (String) -> Unit,
    onCerrar: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var escribiendo by remember { mutableStateOf(false) }
    var nuevaMateria by remember { mutableStateOf("") }

    Dialog(onDismissRequest = onCerrar) {
        Surface(
            color = OrderTema.FondoPanel,
            shape = RoundedCornerShape(OrderTema.RadioTarjeta),
            border = BorderStroke(OrderTema.Linea, OrderTema.BordeFuerte),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(OrderTema.EspacioLg),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
            ) {
                Text(
                    text = "MATERIA",
                    fontFamily = OrderTema.FuenteTitulos,
                    fontWeight = FontWeight.Bold,
                    fontSize = OrderTema.TamanoSeccion,
                    letterSpacing = OrderTema.LetraSeccion,
                    color = OrderTema.Texto,
                )

                if (!escribiendo) {
                    LazyColumn(
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(220.dp),
                        verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
                    ) {
                        item {
                            FichaMateria(
                                texto = tituloSinMateria,
                                tono = 8,
                                onClick = {
                                    onElegir("")
                                    onCerrar()
                                },
                            )
                        }
                        items(materias.size) { i ->
                            val m = materias[i]
                            FichaMateria(
                                texto = m.nombre,
                                tono = m.tono,
                                onClick = {
                                    onElegir(m.nombre)
                                    onCerrar()
                                },
                            )
                        }
                        if (materias.isEmpty()) {
                            item {
                                Text(
                                    text = "Aún no hay materias en el archivo.",
                                    fontSize = OrderTema.TamanoPequeno,
                                    color = OrderTema.TextoTenue,
                                    modifier = Modifier.padding(vertical = OrderTema.EspacioSm),
                                )
                            }
                        }
                    }

                    OutlinedButton(
                        onClick = { escribiendo = true },
                        shape = RoundedCornerShape(OrderTema.Radio),
                        border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(
                            text = "Escribir una nueva",
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.Acento,
                        )
                    }
                } else {
                    Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm)) {
                        Text(
                            text = "Materia nueva",
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.TextoSuave,
                        )
                        OutlinedTextField(
                            value = nuevaMateria,
                            onValueChange = { nuevaMateria = it },
                            placeholder = {
                                Text(
                                    text = "Cálculo",
                                    color = OrderTema.TextoTenue,
                                    fontSize = OrderTema.TamanoPequeno,
                                )
                            },
                            singleLine = true,
                            colors = OutlinedTextFieldDefaults.colors(
                                focusedContainerColor = OrderTema.FondoCampo,
                                unfocusedContainerColor = OrderTema.FondoCampo,
                                focusedBorderColor = OrderTema.AcentoSuave,
                                unfocusedBorderColor = OrderTema.BordeCampo,
                                focusedTextColor = OrderTema.Texto,
                                unfocusedTextColor = OrderTema.Texto,
                                cursorColor = OrderTema.Acento,
                            ),
                            modifier = Modifier.fillMaxWidth(),
                        )
                        Text(
                            text = "Si coincide con una existente, se guarda con su grafía.",
                            fontSize = OrderTema.TamanoHora,
                            color = OrderTema.TextoTenue,
                        )
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    OutlinedButton(
                        onClick = onCerrar,
                        shape = RoundedCornerShape(OrderTema.Radio),
                        border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
                    ) {
                        Text(
                            text = "Cancelar",
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.TextoSuave,
                        )
                    }
                    if (escribiendo) {
                        Spacer(modifier = Modifier.width(OrderTema.EspacioSm))
                        Button(
                            onClick = {
                                scope.launch {
                                    val canon = repositorio.canonicaMateria(nuevaMateria)
                                    onElegir(canon)
                                    onCerrar()
                                }
                            },
                            shape = RoundedCornerShape(OrderTema.Radio),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = OrderTema.AcentoTenue,
                                contentColor = OrderTema.Acento,
                            ),
                            border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
                        ) {
                            Text(
                                text = "Aceptar",
                                fontFamily = OrderTema.FuenteTitulos,
                                fontSize = OrderTema.TamanoPequeno,
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun FichaMateria(
    texto: String,
    tono: Int,
    onClick: () -> Unit,
) {
    val color = OrderTema.colorMateria(tono)
    val fondo = OrderTema.fondoMateria(tono)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(IntrinsicSize.Min)
            .clip(RoundedCornerShape(OrderTema.Radio))
            .background(fondo)
            .border(OrderTema.Linea, OrderTema.BordeSuave, RoundedCornerShape(OrderTema.Radio))
            .clickable(onClick = onClick),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier = Modifier
                .width(OrderTema.AnchoMarca)
                .fillMaxHeight()
                .background(color)
        )
        Text(
            text = texto,
            fontSize = OrderTema.TamanoPequeno,
            color = color,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(horizontal = OrderTema.EspacioMd, vertical = OrderTema.EspacioSm),
        )
    }
}
