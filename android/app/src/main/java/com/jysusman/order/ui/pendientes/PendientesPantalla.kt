package com.jysusman.order.ui.pendientes

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.jysusman.order.ffi.FiltroPendientesFfi
import com.jysusman.order.ffi.PendientesVistaFfi
import com.jysusman.order.ffi.TipoFechaFfi
import com.jysusman.order.ui.comun.CasillaOrder
import com.jysusman.order.ui.comun.ChipOrder
import com.jysusman.order.ui.comun.FilaTarea
import com.jysusman.order.ui.theme.OrderTema

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun PendientesPantalla(
    vista: PendientesVistaFfi?,
    filtro: FiltroPendientesFfi,
    onEditarTarea: (String) -> Unit,
    onMarcarHecha: (String, Boolean) -> Unit,
    onMostrarHechas: (Boolean) -> Unit,
    onFiltroFecha: (TipoFechaFfi) -> Unit,
    onElegirMateria: () -> Unit,
    onElegirDesde: () -> Unit,
    onElegirHasta: () -> Unit,
    onElegirHoraDesde: () -> Unit,
    onElegirHoraHasta: () -> Unit,
    onQuitarFiltros: () -> Unit,
    modifier: Modifier = Modifier,
) {
    if (vista == null) {
        Box(modifier = modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text(text = "Cargando…", color = OrderTema.TextoSuave)
        }
        return
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(horizontal = OrderTema.EspacioMd, vertical = OrderTema.EspacioMd),
        verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
    ) {
        // --- Cabecera y "Mostrar hechas" -----------------------------------
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "PENDIENTES",
                    fontFamily = OrderTema.FuenteTitulos,
                    fontWeight = FontWeight.Bold,
                    fontSize = OrderTema.TamanoSeccion,
                    letterSpacing = OrderTema.LetraSeccion,
                    color = OrderTema.TextoSuave,
                )
                Text(
                    text = vista.resumen,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoTenue,
                )
            }
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                modifier = Modifier.clickable { onMostrarHechas(!filtro.mostrarHechas) },
            ) {
                Text(
                    text = "Mostrar hechas",
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoSuave,
                )
                CasillaOrder(
                    marcada = filtro.mostrarHechas,
                    onToggle = { onMostrarHechas(!filtro.mostrarHechas) },
                )
            }
        }

        // --- Barra de filtros ----------------------------------------------
        FlowRow(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
            verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
        ) {
            ChipOrder(
                texto = "Todas",
                activo = filtro.tipoFecha == TipoFechaFfi.TODAS,
                onClick = { onFiltroFecha(TipoFechaFfi.TODAS) },
            )
            ChipOrder(
                texto = "Hoy",
                activo = filtro.tipoFecha == TipoFechaFfi.HOY,
                onClick = { onFiltroFecha(TipoFechaFfi.HOY) },
            )
            ChipOrder(
                texto = "Semana",
                activo = filtro.tipoFecha == TipoFechaFfi.ESTA_SEMANA,
                onClick = { onFiltroFecha(TipoFechaFfi.ESTA_SEMANA) },
            )
            ChipOrder(
                texto = "Rango",
                activo = filtro.tipoFecha == TipoFechaFfi.RANGO,
                onClick = { onFiltroFecha(TipoFechaFfi.RANGO) },
            )
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            CampoFiltroBoton(
                etiqueta = "Materia",
                valor = filtro.materia.orEmpty(),
                pista = "Todas",
                glifo = "▾",
                onClick = onElegirMateria,
                modifier = Modifier.weight(1f),
            )
            CampoFiltroBoton(
                etiqueta = "Hora desde",
                valor = vista.filtroHoraDesdeTexto,
                pista = "12:00 a. m.",
                glifo = "◷",
                onClick = onElegirHoraDesde,
                modifier = Modifier.weight(1f),
            )
            CampoFiltroBoton(
                etiqueta = "Hora hasta",
                valor = vista.filtroHoraHastaTexto,
                pista = "11:59 p. m.",
                glifo = "◷",
                onClick = onElegirHoraHasta,
                modifier = Modifier.weight(1f),
            )
        }

        if (filtro.tipoFecha == TipoFechaFfi.RANGO) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                CampoFiltroBoton(
                    etiqueta = "Desde",
                    valor = vista.filtroDesdeTexto,
                    pista = "Elegir fecha",
                    glifo = "▦",
                    onClick = onElegirDesde,
                    modifier = Modifier.weight(1f),
                )
                CampoFiltroBoton(
                    etiqueta = "Hasta",
                    valor = vista.filtroHastaTexto,
                    pista = "Elegir fecha",
                    glifo = "▦",
                    onClick = onElegirHasta,
                    modifier = Modifier.weight(1f),
                )
            }
        }

        if (vista.filtroActivo) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
            ) {
                Text(
                    text = "Quitar filtros",
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.Acento,
                    modifier = Modifier
                        .clickable(onClick = onQuitarFiltros)
                        .padding(vertical = OrderTema.Espacio2xs),
                )
            }
        }

        HorizontalDivider(color = OrderTema.BordeSuave, thickness = OrderTema.Linea)

        // --- Lista de tareas -----------------------------------------------
        if (vista.tareas.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
                contentAlignment = Alignment.Center,
            ) {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
                ) {
                    Text(
                        text = if (vista.filtroActivo) "Sin resultados" else "Nada pendiente",
                        fontFamily = OrderTema.FuenteTitulos,
                        fontSize = OrderTema.TamanoSemana,
                        letterSpacing = OrderTema.LetraNormal,
                        color = OrderTema.TextoSuave,
                    )
                    Text(
                        text = if (vista.filtroActivo) {
                            "Ninguna tarea pasa estos filtros."
                        } else {
                            "Las tareas que agregues aparecerán aquí."
                        },
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.TextoTenue,
                        textAlign = TextAlign.Center,
                    )
                }
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
                contentPadding = PaddingValues(bottom = 84.dp),
            ) {
                items(vista.tareas, key = { it.id }) { tarea ->
                    FilaTarea(
                        tarea = tarea,
                        onClick = { onEditarTarea(tarea.id) },
                        onToggleHecha = { hecha -> onMarcarHecha(tarea.id, hecha) },
                    )
                    HorizontalDivider(color = OrderTema.BordeSuave, thickness = OrderTema.Linea)
                }
            }
        }
    }
}

@Composable
private fun CampoFiltroBoton(
    etiqueta: String,
    valor: String,
    pista: String,
    glifo: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier,
        verticalArrangement = Arrangement.spacedBy(OrderTema.Espacio2xs),
    ) {
        Text(
            text = etiqueta,
            fontSize = OrderTema.TamanoHora,
            color = OrderTema.TextoSuave,
        )
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(34.dp)
                .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                .background(OrderTema.FondoCampo)
                .border(OrderTema.Linea, OrderTema.BordeCampo, RoundedCornerShape(OrderTema.RadioPequeno))
                .clickable(onClick = onClick)
                .padding(horizontal = OrderTema.EspacioSm),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
        ) {
            Text(
                text = valor.ifEmpty { pista },
                fontSize = OrderTema.TamanoPequeno,
                color = if (valor.isEmpty()) OrderTema.TextoTenue else OrderTema.Texto,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Text(
                text = glifo,
                fontSize = OrderTema.TamanoHora,
                color = OrderTema.TextoTenue,
            )
        }
    }
}
