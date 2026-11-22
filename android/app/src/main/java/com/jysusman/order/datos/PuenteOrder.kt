package com.jysusman.order.datos

import com.jysusman.order.ffi.AccionVistaFfi
import com.jysusman.order.ffi.AgenteConfigFfi
import com.jysusman.order.ffi.AjustesFormFfi
import com.jysusman.order.ffi.AnioMesFfi
import com.jysusman.order.ffi.AvisoFfi
import com.jysusman.order.ffi.AvisoProgramableFfi
import com.jysusman.order.ffi.CalendarioMesFfi
import com.jysusman.order.ffi.ClaseFormFfi
import com.jysusman.order.ffi.ConflictoFfi
import com.jysusman.order.ffi.DiaFfi
import com.jysusman.order.ffi.DiaVistaFfi
import com.jysusman.order.ffi.FiltroPendientesFfi
import com.jysusman.order.ffi.Hora12Ffi
import com.jysusman.order.ffi.InicioVistaFfi
import com.jysusman.order.ffi.MateriaConTonoFfi
import com.jysusman.order.ffi.PendientesVistaFfi
import com.jysusman.order.ffi.ProveedorInfoFfi
import com.jysusman.order.ffi.RecordatorioFormFfi
import com.jysusman.order.ffi.RespuestaAgenteFfi
import com.jysusman.order.ffi.ResultadoAplicarTodasFfi
import com.jysusman.order.ffi.ResumenArchivoFfi
import com.jysusman.order.ffi.SemanaVistaFfi
import com.jysusman.order.ffi.TareaFormFfi
import com.jysusman.order.ffi.VozProveedorInfoFfi

/**
 * Abstracción del puente FFI hacia `order-core` para poder probar `Repositorio`
 * en JVM sin cargar la librería nativa `.so`.
 */
interface PuenteOrder {
    fun rutaPorDefecto(): String
    fun carpetaPorDefecto(): String
    fun cargar(ruta: String): ResumenArchivoFfi
    fun archivar(ruta: String): ResumenArchivoFfi
    fun detectarConflictos(ruta: String): List<ConflictoFfi>
    fun resolverConflicto(ruta: String, rutaConflicto: String, usarOtra: Boolean)

    fun vistaInicio(ruta: String): InicioVistaFfi
    fun vistaDia(ruta: String, fechaIso: String?): DiaVistaFfi
    fun vistaSemana(ruta: String, lunesIso: String?): SemanaVistaFfi
    fun vistaPendientes(ruta: String, filtro: FiltroPendientesFfi): PendientesVistaFfi
    fun materiasConTono(ruta: String): List<MateriaConTonoFfi>
    fun canonicaMateria(ruta: String, texto: String): String

    fun obtenerTareaForm(ruta: String, id: String?): TareaFormFfi
    fun obtenerClaseForm(ruta: String, id: String?): ClaseFormFfi
    fun obtenerRecordatorioForm(ruta: String, id: String?): RecordatorioFormFfi
    fun obtenerAjustesForm(ruta: String): AjustesFormFfi

    fun guardarTarea(
        ruta: String,
        id: String?,
        titulo: String,
        fechaIso: String,
        hora: String,
        materia: String,
        hecha: Boolean,
    ): ResumenArchivoFfi

    fun marcarTarea(ruta: String, id: String, hecha: Boolean): ResumenArchivoFfi
    fun borrarTarea(ruta: String, id: String): ResumenArchivoFfi

    fun guardarClase(
        ruta: String,
        id: String?,
        materia: String,
        dias: List<DiaFfi>,
        horaInicio: String,
        horaFin: String,
        lugar: String,
    ): ResumenArchivoFfi

    fun borrarClase(ruta: String, id: String): ResumenArchivoFfi

    fun guardarRecordatorio(
        ruta: String,
        id: String?,
        texto: String,
        hora: String,
        dias: List<DiaFfi>,
    ): ResumenArchivoFfi

    fun borrarRecordatorio(ruta: String, id: String): ResumenArchivoFfi

    fun guardarAjustes(
        ruta: String,
        sinSemestre: Boolean,
        inicioIso: String,
        finIso: String,
        horaAviso: String,
        horasAntes: String,
    ): ResumenArchivoFfi

    fun fechaHoyIso(): String
    fun lunesActualIso(): String
    fun sumarDiasIso(fechaIso: String, delta: Int): String
    fun calendarioMes(anio: Int, mes: Int, elegido: Int): CalendarioMesFfi
    fun mesVecino(anio: Int, mes: Int, delta: Int): AnioMesFfi
    fun diasDelMes(anio: Int, mes: Int): Int
    fun horas12Texto(): List<String>
    fun minutos60Texto(): List<String>
    fun formatearHora12(hora12: Int, minuto: Int, tarde: Boolean): String
    fun parsearHora12(texto: String): Hora12Ffi
    fun diasSemanaEtiquetas(): List<String>

    fun proximosAvisos(
        ruta: String,
        desdeIso: String,
        hastaIso: String,
        registroRuta: String,
    ): List<AvisoProgramableFfi>

