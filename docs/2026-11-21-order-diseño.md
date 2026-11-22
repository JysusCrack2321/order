# ORDER — documento de diseño

Fecha: 2026-11-21 · Estado: aprobado por el usuario

## 1. Intención

App de escritorio personal para Linux (niri + barra DMS) donde ver el horario de la semana, las tareas pendientes y recibir avisos. Reemplaza en uso diario a `~/apps/itinerario-app` (esa app se queda instalada pero no se toca ni se reutiliza su código).

**Por qué:** el proyecto anterior creció demasiado (servidor, front, back, multiusuario). El usuario quiere algo **simple, práctico, rápido, ligero, bonito y modificable**. Si ORDER vuelve a crecer en piezas, fracasó aunque funcione.

**Quién lo construye:** el código lo escriben agentes DeepSeek (vía Hermes); Claude solo orquesta, prueba y revisa.

## 2. Objetivos

1. Icono en la bandeja de DMS que aparece al iniciar sesión de niri.
2. Ventana con dos vistas: **Semana** (horario fijo de la U + tareas que vencen cada día) y **Pendientes** (lista por fecha de entrega, marcar hecha con un clic).
3. Botón **+ Tarea**: formulario corto (título, materia, fecha/hora de entrega).
4. Notificaciones de escritorio:
   - tarea por vencer: el día antes y unas horas antes (valores en ajustes);
   - recordatorios fijos recurrentes (p. ej. "Revisar Moodle", "Revisar Classroom") a la hora y días que el usuario defina.
5. Todos los datos en **un solo archivo TOML** legible, editable a mano y por Hermes; la app lo recarga sola cuando cambia.
6. Look: tipografía **Cinzel** (estilo Trajan, romana en mayúsculas, licencia OFL) incrustada en el binario; tema oscuro; colores y tamaños en un archivo `.slint` de tema para modificarlos fácil.

## 3. No-objetivos (primera versión)

- Cuadro de texto para hablar con un agente dentro de la app (queda previsto, ver §7).
- Voz dentro de la app (la voz la pone Hermes).
- Leer tareas de otros proyectos por `.ics` (queda previsto, ver §7).
- Integración real con Moodle o Classroom (solo recordatorios a una hora).
- Prioridades, etiquetas, subtareas, duraciones, planificador automático.
- Celular, sincronización, cuentas, multiusuario.
- Arte final del icono (se hará después, posiblemente con agy).

## 4. Anti-objetivos (si pasa esto, salió mal)

- Hay un servidor, un puerto abierto, una base de datos o un navegador de por medio.
- Hacen falta varios procesos o servicios para que funcione.
- Necesita internet para lo básico.
- El archivo de datos no se entiende a simple vista.
- Arrancar tarda más que abrir Nautilus.

## 5. Enfoques considerados

**Interfaz gráfica** (elegido por el usuario: **Slint**)
- *Slint 1.x* — API estable, interfaz declarativa en archivos `.slint` separados de la lógica (cómodo para agentes y para modificar el look), aspecto moderno. Riesgo: menos ejemplos en el entrenamiento de DeepSeek que GTK; se mitiga con el ciclo compilar-corregir.
- *GTK4 / Relm4* — nativo en Linux, pero bindings 0.x que cambian y documentación escasa.
- *Tauri* — la IA escribe mejor HTML/CSS, pero vuelve a partir la app en front y back.
- iced y egui descartados (API inestable / aspecto de herramienta).

**Dónde vive el agente** (elegido: **Hermes ahora, cuadro en la app después**)
- Hermes edita el archivo TOML con una skill nueva; la app solo observa el archivo. Aprovecha la voz de Hermes y mantiene la app mínima.
- Cuadro de texto en la app llamando a DeepSeek: pospuesto.

**Tareas de otros proyectos** (elegido: **fuentes `.ics` de solo lectura, después**)
- Cada proyecto publica un enlace `.ics` personal con clave secreta (p. ej. `edt.gida.lat/ics/<usuario>?k=…`) filtrado por responsable; ORDER lo lee y muestra con etiqueta.
- CalDAV descartado (vuelve a meter servidor); escribir directo en el TOML de ORDER descartado (acopla apps y pisa cambios).

## 6. Diseño

### 6.1 Piezas
Un solo binario Rust `order`, un solo proceso:
- **UI** con Slint (`ui/*.slint`, más `ui/tema.slint` con colores, tamaños y fuente).
- **Bandeja** por StatusNotifierItem (crate tipo `ksni`); DMS ya lo soporta. Clic: muestra/oculta la ventana. Menú: Abrir, Salir. Cerrar la ventana la oculta, no termina el proceso.
- **Notificaciones** por `org.freedesktop.Notifications` (crate tipo `notify-rust`); DMS es el servidor.
- **Observador del archivo** (crate tipo `notify`): al cambiar el TOML, recarga.
- **Reloj de avisos**: revisa periódicamente qué toca avisar.

### 6.2 Archivo de datos
Ruta: `~/.config/order/order.toml` (*decisión de Claude*: un solo lugar fácil de encontrar). Si no existe, se crea con un ejemplo comentado.

Forma orientativa (el builder puede ajustar nombres, manteniéndolo igual de legible):

```toml
[ajustes]
aviso_dia_antes = "20:00"     # hora del aviso del día anterior
aviso_horas_antes = 3         # aviso N horas antes de vencer

[semestre]
inicio = "2026-08-03"
fin = "2026-12-05"

[[clase]]
materia = "Control"
dia = "lun"                   # lun mar mie jue vie sab dom
inicio = "07:00"
fin = "09:00"
lugar = "453-201"             # opcional

[[tarea]]
id = "t-20261121-01"
titulo = "Taller 2"
materia = "Control"           # opcional
vence = "2026-11-28T23:59"
hecha = false

[[recordatorio]]
texto = "Revisar Moodle"
dias = ["lun", "mar", "mie", "jue", "vie"]
hora = "18:00"
```

