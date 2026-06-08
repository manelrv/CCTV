# CLAUDE.md

Project context for Claude Code. Read this entirely before touching anything.

## What this is

A desktop app that **monitors all Claude Code instances running on the machine** and shows,
in an always-on-top floating window (plus a system tray icon), the status of each one:
whether it is working, waiting for the user to approve something, waiting for input, or done.

The goal is to stop checking terminal after terminal to find out which agent needs attention.

## Architecture in one sentence

Instance state arrives from **two sources**: background sessions are read from
`~/.claude/jobs/` (file watcher), and foreground sessions arrive via **HTTP hooks**
from `~/.claude/settings.json`. Both are merged into a single store that pushes
snapshots to the window (webview) via Tauri events.

```
Claude Code — bg sessions          Claude Code — fg sessions
  ~/.claude/jobs/<id>/state.json        HTTP hooks (POST localhost:8787)
        │  file watcher                       │
        ▼                                     ▼
Tauri app (process always alive in the system tray)
  ├── jobs.rs    → Source A: watcher + parse state.json (notify + dirs)
  ├── server.rs  → Source B: receives hooks (axum)
  ├── state.rs   → hybrid store + merge rule "background wins" + reaper TTL
  ├── tray.rs    → icon + preferences menu
  └── webview    → React floating window (receives snapshots via event)
```

See `docs/ARCHITECTURE.md` for details, `docs/HOOKS.md` for the payload schema,
and `docs/DATA-SOURCES.md` for the source merge rule.

## Stack

- **Tauri 2** (Rust + webview). Chosen for minimal footprint (the app is always
  running) and native support for frameless, transparent, always-on-top windows
  and system tray. If a migration to Electron is ever decided, the React frontend
  can be reused as-is.
- **Backend:** Rust with `axum` (HTTP server for hooks) + `tokio`.
- **Frontend:** React + TypeScript + Vite. Plain CSS (no framework) in
  `src/styles.css`.

## Platforms (in priority order)

1. **macOS** — primary target. Everything works out of the box.
2. **Linux** — X11 works directly. **Wayland: always-on-top depends on the compositor.**
   For Hyprland this is handled via compositor rules, not the window API.
   See `docs/ARCHITECTURE.md#linux--wayland`.
3. **Windows** — no friction.

## App languages

The app must be **multilingual**. The following languages must be present:

1. English
2. Spanish
3. Portuguese
4. German
5. French
6. Italian
7. Catalan
8. Russian

Default language:

1. English

## App architecture

The document `docs/ARCHITECTURE.md` describes the app architecture. Keep it up to date.

## Working rules

- Keep `docs/ROADMAP.md` updated: mark what is done, add whatever you discover.
- The hook schema is the source of truth: if something does not match
  `docs/HOOKS.md`, **verify against the official docs** before improvising types.
  Docs: https://code.claude.com/docs/en/hooks
- Never block Claude Code: the HTTP endpoint must respond `200` with an empty body
  immediately. All logic goes after the response, or in a separate task. A slow
  hook slows down the user's session.
- `TODO(claude-code):` markers scattered through the code mark what remains to be
  implemented. Find them with grep.

## How to start (dev)

```bash
npm install
npm run tauri dev      # starts Vite + compiles Rust + opens the app
```

For hooks to reach the server, the config in `hooks/settings.snippet.json` must be
merged into `~/.claude/settings.json` (see `docs/HOOKS.md#instalación`).

## Current status

**Functionally complete on macOS.** Both sources (jobs watcher + HTTP hooks),
the merge store, reaper TTL, dynamic tray icon, desktop notifications,
click-to-copy / click-to-focus, context-token display, and fullscreen float
(NSPanel) all work and are verified against real sessions. Tray preferences are
live: floating window, always on top, auto-hide, compact, open at login, theme,
opacity, and **language** (auto-detect + manual override). 48 passing tests.

Pending: Linux/Wayland (Hyprland rules), Windows build. See `docs/ROADMAP.md`
for the full phase history and backlog.
