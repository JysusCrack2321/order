package com.jysusman.order

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CalendarViewWeek
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Today
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import com.jysusman.order.ffi.ResumenArchivoFfi
import com.jysusman.order.ffi.cargar
import com.jysusman.order.ffi.rutaPorDefecto
import com.jysusman.order.ui.theme.OrderTema
import com.jysusman.order.ui.theme.OrderTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

enum class Pestana(val titulo: String, val icono: ImageVector) {
    Inicio("Inicio", Icons.Outlined.Home),
    Dia("Día", Icons.Outlined.Today),
    Semana("Semana", Icons.Outlined.CalendarViewWeek),
    Pendientes("Pendientes", Icons.Outlined.CheckCircle),
}

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            OrderTheme {
                OrderAppInicial(
                    rutaFallback = filesDir.resolve("order.toml").absolutePath,
                )
            }
        }
    }
}

@Composable
private fun OrderAppInicial(rutaFallback: String) {
    var pestana by rememberSaveable { mutableIntStateOf(0) }
    var resumen by remember { mutableStateOf<ResumenArchivoFfi?>(null) }
    var errorCarga by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(Unit) {
        withContext(Dispatchers.IO) {
            val ruta = runCatching { cargar(rutaPorDefecto()) }
                .recoverCatching { cargar(rutaFallback) }
            ruta.onSuccess { res ->
                resumen = res
                errorCarga = null
            }.onFailure { err ->
                errorCarga = err.message ?: err.toString()
            }
        }
    }

    Scaffold(
        containerColor = OrderTema.Fondo,
        topBar = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(OrderTema.FondoPanel)
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = OrderTema.EspacioLg, vertical = OrderTema.EspacioMd),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Column {
                        Text(
                            text = "ORDER",
                            fontFamily = OrderTema.FuenteTitulos,
                            fontWeight = FontWeight.Bold,
                            fontSize = OrderTema.TamanoTitulo,
                            letterSpacing = OrderTema.LetraTitulo,
                            color = OrderTema.Texto,
                        )
                        Text(
                            text = "horario · tareas",
                            fontSize = OrderTema.TamanoHora,
                            letterSpacing = OrderTema.LetraNormal,
                            color = OrderTema.TextoTenue,
                        )
                    }
                }
                HorizontalDivider(color = OrderTema.Borde, thickness = OrderTema.Linea)
            }
        },
        bottomBar = {
            Column(modifier = Modifier.fillMaxWidth()) {
                HorizontalDivider(color = OrderTema.Borde, thickness = OrderTema.Linea)
                NavigationBar(
                    containerColor = OrderTema.FondoPanel,
                    tonalElevation = OrderTema.Espacio2xs,
                ) {
                    Pestana.entries.forEachIndexed { idx, item ->
                        NavigationBarItem(
                            selected = pestana == idx,
                            onClick = { pestana = idx },
                            icon = { Icon(item.icono, contentDescription = item.titulo) },
                            label = {
                                Text(
                                    text = item.titulo,
                                    fontFamily = OrderTema.FuenteTitulos,
                                    fontSize = OrderTema.TamanoHora,
                                )
                            },
                            colors = NavigationBarItemDefaults.colors(
                                selectedIconColor = OrderTema.Acento,
                                selectedTextColor = OrderTema.Acento,
                                indicatorColor = OrderTema.AcentoTenue,
                                unselectedIconColor = OrderTema.TextoSuave,
                                unselectedTextColor = OrderTema.TextoSuave,
                            ),
                        )
                    }
                }
            }
        },
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .padding(OrderTema.EspacioLg),
            contentAlignment = Alignment.Center,
        ) {
            val actual = Pestana.entries[pestana]
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Text(
                    text = actual.titulo.uppercase(),
                    fontFamily = OrderTema.FuenteTitulos,
                    fontWeight = FontWeight.Bold,
                    fontSize = OrderTema.TamanoSemana,
                    letterSpacing = OrderTema.LetraSeccion,
                    color = OrderTema.Acento,
                )
                Spacer(modifier = Modifier.height(OrderTema.EspacioSm))
                val estadoTexto = when {
                    resumen != null -> "${resumen!!.numTareas} tareas (${resumen!!.numTareasPendientes} pendientes)"
                    errorCarga != null -> "Sin acceso al archivo: $errorCarga"
                    else -> "Cargando…"
                }
                Text(
                    text = estadoTexto,
                    fontSize = OrderTema.TamanoTexto,
                    color = OrderTema.TextoSuave,
                )
            }
        }
    }
}
