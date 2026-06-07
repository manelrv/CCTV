//! Hook payload types (common fields + those we actually use).
//! serde ignores unknown fields, so we only model what we need.
//! Schema verified at: https://code.claude.com/docs/en/hooks

use serde::Deserialize;

#[derive(Deserialize, Default, Debug)]
pub struct HookPayload {
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub hook_event_name: Option<String>,

    // Path to the .jsonl conversation transcript for this session.
    pub transcript_path: Option<String>,

    // Tool events
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,

    // Notification
    pub message: Option<String>,

    // SessionEnd / other
    pub reason: Option<String>,

    // Terminal env fields — injected by hooks/session-env.sh on SessionStart
    // and UserPromptSubmit. Absent on all other hook types.
    /// Value of $TERM_PROGRAM in the claude process env (e.g. "iTerm.app", "Apple_Terminal").
    pub term_program: Option<String>,
    /// Value of $ITERM_SESSION_ID or $TERM_SESSION_ID (e.g. "w0t0p0:UUID").
    pub term_session_id: Option<String>,
    /// TTY of the claude process parent (e.g. "ttys003"). May be absent for detached sessions.
    pub tty: Option<String>,
    /// Terminal focus deep link (e.g. "warp://session/<32hex>" for Warp). When present,
    /// `open <url>` brings the exact pane to the foreground. Injected from $WARP_FOCUS_URL.
    pub focus_url: Option<String>,
}

/// Resumen corto para mostrar en la fila: "Bash · npm test", "Edit · src/app.ts".
pub fn summarize_detail(p: &HookPayload) -> Option<String> {
    let tool = p.tool_name.as_deref()?;
    let input = p.tool_input.as_ref();
    let arg = input.and_then(|v| match tool {
        "Bash" => v.get("command").and_then(|c| c.as_str()).map(shorten),
        "Edit" | "Write" | "Read" => {
            v.get("file_path").and_then(|c| c.as_str()).map(shorten)
        }
        "Grep" => v.get("pattern").and_then(|c| c.as_str()).map(shorten),
        "Glob" => v.get("pattern").and_then(|c| c.as_str()).map(shorten),
        "WebFetch" => v.get("url").and_then(|c| c.as_str()).map(shorten),
        _ => None,
    });
    Some(match arg {
        Some(a) => format!("{tool} · {a}"),
        None => tool.to_string(),
    })
}

fn shorten(s: &str) -> String {
    const MAX: usize = 48;
    let s = s.trim();
    if s.chars().count() > MAX {
        let truncated: String = s.chars().take(MAX - 1).collect();
        format!("{truncated}…")
    } else {
        s.to_string()
    }
}
