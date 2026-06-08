# WORKLOG — CCTV

Chronological log of work completed. Format: date + phase + concise bullets.

---

## 2026-06-06

### Phase 0 — Compilation and initial startup

- Fixes to get `npm run tauri dev` to compile:
  - Removed spurious `[lib]` section from `Cargo.toml`.
  - Added `macos-private-api` feature to the tauri dependency.
  - Added `.manage(store)` in `main.rs` (missing for the `get_instances` command).
  - Removed duplicate `server.rs` that was at the project root.
- Icons generated with `npx tauri icon icons/icon-app-1024.png`.
- Window shows mock data (seeded into the store under `debug_assertions`) — full pipeline working.

### Phase 1 — Real hooks

- Axum server listening on `127.0.0.1:8787`; `/health` responds OK.
- All 8 routes from `docs/HOOKS.md` parse payload and respond with an immediate empty `200`.
- Hooks merged into `~/.claude/settings.json` with a prior backup (`settings.json.bak-pre-cctv-hooks`).
- Pre-existing `UserPromptSubmit` hook (gentle-ai) coexists in the same array without conflict.
- Real Claude Code session verified: appears in the window and changes state.
- Phase 0 mocks removed from `main.rs`.

### i18n + rename

- App renamed from "Claude Code Monitor" to "CCTV" (name visible in the UI).
- 8 languages implemented: en, es, pt, de, fr, it, ca, ru.
- Key structure: `state.*`, `summary.*`, `empty`.
- Russian has 3 plural forms correctly configured.

### Phase 1b — Hybrid source (Agent View)

- Real schema of `~/.claude/jobs/<id>/state.json` verified empirically with
  session `be4c186b`. Key fields: `sessionId` (camelCase), `state`, `detail`,
  `intent`, `name`, `cwd`, `createdAt`, `updatedAt` (RFC3339), `daemonShort`.
  The `status`, `summary`, `title` fields assumed by the scaffold DO NOT exist.
- `jobs.rs` corrected: `JobState` struct with `serde(rename_all = "camelCase")`;
  defensive `Option` fields; detail with fallback `detail → intent → name`.
- RFC3339 timestamps → epoch secs implemented with a manual parser (no chrono).
  Fallback to file mtime when missing or unparseable.
- `state.rs` extended:
  - `Source { Background, Foreground }` enum added and serialized.
  - `Instance` gains `source: Source` field.
  - `InstanceState` gains `Completed` and `Error` with correct urgency.
  - `apply()` marks `source: Foreground` on insert (hooks path).
  - `set_background_snapshot()`: removes prior background + overlapping foreground, inserts new set.
  - `reap()`: only touches Foreground instances.
  - `project_from_cwd` changed to `pub(crate)` (imported from `jobs.rs`).
- `main.rs`: added `mod jobs` and `jobs::start(store, handle)` call after server spawn.
- `Cargo.toml`: added `notify = "6"` and `dirs = "5"`.
- Frontend:
  - `types.ts`: `InstanceState` gains `"completed"` and `"error"`; `Instance` gains `source: Source`.
  - `InstanceRow.tsx`: `bg`/`fg` badge next to the project name.
  - `styles.css`: `.s-completed` (soft green) and `.s-error` (red) + `.source-badge`.
  - 8 i18n files: `state.completed` and `state.error` added.
- Docs: `ARCHITECTURE.md` updated with hybrid sources section and `jobs.rs` module;
  `CLAUDE.md` ASCII diagram updated to show both sources;
  `ROADMAP.md` Phase 1b section added.

### Repo

- `git init` + `.gitignore` + initial commit `b0555f5` (104 files).

### Phase 2 — State machine + live UI

- Live hybrid smoke test: 4 real instances (3 fg + 1 bg) with urgency ordering,
  bg/fg badge, and tool detail working.
- `project_from_cwd`: `$HOME` → `~` and abbreviation to last 2 segments on deep
  paths (`~/…/CCTV/src-tauri`). First unit tests in the project (4, `cargo test`).
