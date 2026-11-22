# ORDER — Plan de implementación

> **Para agentes:** cada tarea es un encargo independiente. Leer este plan **y** el diseño. Pasos con casillas (`- [ ]`). Commits sin líneas de coautoría.

**Objetivo:** binario Rust `order` que muestra horario semanal, tareas y recordatorios desde un TOML, con icono en la bandeja de DMS y notificaciones de escritorio.

**Arquitectura:** un solo proceso. Hilo principal = bucle de eventos de Slint. Hilos secundarios (bandeja, observador del archivo, reloj de avisos) se comunican con la UI vía `slint::invoke_from_event_loop`. Toda la lógica (datos, semana, avisos) es Rust puro en `src/` y se prueba sin UI.

**Stack:** Rust (cargo 1.100), `slint` 1.x, `serde` + `toml` (lectura), `toml_edit` (escritura preservando comentarios), `chrono`, `dirs`, `ksni` (bandeja SNI, modo bloqueante), `notify-rust` 4 (notificaciones), `notify` (observador de archivos), `tempfile` (solo en tests).

**Diseño:** `docs/2026-11-21-order-diseño.md`

## Restricciones globales

- Un solo binario y un solo proceso. Sin servidor, sin puerto TCP, sin base de datos, sin navegador.
- Sin red para el funcionamiento básico.
- Datos: `~/.config/order/order.toml`; historial: `~/.config/order/historial.toml`; registro de avisos: `~/.local/state/order/enviados.txt`.
- Días en el TOML: `lun mar mie jue vie sab dom`. Horas `HH:MM`. Fechas `YYYY-MM-DD`. Vencimiento `YYYY-MM-DDTHH:MM` (hora local).
- Las hechas con más de 30 días pasan a `historial.toml`.
- Fuente Cinzel (OFL) incrustada; colores y tamaños solo en `ui/tema.slint`.
- Arranca oculto (solo icono). Instancia única. Notificaciones sin botones.
- Todo lo del archivo (tareas, clases, semestre, recordatorios, ajustes) se crea, edita y borra desde la ventana.
- Toda creación de tarea pasa por `ops::agregar_tarea` (el futuro cuadro de agente la reutiliza).

## Puntos de revisión

1. **TOML roto a mano o por Hermes** → la app conserva el último estado válido, muestra la línea del error y **nunca** sobreescribe el archivo roto. (Tarea 2: `no_sobrescribe_archivo_roto`)
2. **Hermes edita el archivo mientras la ventana tiene un formulario abierto** → al guardar se relee el disco y el cambio de Hermes sobrevive. (Tarea 2: `modificar_relee_disco`)
3. **Tarea creada con vencimiento ya pasado o a menos de N horas** → no dispara avisos atrasados en ráfaga; aparece resaltada como vencida. (Tarea 5: `no_avisa_momentos_pasados`)
4. **Semana que cruza fin de semestre o cambio de año** → solo aparecen clases en fechas dentro de `[semestre]`. (Tarea 4: `semana_respeta_semestre`, `semana_cruza_anio`)
5. **Elementos escritos por Hermes sin `id`** → la app les asigna `id` y siguen siendo editables desde la ventana. (Tarea 2: `asigna_ids_faltantes`)

---

## Estructura de archivos

| Archivo | Responsabilidad |
|---|---|
| `src/main.rs` | arranque: instancia única, hilos, bucle Slint |
| `src/modelo.rs` | tipos de datos y deserialización |
| `src/almacen.rs` | leer/escribir el TOML de forma segura |
| `src/ops.rs` | crear/editar/borrar/marcar sobre el documento; archivar |
| `src/semana.rs` | cálculo de la vista Semana y lista de Pendientes |
| `src/avisos.rs` | qué avisar y cuándo; registro de enviados |
| `src/sistema.rs` | bandeja, notificaciones, observador, socket de instancia única |
| `src/puente.rs` | convierte datos ↔ modelos de Slint y conecta callbacks |
| `ui/tema.slint` | colores, tamaños, fuente |
| `ui/app.slint` | ventana principal, pestañas, banner de error |
| `ui/semana.slint`, `ui/pendientes.slint` | vistas |
| `ui/formularios.slint` | formularios de tarea, clase, recordatorio, semestre, ajustes; confirmación de borrado |
| `ui/fuentes/Cinzel.ttf`, `ui/fuentes/OFL.txt` | fuente y licencia |
| `ejemplo.toml` | archivo inicial comentado (incluido con `include_str!`) |

