# Hooks: esquema y mapeo a estados

Fuente de verdad verificada contra https://code.claude.com/docs/en/hooks
(revisado para Claude Code v2.1.x). Si algún campo no encaja en tiempo de
ejecución, vuelve a la doc oficial antes de cambiar los tipos.

## Por qué HTTP hooks

Claude Code soporta hooks de tipo `http`: en lugar de un script shell, hace un
POST con el JSON del evento como cuerpo. Eso nos evita scripts de pegamento: la
app escucha directamente. Detalle importante de su semántica:

> Non-2xx responses, connection failures, and timeouts all produce non-blocking
> errors that allow execution to continue.

O sea, **si la app está cerrada el hook falla en silencio y no frena al agente.**
Aun así ponemos `timeout` corto y el endpoint responde `200` vacío al instante.

## Campos comunes (todos los eventos)

Llegan en el cuerpo del POST, en `application/json`:

| Campo             | Notas                                                        |
| ----------------- | ------------------------------------------------------------ |
| `session_id`      | **La clave del store.** Identifica la instancia.             |
| `transcript_path` | Ruta al `.jsonl` de la conversación.                         |
| `cwd`             | Directorio de trabajo → de aquí derivamos el nombre proyecto.|
| `permission_mode` | `default` / `plan` / `acceptEdits` / `auto` / ...            |
| `hook_event_name` | Nombre del evento que disparó.                               |

Opcionales en subagentes: `agent_id`, `agent_type`.

## Eventos que usamos y a qué estado mapean

| Evento (matcher)                | Endpoint                          | Estado resultante      |
| ------------------------------- | --------------------------------- | ---------------------- |
| `SessionStart`                  | `POST /hooks/session-start`       | `Idle` (recién abierta)|
| `UserPromptSubmit`              | `POST /hooks/user-prompt`         | `Working`              |
| `PreToolUse` (`*`)             | `POST /hooks/pre-tool`            | `Working` (+ detalle)  |
| `PostToolUse` (`*`)            | `POST /hooks/post-tool`           | `Working` (heartbeat)  |
| `Notification` (`permission_prompt`) | `POST /hooks/notification/permission` | `WaitingPermission` |
| `Notification` (`idle_prompt`) | `POST /hooks/notification/idle`   | `WaitingInput`         |
| `Stop`                          | `POST /hooks/stop`                | `Idle` (turno cerrado) |
| `SessionEnd`                    | `POST /hooks/session-end`         | (se elimina del store) |

### Por qué una URL por subtipo de Notification

El `matcher` de `Notification` filtra por **tipo de notificación**
(`permission_prompt`, `idle_prompt`, `auth_success`, `elicitation_*`). Para no
depender de cómo venga el subtipo dentro del cuerpo, registramos un hook HTTP
por matcher y codificamos el subtipo **en la ruta**. Así el servidor sabe el
estado por el endpoint, sea cual sea la forma del payload.

> TODO(claude-code): si confirmas que el subtipo viene en un campo del cuerpo,
> puedes consolidar en un solo `/hooks/notification` y leerlo de ahí.

### Campos específicos relevantes

- `SessionStart`: + `source` (`startup`/`resume`/`clear`/`compact`), `model`.
- `PreToolUse` / `PostToolUse`: + `tool_name`, `tool_input` (y `tool_response`
  en post). Usamos `tool_name` + un resumen de `tool_input` como "detalle" que
  se muestra en la fila (p. ej. `Bash · npm test`, `Edit · src/app.ts`).
- `Notification`: incluye un campo `message` (texto de la notificación).
- `Stop`: + `stop_hook_active` (bool).
- `SessionEnd`: + `reason`.

## Máquina de estados

```
                 UserPromptSubmit / PreToolUse / PostToolUse
                 ┌──────────────────────────────────────────┐
                 ▼                                            │
  SessionStart ─► Idle ──UserPromptSubmit──► Working ─────────┘
                  ▲                            │   │
                  │ Stop                       │   └─ Notification(permission) ─► WaitingPermission
                  └────────────────────────────┘                                        │
                  ▲                            ▲                                         │
                  │ Stop                       └── UserPromptSubmit / PreToolUse ─────────┘
                  │
            Notification(idle) ─► WaitingInput ──UserPromptSubmit──► Working
```

Reglas:

- Cualquier `PreToolUse`/`PostToolUse`/`UserPromptSubmit` actualiza
  `last_event_at` y pone `Working`.
- `permission_prompt` y `idle_prompt` son los dos estados que "te reclaman" →
  son los que suben arriba en la lista y disparan el auto-show de la ventana.
- `Stop` → `Idle` (terminó el turno; tu siguiente movimiento).

## Sesiones muertas (reaper TTL)

Si matas el proceso o crashea, **`SessionEnd` no siempre llega**. Por eso un
reaper revisa periódicamente `last_event_at`:

- Si está `Working` y lleva > `STALE_SECS` sin eventos → `Unknown` (gris).
- Si lleva > `REMOVE_SECS` sin eventos en cualquier estado → se elimina.

Constantes en `src-tauri/src/state.rs`.

## Instalación de los hooks

Fusiona el contenido de `hooks/settings.snippet.json` dentro de tu
`~/.claude/settings.json` (combina la clave `hooks`, no la sobreescribas si ya
tienes otras). Verifica con `/hooks` dentro de Claude Code que aparecen como
fuente `User`.
