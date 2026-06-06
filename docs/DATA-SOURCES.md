# Fuentes de datos: híbrido (Agent View + hooks)

El estado de las instancias viene de **dos sitios** y se fusiona en un único
store. Esto es así porque ninguna fuente sola cubre todas las sesiones.

## Por qué dos fuentes

- **Sesiones en segundo plano** (lanzadas con `/bg`, `claude --bg`, o desde la
  propia Agent View): las gestiona el **supervisor** de Claude Code, que ya
  persiste su estado en disco. No necesitamos hooks para estas: leemos sus
  ficheros.
- **Sesiones en primer plano** (un `claude` normal en una terminal): NO las
  gestiona el supervisor, así que no aparecen en sus ficheros. Para estas
  seguimos usando los **hooks HTTP** (ver `docs/HOOKS.md`).

Un usuario que mezcla ambos flujos necesita las dos fuentes.

## Fuente A — ficheros del supervisor (segundo plano)

Agent View persiste el estado bajo el directorio de config de Claude Code:

| Fichero                          | Contenido                                          |
| -------------------------------- | -------------------------------------------------- |
| `~/.claude/daemon/roster.json`   | Lista de sesiones en marcha (para reconectar)      |
| `~/.claude/jobs/<id>/state.json` | Estado por sesión que alimenta la tabla de Agent View |
| `~/.claude/daemon.log`           | Logs del supervisor                                |

La documentación dice explícitamente que **puedes leer esos ficheros desde un
script para construir automatizaciones propias**. Eso es justo lo que hacemos:
vigilamos `~/.claude/jobs/` (y `roster.json`) con un file watcher, parseamos los
`state.json` y mapeamos su estado al nuestro. El supervisor ya hace la máquina
de estados, las transiciones y la limpieza; nosotros solo leemos y pintamos.

> TODO(claude-code): el esquema exacto de `state.json` NO está documentado campo
> a campo. Antes de fijar los tipos en `jobs.rs`, abre un `state.json` real
> (lanza una sesión con `claude --bg "echo hola"` y mira el fichero) y ajusta la
> struct `JobState`. Parsea defensivo (todo `Option`).
> `claude daemon status` (v2.1.141+) también vuelca estado del subsistema.

### Estados de Agent View → los nuestros

Agent View expone: Working (animado), Needs input (amarillo), Idle (atenuado),
Completed (verde), Failed (rojo), Stopped (gris).

| Agent View   | InstanceState (nuestro) | Color   |
| ------------ | ----------------------- | ------- |
| Working      | `working`               | verde   |
| Needs input  | `waiting_input`         | ámbar   |
| (blocked)    | `waiting_permission`    | rojo    |
| Idle         | `idle`                  | gris    |
| Completed    | `completed`             | verde   |
| Failed       | `error`                 | rojo    |
| Stopped      | `unknown`               | gris    |

> Nota: Agent View distingue "blocked" (filtro `s:blocked`). Si `state.json` lo
> separa de "needs input", mapéalo a `waiting_permission`; si no, todo lo que
> reclame input va a `waiting_input`.

## Fuente B — hooks HTTP (primer plano)

Igual que en `docs/HOOKS.md`. Solo cambia que estas instancias se marcan con
`source = "foreground"`.

## Fusión en el store

Cada `Instance` lleva un campo `source`: `background` | `foreground`.

Regla: **background manda**. Una sesión vive en una fuente u otra, no en las dos
a la vez (cuando mandas a segundo plano una sesión de primer plano, deja de
tener terminal y pasa al supervisor). Implementación:

- El watcher de la Fuente A produce el set completo de sesiones en segundo plano
  en cada rescan y hace `set_background_snapshot(...)`: reemplaza todas las
  entradas `background` y elimina cualquier `foreground` que comparta `id`.
- Los hooks de la Fuente B hacen `apply(...)` con `source = foreground`.
- El **reaper TTL** solo aplica a las `foreground` (las de primer plano pueden
  morir sin `SessionEnd`). Las `background` las limpia el ciclo de vida de los
  ficheros del supervisor.

## UI

La fila muestra una etiqueta discreta del origen: `bg` / `fg`, para que de un
vistazo sepas cuál puedes reabrir con `claude agents` y cuál vive en una
terminal tuya.

> Backlog: Agent View también muestra un punto de color con el estado del PR que
> abre una sesión (amarillo/verde/morado/gris). Si `state.json` lo incluye,
> sería un extra bonito en la fila.