---

### Tarea 1: Proyecto y modelo de datos

**Archivos:** crear `Cargo.toml`, `src/main.rs` (mínimo), `src/modelo.rs`, `ejemplo.toml`, `.gitignore`. Pruebas en `src/modelo.rs` (`#[cfg(test)]`).

**Interfaces producidas:**
```rust
pub enum Dia { Lun, Mar, Mie, Jue, Vie, Sab, Dom }   // serde: "lun".."dom"
pub struct Ajustes { pub aviso_dia_antes: NaiveTime, pub aviso_horas_antes: u32 }
pub struct Semestre { pub inicio: NaiveDate, pub fin: NaiveDate }
pub struct Clase { pub id: String, pub materia: String, pub dia: Dia, pub inicio: NaiveTime, pub fin: NaiveTime, pub lugar: Option<String> }
pub struct Tarea { pub id: String, pub titulo: String, pub materia: Option<String>, pub vence: NaiveDateTime, pub hecha: bool, pub hecha_en: Option<NaiveDate> }
pub struct Recordatorio { pub id: String, pub texto: String, pub dias: Vec<Dia>, pub hora: NaiveTime }
pub struct Datos { pub ajustes: Ajustes, pub semestre: Option<Semestre>, pub clases: Vec<Clase>, pub tareas: Vec<Tarea>, pub recordatorios: Vec<Recordatorio> }
pub struct ErrorDatos { pub linea: Option<usize>, pub mensaje: String }
pub fn parsear(texto: &str) -> Result<Datos, ErrorDatos>
```
Claves TOML: `[ajustes]`, `[semestre]`, `[[clase]]`, `[[tarea]]`, `[[recordatorio]]`. `id` es opcional al leer (vacío si falta). Valores por defecto de ajustes: `20:00` y `3`.

- [ ] `cargo new order` en `~/apps/order` (ya existe `docs/`), `git init`.
- [ ] Escribir `ejemplo.toml` con la forma del diseño §6.2 más `[semestre]`, comentado en español.
- [ ] Pruebas que fallan:
  - `parsea_ejemplo`: `parsear(include_str!("../ejemplo.toml"))` es `Ok` y tiene ≥1 clase, ≥1 tarea, ≥1 recordatorio.
  - `error_con_linea`: texto con `hora = 25:99` en la línea 4 → `Err(e)` con `e.linea == Some(4)`.
  - `ajustes_por_defecto`: texto sin `[ajustes]` → `aviso_horas_antes == 3`.
  - `id_faltante_es_vacio`: `[[tarea]]` sin `id` → `tareas[0].id == ""`.
- [ ] Implementar `modelo.rs`. `cargo test` → pasa.
- [ ] Commit `feat: modelo de datos`.

### Tarea 1b: Advertencias por claves desconocidas

**Archivos:** modificar `src/modelo.rs`.

**Interfaces producidas:** `pub fn advertencias(texto: &str) -> Vec<ErrorDatos>` (no cambia `parsear`). Devuelve una entrada por cada clave que el modelo no conoce, con línea y mensaje del tipo `clave desconocida "titlo" en [[tarea]]`. Las advertencias **no bloquean**: los datos se cargan igual. La UI (Tarea 6) las muestra en un banner amarillo, distinto del rojo de error.

