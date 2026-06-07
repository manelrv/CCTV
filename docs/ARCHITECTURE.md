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

- `main.rs` — punto de entrada. Arranca Tauri, registra `tauri-plugin-autostart`,
  inicializa `PrefsState` como managed state, lanza el servidor axum, el watcher
  de jobs y el reaper. Expone comandos `get_instances` y `get_prefs`.
- `server.rs` — router axum. Una ruta por evento/subtipo (ver `docs/HOOKS.md`).
  Cada handler: parsea → aplica al store → llama `refresh::refresh()` → responde `200`.
- `state.rs` — `InstanceState` (enum con `Completed` y `Error`), `Instance`
  (struct con campo `source: Source`), `Source` (enum `Background`/`Foreground`),
  `Store` (`Mutex<HashMap<session_id, Instance>>`), las transiciones y el reaper TTL
  (solo foreground). Exporta `project_from_cwd` como `pub(crate)`.
- `jobs.rs` — Fuente A: file watcher sobre `~/.claude/jobs/` (crate `notify`).
  Parsea el esquema real de `state.json` (verificado empíricamente 2026-06-06).
  RFC3339 → epoch secs sin chrono: parser manual.
- `refresh.rs` — propagacion centralizada del estado. `refresh(app, store)` es el
  ÚNICO punto de emision: emite snapshot al webview, actualiza icono/titulo de
  bandeja (calm/alert segun `attention_count()`), y aplica auto-hide/show usando
  `PrefsState` (managed state, sin I/O). Tambien exporta `apply_auto_hide()` y
  `tray_variant()` (testeable sin runtime Tauri).
- `tray.rs` — icono + menú de preferencias. Todos los toggles cableados: floating,
  always_on_top, auto_hide, compact (emite evento "prefs" al frontend), open_at_login
  (via `tauri-plugin-autostart`). `persist_and_sync()` actualiza disco + managed state.
- `config.rs` — persistencia de preferencias. `load_from_path()` y
  `default_prefs_path()` permiten inicializar `PrefsState` antes del setup().
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

`refresh::refresh(app, store)` es el ÚNICO punto de emisión. Lo llaman `server.rs`,
`jobs.rs` y el reaper de `main.rs`. Emite dos eventos:
- `"instances"` — snapshot completo de instancias (el array; sin diffs).
- `"prefs"` — solo cuando cambia una preferencia (compact toggle desde `tray.rs`).

El frontend escucha con `listen()`. No hay polling.

## Ventana flotante

Config estática en `tauri.conf.json`: `decorations: false`, `transparent: true`,
`alwaysOnTop: true`, `skipTaskbar: true`, `visible: false` (arranca oculta).
`macOSPrivateApi: true` es necesario para la transparencia en macOS.

En runtime (setup de `main.rs`):
- `set_visible_on_all_workspaces(true)` para que siga visible al cambiar de
  espacio.

### macOS — Por qué NSPanel es obligatorio

Para flotar **sobre apps en fullscreen**, un `NSWindow` ordinario es insuficiente
aunque se apliquen todos los bits correctos:

- `collectionBehavior = CanJoinAllSpaces | FullScreenAuxiliary` (0x101)
- `level = NSPopUpMenuWindowLevel` (101)
- `ActivationPolicy::Accessory`

Todo esto fue verificado empíricamente (confirmado via logs de la app). Aun así,
la ventana desaparecía al entrar otra app en fullscreen. La causa: macOS
internamente requiere que la ventana sea una subclase de **NSPanel** para
respetar `FullScreenAuxiliary` en el Space de fullscreen de otra app.

**Solución:** plugin `tauri-nspanel` (branch `v2.1`) que convierte el
`WebviewWindow` en un `NSPanel` subclass real. En `setup()` se llama
`macos::setup_panel(&w)` (`src/macos.rs`) que:

1. Convierte la ventana: `window.to_panel::<MonitorPanel>()` (trait
   `WebviewWindowExt` del plugin). El panel queda registrado en el
   `WebviewPanelManager` del plugin y es recuperable con
   `app.get_webview_panel("monitor")`.
2. Estilo no-activating: `StyleMask::empty().nonactivating_panel()` — el panel
   no roba el foco de la app activa (incluso en fullscreen).
3. Nivel `PanelLevel::Status` (25) — mismo nivel que los indicadores de la
   barra de estado del sistema.
4. `CollectionBehavior`: `can_join_all_spaces() + full_screen_auxiliary() +
   stationary()` — visible en todos los Spaces, admitido en fullscreen Spaces,
   no se mueve con Exposé.

El plugin usa las mismas versiones de `objc2`/`objc2-app-kit`/`objc2-foundation`
que Tauri trae como dependencias transitivas — sin duplicado en el binario.

`tray.rs` y `refresh.rs` llaman a `app.get_webview_panel("monitor")` para
mostrar/ocultar el panel (en lugar de `get_webview_window`), usando el trait
`tauri_nspanel::ManagerExt`. Si el panel no está disponible (race en init o
plataforma no-macOS), hacen fallback a `get_webview_window`.

### Linux / Wayland

- **X11:** funciona directo.
- **Wayland:** el always-on-top no lo controla la app sino el compositor. En
  **Hyprland** se resuelve con reglas (ajusta `class`/`title` a los reales):
  ```
  windowrulev2 = float, class:^(cctv)$
  windowrulev2 = pin, class:^(cctv)$
  windowrulev2 = nofocus, class:^(cctv)$
  ```
  > TODO(claude-code): documentar la `class` real que reporta la ventana en
  > Wayland y dejar el snippet listo en el README.

### Windows

- `always_on_top` + transparencia sin fricción. `skipTaskbar` oculta de la barra.

## Bandeja y preferencias

Menú con toggles (estado persistido en `config.rs` + `PrefsState` managed state):

- `floating_window` — mostrar/ocultar la ventana.
- `always_on_top` — fijar encima (`set_always_on_top`).
- `auto_hide` — ocultar cuando `attention_count()==0`; reaparece ante
  `WaitingPermission`/`WaitingInput` (solo si `floating_window` está activo).
- `compact` — modo compacto: emite evento "prefs" al frontend, que aplica clase
  CSS `.compact` (oculta `.detail`, reduce padding). Sin recarga.
- `open_at_login` — autoarranque via `tauri-plugin-autostart` (LaunchAgent en macOS).

El icono cambia entre calm y alert segun `attention_count()`. En macOS el titulo
de la bandeja muestra el numero de instancias que reclaman.