- Remaining bg states verified empirically with real jobs
  (`claude --bg` + `claude stop`):
  - `stopped` (manually stopped), `failed` (invalid model), `blocked` (question).
  - Key finding: permission vs input are NOT distinguished by `state` —
    `working`+`tempo=blocked` → permission; `blocked`+`blocked` → input.
  - `map_state(state, tempo)` rewritten; `needs` field (question or
    "approve Tool: path") used as priority detail.
- CLI footgun documented: `claude --bg --help` launches a real job instead of
  showing help; stop is `claude stop <id>` (not a subcommand of `agents`).

### Phase 3 — Dead sessions (reaper)

- 7 new unit tests: TTL stale/remove, foreground-only scope of the reaper,
  and "background wins" rule of `set_background_snapshot` (11 tests total).
- `GET /debug/snapshot` endpoint added in `server.rs`: store introspection via
  curl, loopback only. Essential for verifying without looking at the UI.
- Real test: headless session (`claude -p`) killed with `kill -9` (no
  `SessionEnd`) → `working` → `unknown` after ~230s. Verified via snapshot.
- Discoveries:
  - `claude -p` (headless) DOES fire hooks — appeared in the store when launched.
  - Restarting the app clears fg instances (in-memory store); they reappear
    with the next hook from each live session. Expected, not a bug.
  - macOS does not have `timeout` (coreutils); watch out in test scripts.
  - `claude -p --debug "prompt"` parses incorrectly: `--debug` swallows the prompt.
    Correct order: `claude --debug hooks -p "prompt"`.

## 2026-06-07

### Phase 4 — System tray and preferences

- New `refresh.rs`: single propagation point (webview + tray icon + numeric title +
  auto-hide). Replaces the 3 scattered emissions that existed in `server.rs`,
  `jobs.rs`, and the reaper in `main.rs`.
- Dynamic tray icon: calm/alert based on `attention_count()`, embedded with
  `include_bytes!` from `icons/`. Counter as title on macOS.
- All 5 menu toggles wired (floating, on-top, auto-hide, compact,
  autostart via `tauri-plugin-autostart`).
- `PrefsState` (Mutex managed state): prefs in memory, zero disk I/O per hook event.
- Compact mode in frontend: "prefs" event + `.compact` CSS class.
- 4 new tests (15 total).
- Gotchas: `try_state()` returns `Option`, not `Result`; the autostart plugin
  trait is `autolaunch()`; `include_bytes!` paths from `src/` are `../../icons/`.

### Phase 5 — Float over fullscreen on macOS

- Final recipe (ALL THREE pieces are required):
  1. **NSPanel** via `tauri-nspanel` plugin (branch v2.1): a regular Tauri NSWindow
     does NOT enter the fullscreen Space even with collectionBehavior
     0x101 (AllSpaces|FullScreenAuxiliary) and level 101 — verified
     empirically with logs. macOS restricts this to NSPanel subclasses
     (undocumented by Apple).
  2. **ActivationPolicy::Accessory** — menubar utility, no Dock icon.
  3. **Main-thread dispatch**: the raw panel handle does direct msg_send;
     calling show()/hide() from tokio/watcher/reaper threads aborts with
     SIGTRAP. Tauri APIs re-dispatch internally; the panel does NOT.
     Solution: `refresh::set_panel_visible()` with `run_on_main_thread` and the
     panel resolved inside the closure.
- Non-activating panel: clicking the monitor does not steal focus from the active app.
- Crash diagnosed via `~/Library/Logs/DiagnosticReports/*.ips`
  (exit 133 = SIGTRAP; faultingThread showed apply_auto_hide → orderOut).

### TTL filter for finished background jobs

- Discovery: the supervisor NEVER deletes the state.json of finished jobs —
  the design assumption "background is cleaned by the supervisor file
  lifecycle" was wrong, so finished jobs accumulated in the window forever.
- Fix in two pieces (one alone is not enough):
  - `jobs::scan()` skips terminal jobs (done/stopped/failed) older than
    `REMOVE_SECS` — prevents loading/re-inserting fossils on disk events.
  - `reap()` also expires terminal background instances by the same TTL —
    covers time passing without any filesystem event.
- Recent completions stay visible (useful feedback); 4 new tests (19 total).
- Verified live: store went from 8 instances (7 fossils) to the 2 real ones.

### Context occupancy label per row