- [ ] Pruebas que fallan: `advierte_clave_mal_escrita` (`titlo = "x"` en la línea 3 de un `[[tarea]]` → 1 advertencia con `linea == Some(3)` y el mensaje contiene `titlo`); `ejemplo_sin_advertencias` (`advertencias(include_str!("../ejemplo.toml"))` está vacío).
- [ ] Implementar (p. ej. con `serde_ignored` o comparando contra las claves conocidas). `cargo test` → pasa.
- [ ] Commit `feat: advertencias por claves desconocidas`.

### Tarea 2: Almacén (lectura/escritura segura)

**Archivos:** crear `src/almacen.rs`.

**Interfaces producidas:**
```rust
pub struct Almacen { pub ruta: PathBuf }
impl Almacen {
    pub fn por_defecto() -> Almacen                       // ~/.config/order/order.toml
    pub fn cargar(&self) -> Result<Datos, ErrorDatos>     // crea el archivo desde ejemplo.toml si no existe
    pub fn modificar<F>(&self, f: F) -> Result<Datos, ErrorDatos>
        where F: FnOnce(&mut toml_edit::DocumentMut) -> Result<(), String>
}
pub fn nuevo_id(prefijo: char) -> String                  // p. ej. "t-20261121-a3f9"
```
`modificar`: relee el disco, parsea a `DocumentMut`, **si el disco no parsea devuelve `Err` sin escribir**, asigna `id` a elementos que no lo tienen (`t-`, `c-`, `r-`), aplica `f`, valida con `modelo::parsear`, escribe atómico (temporal en el mismo directorio + `rename`). `cargar` también asigna ids faltantes (vía `modificar` con cierre vacío) si encuentra alguno.

- [ ] Pruebas que fallan (cada una con un directorio temporal):
  - `crea_desde_ejemplo`: ruta inexistente → `cargar()` es `Ok` y el archivo existe.
  - `conserva_comentarios`: archivo con `# mi nota` → tras `modificar` que cambia `aviso_horas_antes`, el texto sigue conteniendo `# mi nota`.
  - `modificar_relee_disco`: cargar; escribir externamente una tarea nueva en el archivo; `modificar` que marca otra tarea → el resultado contiene ambas.
  - `no_sobrescribe_archivo_roto`: archivo inválido → `modificar` es `Err` y el contenido en disco es idéntico al de antes.
  - `asigna_ids_faltantes`: `[[clase]]` sin `id` → tras `cargar`, `clases[0].id` empieza por `c-` y está escrito en disco.
- [ ] Implementar. `cargo test` → pasa.
- [ ] Commit `feat: almacén con escritura atómica`.

### Tarea 3: Operaciones de edición y archivo

**Archivos:** crear `src/ops.rs`.

**Interfaces consumidas:** `Almacen::modificar`, tipos de `modelo`.
**Interfaces producidas** (todas operan sobre `&mut DocumentMut` y devuelven `Result<(), String>`; error si el `id` no existe):
```rust
pub struct NuevaTarea { pub titulo: String, pub materia: Option<String>, pub vence: NaiveDateTime }
pub fn agregar_tarea(doc, t: &NuevaTarea) -> Result<String, String>   // devuelve el id
pub fn editar_tarea(doc, id: &str, t: &NuevaTarea)
pub fn marcar_tarea(doc, id: &str, hecha: bool, hoy: NaiveDate)      // pone/quita hecha_en
pub fn borrar_tarea(doc, id: &str)
pub fn guardar_clase(doc, c: &Clase)              // id vacío = crear, si no = reemplazar
pub fn borrar_clase(doc, id: &str)
pub fn guardar_recordatorio(doc, r: &Recordatorio) // id vacío = crear
pub fn borrar_recordatorio(doc, id: &str)
pub fn guardar_semestre(doc, s: Option<&Semestre>)
pub fn guardar_ajustes(doc, a: &Ajustes)
pub fn archivar_hechas(doc, historial: &mut DocumentMut, hoy: NaiveDate) -> usize  // > 30 días
```
Editar = reemplazar campos dentro de la tabla existente (no recrearla) para conservar comentarios cercanos.

