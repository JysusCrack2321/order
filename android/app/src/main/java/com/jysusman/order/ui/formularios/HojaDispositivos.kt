package com.jysusman.order.ui.formularios

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import com.jysusman.order.datos.Repositorio
import com.jysusman.order.ui.theme.OrderTema
import kotlinx.coroutines.launch

@Composable
fun HojaDispositivos(
    repositorio: Repositorio,
    alVolver: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val estado by repositorio.estado.collectAsStateWithLifecycle()

    var nombreLocal by remember(estado.p2pNombre) { mutableStateOf(estado.p2pNombre) }
    var escribiendoCodigo by remember { mutableStateOf(false) }
    var entradaCodigo by remember { mutableStateOf("") }
    var avisoCopia by remember { mutableStateOf("") }

    LaunchedEffect(Unit) {
        repositorio.refrescarDispositivosP2p()
    }

    fun abrirEscanerQr() {
        val opciones = GmsBarcodeScannerOptions.Builder()
            .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
            .build()
        val scanner = GmsBarcodeScanning.getClient(context, opciones)
        scanner.startScan()
            .addOnSuccessListener { barcode ->
                val crudo = barcode.rawValue.orEmpty().trim()
                if (crudo.isNotEmpty()) {
                    scope.launch { repositorio.emparejarP2p(crudo) }
                }
            }
            .addOnFailureListener { err ->
                repositorio.anotarErrorP2p(
                    err.message ?: "No se pudo abrir el escáner; usa «Escribir código»."
                )
            }
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
                text = "Dispositivos",
                fontFamily = OrderTema.FuenteTitulos,
                fontWeight = FontWeight.Bold,
                fontSize = OrderTema.TamanoSemana,
                letterSpacing = OrderTema.LetraNormal,
                color = OrderTema.Texto,
            )
            Text(
                text = "Sincroniza tu agenda entre PC y celular sin cuentas ni servidores.",
                fontSize = OrderTema.TamanoHora,
                color = OrderTema.TextoTenue,
            )
        }

        if (estado.p2pError.isNotEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                    .background(OrderTema.FondoError)
                    .border(OrderTema.Linea, OrderTema.BordeError, RoundedCornerShape(OrderTema.RadioPequeno))
                    .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioSm),
            ) {
                Text(
                    text = estado.p2pError,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoError,
                )
            }
        }

        // Nombre de este dispositivo
        Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs)) {
            Text(
                text = "Este dispositivo",
                fontSize = OrderTema.TamanoPequeno,
                letterSpacing = OrderTema.LetraNormal,
                color = OrderTema.TextoSuave,
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                OutlinedTextField(
                    value = nombreLocal,
                    onValueChange = { nombreLocal = it },
                    placeholder = {
                        Text(
                            text = "Teléfono",
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
                    modifier = Modifier.weight(1f),
                )
                OutlinedButton(
                    onClick = { scope.launch { repositorio.cambiarNombreDispositivoP2p(nombreLocal) } },
                    shape = RoundedCornerShape(OrderTema.Radio),
                    border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
                ) {
                    Text(
                        text = "Guardar",
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.Texto,
                    )
                }
            }
        }

        val codigo = estado.p2pCodigo
        if (codigo != null) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(OrderTema.Radio))
                    .background(OrderTema.FondoCampo)
                    .border(OrderTema.Linea, OrderTema.BordeCampo, RoundedCornerShape(OrderTema.Radio))
                    .padding(OrderTema.EspacioMd),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                Text(
                    text = "Escanea este código desde tu otro dispositivo:",
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoSuave,
                    textAlign = TextAlign.Center,
                )

                MatrizQr(
                    tamano = codigo.qrTamano.toInt(),
                    modulos = codigo.qrModulos,
                    modifier = Modifier.size(196.dp),
                )

                Text(
                    text = codigo.palabras,
                    fontSize = OrderTema.TamanoSeccion,
                    fontWeight = FontWeight.Bold,
                    color = OrderTema.Acento,
                    textAlign = TextAlign.Center,
                )

                Text(
                    text = "Esperando… (válido 10 min)",
                    fontSize = OrderTema.TamanoHora,
                    color = OrderTema.TextoTenue,
                )

                if (avisoCopia.isNotEmpty()) {
                    Text(
                        text = avisoCopia,
                        fontSize = OrderTema.TamanoHora,
                        color = OrderTema.TextoSuave,
                    )
                }

                Row(
                    horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                ) {
                    OutlinedButton(
                        onClick = {
                            val cb = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
                            cb?.setPrimaryClip(ClipData.newPlainText("ORDER emparejamiento", codigo.enlace))
                            avisoCopia = "Enlace copiado."
                        },
                        shape = RoundedCornerShape(OrderTema.Radio),
                        border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
                    ) {
                        Text(
                            text = "Copiar enlace",
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.Texto,
                        )
                    }
                    OutlinedButton(
                        onClick = {
                            avisoCopia = ""
                            repositorio.cerrarCodigoEmparejamientoP2p()
                        },
                        shape = RoundedCornerShape(OrderTema.Radio),
                        border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
                    ) {
                        Text(
                            text = "Listo",
                            fontSize = OrderTema.TamanoPequeno,
                            color = OrderTema.Acento,
                        )
                    }
                }
            }
        } else {
            // Lista de dispositivos emparejados
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                    .background(OrderTema.FondoCampo)
                    .border(OrderTema.Linea, OrderTema.BordeCampo, RoundedCornerShape(OrderTema.RadioPequeno)),
            ) {
                if (estado.p2pDispositivos.isEmpty()) {
                    Text(
                        text = "Sin dispositivos emparejados.",
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.TextoTenue,
                        modifier = Modifier.padding(OrderTema.EspacioSm),
                    )
                } else {
                    estado.p2pDispositivos.forEachIndexed { idx, disp ->
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
                                Text(
                                    text = disp.nombre,
                                    fontSize = OrderTema.TamanoPequeno,
                                    color = OrderTema.Texto,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                Text(
                                    text = disp.textoEstado,
                                    fontSize = OrderTema.TamanoHora,
                                    color = if (disp.enLinea) OrderTema.TextoSuave else OrderTema.VencidaTexto,
                                )
                            }
                            OutlinedButton(
                                onClick = {
                                    if (!estado.p2pOcupado) {
                                        scope.launch { repositorio.quitarDispositivoP2p(disp.id) }
                                    }
                                },
                                enabled = !estado.p2pOcupado,
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
                        if (idx < estado.p2pDispositivos.lastIndex) {
                            HorizontalDivider(color = OrderTema.BordeSuave, thickness = OrderTema.Linea)
                        }
                    }
                }
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
            ) {
                Button(
                    onClick = { scope.launch { repositorio.generarCodigoEmparejamientoP2p() } },
                    enabled = !estado.p2pOcupado,
                    shape = RoundedCornerShape(OrderTema.Radio),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = OrderTema.AcentoTenue,
                        contentColor = OrderTema.Acento,
                    ),
                    border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
                    modifier = Modifier.weight(1f),
                ) {
                    Text(
                        text = "Agregar dispositivo",
                        fontFamily = OrderTema.FuenteTitulos,
                        fontSize = OrderTema.TamanoHora,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }

                OutlinedButton(
                    onClick = { abrirEscanerQr() },
                    enabled = !estado.p2pOcupado,
                    shape = RoundedCornerShape(OrderTema.Radio),
                    border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
                    modifier = Modifier.weight(1f),
                ) {
                    Text(
                        text = "Escanear QR",
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.Acento,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }

            OutlinedButton(
                onClick = { escribiendoCodigo = !escribiendoCodigo },
                shape = RoundedCornerShape(OrderTema.Radio),
                border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(
                    text = if (escribiendoCodigo) "Ocultar entrada de código" else "Escribir código",
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoSuave,
                )
            }

            if (escribiendoCodigo) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                ) {
                    OutlinedTextField(
                        value = entradaCodigo,
                        onValueChange = { entradaCodigo = it },
                        enabled = !estado.p2pOcupado,
                        placeholder = {
                            Text(
                                text = "8 palabras o order://emparejar?…",
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
                        modifier = Modifier.weight(1f),
                    )
                    Button(
                        onClick = {
                            if (estado.p2pOcupado) return@Button
                            scope.launch {
                                val ok = repositorio.emparejarP2p(entradaCodigo)
                                if (ok) {
                                    entradaCodigo = ""
                                    escribiendoCodigo = false
                                }
                            }
                        },
                        enabled = !estado.p2pOcupado,
                        shape = RoundedCornerShape(OrderTema.Radio),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = OrderTema.AcentoTenue,
                            contentColor = OrderTema.Acento,
                        ),
                        border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
                    ) {
                        Text(
                            text = if (estado.p2pOcupado) "Conectando…" else "Emparejar",
                            fontSize = OrderTema.TamanoPequeno,
                        )
                    }
                }
            }
        }

        if (estado.p2pAviso.isNotEmpty()) {
            Text(
                text = estado.p2pAviso,
                fontSize = OrderTema.TamanoPequeno,
                color = OrderTema.TextoSuave,
            )
        }

        OutlinedButton(
            onClick = {
                if (!estado.p2pOcupado) {
                    scope.launch { repositorio.sincronizarP2p(silencioso = false) }
                }
            },
            enabled = !estado.p2pOcupado && estado.p2pCodigo == null,
            shape = RoundedCornerShape(OrderTema.Radio),
            border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(
                text = if (estado.p2pOcupado || estado.p2pSincronizando) "Sincronizando…" else "Sincronizar ahora",
                fontSize = OrderTema.TamanoPequeno,
                color = OrderTema.Texto,
            )
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
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
        }

        Spacer(modifier = Modifier.height(OrderTema.EspacioMd))
    }
}

@Composable
private fun MatrizQr(
    tamano: Int,
    modulos: List<Boolean>,
    modifier: Modifier = Modifier,
) {
    if (tamano <= 0 || modulos.size != tamano * tamano) return
    val oscuro = OrderTema.Fondo
    val claro = OrderTema.Texto
    Canvas(modifier = modifier) {
        drawRect(color = claro)
        val margen = 2
        val total = tamano + 2 * margen
        val celda = size.minDimension / total
        for (y in 0 until tamano) {
            for (x in 0 until tamano) {
                if (modulos[y * tamano + x]) {
                    drawRect(
                        color = oscuro,
                        topLeft = Offset((x + margen) * celda, (y + margen) * celda),
                        size = Size(celda + 0.5f, celda + 0.5f),
                    )
                }
            }
        }
    }
}
