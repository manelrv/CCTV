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

---

_Final verification: `cargo check` 0 errors · `cargo test` 30/30 · `tsc --noEmit` 0 errors · `npm run build` clean._