    fun anotarAviso(registroRuta: String, clave: String)
    fun resumenDelDia(ruta: String, ahoraIso: String, registroRuta: String): AvisoFfi?

    fun proveedoresAgente(): List<ProveedorInfoFfi>
    fun proveedoresVoz(): List<VozProveedorInfoFfi>
    fun leerAgenteConfig(rutaConfig: String): AgenteConfigFfi
    fun guardarAgenteConfig(
        rutaConfig: String,
        proveedor: Int,
        url: String,
        modelo: String,
        apiKey: String,
        vozProveedor: Int,
        vozUrl: String,
        vozModelo: String,
        vozKey: String,
    ): AgenteConfigFfi

    fun enviarAlAgente(rutaDatos: String, rutaConfig: String, texto: String): RespuestaAgenteFfi
    fun aplicarAccion(rutaDatos: String, accionId: String): ResumenArchivoFfi
    fun descartarAccion(accionId: String)
    fun aplicarTodas(rutaDatos: String, accionIds: List<String>): ResultadoAplicarTodasFfi

    fun wavDePcm16(muestras: List<Short>, frecuencia: UInt): ByteArray
    fun transcribir(rutaConfig: String, wav: ByteArray): String
}

class PuenteOrderNativo : PuenteOrder {
    override fun rutaPorDefecto(): String = com.jysusman.order.ffi.rutaPorDefecto()
    override fun carpetaPorDefecto(): String = com.jysusman.order.ffi.carpetaPorDefecto()
    override fun cargar(ruta: String): ResumenArchivoFfi = com.jysusman.order.ffi.cargar(ruta)
    override fun archivar(ruta: String): ResumenArchivoFfi = com.jysusman.order.ffi.archivar(ruta)
    override fun detectarConflictos(ruta: String): List<ConflictoFfi> =
        com.jysusman.order.ffi.detectarConflictos(ruta)

    override fun resolverConflicto(ruta: String, rutaConflicto: String, usarOtra: Boolean) =
        com.jysusman.order.ffi.resolverConflicto(ruta, rutaConflicto, usarOtra)

    override fun vistaInicio(ruta: String): InicioVistaFfi =
        com.jysusman.order.ffi.vistaInicio(ruta)

    override fun vistaDia(ruta: String, fechaIso: String?): DiaVistaFfi =
        com.jysusman.order.ffi.vistaDia(ruta, fechaIso)

    override fun vistaSemana(ruta: String, lunesIso: String?): SemanaVistaFfi =
        com.jysusman.order.ffi.vistaSemana(ruta, lunesIso)

    override fun vistaPendientes(ruta: String, filtro: FiltroPendientesFfi): PendientesVistaFfi =
        com.jysusman.order.ffi.vistaPendientes(ruta, filtro)

    override fun materiasConTono(ruta: String): List<MateriaConTonoFfi> =
        com.jysusman.order.ffi.materiasConTono(ruta)

    override fun canonicaMateria(ruta: String, texto: String): String =
        com.jysusman.order.ffi.canonicaMateria(ruta, texto)

    override fun obtenerTareaForm(ruta: String, id: String?): TareaFormFfi =
        com.jysusman.order.ffi.obtenerTareaForm(ruta, id)

    override fun obtenerClaseForm(ruta: String, id: String?): ClaseFormFfi =
        com.jysusman.order.ffi.obtenerClaseForm(ruta, id)

    override fun obtenerRecordatorioForm(ruta: String, id: String?): RecordatorioFormFfi =
        com.jysusman.order.ffi.obtenerRecordatorioForm(ruta, id)

    override fun obtenerAjustesForm(ruta: String): AjustesFormFfi =
        com.jysusman.order.ffi.obtenerAjustesForm(ruta)

    override fun guardarTarea(
        ruta: String,
        id: String?,
        titulo: String,
        fechaIso: String,
        hora: String,
        materia: String,
        hecha: Boolean,
    ): ResumenArchivoFfi =
        com.jysusman.order.ffi.guardarTarea(ruta, id, titulo, fechaIso, hora, materia, hecha)

    override fun marcarTarea(ruta: String, id: String, hecha: Boolean): ResumenArchivoFfi =
        com.jysusman.order.ffi.marcarTarea(ruta, id, hecha)

    override fun borrarTarea(ruta: String, id: String): ResumenArchivoFfi =
        com.jysusman.order.ffi.borrarTarea(ruta, id)

    override fun guardarClase(
        ruta: String,
        id: String?,
        materia: String,
        dias: List<DiaFfi>,
        horaInicio: String,
        horaFin: String,
        lugar: String,
    ): ResumenArchivoFfi =
        com.jysusman.order.ffi.guardarClase(ruta, id, materia, dias, horaInicio, horaFin, lugar)

    override fun borrarClase(ruta: String, id: String): ResumenArchivoFfi =
        com.jysusman.order.ffi.borrarClase(ruta, id)

