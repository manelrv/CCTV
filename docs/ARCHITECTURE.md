# Arquitectura

## Proceso único

El icono de la bandeja es el proceso que **vive siempre**. Hostea el servidor
HTTP de los hooks y mantiene el estado. La ventana flotante es solo una *vista*
del mismo proceso que se muestra/oculta. Ventajas:

- El listener de los hooks está vivo mientras el icono esté en la bandeja,
  aunque la ventana esté cerrada (resuelve el "el endpoint tiene que existir").
- Un solo binario, sin daemon aparte.

## Fuentes de estado (híbrido)

El estado de las instancias viene de dos fuentes. Ver `docs/DATA-SOURCES.md`.

- **Fuente A — ficheros del supervisor** (`jobs.rs`): lee `~/.claude/jobs/<id>/state.json`
  con un file watcher (`notify`). Cubre sesiones en segundo plano (`/bg`,
  `claude --bg`, Agent View). Produce instancias con `source = background`.
- **Fuente B — hooks HTTP** (`server.rs`): recibe POST de Claude Code en
  `localhost:8787`. Cubre sesiones en primer plano (terminal normal). Produce
  instancias con `source = foreground`.

Regla de fusión: **background manda**. `set_background_snapshot()` en `state.rs`
elimina cualquier entrada foreground que comparta `session_id` con las incoming
de la Fuente A. El reaper TTL solo actúa sobre foreground (las background las
gestiona el ciclo de vida de los ficheros del supervisor).

## Componentes (src-tauri/src)

- `main.rs` — punto de entrada. Arranca Tauri, configura la ventana, lanza el
  servidor axum en una task de tokio, arranca el watcher de jobs, construye la
  bandeja y el reaper.
- `server.rs` — router axum. Una ruta por evento/subtipo (ver `docs/HOOKS.md`).
  Cada handler: parsea → aplica al store → emite snapshot → responde `200`.
- `state.rs` — `InstanceState` (enum con `Completed` y `Error`), `Instance`
  (struct con campo `source: Source`), `Source` (enum `Background`/`Foreground`),
  `Store` (`Mutex<HashMap<session_id, Instance>>`), las transiciones y el reaper TTL
  (solo foreground). Exporta `project_from_cwd` como `pub(crate)`.
- `jobs.rs` — Fuente A: file watcher sobre `~/.claude/jobs/` (crate `notify`).
  Parsea el esquema real de `state.json` (verificado empíricamente 2026-06-06).
  RFC3339 → epoch secs sin chrono: parser manual.
- `tray.rs` — icono + menú de preferencias (toggles) + acciones (mostrar/ocultar
  ventana, salir).
- `config.rs` — persistencia de preferencias del usuario en un JSON del config
  dir de la app.
- `hooks.rs` — tipos serde de los payloads.

## Frontend (src)

- `App.tsx` — se suscribe al evento `instances` de Tauri, guarda el snapshot en
  estado, renderiza `MonitorWindow`.
- `components/MonitorWindow.tsx` — el panel: barra de título (zona arrastrable),
  resumen de conteo, lista de filas **ordenadas por urgencia**.
- `components/InstanceRow.tsx` — una fila: dot de color + proyecto + detalle +
  estado + tiempo en estado.
- `lib/ipc.ts` — wrapper de `listen()` de Tauri.
- `types.ts` — espejo TS de los tipos de Rust.

## Empuje de estado al webview

El backend emite `app.emit("instances", snapshot)` en cada cambio. El frontend
escucha con `listen("instances", ...)`. No hay polling. El snapshot es el array
completo de instancias (son pocas; no merece la pena hacer diffs).

## Ventana flotante

Config estática en `tauri.conf.json`: `decorations: false`, `transparent: true`,
`alwaysOnTop: true`, `skipTaskbar: true`, `visible: false` (arranca oculta).
`macOSPrivateApi: true` es necesario para la transparencia en macOS.

En runtime (setup de `main.rs`):
- `set_visible_on_all_workspaces(true)` para que siga visible al cambiar de
  espacio.

### macOS

- Para que flote **sobre apps en fullscreen** puede no bastar `always_on_top`;
  hace falta subir el nivel de ventana (`NSWindow` level a `floating` o
  `screenSaver`) y el collection behavior.
  > TODO(claude-code): si `set_visible_on_all_workspaces` no cubre el caso
  > fullscreen, implementar vía `objc2`/`cocoa` sobre el `NSWindow` nativo
  > (`ns_window()` del `WebviewWindow`).

### Linux / Wayland

- **X11:** funciona directo.
- **Wayland:** el always-on-top no lo controla la app sino el compositor. En
  **Hyprland** se resuelve con reglas (ajusta `class`/`title` a los reales):
  ```
  windowrulev2 = float, class:^(claude-code-monitor)$
  windowrulev2 = pin, class:^(claude-code-monitor)$
  windowrulev2 = nofocus, class:^(claude-code-monitor)$
  ```
  > TODO(claude-code): documentar la `class` real que reporta la ventana en
  > Wayland y dejar el snippet listo en el README.

### Windows

- `always_on_top` + transparencia sin fricción. `skipTaskbar` oculta de la barra.

## Bandeja y preferencias

Menú con toggles (estado persistido en `config.rs`):

- `floating_window` — mostrar/ocultar la ventana.
- `always_on_top` — fijar encima.
- `auto_hide` — ocultar cuando nada reclama; reaparece ante
  `WaitingPermission`/`WaitingInput`.
- `compact` — modo compacto (solo dots) vs expandido (con detalle de tool).
- `open_at_login` — autoarranque.

El icono puede reflejar el estado más urgente (color/contador). En macOS la
barra admite texto junto al icono → mostrar el nº de instancias que reclaman.

> TODO(claude-code): cablear cada toggle a su efecto real. El scaffold deja la
> estructura y el show/hide + salir funcionando.
