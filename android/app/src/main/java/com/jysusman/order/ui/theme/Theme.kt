package com.jysusman.order.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.jysusman.order.R

object OrderTema {
    // --- Tipografía --------------------------------------------------------
    val FuenteTitulos = FontFamily(
        Font(R.font.cinzel_regular, FontWeight.Normal),
        Font(R.font.cinzel_bold, FontWeight.Bold),
    )
    val FuenteTexto = FontFamily.SansSerif

    val TamanoTitulo = 25.sp
    val TamanoSemana = 17.sp
    val TamanoPestana = 14.sp
    val TamanoSeccion = 14.sp
    val TamanoDia = 13.sp
    val TamanoTexto = 14.sp
    val TamanoBloque = 12.sp
    val TamanoPequeno = 12.sp
    val TamanoHora = 11.sp
    val TamanoPie = 12.sp

    // --- Paleta: carbón cálido, tinta marfil y oro viejo -------------------
    val Fondo = Color(0xFF101014)
    val FondoPanel = Color(0xFF16161C)
    val FondoPanelAlto = Color(0xFF1C1C24)
    val FondoHover = Color(0xFF21212B)
    val FondoHoy = Color(0xFF17171F)
    val FondoClase = Color(0xFF1E1E27)
    val Borde = Color(0xFF2A2A35)
    val BordeSuave = Color(0xFF202029)
    val BordeFuerte = Color(0xFF3A3A48)
    val BordeClase = Color(0xFF33333F)
    val LineaHora = Color(0xFF1E1E26)

    val Texto = Color(0xFFE9E4D8)
    val TextoSuave = Color(0xFFA8A296)
    val TextoTenue = Color(0xFF6C685F)

    val Acento = Color(0xFFC7A96B)
    val AcentoSuave = Color(0xFF8E7645)
    val AcentoTenue = Color(0xFF2A2418)

    val Vencida = Color(0xFFC6756B)
    val VencidaTexto = Color(0xFFE0A49C)

    // --- Paleta de materias ------------------------------------------------
    val Materias = listOf(
        Color(0xFFC2A878), // oro viejo
        Color(0xFF9FB48A), // salvia
        Color(0xFF7FAE9F), // verde azulado
        Color(0xFF8FA6C0), // azul polvo
        Color(0xFFA89EC4), // lavanda
        Color(0xFFC39AA6), // rosa viejo
        Color(0xFFC9977F), // arcilla
        Color(0xFFB3A68C), // arena
        Color(0xFF94908A), // piedra
    )

    val MateriasFondo = listOf(
        Color(0xFF26211A),
        Color(0xFF1D241B),
        Color(0xFF192523),
        Color(0xFF1B222C),
        Color(0xFF22202C),
        Color(0xFF291F24),
        Color(0xFF291F1A),
        Color(0xFF23211C),
        Color(0xFF201F1D),
    )

    fun colorMateria(tono: Int): Color =
        Materias[tono.coerceIn(0, Materias.lastIndex)]

    fun fondoMateria(tono: Int): Color =
        MateriasFondo[tono.coerceIn(0, MateriasFondo.lastIndex)]

    // --- Avisos ------------------------------------------------------------
    val FondoError = Color(0xFF3A1D1B)
    val BordeError = Color(0xFF8A4238)
    val TextoError = Color(0xFFF2B6AD)
    val FondoAviso = Color(0xFF372E1B)
    val BordeAviso = Color(0xFF8A6F32)
    val TextoAviso = Color(0xFFEBD08C)

    // --- Formularios y velo ------------------------------------------------
    val FondoVelo = Color(0xCC08080C)
    val FondoCampo = Color(0xFF121218)
    val BordeCampo = Color(0xFF2E2E3A)

