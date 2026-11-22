package com.jysusman.order.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.provider.Settings
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import com.jysusman.order.ui.theme.OrderTema

fun tienePermisoArchivos(): Boolean = Environment.isExternalStorageManager()

fun abrirAjustesPermisoArchivos(context: Context) {
    val especifico = Intent(
        Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
        Uri.parse("package:${context.packageName}"),
    )
    val general = Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION)
    runCatching { context.startActivity(especifico) }
        .onFailure { context.startActivity(general) }
}

@Composable
fun PantallaPermisoArchivos(
    carpeta: String,
    onAbrirAjustes: () -> Unit,
    onReintentar: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .fillMaxSize()
            .background(OrderTema.Fondo)
            .padding(OrderTema.EspacioXl),
        contentAlignment = Alignment.Center,
    ) {
        Surface(
            color = OrderTema.Tarjeta,
            shape = RoundedCornerShape(OrderTema.RadioTarjeta),
            border = BorderStroke(OrderTema.Linea, OrderTema.TarjetaBorde),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(OrderTema.EspacioLg),
                verticalArrangement = Arrangement.spacedBy(OrderTema.EspacioMd),
            ) {
                Text(
                    text = "PERMISO DE ARCHIVOS",
                    fontFamily = OrderTema.FuenteTitulos,
                    fontWeight = FontWeight.Bold,
                    fontSize = OrderTema.TamanoSeccion,
                    letterSpacing = OrderTema.LetraSeccion,
                    color = OrderTema.Acento,
                )
                Text(
                    text = "ORDER lee y guarda el mismo order.toml que sincroniza Syncthing.",
                    fontSize = OrderTema.TamanoTexto,
                    color = OrderTema.Texto,
                )
                Text(
                    text = "Carpeta: $carpeta\n\nActiva «Acceso a todos los archivos» para abrir o crear el archivo en esa carpeta.",
                    fontSize = OrderTema.TamanoPequeno,
                    color = OrderTema.TextoSuave,
                )
                Spacer(modifier = Modifier.height(OrderTema.EspacioXs))
                Button(
                    onClick = onAbrirAjustes,
                    shape = RoundedCornerShape(OrderTema.Radio),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = OrderTema.AcentoTenue,
                        contentColor = OrderTema.Acento,
                    ),
                    border = BorderStroke(OrderTema.Linea, OrderTema.AcentoSuave),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        text = "Abrir ajustes del permiso",
                        fontFamily = OrderTema.FuenteTitulos,
                        fontSize = OrderTema.TamanoPequeno,
                    )
                }
                OutlinedButton(
                    onClick = onReintentar,
                    shape = RoundedCornerShape(OrderTema.Radio),
                    border = BorderStroke(OrderTema.Linea, OrderTema.Borde),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        text = "Ya lo activé, continuar",
                        fontSize = OrderTema.TamanoPequeno,
                        color = OrderTema.TextoSuave,
                    )
                }
            }
        }
    }
}