- [ ] Pruebas que fallan:
  - `agregar_y_editar_tarea`: agregar → editar título → `parsear` muestra el título nuevo y mismo id.
  - `marcar_y_desmarcar`: `marcar(true, 2026-11-21)` → `hecha_en == Some(2026-11-21)`; `marcar(false, …)` → `hecha == false`, `hecha_en == None`.
  - `borrar_id_inexistente_falla`: `borrar_tarea(doc, "x")` es `Err`.
  - `crud_clase_y_recordatorio`: crear, reemplazar y borrar cada uno; la cuenta final es la inicial.
  - `archiva_solo_mayores_de_30`: hechas el 2026-10-01 y el 2026-11-15, hoy 2026-11-21 → devuelve 1, la de octubre está en `historial` y no en `doc`.
- [ ] Implementar. `cargo test` → pasa.
- [ ] Commit `feat: operaciones de edición`.

### Tarea 4: Vista Semana y Pendientes (lógica)

**Archivos:** crear `src/semana.rs`.

**Interfaces producidas:**
```rust
pub struct Bloque { pub clase_id: String, pub materia: String, pub inicio: NaiveTime, pub fin: NaiveTime, pub lugar: Option<String> }
pub struct DiaVista { pub fecha: NaiveDate, pub bloques: Vec<Bloque>, pub tareas: Vec<Tarea> }   // tareas que vencen ese día
pub fn lunes_de(fecha: NaiveDate) -> NaiveDate
pub fn semana(datos: &Datos, lunes: NaiveDate) -> Vec<DiaVista>   // lun–vie; + sáb/dom solo si hay clases esos días
pub fn pendientes(datos: &Datos) -> Vec<Tarea>                     // no hechas, por `vence` ascendente
pub fn vencida(t: &Tarea, ahora: NaiveDateTime) -> bool
```
Sin `[semestre]` → las clases aparecen todas las semanas. Con semestre → solo fechas en `[inicio, fin]`.

- [ ] Pruebas que fallan:
  - `semana_respeta_semestre`: semestre termina el viernes 2026-12-04 (jueves 3) → en la semana del 2026-11-30 aparece la clase del lunes y en la del 2026-12-07 no.
  - `semana_cruza_anio`: `lunes_de(2027-01-01) == 2026-12-28`; `semana` devuelve fechas del 28-dic al 1-ene.
  - `fin_de_semana_solo_si_hay_clases`: sin clases sáb/dom → 5 días; con una clase `sab` → 6.
  - `pendientes_ordenados_sin_hechas`.
- [ ] Implementar. `cargo test` → pasa.
- [ ] Commit `feat: cálculo de semana y pendientes`.

### Tarea 5: Avisos

**Archivos:** crear `src/avisos.rs`.

**Interfaces producidas:**
```rust
pub struct Aviso { pub clave: String, pub titulo: String, pub cuerpo: String }
pub fn avisos_debidos(datos: &Datos, ahora: NaiveDateTime, enviados: &HashSet<String>) -> Vec<Aviso>
pub fn resumen_arranque(datos: &Datos, ahora: NaiveDateTime) -> Option<Aviso>
pub struct Registro { pub ruta: PathBuf }   // ~/.local/state/order/enviados.txt, una clave por línea
impl Registro { pub fn leer(&self) -> HashSet<String>; pub fn anotar(&self, clave: &str); pub fn podar(&self, antes_de: NaiveDate) }
```
Claves: `t:{id}:dia:{fecha de vence}`, `t:{id}:horas:{fecha de vence}`, `r:{id}:{YYYY-MM-DD}` (todas terminan en fecha para poder podar; si cambia el vencimiento, la clave cambia y se vuelve a avisar). Un momento es "debido" si `momento <= ahora < momento + 15 min` y su clave no está en `enviados`. `resumen_arranque`: si hay tareas vencidas o que vencen hoy → un aviso "Tienes N tareas vencidas o por vencer hoy"; si no, `None`.