Reglas:
- La app escribe de forma atómica (temporal + renombrar) y **relee el archivo justo antes de escribir** para no pisar cambios de Hermes o manuales.
- Si el archivo tiene un error de sintaxis, la app **conserva el último estado válido** y muestra un aviso visible con la línea del error; no sobreescribe el archivo roto.
- Los comentarios del usuario en el TOML deberían sobrevivir a las escrituras de la app (p. ej. con `toml_edit`).

### 6.3 Avisos
- Para cada tarea no hecha: aviso a `aviso_dia_antes` del día anterior y otro `aviso_horas_antes` antes de `vence`.
- Recordatorios: en los `dias` indicados a la `hora`.
- *Decisión de Claude:* las notificaciones no llevan botones (ni "Marcar hecha" ni "Posponer"); solo avisan.
- No repetir un aviso ya mostrado, ni siquiera tras reiniciar la app (registro pequeño en `~/.local/state/order/`).
- *Decisión de Claude:* avisos que se perdieron con el equipo apagado no se disparan en ráfaga; como mucho uno de resumen al arrancar ("tienes N tareas vencidas o por vencer hoy").

### 6.4 Ventana
```
 ORDER                                   [ + Tarea ]
 ─────────────────────────────────────────────────
  Semana  │  Pendientes
 ─────────────────────────────────────────────────
        Lun      Mar      Mié      Jue      Vie
  07   Control           Control
  09            Robótica          Robótica
 ─────────────────────────────────────────────────
  Hoy: 2 tareas pendientes · Revisar Moodle 18:00
```
- Semana: columnas lun–vie (sáb/dom solo si hay clases), bloques de clase, marca de tareas que vencen ese día, indicador del momento actual.
- Pendientes: tareas no hechas ordenadas por `vence`; las vencidas resaltadas; casilla para marcar hecha.
- **Navegación de semanas** (decisión del usuario): flechas ‹ › para ir a la semana anterior/siguiente y un botón "Hoy" para volver. Las clases solo aparecen dentro del rango del semestre (`[semestre] inicio`/`fin` en el TOML). Festivos: fuera de la primera versión.
- **Tareas desde la ventana** (decisión del usuario): edición total. Crear, editar todos los campos (mismo formulario que "+ Tarea", precargado), marcar y desmarcar como hecha, y borrar con confirmación.
- **Tareas hechas** (*decisión de Claude*): se ocultan de Pendientes; las que llevan más de 30 días hechas se mueven a `~/.config/order/historial.toml` para que `order.toml` siga limpio durante el semestre.
- **Principio general** (decisión del usuario): aunque exista el agente, **todo lo que hay en el archivo se puede editar a mano desde la ventana**. El usuario quiere control total.
- **Horario** (clases y rango del semestre) y **recordatorios**: crear, editar todos los campos y borrar desde la ventana, con formularios del mismo estilo que el de tareas. Los **ajustes de avisos** también se editan desde la ventana.

### 6.5 Arranque
- Línea en `~/.config/niri/config.kdl`: `spawn-at-startup "order"` (la añade el usuario o Claude con su permiso).
- *Decisión de Claude:* al iniciar sesión arranca **solo como icono** en la bandeja, sin abrir la ventana; la ventana se crea la primera vez que se pide.
- Instancia única: si `order` ya corre, lanzarlo otra vez solo muestra la ventana existente.

### 6.6 Hermes
Skill nueva de Hermes (p. ej. `order-tareas`, en `~/.hermes/skills/`) que explica el formato del TOML y cómo agregar/marcar tareas, clases y recordatorios editando el archivo. No reemplaza ni toca la skill vieja `gida-itinerario-tareas`. La carga inicial del horario de la U se hace pasándole a Hermes el PDF o la captura.

## 7. Previsto para después (no construir, pero no cerrar la puerta)
- **Cuadro del agente:** toda creación de tareas pasa por una única función (p. ej. `agregar_tarea`) que hoy usa el formulario; el cuadro de texto futuro llamará a DeepSeek (key en `~/.hermes/.env`), mostrará la tarea propuesta para confirmar y usará esa misma función.
- **Fuentes `.ics`:** sección `[[fuente]]` en el TOML con `nombre` y `url`; sus tareas se muestran con etiqueta, solo lectura. El modelo interno de tarea debería admitir un campo de origen.

## 8. Restricciones
- Rust (toolchain del sistema, cargo 1.100), Slint 1.x, Linux/Wayland bajo niri.
- Sin red para el funcionamiento básico.
- Proyecto en `~/apps/order`.

## 9. Forma del plan
El plan de implementación se corta en encargos pequeños e independientes para agentes DeepSeek, cada uno con un criterio de "listo" comprobable (`cargo build`, `cargo test` o una prueba manual corta). La capa de datos (leer/escribir TOML; crear/editar/borrar tareas, clases, recordatorios y ajustes; marcar tarea; archivar hechas) va primero y se prueba sin UI.

## 10. Queda a criterio del builder
- Elección exacta de crates (bandeja, notificaciones, observador, TOML).
- Organización de módulos y archivos `.slint`.
- Paleta concreta del tema oscuro (Cinzel para títulos y cuerpo legible; si Cinzel en tamaño pequeño no se lee bien, usar otra fuente solo para texto pequeño y avisar).
- Frecuencia del reloj de avisos.
- Formato del `id` de tarea.
