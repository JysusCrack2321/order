package com.jysusman.order.ui.agente

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CheckboxDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import com.jysusman.order.datos.PreferenciasOrder
import com.jysusman.order.datos.Repositorio
import com.jysusman.order.ffi.ProveedorInfoFfi
import com.jysusman.order.ffi.VozProveedorInfoFfi
import com.jysusman.order.ui.comun.BotonBorde
import com.jysusman.order.ui.comun.ChipOrder
import com.jysusman.order.ui.theme.OrderTema
import kotlinx.coroutines.launch

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun HojaAjustesAgente(
    repositorio: Repositorio,
    alVolver: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    var listo by remember { mutableStateOf(false) }
    var proveedores by remember { mutableStateOf<List<ProveedorInfoFfi>>(emptyList()) }
    var proveedoresVoz by remember { mutableStateOf<List<VozProveedorInfoFfi>>(emptyList()) }

    var proveedor by remember { mutableIntStateOf(0) }
    var urlBase by remember { mutableStateOf("") }
    var modelo by remember { mutableStateOf("") }
    var apiKey by remember { mutableStateOf("") }
    var verKey by remember { mutableStateOf(false) }
    var veImagenes by remember { mutableStateOf(false) }
    var limiteContexto by remember { mutableStateOf("60000") }

    var vozProveedor by remember { mutableIntStateOf(0) }
    var vozUrlBase by remember { mutableStateOf("") }
    var vozModelo by remember { mutableStateOf("") }
    var vozApiKey by remember { mutableStateOf("") }
    var verVozKey by remember { mutableStateOf(false) }
    var usarVozAndroid by remember { mutableStateOf(PreferenciasOrder.usarVozAndroid(context)) }

    var avanzado by remember { mutableStateOf(false) }
    var errorMsg by remember { mutableStateOf("") }
    var campoMal by remember { mutableIntStateOf(0) }

    LaunchedEffect(Unit) {
        proveedores = repositorio.proveedoresAgente()
        proveedoresVoz = repositorio.proveedoresVoz()
        repositorio.leerAgenteConfig().onSuccess { cfg ->
            proveedor = cfg.proveedor
            urlBase = cfg.urlBase
            modelo = cfg.modelo
            apiKey = cfg.apiKey
            veImagenes = cfg.veImagenes
            limiteContexto = cfg.limiteContextoTokens.toString()
            vozProveedor = cfg.vozProveedor
            vozUrlBase = cfg.vozUrlBase
            vozModelo = cfg.vozModelo
            vozApiKey = cfg.vozApiKey
            val actual = proveedores.find { it.codigo == cfg.proveedor }
            avanzado = actual?.urlClave == null
            listo = true
        }.onFailure { err ->
            errorMsg = err.message ?: "No se pudo leer agente.toml."
            listo = true
        }
    }

    val proveedorActual = proveedores.find { it.codigo == proveedor }
    val vozActual = proveedoresVoz.find { it.codigo == vozProveedor }
    val mostrarAvanzado = avanzado || proveedorActual?.urlClave == null

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = OrderTema.EspacioLg, vertical = OrderTema.EspacioMd),
        verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(OrderTema.Espacio2xs)) {
            Text(
                text = "Agente y voz",
                fontFamily = OrderTema.FuenteTitulos,
                fontWeight = FontWeight.Bold,
                fontSize = OrderTema.TamanoSemana,
                letterSpacing = OrderTema.LetraNormal,
                color = OrderTema.Texto,
            )
            Text(
                text = "Responde en la barra de abajo. La clave vive solo en este teléfono.",
                fontSize = OrderTema.TamanoHora,
                color = OrderTema.TextoTenue,
            )
        }

        if (errorMsg.isNotEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(OrderTema.RadioPequeno))
                    .background(OrderTema.FondoError)
                    .border(OrderTema.Linea, OrderTema.BordeError, RoundedCornerShape(OrderTema.RadioPequeno))
                    .padding(horizontal = OrderTema.EspacioSm, vertical = OrderTema.EspacioSm),
            ) {
                Text(
                    text = errorMsg,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoError,
                )
            }
        }

        Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs)) {
            Text(
                text = "Proveedor",
                fontSize = OrderTema.TamanoPequeno,
                letterSpacing = OrderTema.LetraNormal,
                color = OrderTema.TextoSuave,
            )
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
            ) {
                proveedores.forEach { info ->
                    val etiqueta = if (info.etiqueta != null) {
                        "${info.nombre} · ${info.etiqueta}"
                    } else {
                        info.nombre
                    }
                    ChipOrder(
                        texto = etiqueta,
                        activo = proveedor == info.codigo,
                        onClick = {
                            proveedor = info.codigo
                            urlBase = info.urlBase
                            modelo = info.modelo
                            if (info.urlClave == null) avanzado = true
                        },
                    )
                }
            }
        }

        proveedorActual?.urlClave?.let { urlClave ->
            BotonBorde(
                texto = "Conseguir mi API key",
                onClick = { abrirNavegador(context, urlClave) },
            )
        }

        CampoClaveAgente(
            etiqueta = "API key",
            valor = apiKey,
            alCambiar = {
                apiKey = it
                if (campoMal == 5) campoMal = 0
            },
            pista = if (proveedor == 0) "AIza…" else "sk-…",
            mostrar = verKey,
            alAlternarMostrar = { verKey = !verKey },
            mal = campoMal == 5,
        )

        Text(
            text = "Pega aquí la clave que te da el proveedor.",
            fontSize = OrderTema.TamanoHora,
            color = OrderTema.TextoTenue,
        )

        HorizontalDivider(color = OrderTema.BordeSuave, thickness = OrderTema.Linea)

        Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs)) {
            Text(
                text = "Voz",
                fontSize = OrderTema.TamanoPequeno,
                letterSpacing = OrderTema.LetraNormal,
                color = OrderTema.TextoSuave,
            )
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs),
            ) {
                ChipOrder(
                    texto = "Android (gratis)",
                    activo = usarVozAndroid,
                    onClick = { usarVozAndroid = true },
                )
                proveedoresVoz.forEach { info ->
                    ChipOrder(
                        texto = info.nombre,
                        activo = !usarVozAndroid && vozProveedor == info.codigo,
                        onClick = {
                            usarVozAndroid = false
                            vozProveedor = info.codigo
                            vozUrlBase = info.urlBase
                            vozModelo = info.modelo
                        },
                    )
                }
            }
        }

        if (usarVozAndroid) {
            Text(
                text = "Usa el reconocimiento de voz del sistema (es-CO), sin clave ni costo.",
                fontSize = OrderTema.TamanoHora,
                color = OrderTema.TextoTenue,
            )
        } else {
            vozActual?.let { info ->
                BotonBorde(
                    texto = "Conseguir mi API key",
                    onClick = { abrirNavegador(context, info.urlClave) },
                )
            }

            CampoClaveAgente(
                etiqueta = "API key de voz",
                valor = vozApiKey,
                alCambiar = { vozApiKey = it },
                pista = if (vozProveedor == 1) "gsk_…" else "sk-…",
                mostrar = verVozKey,
                alAlternarMostrar = { verVozKey = !verVozKey },
                mal = false,
            )
        }

        HorizontalDivider(color = OrderTema.BordeSuave, thickness = OrderTema.Linea)

        BotonBorde(
            texto = if (mostrarAvanzado) "Avanzado ▾" else "Avanzado ▸",
            onClick = { avanzado = !avanzado },
        )

        if (mostrarAvanzado) {
            Text(
                text = "URL base y modelo, para escribirlos a mano.",
                fontSize = OrderTema.TamanoHora,
                color = OrderTema.TextoTenue,
            )
            CampoTextoAgente(
                etiqueta = "URL base del agente",
                valor = urlBase,
                alCambiar = {
                    urlBase = it
                    if (campoMal == 1) campoMal = 0
                },
                pista = "https://…",
                mal = campoMal == 1,
            )
            CampoTextoAgente(
                etiqueta = "Modelo del agente",
                valor = modelo,
                alCambiar = {
                    modelo = it
                    if (campoMal == 2) campoMal = 0
                },
                pista = "modelo",
                mal = campoMal == 2,
            )
            if (!usarVozAndroid) {
                CampoTextoAgente(
                    etiqueta = "URL base de la voz",
                    valor = vozUrlBase,
                    alCambiar = {
                        vozUrlBase = it
                        if (campoMal == 3) campoMal = 0
                    },
                    pista = "https://…",
                    mal = campoMal == 3,
                )
                CampoTextoAgente(
                    etiqueta = "Modelo de la voz",
                    valor = vozModelo,
                    alCambiar = {
                        vozModelo = it
                        if (campoMal == 4) campoMal = 0
                    },
                    pista = "modelo",
                    mal = campoMal == 4,
                )
            }
            CampoTextoAgente(
                etiqueta = "Límite de contexto (tokens)",
                valor = limiteContexto,
                alCambiar = {
                    limiteContexto = it
                    if (campoMal == 6) campoMal = 0
                },
                pista = "60000",
                mal = campoMal == 6,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            )
            if (proveedorActual?.urlClave == null) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { veImagenes = !veImagenes }
                        .padding(vertical = OrderTema.EspacioXs),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
                ) {
                    Checkbox(
                        checked = veImagenes,
                        onCheckedChange = { veImagenes = it },
                        colors = CheckboxDefaults.colors(
                            checkedColor = OrderTema.Acento,
                            uncheckedColor = OrderTema.BordeFuerte,
                            checkmarkColor = OrderTema.Fondo,
                        ),
                    )
                    Text(
                        text = "El modelo ve imágenes",
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.Texto,
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(OrderTema.EspacioXs))

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.End,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedButton(
                onClick = alVolver,
                shape = RoundedCornerShape(OrderTema.Radio),
                border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
            ) {
                Text(
                    text = "Cancelar",
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoSuave,
                )
            }
            Spacer(modifier = Modifier.padding(horizontal = OrderTema.EspacioXs))
            Button(
                onClick = {
                    if (!listo) return@Button
                    scope.launch {
                        val err = repositorio.guardarAgenteConfig(
                            proveedor = proveedor,
                            url = urlBase,
                            modelo = modelo,
                            apiKey = apiKey,
                            veImagenes = veImagenes,
                            limiteContexto = limiteContexto,
                            vozProveedor = vozProveedor,
                            vozUrl = vozUrlBase,
                            vozModelo = vozModelo,
                            vozKey = vozApiKey,
                        )
                        if (err == null) {
                            PreferenciasOrder.guardarUsarVozAndroid(context, usarVozAndroid)
                            alVolver()
                        } else {
                            if (err.campo in 1..4 || err.campo == 6) avanzado = true
                            campoMal = err.campo
                            errorMsg = err.mensaje
                        }
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
                    text = "Guardar",
                    fontFamily = OrderTema.FuenteTitulos,
                    fontSize = OrderTema.TamanoPequeno,
                )
            }
        }

        Spacer(modifier = Modifier.height(OrderTema.EspacioMd))
    }
}

