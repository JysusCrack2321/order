---
name: order-tareas
description: Add, edit or close ORDER tasks, classes and reminders (TOML).
version: 1.0.0
author: Hermes
license: MIT
metadata:
  hermes:
    tags: [order, tareas, clases, recordatorios, toml, horario]
    related_skills: [gida-itinerario-tareas]
---

# Tareas de ORDER

## When to Use

- Hay que agregar, editar, marcar hecha o borrar una tarea, una clase, un
  recordatorio o el rango del semestre de **ORDER** (`~/apps/order`).
- El usuario dicta algo como "agrega una tarea de cálculo para el viernes" o
  "marca como hecha la del informe".
- Se necesita consultar el horario o lo pendiente sin abrir la ventana.

ORDER es la app de escritorio del horario y las tareas. **No hay API ni base de
datos**: todo se edita en un archivo TOML y la app lo relee sola cuando cambia.
No hace falta reiniciarla ni avisarle nada.

## Dónde está

```
~/.config/order/order.toml     # ajustes, semestre, clases, tareas, recordatorios
~/.config/order/historial.toml # tareas hechas con más de 30 días (las mueve la app)
```

Para probar sin tocar los datos reales: `ORDER_ARCHIVO=/tmp/copia.toml order`
(mueve también el registro de avisos y el socket de instancia única).

## Formato

```toml
[ajustes]
aviso_dia_antes = "20:00"   # hora del aviso del día anterior al vencimiento
aviso_horas_antes = 3       # aviso N horas antes de vencer

[semestre]                  # opcional: sin esta sección las clases se repiten siempre
inicio = "2026-08-03"
fin = "2026-12-05"

[[clase]]
materia = "Control"
dias = ["lun", "mie"]       # lun mar mie jue vie sab dom
inicio = "07:00"
fin = "09:00"
lugar = "453-201"           # opcional

[[tarea]]
id = "t-20261121-01"        # opcional: si falta, la app lo asigna
titulo = "Taller 2"
materia = "Control"         # opcional
vence = "2026-11-28T23:59"  # fecha y hora local
hecha = false

[[tarea]]
titulo = "Informe de laboratorio"
materia = "Robótica"
vence = "2026-12-02T18:00"
hecha = true
hecha_en = "2026-11-21"     # día en que se marcó

[[recordatorio]]
texto = "Revisar Moodle"
dias = ["lun", "mar", "mie", "jue", "vie"]
hora = "18:00"
```

### Reglas del archivo

- **Horas y fechas siempre entre comillas**: `"07:00"`, `"2026-11-28T23:59"`,
  `"2026-11-28"`. Sin comillas el archivo no parsea.
- Horas en 24 h `HH:MM`. Fechas `YYYY-MM-DD`. Vencimiento `YYYY-MM-DDTHH:MM`.
- `dias` es una lista; el formato viejo `dia = "lun"` se acepta y se convierte
  solo, pero escribe siempre `dias`.
- Los `id` son opcionales: la app asigna `t-…`, `c-…` o `r-…` al leer. Si
  editas un elemento, **conserva su `id`** para no duplicarlo.
- `materia` debe usar **la misma grafía** que las clases y tareas existentes
  (`Control`, no `control` ni `COnttol`): el color y el conteo por materia se
  agrupan por el texto exacto. Antes de escribir una materia nueva, mira las que
  ya hay en el archivo.
- **Nunca** borres comentarios ni secciones que no entiendas (`[ajustes]`,
  `[semestre]`, `[[fuente]]` del futuro…). Para editar usa reemplazos puntuales
  (patch), no reescribas el archivo completo.
- Si el archivo tiene un error, la app conserva el último estado válido y
  muestra la línea del error; no la sobreescribe. Después de editar, valida:
  `python3 -c "import tomllib,sys;tomllib.load(open(sys.argv[1],'rb'))" ~/.config/order/order.toml`

## Operaciones

**Agregar una tarea** — añade un bloque `[[tarea]]` al final del grupo de
tareas (antes de los recordatorios). Sin `id`.

**Editar una tarea** — cambia solo el campo pedido dentro de su bloque
(`titulo`, `materia`, `vence`). Si cambia el vencimiento, los avisos se recalculan
solos.

**Marcar hecha** — en su bloque:

```toml
hecha = true
hecha_en = "2026-11-22"
```

`hecha_en` es la fecha de hoy (`date +%F`). Para desmarcar: `hecha = false` y
borra la línea `hecha_en`. Una tarea hecha sigue visible (tachada) y a los 30
días la app la mueve a `historial.toml`.

**Borrar** — quita el bloque `[[tarea]]` completo (desde la línea `[[tarea]]`
hasta la última clave antes del siguiente `[[…]]` o sección). No borres el
comentario que encabeza el grupo.

**Clases y recordatorios** — mismo criterio: agregar un bloque nuevo (sin `id`)
o editar los campos del bloque existente. `fin` debe ser mayor que `inicio`.

## Comprobar

- La app recarga sola: guarda el archivo y la ventana cambia en ~2 s. No hace
  falta reiniciar ni lanzar nada.
- Sin abrir ventana: `cd ~/apps/order && cargo test` y `cargo build` (sin
  warnings) siguen siendo la red de seguridad del código, no del archivo.
- Para ver el resultado de una edición real, lee el archivo de vuelta y confirma
  que el bloque quedó donde debía y con la grafía pedida.

## Trampas

- `aviso_horas_antes` es número (`3`), no cadena.
- Un `vence` ya pasado no dispara avisos atrasados: la tarea aparece resaltada
  como vencida.
- Si la app está corriendo y editas el archivo, no hay conflicto: ella relee
  antes de escribir; aun así, evita guardar mientras un formulario está abierto
  en la ventana (al guardar el formulario, relee el disco y conserva lo tuyo).
- No toques la skill `gida-itinerario-tareas`: es de la app vieja del Itinerario,
  que sigue instalada pero no se usa.