- [ ] Pruebas que fallan:
  - `aviso_dia_antes`: tarea vence 2026-11-28T23:59, `ahora` = 2026-11-27T20:05 → un aviso con clave `t:{id}:dia:2026-11-28`.
  - `aviso_horas_antes`: `ahora` = 2026-11-28T20:59 → clave `t:{id}:horas:2026-11-28`.
  - `no_repite`: misma llamada con la clave en `enviados` → vacío.
  - `no_avisa_momentos_pasados`: `ahora` = 2026-11-28T22:00 (pasó la ventana de 15 min de ambos) → vacío.
  - `recordatorio_por_dia`: recordatorio `lun` 18:00, `ahora` lunes 18:03 → aviso; martes 18:03 → vacío.
  - `ignora_hechas`.
- [ ] Implementar. `cargo test` → pasa.
- [ ] Commit `feat: cálculo de avisos`.

### Tarea 6: Ventana — tema, Semana y Pendientes

**Archivos:** crear `ui/tema.slint`, `ui/app.slint`, `ui/semana.slint`, `ui/pendientes.slint`, `ui/fuentes/`, `build.rs`, `src/puente.rs`; modificar `src/main.rs`.

**Interfaces consumidas:** `Almacen::cargar`, `semana::{semana, pendientes, lunes_de, vencida}`.
**Interfaces producidas:** `puente::mostrar_ventana()` crea la ventana la primera vez y la muestra; `puente::refrescar(datos: &Datos)` actualiza los modelos; `puente::mostrar_error(e: Option<&ErrorDatos>)` controla el banner rojo; `puente::mostrar_advertencias(a: &[ErrorDatos])` controla el banner amarillo (Tarea 1b).

- [ ] Descargar Cinzel (Google Fonts, OFL) a `ui/fuentes/` con su `OFL.txt`; importarla en `tema.slint`.
- [ ] `tema.slint`: global `Tema` con colores (tema oscuro), espaciados, tamaños de letra y familia. Ningún color fuera de este archivo.
- [ ] `app.slint`: título "ORDER", pestañas Semana/Pendientes, botón "+ Tarea", barra inferior "Hoy: N tareas pendientes · próximo recordatorio", banner de error rojo con el mensaje y la línea.
- [ ] `semana.slint`: flechas ‹ ›, botón "Hoy", rótulo de la semana, columnas por día con bloques de clase posicionados por hora, marcas de tareas que vencen, línea de "ahora" si es la semana actual.
- [ ] `pendientes.slint`: lista con casilla de hecha, título, materia, fecha; vencidas resaltadas.
- [ ] Comprobación manual: `cargo run` con `ejemplo.toml` → la ventana muestra el ejemplo; ‹ › cambian de semana; "Hoy" vuelve; romper el TOML a mano → banner rojo con la línea y la vista anterior intacta (la recarga en vivo llega en la Tarea 8; aquí basta con relanzar).
- [ ] Commit `feat: ventana principal`.

### Tarea 7: Formularios de edición

**Archivos:** crear `ui/formularios.slint`; modificar `ui/app.slint`, `ui/semana.slint`, `ui/pendientes.slint`, `src/puente.rs`.

**Interfaces consumidas:** todas las de `ops` (Tarea 3) a través de `Almacen::modificar`.

- [ ] Formulario de **tarea** (título, materia, fecha, hora): crear desde "+ Tarea" (llama `ops::agregar_tarea`), editar al hacer clic en una tarea, botón borrar con confirmación, casilla hecha/no hecha.
- [ ] Formulario de **clase** (materia, día, inicio, fin, lugar): crear con un botón en Semana, editar al hacer clic en un bloque, borrar con confirmación.
- [ ] Ventana de **ajustes** (botón en la cabecera): semestre (inicio/fin, o sin semestre), `aviso_dia_antes`, `aviso_horas_antes`, y lista de **recordatorios** con crear/editar/borrar (texto, días como casillas, hora).
- [ ] Validación en el formulario: campos obligatorios vacíos y horas con fin ≤ inicio → no guarda y marca el campo.
- [ ] Al abrir la app, llamar `ops::archivar_hechas` una vez.
- [ ] Comprobación manual: crear, editar y borrar uno de cada tipo; abrir `order.toml` y ver los cambios con los comentarios intactos.
- [ ] Commit `feat: formularios de edición`.

