# Hooks: schema and state mapping

Source of truth verified against https://code.claude.com/docs/en/hooks
(reviewed for Claude Code v2.1.x). If any field doesn't match at runtime,
check the official docs before changing types.

## Why HTTP hooks

Claude Code supports `http`-type hooks: instead of a shell script, it makes a
POST with the event JSON as the body. This avoids glue scripts — the app listens
directly. Important detail about their semantics:

> Non-2xx responses, connection failures, and timeouts all produce non-blocking
> errors that allow execution to continue.

In other words, **if the app is closed the hook fails silently and does not block the agent.**
We still set a short `timeout` and the endpoint responds with an empty `200` immediately.

## Common fields (all events)

Delivered in the POST body as `application/json`:

| Field             | Notes                                                        |
| ----------------- | ------------------------------------------------------------ |
| `session_id`      | **The store key.** Identifies the instance.                  |
| `transcript_path` | Path to the conversation `.jsonl` file.                      |
| `cwd`             | Working directory → used to derive the project name.         |
| `permission_mode` | `default` / `plan` / `acceptEdits` / `auto` / ...           |
| `hook_event_name` | Name of the event that fired.                                |

Optional in sub-agents: `agent_id`, `agent_type`.

## Events we use and which state they map to

| Event (matcher)                       | Endpoint                              | Resulting state         |
| ------------------------------------- | ------------------------------------- | ----------------------- |
| `SessionStart`                        | `POST /hooks/session-start`           | `Idle` (just opened)    |
| `UserPromptSubmit`                    | `POST /hooks/user-prompt`             | `Working`               |
| `PreToolUse` (`*`)                    | `POST /hooks/pre-tool`                | `Working` (+ detail)    |
| `PostToolUse` (`*`)                   | `POST /hooks/post-tool`               | `Working` (heartbeat)   |
| `Notification` (`permission_prompt`)  | `POST /hooks/notification/permission` | `WaitingPermission`     |
| `Notification` (`idle_prompt`)        | `POST /hooks/notification/idle`       | `WaitingInput`          |
| `Stop`                                | `POST /hooks/stop`                    | `Idle` (turn closed)    |
| `SessionEnd`                          | `POST /hooks/session-end`             | (removed from store)    |

### Why a separate URL per Notification subtype

The `Notification` `matcher` filters by **notification type**
(`permission_prompt`, `idle_prompt`, `auth_success`, `elicitation_*`). To avoid
depending on how the subtype arrives inside the body, we register one HTTP hook
per matcher and encode the subtype **in the path**. That way the server knows the
state from the endpoint, regardless of the payload shape.

> TODO(claude-code): if you confirm the subtype comes in a body field,
> you can consolidate into a single `/hooks/notification` and read it from there.

### Relevant event-specific fields

- `SessionStart`: + `source` (`startup`/`resume`/`clear`/`compact`), `model`.
- `PreToolUse` / `PostToolUse`: + `tool_name`, `tool_input` (and `tool_response`
  in post). We use `tool_name` + a summary of `tool_input` as the "detail" shown
  in the row (e.g. `Bash · npm test`, `Edit · src/app.ts`).
- `Notification`: includes a `message` field (notification text).
- `Stop`: + `stop_hook_active` (bool).
- `SessionEnd`: + `reason`.

## State machine

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

Rules:

- Any `PreToolUse`/`PostToolUse`/`UserPromptSubmit` updates
  `last_event_at` and sets `Working`.
- `permission_prompt` and `idle_prompt` are the two states that "need attention" →
  they rise to the top of the list and trigger the auto-show of the floating window.
- `Stop` → `Idle` (the turn ended; your next move).

## Dead sessions (reaper TTL)

If you kill the process or it crashes, **`SessionEnd` does not always arrive**. That's
why a reaper periodically checks `last_event_at`:

- If it's `Working` and has had no events for > `STALE_SECS` → `Unknown` (grey).
- If it has had no events for > `REMOVE_SECS` in any state → removed.

Constants in `src-tauri/src/state.rs`.

## Installing the hooks

Merge the contents of `hooks/settings.snippet.json` into your
`~/.claude/settings.json` (merge the `hooks` key, do not overwrite it if you
already have others). Verify with `/hooks` inside Claude Code that they appear
as `User` source.
