# Roadmap

Mark `[x]` when done. Each phase should leave the app in a runnable state.

## Phase 0 — Compiles and runs ✅
- [x] `npm install` and `npm run tauri dev` compile and open a window.
      (Fixes: spurious `[lib]` in Cargo.toml, `macos-private-api` feature,
      `.manage(store)` in main.rs, duplicate `server.rs` at root removed.)
- [x] Generate icons (`npx tauri icon icons/icon-app-1024.png`). Sources in
      `icons/` (app + tray calm/alert for phase 4).
- [x] The window shows *mock* data with the row UI. Seeded in the store
      (`main.rs`, only under `debug_assertions`) — exercises the full pipeline.
      Remove in phase 1 (marked with TODO).

## Phase 1 — Receive real hooks ✅
- [x] axum server listening on `127.0.0.1:8787` (`/health` responds OK).
- [x] All 8 routes from `docs/HOOKS.md` parse the payload and respond with an empty `200`.
      (The `Notification` ones still need to be exercised with real sessions — phase 2.)
- [x] Merge `hooks/settings.snippet.json` into `~/.claude/settings.json`.
      Backup at `settings.json.bak-pre-cctv-hooks`; the previous
      `UserPromptSubmit` hook (gentle-ai) coexists in the same array.
- [x] A real Claude Code session appears in the window and changes state.
      Phase 0 mocks removed from `main.rs`.

## Phase 1b — Hybrid source (Agent View)
- [x] Watcher for `~/.claude/jobs/` integrated (`jobs.rs`, crates `notify` + `dirs`).
- [x] `state.json` schema verified empirically (sessionId camelCase,
      detail/intent, createdAt/updatedAt RFC3339; `name` field also present).
- [x] "Background wins" rule implemented in `set_background_snapshot()`.
- [x] Reaper TTL scoped to foreground instances only.
- [x] `Completed` and `Error` states added to `InstanceState` with urgency.
- [x] Discrete `bg`/`fg` badge on each row in the UI.
- [x] Translations for `state.completed` and `state.error` in all 8 languages.
- [x] Exercise unobserved background states with real sessions.
      Finding: `state` alone does not distinguish permission from input — the key is the
      combination `state`+`tempo` (working+blocked → permission; blocked+blocked
      → input). `map_state` adjusted; `needs` field used as detail.

## Phase 2 — State machine + live UI ✅
- [x] `state.rs` transitions complete and tested with real sessions
      (foreground via hooks; background via experiments with `claude --bg` + `claude stop`).
- [x] `emit("instances", ...)` and the frontend renders changes live.
- [x] Sort by urgency (permission > input > error > working > unknown >
      idle > completed).
- [x] Derive project name from `cwd`: `$HOME` → `~`, abbreviated to the
      last 2 segments when deeply nested. With unit tests (`cargo test`).
- [x] Tool detail summary (`tool_name` + trimmed `tool_input`),
      verified live ("Bash · git ls-remote …").

## Phase 3 — Dead sessions ✅
- [x] Reaper TTL: stale `Working` → `Unknown`; too stale → remove. Covered
      with 7 unit tests (TTL, foreground-only scope, merge rule).
- [x] Tested by killing a session forcefully (`kill -9`, without `SessionEnd`):
      `working` → `unknown` verified live at ~230s via
      `GET /debug/snapshot` (new introspection endpoint, loopback only).
- Note: the store is pure memory — restarting the app clears foreground instances
      until their sessions emit the next hook. This is expected behavior, not a bug.

## Phase 4 — System tray and preferences ✅
- [x] Icon reflects state: calm (tray-calm-64.png) when attention_count==0,
      alert (tray-alert-64.png) when >0. Numeric title in macOS next to the icon.
- [x] Centralized propagation in `refresh.rs::refresh()`: replaces the three
      scattered emit points (server.rs, jobs.rs, main.rs reaper).
- [x] Menu toggles wired up:
      - `floating_window`: show/hide window (already working, verified).
      - `always_on_top`: set_always_on_top (already working, verified).
      - `auto_hide`: hides the window when attention==0; shows it when >0
        (only if `floating_window` is active). Applies immediately on toggle.
      - `compact`: emits a "prefs" event to the frontend; applies `.compact` CSS class
        (hides `.detail`, reduces row padding).
      - `open_at_login`: uses `tauri-plugin-autostart` (enable/disable via
        `ManagerExt::autolaunch()`).
- [x] `PrefsState` as managed state (`Mutex<Prefs>`): refresh() reads prefs without
      disk I/O on each hook event.
- [x] `config.rs`: added `load_from_path()` and `default_prefs_path()` to
      initialize the managed state before setup().
- [x] Frontend: `Prefs` type in types.ts, `onPrefs`/`fetchPrefs` in ipc.ts,
      `compact` prop in MonitorWindow, `.compact` class in styles.css.
- [x] 4 new tests in `refresh.rs` (tray_variant + Prefs serde). Total: 15.

## Phase 5 — Platform polish
- [x] macOS: float over fullscreen. Integration of `tauri-nspanel` (branch
      `v2.1`, commit `a3122e89`). `macos.rs` converts the `WebviewWindow` into an
      `NSPanel` subclass via `to_panel::<MonitorPanel>()` and configures it with:
      - `StyleMask::empty().nonactivating_panel()` — does not steal focus.
      - `PanelLevel::Status` (25) — penetrates the fullscreen Space.
      - `CollectionBehavior::can_join_all_spaces + full_screen_auxiliary + stationary`.
      The NSWindow+FullScreenAuxiliary+level 101 approach was discarded after
      empirical verification: the window disappears anyway when entering fullscreen
      even when the bits are correctly applied (confirmed by logs). NSPanel is
      required by macOS for this guarantee.
      Manual verification pending (requires GUI fullscreen).
- [ ] Linux/Wayland: Hyprland rule documented and actual `class` verified.
- [ ] Windows: build and test.
- [ ] Packaging: `.dmg` / `.AppImage`+`.deb` / `.msi`.

## Ideas / backlog
- Click on a row → bring that terminal to the foreground (difficult cross-platform) or
  copy the `cwd`.
- Session time history (SQLite) for metrics.
- Desktop notification when transitioning to `WaitingPermission` (via the hook's own
  `terminalSequence`, or native app notification).
- Sub-agents: `SubagentStart`/`SubagentStop` as nested sub-rows.