    // --- Inicio: franja de cabecera y tarjetas -----------------------------
    val DegradadoInicio = Brush.verticalGradient(
        colors = listOf(Color(0xFF20202B), Color(0xFF101014))
    )
    val Tarjeta = Color(0xFF17171F)
    val TarjetaBorde = Color(0xFF262631)
    val RadioTarjeta = 10.dp
    val RellenoTarjeta = 14.dp

    val AcentoHoy = Color(0xFFC2A878)
    val AcentoHoyTareas = Color(0xFF9FB48A)
    val AcentoVencidas = Color(0xFFC6756B)
    val AcentoProximas = Color(0xFF8FA6C0)
    val AcentoRecordatorios = Color(0xFFA89EC4)
    val AcentoMaterias = Color(0xFF7FAE9F)

    // --- Espaciado y radios ------------------------------------------------
    val Espacio2xs = 2.dp
    val EspacioXs = 4.dp
    val EspacioSm = 8.dp
    val EspacioMd = 12.dp
    val EspacioLg = 18.dp
    val EspacioXl = 26.dp

    val Radio = 6.dp
    val RadioPequeno = 4.dp
    val Linea = 1.dp
    val Trazo = 2.dp
    val AnchoMarca = 3.dp
    val AltoCheck = 20.dp
    val AltoHora = 48.dp
    val AltoPildora = 22.dp

    val LetraTitulo = 3.sp
    val LetraNormal = 1.sp
    val LetraSeccion = 2.sp
}

private val OrderColorScheme = darkColorScheme(
    primary = OrderTema.Acento,
    onPrimary = OrderTema.Fondo,
    primaryContainer = OrderTema.AcentoTenue,
    onPrimaryContainer = OrderTema.Acento,
    secondary = OrderTema.AcentoSuave,
    onSecondary = OrderTema.Texto,
    background = OrderTema.Fondo,
    onBackground = OrderTema.Texto,
    surface = OrderTema.FondoPanel,
    onSurface = OrderTema.Texto,
    surfaceVariant = OrderTema.FondoPanelAlto,
    onSurfaceVariant = OrderTema.TextoSuave,
    outline = OrderTema.Borde,
    outlineVariant = OrderTema.BordeSuave,
    error = OrderTema.Vencida,
    onError = OrderTema.Fondo,
    errorContainer = OrderTema.FondoError,
    onErrorContainer = OrderTema.TextoError,
)

private val OrderTypography = Typography(
    headlineMedium = TextStyle(
        fontFamily = OrderTema.FuenteTitulos,
        fontWeight = FontWeight.Bold,
        fontSize = OrderTema.TamanoTitulo,
        letterSpacing = OrderTema.LetraTitulo,
        color = OrderTema.Texto,
    ),
    titleMedium = TextStyle(
        fontFamily = OrderTema.FuenteTitulos,
        fontWeight = FontWeight.Bold,
        fontSize = OrderTema.TamanoSemana,
        letterSpacing = OrderTema.LetraNormal,
        color = OrderTema.Texto,
    ),
    titleSmall = TextStyle(
        fontFamily = OrderTema.FuenteTitulos,
        fontWeight = FontWeight.Bold,
        fontSize = OrderTema.TamanoSeccion,
        letterSpacing = OrderTema.LetraSeccion,
        color = OrderTema.TextoSuave,
    ),
    bodyMedium = TextStyle(
        fontFamily = OrderTema.FuenteTexto,
        fontSize = OrderTema.TamanoTexto,
        color = OrderTema.Texto,
    ),
    bodySmall = TextStyle(
        fontFamily = OrderTema.FuenteTexto,
        fontSize = OrderTema.TamanoPequeno,
        color = OrderTema.TextoSuave,
    ),
    labelSmall = TextStyle(
        fontFamily = OrderTema.FuenteTexto,
        fontSize = OrderTema.TamanoHora,
        color = OrderTema.TextoTenue,
    ),
)

@Composable
fun OrderTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = OrderColorScheme,
        typography = OrderTypography,
        content = content,
    )
}
