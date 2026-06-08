# CCTV

**C**laude **C**ode **T**ele**v**isió — an always-on-top floating window plus a
system tray icon showing the live state of **every Claude Code instance**
running on your machine: working, waiting for your approval, waiting for input,
completed, or failed.

Stop cycling through terminals to find out which agent needs you — the one that
does comes to you.

![states](docs/states.png)
<!-- TODO(claude-code): add an up-to-date UI screenshot -->

## Features

- **Hybrid dual-source monitoring** — covers both kinds of sessions:
  - *Background* (`claude --bg`, Agent View): a file watcher reads the
    `state.json` files the Claude Code supervisor persists under
    `~/.claude/jobs/`. No configuration needed.
  - *Foreground* (regular `claude` in a terminal): HTTP hooks POST to a local
    server embedded in the app. Hook endpoints respond instantly and never
    slow your sessions down.
- **Urgency-ordered list** — waiting for permission > waiting for input >
  error > working > no signal > idle > completed. The row that needs you is
  always on top, with the pending question or `approve Tool: path` as detail.
- **Dynamic tray icon** — calm/alert variants plus a numeric attention counter
  in the macOS menu bar.
- **Preferences, all functional** — floating window, always on top, auto-hide
  when nothing needs attention (auto-show when something does), compact mode,
  open at login.
- **Floats above fullscreen apps (macOS)** — implemented as a non-activating
  `NSPanel`: visible on every Space, never steals focus from your active app.
- **Dead-session reaper** — foreground sessions killed without cleanup turn to
  *No signal* after 3 minutes and are dropped after 30.
- **8 languages** — English (default), Spanish, Portuguese, German, French,
  Italian, Catalan, Russian. Auto-detected from the system locale, or pinned
  manually from the tray's **Language** submenu. Correct plural rules included.
- **Scriptable introspection** — `GET 127.0.0.1:8787/debug/snapshot` returns
  the full store as JSON (loopback only).

## How it works

```
Claude Code — bg sessions            Claude Code — fg sessions
  ~/.claude/jobs/<id>/state.json         HTTP hooks (POST localhost:8787)
        │  file watcher                       │
        ▼                                     ▼
CCTV (always-alive tray process)
  ├── jobs.rs    → source A: watch + parse supervisor state files
  ├── server.rs  → source B: hook receiver (axum)
  ├── state.rs   → hybrid store, "background wins" merge rule, TTL reaper
  ├── refresh.rs → single propagation point: webview, tray icon, auto-hide
  └── webview    → React floating window (event-driven, no polling)
```

Full details in [`CLAUDE.md`](CLAUDE.md), [`docs/DATA-SOURCES.md`](docs/DATA-SOURCES.md)
and [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Requirements

- Node.js 18+
- Stable Rust + the Tauri 2 toolchain (https://v2.tauri.app/start/prerequisites/)

## Development

```bash
npm install
npm run tauri dev
```

Tests live on the Rust side:

```bash
cd src-tauri && cargo test
```

## Connecting the hooks (foreground sessions)

Merge the `hooks` key from
[`hooks/settings.snippet.json`](hooks/settings.snippet.json) into your
`~/.claude/settings.json`. Inside Claude Code, `/hooks` should list them with
source `User`. From then on, every session you open shows up in the window.
Background sessions require no setup.

## Project status

Functionally complete on macOS: hybrid sources verified against real sessions,
state machine and reaper covered by unit tests plus live kill tests, dynamic
tray with working preferences, i18n (auto-detect + manual override), and
fullscreen float. 48 passing tests.

Pending: Linux/Wayland (Hyprland rules drafted in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)), Windows build, and
distribution packaging. Full history in [`WORKLOG.md`](WORKLOG.md), phase
tracking in [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Platforms

macOS (primary) → Linux → Windows.