@Composable
private fun CampoClaveAgente(
    etiqueta: String,
    valor: String,
    alCambiar: (String) -> Unit,
    pista: String,
    mostrar: Boolean,
    alAlternarMostrar: () -> Unit,
    mal: Boolean,
) {
    Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs)) {
        Text(
            text = etiqueta,
            fontSize = OrderTema.TamanoPequeno,
            letterSpacing = OrderTema.LetraNormal,
            color = if (mal) OrderTema.TextoError else OrderTema.TextoSuave,
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(OrderTema.EspacioSm),
        ) {
            OutlinedTextField(
                value = valor,
                onValueChange = alCambiar,
                placeholder = {
                    Text(
                        text = pista,
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.TextoTenue,
                    )
                },
                singleLine = true,
                visualTransformation = if (mostrar) VisualTransformation.None else PasswordVisualTransformation(),
                colors = OutlinedTextFieldDefaults.colors(
                    focusedContainerColor = OrderTema.FondoCampo,
                    unfocusedContainerColor = OrderTema.FondoCampo,
                    focusedBorderColor = if (mal) OrderTema.Vencida else OrderTema.AcentoSuave,
                    unfocusedBorderColor = if (mal) OrderTema.Vencida else OrderTema.BordeCampo,
                    focusedTextColor = OrderTema.Texto,
                    unfocusedTextColor = OrderTema.Texto,
                    cursorColor = OrderTema.Acento,
                ),
                modifier = Modifier.weight(1f),
            )
            BotonBorde(
                texto = if (mostrar) "Ocultar" else "Mostrar",
                onClick = alAlternarMostrar,
            )
        }
    }
}

