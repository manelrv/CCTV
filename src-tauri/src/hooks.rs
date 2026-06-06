//! Tipos de los payloads de los hooks (campos comunes + los que usamos).
//! serde ignora campos desconocidos, asi que solo modelamos lo necesario.
//! Esquema verificado: https://code.claude.com/docs/en/hooks

use serde::Deserialize;

#[derive(Deserialize, Default, Debug)]
pub struct HookPayload {
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub hook_event_name: Option<String>,

    // Eventos de herramienta
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,

    // Notification
    pub message: Option<String>,

    // SessionEnd / otros
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
