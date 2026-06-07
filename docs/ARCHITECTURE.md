# Architecture

## Single process

The system tray icon is the process that **stays alive permanently**. It hosts
the hook HTTP server and maintains state. The floating window is just a *view*
of the same process that is shown/hidden. Benefits:

- The hook listener is alive as long as the icon is in the system tray,
  even when the window is closed (solves the "the endpoint must exist" problem).
- A single binary, no separate daemon.

## State sources (hybrid)

Instance state comes from two sources. See `docs/DATA-SOURCES.md`.

- **Source A — supervisor files** (`jobs.rs`): reads `~/.claude/jobs/<id>/state.json`
  with a file watcher (`notify`). Covers background sessions (`/bg`,
  `claude --bg`, Agent View). Produces instances with `source = background`.
- **Source B — HTTP hooks** (`server.rs`): receives POST from Claude Code at
  `localhost:8787`. Covers foreground sessions (normal terminal). Produces
  instances with `source = foreground`.

Merge rule: **background wins**. `set_background_snapshot()` in `state.rs`
removes any foreground entry that shares a `session_id` with incoming
entries from Source A. The TTL reaper only acts on foreground (background entries
are managed by the supervisor files lifecycle).

## Components (src-tauri/src)

- `main.rs` — entry point. Starts Tauri, registers `tauri-plugin-autostart`,
  initializes `PrefsState` as managed state, launches the axum server, the jobs
  watcher, and the reaper. Exposes commands `get_instances` and `get_prefs`.
- `server.rs` — axum router. One route per event/subtype (see `docs/HOOKS.md`).
  Each handler: parses → applies to the store → calls `refresh::refresh()` → responds `200`.
- `state.rs` — `InstanceState` (enum with `Completed` and `Error`), `Instance`
  (struct with field `source: Source`), `Source` (enum `Background`/`Foreground`),
  `Store` (`Mutex<HashMap<session_id, Instance>>`), the transitions, and the TTL reaper
  (foreground only). Exports `project_from_cwd` as `pub(crate)`.
- `jobs.rs` — Source A: file watcher on `~/.claude/jobs/` (crate `notify`).
  Parses the real `state.json` schema (empirically verified 2026-06-06).
  RFC3339 → epoch secs without chrono: manual parser.
- `refresh.rs` — centralized state propagation. `refresh(app, store)` is the
  ONLY emission point: emits snapshot to the webview, updates the system tray
  icon/title (calm/alert based on `attention_count()`), and applies auto-hide/show
  using `PrefsState` (managed state, no I/O). Also exports `apply_auto_hide()` and
  `tray_variant()` (testable without Tauri runtime).
- `tray.rs` — system tray icon + preferences menu. All toggles wired up: floating,
  always_on_top, auto_hide, compact (emits "prefs" event to the frontend), open_at_login
  (via `tauri-plugin-autostart`). `persist_and_sync()` updates disk + managed state.
- `config.rs` — preferences persistence. `load_from_path()` and
  `default_prefs_path()` allow initializing `PrefsState` before setup().
- `hooks.rs` — serde types for the payloads.
- `transcript.rs` — reads context token occupancy from Claude Code transcript files
  (`~/.claude/projects/<slug>/<session_id>.jsonl`). Exports `cwd_to_slug` (replaces
  every `/` in a cwd with `-`; verified empirically against real projects), `transcript_path`
  (derives the full path from cwd + session_id), and `read_context_tokens` (seeks to the
  last 256 KiB, skips the first partial line, scans for the last `message.usage` block, and
  returns `input_tokens + cache_read_input_tokens + cache_creation_input_tokens`). All I/O
  errors return `None`. Used by `jobs.rs` (synchronous, during scan) and `server.rs`
  (throttled async spawn, at most once per session per 10 s, after the handler has already
  responded 200).

## Frontend (src)

- `App.tsx` — subscribes to the `instances` Tauri event, stores the snapshot in
  state, renders `MonitorWindow`.
- `components/MonitorWindow.tsx` — the panel: title bar (draggable area),
  count summary, list of rows **sorted by urgency**.
- `components/InstanceRow.tsx` — one row: color dot + project + detail +
  state + time in state.
