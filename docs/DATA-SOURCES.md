# Data sources: hybrid (Agent View + hooks)

Instance state comes from **two places** and is merged into a single
store. This is necessary because neither source alone covers all sessions.

## Why two sources

- **Background sessions** (launched with `/bg`, `claude --bg`, or from
  Agent View itself): managed by the Claude Code **supervisor**, which already
  persists their state to disk. We don't need hooks for these: we read their
  files.
- **Foreground sessions** (a normal `claude` in a terminal): NOT managed by
  the supervisor, so they don't appear in its files. For these we keep using
  **HTTP hooks** (see `docs/HOOKS.md`).

A user mixing both workflows needs both sources.

## Source A — supervisor files (background)

Agent View persists state under the Claude Code config directory:

| File                             | Contents                                               |
| -------------------------------- | ------------------------------------------------------ |
| `~/.claude/daemon/roster.json`   | List of running sessions (for reconnecting)            |
| `~/.claude/jobs/<id>/state.json` | Per-session state that feeds the Agent View table      |
| `~/.claude/daemon.log`           | Supervisor logs                                        |

The documentation explicitly states that **you can read those files from a
script to build your own automations**. That is exactly what we do:
we watch `~/.claude/jobs/` (and `roster.json`) with a file watcher, parse the
`state.json` files, and map their state to ours. The supervisor already handles
the state machine, transitions, and cleanup; we just read and display.

> TODO(claude-code): the exact schema of `state.json` is NOT documented
> field by field. Before fixing the types in `jobs.rs`, open a real
> `state.json` (launch a session with `claude --bg "echo hello"` and inspect
> the file) and adjust the `JobState` struct. Parse defensively (everything
> as `Option`).
> `claude daemon status` (v2.1.141+) also dumps subsystem state.

### Agent View states → ours

Agent View exposes: Working (animated), Needs input (yellow), Idle (dimmed),
Completed (green), Failed (red), Stopped (gray).

| Agent View   | InstanceState (ours)    | Color  |
| ------------ | ----------------------- | ------ |
| Working      | `working`               | green  |
| Needs input  | `waiting_input`         | amber  |
| (blocked)    | `waiting_permission`    | red    |
| Idle         | `idle`                  | gray   |
| Completed    | `completed`             | green  |
| Failed       | `error`                 | red    |
| Stopped      | `unknown`               | gray   |

> Note: Agent View distinguishes "blocked" (filter `s:blocked`). If `state.json`
> separates it from "needs input", map it to `waiting_permission`; otherwise,
> everything that requests input goes to `waiting_input`.

## Source B — HTTP hooks (foreground)

Same as in `docs/HOOKS.md`. The only difference is that these instances are
marked with `source = "foreground"`.

## Merging in the store

Each `Instance` carries a `source` field: `background` | `foreground`.

Rule: **background wins**. A session lives in one source or the other, not in
both at the same time (when a foreground session is sent to the background, it
loses its terminal and is handed off to the supervisor). Implementation:

- The Source A watcher produces the complete set of background sessions on each
  rescan and calls `set_background_snapshot(...)`: replaces all `background`
  entries and removes any `foreground` entry that shares an `id`.
- Source B hooks call `apply(...)` with `source = foreground`.
- The **TTL reaper** applies to `foreground` entries (foreground sessions can
  die without a `SessionEnd`) and, as the single exception for `background`,
  to background jobs in a **terminal state** (`done`/`stopped`/`failed`):
  verified empirically, the supervisor NEVER deletes the `state.json` of
  finished jobs, so without a TTL they would accumulate forever. Both
  `jobs::scan()` (skip on read) and `reap()` (expire over time) apply the
  same `REMOVE_SECS` threshold. Active background jobs are never reaped.

## UI

The row shows a discreet source label: `bg` / `fg`, so you can tell at a glance
which ones you can reopen with `claude agents` and which ones live in a
terminal of yours.

> Backlog: Agent View also shows a color dot with the PR status opened by a
> session (yellow/green/purple/gray). If `state.json` includes it,
> it would be a nice extra on the row.
