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