- **What**: Each instance row now shows a muted monospace token count (e.g. `304k`)
  next to the elapsed time, reflecting the current context window occupancy.
- **Why**: Lets users know at a glance how full each session's context is, without
  opening a terminal.
- **Slug rule (empirically verified)**: Claude Code derives the transcript directory slug
  from the cwd by replacing every `/` with `-`. Since cwd is always absolute (starts with
  `/`), the slug starts with `-`. No dots are introduced.
  Example: `/Users/manelrv/side-projects/CCTV/src-tauri` →
  `-Users-manelrv-side-projects-CCTV-src-tauri`. Verified by cross-checking
  `~/.claude/jobs/be4c186b/state.json` (cwd) against
  `~/.claude/projects/-Users-manelrv-side-projects-CCTV-src-tauri/be4c186b-…-….jsonl`.
- **Token formula**: `input_tokens + cache_read_input_tokens + cache_creation_input_tokens`
  from the last JSON line with a `message.usage` field. Observed example: 2 + 37545 + 10027 = 47574.
- **New module**: `src-tauri/src/transcript.rs` — `cwd_to_slug`, `transcript_path`,
  `read_context_tokens` (seeks to last 256 KiB, skip first partial line, scan for last usage).
- **Background jobs** (`jobs.rs`): tokens read synchronously in `scan()` (runs on its own
  thread; small job count).
- **Foreground hooks** (`server.rs`): tokens read in a throttled `spawn_transcript_read`
  (at most once per session per 10 s). The handler responds 200 immediately; the spawn
  happens after.
- **State**: `Instance` gains `context_tokens: Option<u64>` (serialized; None → null).
  `Store::set_context_tokens` updates the value without touching `last_event_at`. `apply()`
  does not clobber an existing value on re-entry.
- **Frontend**: `Instance.context_tokens: number | null`; `formatTokens` helper in
  `types.ts` (<1000 → as-is, ≥1000 → `Math.round(n/1000)+"k"`); `.ctx-tokens` CSS class
  (10px mono, `--text-faint`, 0.7 opacity); visible in both normal and compact modes.
- **Test count**: 19 → 30 (+11: 4 state tests, 7 transcript tests).
- **Dev dependency added**: `tempfile = "3"` for `.jsonl` fixture files in transcript tests.

### Desktop notifications on attention transition

- **What**: fires one native OS notification per instance the moment it enters
  `WaitingPermission` or `WaitingInput`. Does NOT spam — a session that stays
  in attention across multiple refreshes only notifies once.
- **Plugin**: `tauri-plugin-notification = "2"` (crate: `tauri-plugin-notification v2.3.3`,
  uses `notify-rust` + `mac-notification-sys` on macOS). Registered via
  `tauri_plugin_notification::init()` in `main.rs`. Capability entry
  `"notification:default"` added to `capabilities/default.json`.
- **Transition detection** (`refresh.rs`):
  - `AttentionState(Mutex<HashSet<String>>)` — managed state tracking the set of
    session_ids currently in attention.
  - `newly_attention(prev, current) -> Vec<String>` — pure diff function; returns
    ids in `current` but not in `prev`. Unit-tested (4 new tests).
  - On each `refresh()` call: build current set from snapshot, diff vs stored,
    notify new entries, replace stored with current.
- **Notification content**: title = `instance.project` (e.g. `~/dev/CCTV`);
  body = `instance.detail` if present, else localized fallback from `i18n.rs`
  (`notif_permission` / `notif_input`).
- **i18n**: two new strings in `TrayStrings` for all 8 languages (en/es/pt/de/fr/it/ca/ru).
- **Threading**: dispatched via `run_on_main_thread` — same precaution as
  `set_panel_visible`. Apple recommends main-thread access for
  `UNUserNotificationCenter`; the SIGTRAP lesson from NSPanel applies.
- **First-launch behavior**: instances already in attention when the app starts
  ARE notified on first refresh. Decision: the user just opened the app;
  knowing what is pending immediately is the right behavior.