- `lib/ipc.ts` — wrapper for Tauri's `listen()`.
- `types.ts` — TypeScript mirror of the Rust types.

## Pushing state to the webview

`refresh::refresh(app, store)` is the ONLY emission point. It is called by `server.rs`,
`jobs.rs`, and the reaper in `main.rs`. It emits two events:
- `"instances"` — full snapshot of instances (the array; no diffs).
- `"prefs"` — only when a preference changes (compact toggle from `tray.rs`).

The frontend listens with `listen()`. No polling.

## Floating window

Static config in `tauri.conf.json`: `decorations: false`, `transparent: true`,
`alwaysOnTop: true`, `skipTaskbar: true`, `visible: false` (starts hidden).
`macOSPrivateApi: true` is required for transparency on macOS.

At runtime (setup in `main.rs`):
- `set_visible_on_all_workspaces(true)` so it stays visible when switching
  spaces.

### macOS — Why NSPanel is mandatory

To float **over fullscreen apps**, an ordinary `NSWindow` is insufficient
even when all the correct bits are applied:

- `collectionBehavior = CanJoinAllSpaces | FullScreenAuxiliary` (0x101)
- `level = NSPopUpMenuWindowLevel` (101)
- `ActivationPolicy::Accessory`

All of this was empirically verified (confirmed via app logs). Even so,
the window disappeared when another app went fullscreen. The cause: macOS
internally requires the window to be a subclass of **NSPanel** to
respect `FullScreenAuxiliary` in another app's fullscreen Space.

**Solution:** plugin `tauri-nspanel` (branch `v2.1`) that converts the
`WebviewWindow` into a real `NSPanel` subclass. In `setup()`, `macos::setup_panel(&w)`
is called (`src/macos.rs`), which:

1. Converts the window: `window.to_panel::<MonitorPanel>()` (the plugin's
   `WebviewWindowExt` trait). The panel is registered in the plugin's
   `WebviewPanelManager` and can be retrieved with
   `app.get_webview_panel("monitor")`.
2. Non-activating style: `StyleMask::empty().nonactivating_panel()` — the panel
   does not steal focus from the active app (even in fullscreen).
3. Level `PanelLevel::Status` (25) — same level as system status bar indicators.
4. `CollectionBehavior`: `can_join_all_spaces() + full_screen_auxiliary() +
   stationary()` — visible on all Spaces, admitted in fullscreen Spaces,
   does not move with Exposé.

The plugin uses the same versions of `objc2`/`objc2-app-kit`/`objc2-foundation`
that Tauri brings as transitive dependencies — no duplication in the binary.

`tray.rs` and `refresh.rs` call `app.get_webview_panel("monitor")` to
show/hide the panel (instead of `get_webview_window`), using the
`tauri_nspanel::ManagerExt` trait. If the panel is not available (init race or
non-macOS platform), they fall back to `get_webview_window`.

### Linux / Wayland

- **X11:** works out of the box.
- **Wayland:** always-on-top is controlled by the compositor, not the app. On
  **Hyprland** it is solved with rules (adjust `class`/`title` to the real values):
  ```
  windowrulev2 = float, class:^(cctv)$
  windowrulev2 = pin, class:^(cctv)$
  windowrulev2 = nofocus, class:^(cctv)$
  ```
  > TODO(claude-code): document the real `class` the window reports on
  > Wayland and leave the snippet ready in the README.

### Windows

- `always_on_top` + transparency without friction. `skipTaskbar` hides from the taskbar.

## System tray and preferences

Menu with toggles (state persisted in `config.rs` + `PrefsState` managed state):

- `floating_window` — show/hide the window.
- `always_on_top` — pin on top (`set_always_on_top`).
- `auto_hide` — hide when `attention_count()==0`; reappears on
  `WaitingPermission`/`WaitingInput` (only if `floating_window` is active).
- `compact` — compact mode: emits "prefs" event to the frontend, which applies
  the CSS class `.compact` (hides `.detail`, reduces padding). No reload.
- `open_at_login` — autostart via `tauri-plugin-autostart` (LaunchAgent on macOS).

The icon alternates between calm and alert based on `attention_count()`. On macOS the
system tray title shows the number of instances requesting attention.