@Composable
private fun CampoTextoAgente(
    etiqueta: String,
    valor: String,
    alCambiar: (String) -> Unit,
    pista: String,
    mal: Boolean,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
) {
    Column(verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioXs)) {
        Text(
            text = etiqueta,
            fontSize = OrderTema.TamanoPequeno,
            letterSpacing = OrderTema.LetraNormal,
            color = if (mal) OrderTema.TextoError else OrderTema.TextoSuave,
        )
        OutlinedTextField(
            value = valor,
            onValueChange = alCambiar,
            placeholder = {
                Text(
                    text = pista,
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoTenue,
                )
            },
            singleLine = true,
            keyboardOptions = keyboardOptions,
            colors = OutlinedTextFieldDefaults.colors(
                focusedContainerColor = OrderTema.FondoCampo,
                unfocusedContainerColor = OrderTema.FondoCampo,
                focusedBorderColor = if (mal) OrderTema.Vencida else OrderTema.AcentoSuave,
                unfocusedBorderColor = if (mal) OrderTema.Vencida else OrderTema.BordeCampo,
                focusedTextColor = OrderTema.Texto,
                unfocusedTextColor = OrderTema.Texto,
                cursorColor = OrderTema.Acento,
            ),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

private fun abrirNavegador(context: Context, url: String) {
    val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
    runCatching { context.startActivity(intent) }
}