- **Test count**: 30 → 34 (+4 `newly_attention` tests).
- **GOTCHA (verified by elimination)**: macOS silently drops notifications from
  non-bundled binaries — the `tauri dev` raw binary gets `Ok()` from `show()`,
  no permission prompt, nothing shown, no error anywhere. Only the bundled
  `.app` (`npm run tauri build` → `target/release/bundle/macos/CCTV.app`)
  prompts for permission and actually notifies. Verified live with the bundle.
- **Testing trick**: fake hooks via curl to `/hooks/notification/permission`
  with an arbitrary session_id force attention transitions end-to-end without
  real sessions.

### Readable elapsed times

- Times ≥60 min render as "17h 18m" instead of unreadable "1038:19" (`InstanceRow`).

### Click-to-copy row

- **What**: Clicking any instance row copies a useful payload to the clipboard and shows
  a brief "Copied" confirmation in the state label area for 1.2 s before reverting.
  - Background rows (`source === "background"`): copies `claude attach <shortId>` where
    `shortId` is the first UUID segment of `session_id` (verified: equals `daemonShort`).
  - Foreground rows: copies the instance `cwd`.
- **Plugin**: `tauri-plugin-clipboard-manager = "2"` (Rust) + `@tauri-apps/plugin-clipboard-manager`
  (npm). Registered via `tauri_plugin_clipboard_manager::init()` in `main.rs`.
  Capability: `"clipboard-manager:allow-write-text"` in `capabilities/default.json`.
  Uses `writeText()` from the plugin — NOT `navigator.clipboard` (flaky in WKWebView).
- **Pure helper**: `copyPayload(inst: Instance): string` added to `types.ts` — trivially
  testable without a JS test runner.
- **i18n**: `copied` key added to all 8 locale files:
  en "Copied" · es "Copiado" · pt "Copiado" · de "Kopiert" · fr "Copié" ·
  it "Copiato" · ca "Copiat" · ru "Скопировано".
- **InstanceRow.tsx**: `onClick` handler, `copied` boolean state (useState), cursor pointer
  via inline style, `t("copied")` replaces the state label during the 1.2 s window.
- **NSPanel note**: the monitor is a non-activating NSPanel — clicking does not steal focus
  from the active app, but mouse events do reach the webview, so click-to-copy works
  correctly. Requires manual verification.
- **Verification**: `tsc --noEmit` 0 errors · `npm run build` clean · `cargo check` 0 errors
  · `cargo test` 34/34 (no new Rust code, test count unchanged).
- **Bonus fix**: tray Quit was dead since the scaffold — `ExitRequested` was prevented
  unconditionally (the keep-alive-in-tray handler). Now only window-close
  (`code: None`) is prevented; explicit `app.exit(0)` passes through.

### Token counter thresholds + inFlight badge

- **Token colors**: `tokenLevel()` in types.ts — amber at 75%, red at 90% of the
  inferred window. The transcript model field does NOT distinguish 200k from 1M
  variants (verified: a 307k session reports plain "claude-opus-4-8"), so the
  window is inferred: >200k tokens ⇒ 1M. Monotonic per session; known limitation:
  a 1M session between 150k–200k warns early until it crosses 200k.
- **inFlight badge**: bg jobs show "⚙ N" (tasks + queued) next to the source badge
  while the supervisor runs subtasks. Shape verified empirically:
  `inFlight: {"tasks": 1, "queued": 0, "kinds": ["local_bash"]}` during a live
  shell task. Parsed in `jobs.rs` (`InFlight` struct), `Instance.in_flight_tasks`,
  end-to-end verified via `/debug/snapshot`.

### Click-to-focus (macOS terminal focus on row click)

- **What**: Clicking a foreground row that has terminal info now brings the terminal
  window/tab hosting that Claude Code session to the foreground instead of copying.
  Alt+click, background rows, and rows with no terminal info keep the copy behavior.
  If focus fails (AppleScript error, Automation permission denied, non-macOS), falls back to copy.
- **Env-capture hook** (`hooks/session-env.sh`): new command-type hook script that enriches
  the Claude Code hook payload with `term_program`, `term_session_id`, and `tty` by reading
  the claude process environment (`$TERM_PROGRAM`, `$ITERM_SESSION_ID`/`$TERM_SESSION_ID`,
  `ps -o tty= -p $PPID`). Python3 used for JSON safety (values passed via env vars — no
  shell interpolation). Always exits 0; curl has a 2s hard timeout. Silent no-op when app is down.