### Tarea 8: Proceso — bandeja, instancia única, recarga y notificaciones

**Archivos:** crear `src/sistema.rs`; modificar `src/main.rs`.

**Interfaces consumidas:** `puente::{mostrar_ventana, refrescar, mostrar_error}`, `Almacen`, `avisos::*`.

- [ ] **Instancia única:** socket Unix en `$XDG_RUNTIME_DIR/order.sock`. Si se conecta a uno vivo, envía `mostrar` y sale con código 0; si el archivo existe pero nadie responde, lo borra y sigue como instancia principal.
- [ ] **Arranque oculto:** el bucle de Slint corre sin ventana (`slint::run_event_loop_until_quit`); la ventana se crea en el primer `mostrar_ventana`. Cerrar la ventana la oculta.
- [ ] **Bandeja** (`ksni`, hilo propio): icono (provisional: uno temático del sistema, p. ej. `view-calendar`), clic izquierdo alterna la ventana, menú "Abrir" / "Salir".
- [ ] **Observador** (`notify`) sobre el directorio `~/.config/order/`: al cambiar `order.toml`, recargar; `Ok` → `refrescar`; `Err` → `mostrar_error` y mantener el último estado.
- [ ] **Reloj** (hilo, cada 30 s): `avisos_debidos` → enviar con `notify-rust` (aplicación "ORDER") → `Registro::anotar`. Al arrancar: `resumen_arranque` una vez y `Registro::podar` de claves con más de 7 días.
- [ ] Comprobación manual: lanzar `order` → solo aparece el icono en DMS; lanzar `order` otra vez → se abre la ventana y el segundo proceso termina; editar el TOML a mano → la ventana cambia sola en menos de 2 s; crear una tarea que vence en `aviso_horas_antes` h + 2 min → llega la notificación ~2 min después, y una sola vez.
- [ ] Commit `feat: bandeja, recarga y notificaciones`.

### Tarea 9: Instalación, arranque con niri y skill de Hermes

**Archivos:** crear `README.md`, `hermes/order-tareas/SKILL.md` (copia de trabajo dentro del repo).

- [ ] `cargo install --path .` → `order` queda en `~/.cargo/bin` (comprobar que está en el PATH de niri; si no, usar ruta absoluta en el `spawn-at-startup`).
- [ ] **Con permiso del usuario**: agregar `spawn-at-startup "order"` a `~/.config/niri/config.kdl`.
- [ ] Crear `order.desktop` en el repo (`Type=Application`, `Name=ORDER`, `Exec=order`, `Icon=view-calendar`, `X-GNOME-Autostart-enabled=true`, `NoDisplay=false`) e instalarlo en `~/.local/share/applications/` (lanzador) y, para entornos con autoarranque XDG (KDE, GNOME), en `~/.config/autostart/`. En niri no hace falta, porque niri no lee `autostart`, y no se duplica porque la instancia única absorbe un segundo arranque. Documentar en el README que en GNOME el icono requiere la extensión AppIndicator.
- [ ] Skill de Hermes `order-tareas`: describe la ruta del archivo, el formato de cada sección con un ejemplo, que los `id` se pueden omitir (la app los asigna), cómo marcar hecha (`hecha = true`, `hecha_en`), y que nunca debe borrar comentarios ni secciones que no entiende. **Con permiso del usuario**, copiarla a `~/.hermes/skills/order-tareas/`. No tocar `gida-itinerario-tareas`.
- [ ] `README.md`: qué es, dónde está el archivo, cómo cambiar colores (`ui/tema.slint`), cómo reinstalar.
- [ ] Comprobación manual: cerrar sesión y volver a entrar → el icono aparece; pedirle a Hermes "agrega una tarea de prueba para mañana" → aparece en la ventana sin tocar nada.
- [ ] Commit `docs: instalación y skill de Hermes`.
