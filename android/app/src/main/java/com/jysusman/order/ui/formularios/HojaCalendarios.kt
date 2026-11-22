package com.jysusman.order.ui.formularios

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.jysusman.order.datos.Repositorio
import com.jysusman.order.ffi.MateriaConTonoFfi
import com.jysusman.order.ui.comun.PildoraMateria
import com.jysusman.order.ui.selectores.SelectorMateriaDialog
import com.jysusman.order.ui.theme.OrderTema
import kotlinx.coroutines.launch

@Composable
fun HojaCalendarios(
    repositorio: Repositorio,
    materias: List<MateriaConTonoFfi>,
    alVolver: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val estado by repositorio.estado.collectAsStateWithLifecycle()

    var url by remember { mutableStateOf("") }
    var nombre by remember { mutableStateOf("") }
    var materia by remember { mutableStateOf("") }
    var selectorMateria by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        repositorio.cargarCalendarios()
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = OrderTema.EspacioLg, vertical = OrderTema.EspacioMd),
        verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(OrderTema.Espacio2xs)) {
            Text(
                text = "Calendarios",
                fontFamily = OrderTema.FuenteTitulos,
                fontWeight = FontWeight.Bold,
                fontSize = OrderTema.TamanoSemana,
                letterSpacing = OrderTema.LetraNormal,
                color = OrderTema.Texto,
            )
            Text(
                text = "Eventos iCal (.ics / webcal://) como tareas.",
                fontSize = OrderTema.TamanoHora,
                color = OrderTema.TextoTenue,
            )
        }

        if (estado.calendariosError.isNotEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                    .background(OrderTema.FondoError)
                    .border(OrderTema.Linea, OrderTema.BordeError, RoundedCornerShape(OrderTema.RadioPequeno))
                    .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioSm),
            ) {
                Text(
                    text = estado.calendariosError,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoError,
                )
            }
        }

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                .background(OrderTema.FondoCampo)
                .border(OrderTema.Linea, OrderTema.BordeCampo, RoundedCornerShape(OrderTema.RadioPequeno)),
        ) {
            if (estado.calendarios.isEmpty()) {
                Text(
                    text = "Sin calendarios agregados.",
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoTenue,
                    modifier = Modifier.padding(OrderTema.EspacioSm),
                )
            } else {
                estado.calendarios.forEachIndexed { idx, cal ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioSm),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                    ) {
                        Column(
                            modifier = Modifier.weight(1f),
                            verticalArrangement = Arrangement.spacedBy(OrderTema.Espacio2xs),
                        ) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
                            ) {
                                Text(
                                    text = cal.nombre,
                                    fontSize = OrderTema.TamanoPequeno,
                                    color = OrderTema.Texto,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                if (cal.materia.isNotBlank()) {
                                    PildoraMateria(texto = cal.materia, tono = cal.tono)
                                }
                            }
                            Text(
                                text = cal.ultimaTexto,
                                fontSize = OrderTema.TamanoHora,
                                color = OrderTema.TextoSuave,
                            )
                        }
                        OutlinedButton(
                            onClick = {
                                if (!estado.calendariosOcupado) {
                                    scope.launch { repositorio.quitarCalendario(cal.id) }
                                }
                            },
                            enabled = !estado.calendariosOcupado,
                            shape = RoundedCornerShape(OrderTema.Radio),
                            border = BorderStroke(OrderTema.Linea, OrderTema.BordeError),
                        ) {
                            Text(
                                text = "Quitar",
                                fontSize = OrderTema.TamanoHora,
                                color = OrderTema.VencidaTexto,
                            )
                        }
                    }
                    if (idx < estado.calendarios.lastIndex) {
                        HorizontalDivider(color = OrderTema.BordeSuave, thickness = OrderTema.Linea)
                    }
                }
            }
        }

        Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs)) {
            Text(
                text = "Dirección iCal (.ics o webcal://)",
                fontSize = OrderTema.TamanoPequeno,
                letterSpacing = OrderTema.LetraNormal,
                color = OrderTema.TextoSuave,
            )
            OutlinedTextField(
                value = url,
                onValueChange = { url = it },
                enabled = !estado.calendariosOcupado,
                placeholder = {
                    Text(
                        text = "https://calendar.google.com/calendar/ical/…/basic.ics",
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.TextoTenue,
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
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
        ) {
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
            ) {
                Text(
                    text = "Nombre (opcional)",
                    fontSize = OrderTema.TamanoPequeno,
                    letterSpacing = OrderTema.LetraNormal,
                    color = OrderTema.TextoSuave,
                )
                OutlinedTextField(
                    value = nombre,
                    onValueChange = { nombre = it },
                    enabled = !estado.calendariosOcupado,
                    placeholder = {
                        Text(
                            text = "Universidad",
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.TextoTenue,
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
            }

            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
            ) {
                Text(
                    text = "Materia (opcional)",
                    fontSize = OrderTema.TamanoPequeno,
                    letterSpacing = OrderTema.LetraNormal,
                    color = OrderTema.TextoSuave,
                )
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(54.dp)
                        .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                        .background(OrderTema.FondoCampo)
                        .border(OrderTema.Linea, OrderTema.BordeCampo, RoundedCornerShape(OrderTema.RadioPequeno))
                        .clickable(enabled = !estado.calendariosOcupado) { selectorMateria = true }
                        .padding(horizontal = OrderTema.EspacioSm),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = materia.ifEmpty { "Automática" },
                        fontSize = OrderTema.TamanoPequeno,
                        color = if (materia.isEmpty()) OrderTema.TextoTenue else OrderTema.Texto,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    Text(
                        text = "▾",
                        fontSize = OrderTema.TamanoHora,
                        color = OrderTema.TextoTenue,
                    )
                }
            }
        }

        if (estado.calendariosAviso.isNotEmpty()) {
            Text(
                text = estado.calendariosAviso,
                fontSize = OrderTema.TamanoPequeno,
                color = OrderTema.TextoSuave,
            )
        }

        OutlinedButton(
            onClick = {
                if (!estado.calendariosOcupado) {
                    scope.launch { repositorio.sincronizarCalendarios(silencioso = false) }
                }
            },
            enabled = !estado.calendariosOcupado,
            shape = RoundedCornerShape(OrderTema.Radio),
            border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(
                text = if (estado.calendariosOcupado) "Sincronizando…" else "Sincronizar ahora",
                fontSize = OrderTema.TamanoPequeno,
                color = OrderTema.Texto,
            )
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
        ) {
            Spacer(modifier = Modifier.weight(1f))
            OutlinedButton(
                onClick = alVolver,
                shape = RoundedCornerShape(OrderTema.Radio),
                border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
            ) {
                Text(
                    text = "Volver",
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoSuave,
                )
            }
            Button(
                onClick = {
                    if (estado.calendariosOcupado) return@Button
                    scope.launch {
                        val ok = repositorio.agregarCalendario(url, nombre, materia)
                        if (ok) {
                            url = ""
                            nombre = ""
                            materia = ""
                        }
                    }
                },
                enabled = !estado.calendariosOcupado,
                shape = RoundedCornerShape(OrderTema.Radio),
                colors = ButtonDefaults.buttonColors(
                    containerColor = OrderTema.AcentoTenue,
                    contentColor = OrderTema.Acento,
                ),
                border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
            ) {
                Text(
                    text = if (estado.calendariosOcupado) "Agregando…" else "Agregar calendario",
                    fontFamily = OrderTema.FuenteTitulos,
                    fontSize = OrderTema.TamanoPequeno,
                )
            }
        }

        Spacer(modifier = Modifier.height(OrderTema.EspacioMd))
    }

    if (selectorMateria) {
        SelectorMateriaDialog(
            materias = materias,
            repositorio = repositorio,
            onElegir = { materia = it },
            onCerrar = { selectorMateria = false },
        )
    }
}