- **settings.snippet.json**: `SessionStart` and `UserPromptSubmit` converted from `type: http`
  to `type: command` invoking the script. All other hooks remain HTTP.
- **Backend**:
  - `hooks.rs`: `HookPayload` gains `term_program`, `term_session_id`, `tty` (all `Option<String>`).
  - `state.rs`: `TerminalRef { program, session_id, tty }` struct added (Clone, Serialize, PartialEq, Debug);
    `Instance.terminal: Option<TerminalRef>` field added; `Store::set_terminal()` method; `apply()` does
    NOT clobber existing terminal info (sets None only on new inserts); `Store::inner_snapshot_terminal()`
    for the Tauri command lookup; `mk()` test helper updated.
  - `jobs.rs`: `Instance` construction updated with `terminal: None`.
  - `server.rs`: `session_start` and `user_prompt` handlers call `set_terminal()` when `term_program` is present.
  - `focus.rs`: NEW macOS-only module. Three tiers: (1) iTerm2 by session UUID via AppleScript;
    (2) Apple Terminal by tty via AppleScript; (3) generic app activation via TERM_PROGRAM→app name map.
    Injection safety: UUID validated as hex+hyphen 36 chars; tty as alphanumeric+'/'. Values that fail
    validation fall through to the next tier. App names in tier 3 come from a static map — no user data
    is interpolated into that string.
  - `main.rs`: `mod focus` added; `focus_session(session_id)` Tauri command registered.
- **Frontend**:
  - `types.ts`: `Instance.terminal` field added.
  - `InstanceRow.tsx`: `handleClick` converted to async; invokes `focus_session` for foreground+terminal
    rows; falls back to copy if it returns false or throws. Alt+click always copies.