    override fun guardarRecordatorio(
        ruta: String,
        id: String?,
        texto: String,
        hora: String,
        dias: List<DiaFfi>,
    ): ResumenArchivoFfi =
        com.jysusman.order.ffi.guardarRecordatorio(ruta, id, texto, hora, dias)

    override fun borrarRecordatorio(ruta: String, id: String): ResumenArchivoFfi =
        com.jysusman.order.ffi.borrarRecordatorio(ruta, id)

    override fun guardarAjustes(
        ruta: String,
        sinSemestre: Boolean,
        inicioIso: String,
        finIso: String,
        horaAviso: String,
        horasAntes: String,
    ): ResumenArchivoFfi =
        com.jysusman.order.ffi.guardarAjustes(
            ruta,
            sinSemestre,
            inicioIso,
            finIso,
            horaAviso,
            horasAntes,
        )

    override fun fechaHoyIso(): String = com.jysusman.order.ffi.fechaHoyIso()
    override fun lunesActualIso(): String = com.jysusman.order.ffi.lunesActualIso()
    override fun sumarDiasIso(fechaIso: String, delta: Int): String =
        com.jysusman.order.ffi.sumarDiasIso(fechaIso, delta)

    override fun calendarioMes(anio: Int, mes: Int, elegido: Int): CalendarioMesFfi =
        com.jysusman.order.ffi.calendarioMes(anio, mes.toUInt(), elegido.toUInt())

    override fun mesVecino(anio: Int, mes: Int, delta: Int): AnioMesFfi =
        com.jysusman.order.ffi.mesVecino(anio, mes.toUInt(), delta)

    override fun diasDelMes(anio: Int, mes: Int): Int =
        com.jysusman.order.ffi.diasDelMes(anio, mes.toUInt())

    override fun horas12Texto(): List<String> = com.jysusman.order.ffi.horas12Texto()
    override fun minutos60Texto(): List<String> = com.jysusman.order.ffi.minutos60Texto()
    override fun formatearHora12(hora12: Int, minuto: Int, tarde: Boolean): String =
        com.jysusman.order.ffi.formatearHora12(hora12.toUInt(), minuto.toUInt(), tarde)

    override fun parsearHora12(texto: String): Hora12Ffi =
        com.jysusman.order.ffi.parsearHora12(texto)

    override fun diasSemanaEtiquetas(): List<String> =
        com.jysusman.order.ffi.diasSemanaEtiquetas()

    override fun proximosAvisos(
        ruta: String,
        desdeIso: String,
        hastaIso: String,
        registroRuta: String,
    ): List<AvisoProgramableFfi> =
        com.jysusman.order.ffi.proximosAvisos(ruta, desdeIso, hastaIso, registroRuta)

    override fun anotarAviso(registroRuta: String, clave: String) =
        com.jysusman.order.ffi.anotarAviso(registroRuta, clave)

    override fun resumenDelDia(ruta: String, ahoraIso: String, registroRuta: String): AvisoFfi? =
        com.jysusman.order.ffi.resumenDelDia(ruta, ahoraIso, registroRuta)

    override fun proveedoresAgente(): List<ProveedorInfoFfi> =
        com.jysusman.order.ffi.proveedoresAgente()

    override fun proveedoresVoz(): List<VozProveedorInfoFfi> =
        com.jysusman.order.ffi.proveedoresVoz()

    override fun leerAgenteConfig(rutaConfig: String): AgenteConfigFfi =
        com.jysusman.order.ffi.leerAgenteConfig(rutaConfig)

    override fun guardarAgenteConfig(
        rutaConfig: String,
        proveedor: Int,
        url: String,
        modelo: String,
        apiKey: String,
        vozProveedor: Int,
        vozUrl: String,
        vozModelo: String,
        vozKey: String,
    ): AgenteConfigFfi = com.jysusman.order.ffi.guardarAgenteConfig(
        rutaConfig,
        proveedor,
        url,
        modelo,
        apiKey,
        vozProveedor,
        vozUrl,
        vozModelo,
        vozKey,
    )

    override fun enviarAlAgente(
        rutaDatos: String,
        rutaConfig: String,
        texto: String,
    ): RespuestaAgenteFfi = com.jysusman.order.ffi.enviarAlAgente(rutaDatos, rutaConfig, texto)

    override fun aplicarAccion(rutaDatos: String, accionId: String): ResumenArchivoFfi =
        com.jysusman.order.ffi.aplicarAccion(rutaDatos, accionId)

    override fun descartarAccion(accionId: String) =
        com.jysusman.order.ffi.descartarAccion(accionId)

    override fun aplicarTodas(
        rutaDatos: String,
        accionIds: List<String>,
    ): ResultadoAplicarTodasFfi = com.jysusman.order.ffi.aplicarTodas(rutaDatos, accionIds)

    override fun wavDePcm16(muestras: List<Short>, frecuencia: UInt): ByteArray =
        com.jysusman.order.ffi.wavDePcm16(muestras, frecuencia)

    override fun transcribir(rutaConfig: String, wav: ByteArray): String =
        com.jysusman.order.ffi.transcribir(rutaConfig, wav)
}