- **New tests**: 4 (set_terminal stores, unchanged returns false, unknown session, apply doesn't clobber).
  Total: 38.
- **Verification**: `cargo check` 0 errors · `cargo test` 38/38 · `tsc --noEmit` 0 errors · `npm run build` clean.
- **GOTCHA**: macOS Automation permission prompt appears on first `focus_session` call; after granting once, it
  is remembered. There is no way to pre-grant this — the user will see the system prompt on first click.
- **BUGFIX (post-implementation)**: the iTerm2 AppleScript used `unique identifier of s`, which is
  a SYNTAX ERROR in iTerm2's dictionary — the correct property is `id of s`. The script failed every
  time and silently fell through to plain app activation (looked like "focus doesn't work"). Diagnosed
  by running the script against a live iTerm2 session; corrected to `id of s` + `set index of w to 1`.
  Verified live: iTerm2 now focuses the exact tab.
- **Warp**: confirmed there is NO fix possible — Warp has no AppleScript/Shortcuts support and no way to
  focus an existing session (warp:// only opens NEW tabs; feature request warpdotdev/warp#8611 pending).
  Warp stays at tier 3 (app activation). iTerm2/Apple Terminal get exact-tab focus.
- **Verified live**: iTerm2 exact-tab focus works. App was NOT broken — old sessions showed `term: None`
  because they registered before the settings.json hook change (hooks load at session start).

### Tier-0 focus URL — generic terminal deep-link focus (Warp)

- **What**: Added a new tier 0 to `focus_terminal()` — higher priority than the
  iTerm2/Terminal/program tiers. Any terminal that exposes a focus deep link gets
  exact-pane focus via `open <url>` through this tier.
- **Warp support**: Warp PR #11130 (merged) exposes `WARP_FOCUS_URL=warp://session/<32hex>`
  (or `warposs://session/<32hex>` for the OSS build) in the claude process environment.
  Running `open "$WARP_FOCUS_URL"` brings the exact Warp pane to the foreground.
  Verified live: `WARP_FOCUS_URL=warp://session/9f6d05b9e7974a4fb0c5c489a44a3dbf`.
- **Changes**:
  - `hooks/session-env.sh`: captures `$WARP_FOCUS_URL` → `focus_url` JSON field.
    Passed via env var to python3 (same injection-safe pattern as the other fields).
  - `hooks.rs`: `HookPayload` gains `focus_url: Option<String>`.
  - `state.rs`: `TerminalRef` gains `focus_url: Option<String>`.
  - `server.rs`: `terminal_ref()` includes `focus_url` from the payload.
  - `focus.rs`: tier 0 added before iTerm2 check; `is_valid_focus_url()` validates
    scheme (`warp://` or `warposs://`), length (< 256), and chars (`[A-Za-z0-9:/._-]`).
    `open` is called via argv — not a shell. 9 new unit tests for the validator.
  - `types.ts`: `terminal.focus_url: string | null` added to the `Instance` type.
- **Design decision**: tier 0 takes priority when `focus_url` is present — it is the
  most direct path (OS deep link). iTerm2 and Apple Terminal generally do not set
  `focus_url`, so they continue using their existing tiers unchanged.
- **Test count**: 38 → 47 (+9 `is_valid_focus_url` tests, +1 `set_terminal_focus_url_round_trips`).
- **Verification**: `cargo check` 0 errors · `cargo test` 47/47 · `tsc --noEmit` 0 errors · `npm run build` clean.

### Auto-resize window height to fit content

- **What**: The floating window now auto-resizes its height to fit the number of
  sessions instead of staying at a fixed 480 px with empty space below.
- **Approach**: measurement-based (ResizeObserver on the root `.panel` div reads
  `scrollHeight`). Bounds: min 120 px (titlebar + summary + empty placeholder),
  max 600 px (rows beyond this scroll within the capped window via `overflow-y: auto`).
- **Feedback-loop guard**: `lastSentHeight` ref — `setSize` is called only when the
  clamped target differs from the last sent value. rAF debounce collapses multiple
  observer callbacks per frame into one resize call. This prevents the classic
  ResizeObserver→setSize→window resizes→observer fires again storm.
- **CSS change**: `.panel` changed from `height: 100%` to `min-height: 100%` so
  `scrollHeight` reflects natural content height, not the constrained OS-window height.
  No other CSS changes were needed — `.list` already had `overflow-y: auto`.
- **Effect deps**: `[instances, compact]` — re-measures on both list and layout changes.
- **macOS NSPanel note**: `setSize` routes through Tauri's JS→IPC→main-thread path,
  so it is safe; no change to panel config needed. Limitation: if the user
  manually drags the window to a different height, auto-resize will re-apply on
  the next instance-list change.
- **Width**: constant at 360 px (matches `tauri.conf.json`); not modified.
- **Files**: `src/components/MonitorWindow.tsx`, `src/styles.css`,
  `docs/ARCHITECTURE.md`.
- **Verification**: `tsc --noEmit` 0 errors · `npm run build` clean ·
  `cargo check` 0 errors · `cargo test` 47/47 (no Rust changes).

### Preferences window — opacity slider + theme selector

- **What**: Dedicated preferences window opened from tray "Preferences…". Two controls:
  opacity slider (30–100%) and theme selector (System / Dark / Light). Both persist to
  `prefs.json` and apply live to the monitor window without restart.
- **Second window** (`tauri.conf.json`): `label: "preferences"`, 320×240, `decorations: true`,
  `resizable: false`, `alwaysOnTop: false`, `visible: false`. Normal OS settings dialog.
- **Routing** (`main.tsx`): `getCurrentWindow().label === "preferences"` → render
  `<Preferences/>`; else render existing `<App/>`. Both share the Vite dist.
- **Prefs model** (`config.rs`):
  - `opacity: u8` — percent 30..=100, default 92 (matches original `--bg` alpha).
  - `theme: String` — "system" | "dark" | "light", default "system".
  - Both use `#[serde(default = "...")]` helper fns so old `prefs.json` (missing fields)
    deserializes to defaults. Backward-compatible.
- **New Tauri commands** (`main.rs`): `set_opacity(u8)` (clamps to 30..=100) and
  `set_theme(String)` (validates against allowed set). Both: update PrefsState, save to disk,
  emit "prefs". `use tauri::Emitter` added (was missing — caught by `cargo check`).
- **Tray** (`tray.rs`): "Preferences…" `MenuItemBuilder` item added before Quit (with separator).
  Handler: `open_preferences_window()` → `get_webview_window("preferences").show() + set_focus()`.
- **i18n** (`i18n.rs`): `preferences: &'static str` field added to `TrayStrings`; 8 translations.
- **Frontend** (`Preferences.tsx`): range input (30–100) + 3 radio buttons. Invokes `setOpacity`
  (debounced 80 ms) and `setTheme` (immediate). Loads current prefs on mount via `fetchPrefs`.
- **Applying opacity** (`MonitorWindow.tsx` + `styles.css`):
  - CSS: `--bg` changed from `rgba(28,28,30,0.92)` to `rgba(28,28,30,var(--panel-opacity))`.
    `--panel-opacity: 0.92` as default.
  - JS: `useEffect([opacity, theme])` sets `document.documentElement.style.setProperty("--panel-opacity", opacity/100)`.
  - Only the panel background alpha changes. Text stays fully opaque.
- **Applying theme** (`MonitorWindow.tsx` + `styles.css`):
  - JS: `document.documentElement.setAttribute("data-theme", "dark"|"light")`.
    `resolveTheme(theme, prefersDark)` helper: "dark"→dark, "light"→light, "system"→OS preference.
  - "system": adds `matchMedia.addEventListener("change", applyTheme)` and removes on cleanup.
  - CSS: `[data-theme="light"]` block overrides `--bg`, `--bg-elev`, `--border`, `--text`,
    `--text-dim`, `--text-faint`. Light palette: `rgba(242,242,247,alpha)` bg, dark text.
    Accent colors unchanged — readable on both palettes.
- **`types.ts`**: `Prefs.opacity: number`, `Prefs.theme: string`, `Theme` type added.
- **`ipc.ts`**: `setOpacity(value: number)` and `setTheme(value: string)` exports. Default
  fallback in `fetchPrefs` updated to include `opacity: 92` and `theme: "system"`.
- **`App.tsx`**: `DEFAULT_PREFS` updated; `opacity` and `theme` props passed to `MonitorWindow`.
- **Locale files** (all 8): `preferences.title`, `preferences.opacity`, `preferences.theme`,
  `preferences.theme_system`, `preferences.theme_dark`, `preferences.theme_light` added.
- **New test**: `prefs_serde_defaults_for_missing_fields` — verifies old JSON without the
  new fields deserializes to `opacity: 92`, `theme: "system"`. Test count: 47 → 48.
- **Verification**: `cargo check` 0 errors · `cargo test` 48/48 · `tsc --noEmit` 0 errors ·
  `npm run build` clean.

## 2026-06-08

### Preferences window removed — theme and opacity moved to tray submenus

- **What**: Reverted the dedicated Preferences window in favour of native tray
  submenus. Opacity is now preset-based (six steps: 100% / 90% / 80% / 70% /
  60% / 50%) because native menus cannot host sliders.
- **tray.rs**: `build_menu()` extracted as a standalone function (called on startup
  and after each change). Theme submenu (`SubmenuBuilder` + three `CheckMenuItemBuilder`)
  and opacity submenu (six presets). After any theme/opacity change the menu is
  rebuilt via `rebuild_menu()` so check marks reflect the new state.
  `open_preferences_window()` helper removed.
- **main.rs**: `set_opacity` and `set_theme` Tauri commands removed (no longer
  called from JS). `use tauri::Emitter` import removed (unused after the removal).
- **tauri.conf.json**: second window definition (`label: "preferences"`) removed.
  Trailing comma fixed.
- **src/components/Preferences.tsx**: deleted.
- **src/main.tsx**: label-based routing removed; always renders `<App/>`.
- **src/lib/ipc.ts**: `setOpacity` / `setTheme` exports removed.
- **i18n.rs**: `preferences` field removed from `TrayStrings`; five new fields
  added: `theme`, `theme_system`, `theme_dark`, `theme_light`, `opacity` — all
  8 languages.
- **Frontend locales** (en/es/pt/de/fr/it/ca/ru): `preferences.*` block removed
  from all 8 files.
- **Verification**: `cargo check` 0 errors · `cargo test` 48/48 · `tsc --noEmit`
  0 errors · `npm run build` clean.

---

_Final verification: `cargo check` 0 errors · `cargo test` 48/48 · `tsc --noEmit` 0 errors · `npm run build` clean._
