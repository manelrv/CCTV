//! Terminal focus: brings the terminal window/tab hosting a Claude Code session
//! to the foreground when the user clicks a foreground row.
//!
//! Four tiers (tried in order of specificity):
//!
//! 0. **Generic focus URL** — if `TerminalRef.focus_url` is set and passes
//!    validation, runs `open <url>` via `std::process::Command` (argv, not shell).
//!    Warp exposes `WARP_FOCUS_URL=warp://session/<32hex>` (or `warposs://…` for
//!    the OSS build) in the claude process environment; the session-env.sh hook
//!    captures it. Any terminal that exposes a similar deep link gets exact-pane
//!    focus through this tier automatically. Returns true if `open` exits 0.
//!
//! 1. **iTerm2** — uses AppleScript to locate the session by its UUID
//!    (the part after "wXtYpZ:" in ITERM_SESSION_ID). Selects the tab and
//!    activates the app. Most precise: targets the exact pane.
//!    NOTE: the session property is `id`, NOT `unique identifier` — the latter
//!    is a syntax error in iTerm2's dictionary and made the whole script fail
//!    (verified empirically against a live iTerm2 session).
//!
//! 2. **Apple Terminal** — uses AppleScript to find the tab whose `tty`
//!    matches "/dev/<tty>" from the captured tty field. Selects that tab
//!    and brings the window to front.
//!
//! 3. **Anything else with a program name** — best-effort: maps known
//!    TERM_PROGRAM values to app names and activates the app via AppleScript.
//!    No tab targeting (we lack a generic cross-terminal protocol for that).
//!
//! INJECTION SAFETY
//! ================
//! AppleScript is invoked via `osascript -e` with the script text built in
//! Rust. Values from the environment (session UUID, tty) are validated against
//! strict allowlists before being interpolated into the script:
//!   - UUID: must match [0-9A-Fa-f-]{36} (standard UUID format).
//!   - tty: must match [A-Za-z0-9/]+ (e.g. "ttys003", "pts/0").
//! Any value that fails validation causes the function to fall back to the
//! next tier (app activation only) rather than risk script injection.
//!
//! THREAD SAFETY
//! =============
//! Tauri commands run on the async runtime (not the main thread). Spawning
//! `std::process::Command` (synchronous) is fine from any thread. The
//! osascript process inherits the current process environment and has no
//! Tauri-internal main-thread requirement.
#![cfg(target_os = "macos")]

use crate::state::TerminalRef;
use std::process::Command;

// ─── Validation helpers ────────────────────────────────────────────────────

/// Returns true if `s` looks like a standard UUID (hex digits + hyphens, 36 chars).
fn is_valid_uuid(s: &str) -> bool {
    s.len() == 36 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Returns true if `s` is a safe tty name (alphanumeric + '/' only, e.g. "ttys003", "pts/0").
fn is_valid_tty(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '/')
}

/// Returns true if `s` is a valid terminal focus URL.
///
/// Accepts only `warp://` and `warposs://` schemes (the two Warp deep link variants).
/// Additional constraints:
/// - Total length < 256 characters.
/// - Only URL-safe characters: `[A-Za-z0-9:/._-]` — no spaces, no shell metacharacters.
///
/// Any value that fails these checks is rejected and focus falls through to lower tiers.
/// The URL is passed to `open` via argv (not a shell), so injection via shell
/// metacharacters is not possible — but validation is defence-in-depth and guards
/// against unexpected values from a tampered environment.
pub(crate) fn is_valid_focus_url(s: &str) -> bool {
    if s.len() >= 256 {
        return false;
    }
    let has_valid_scheme = s.starts_with("warp://") || s.starts_with("warposs://");
    if !has_valid_scheme {
        return false;
    }
    s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '/' | '.' | '_' | '-'))
}

// ─── TERM_PROGRAM → app name map ──────────────────────────────────────────

/// Maps known TERM_PROGRAM values to the corresponding macOS app name used by
/// AppleScript `tell application`. TERM_PROGRAM is not always the app name
/// (e.g. "Apple_Terminal" → "Terminal", "WarpTerminal" → "Warp").
fn app_name_for_program(program: &str) -> &str {
    match program {
        p if p.contains("iTerm") => "iTerm2",
        "Apple_Terminal" => "Terminal",
        "WarpTerminal" => "Warp",
        "Hyper" => "Hyper",
        "Tabby" => "Tabby",
        "kitty" => "kitty",
        "alacritty" | "Alacritty" => "Alacritty",
        other => other,
    }
}

// ─── Public entry point ────────────────────────────────────────────────────

/// Attempts to bring the terminal hosting the session described by `term` to the
/// foreground. Returns true if focus was successfully achieved (or best-effort
/// app activation succeeded), false if nothing could be done.
pub fn focus_terminal(term: &TerminalRef) -> bool {
    let program = term.program.as_str();

    // Tier 0: generic focus URL — highest priority.
    // Any terminal that exposes a deep link (e.g. Warp via WARP_FOCUS_URL) gets
    // exact-pane focus via `open <url>`. The URL is passed as an argv argument —
    // not through a shell — so shell injection is not possible. Validation is
    // defence-in-depth: rejects unexpected schemes and non-URL-safe characters.
    if let Some(ref url) = term.focus_url {
        if is_valid_focus_url(url) {
            let ok = Command::new("open").arg(url).status().map(|s| s.success()).unwrap_or(false);
            if ok {
                return true;
            }
            // open failed — fall through to terminal-specific tiers.
        }
    }

    // Tier 1: iTerm2 — target by session UUID.
    if program.contains("iTerm") {
        if let Some(ref sid) = term.session_id {
            // Strip the "wXtYpZ:" prefix to get the bare UUID.
            let uuid = sid.split(':').last().unwrap_or("");
            if is_valid_uuid(uuid) {
                if focus_iterm_by_session_id(uuid) {
                    return true;
                }
            }
        }
        // Fall through to app activation.
        return activate_app("iTerm2");
    }

    // Tier 2: Apple Terminal — target by tty.
    if program == "Apple_Terminal" {
        if let Some(ref tty) = term.tty {
            if is_valid_tty(tty) {
                if focus_terminal_app_by_tty(tty) {
                    return true;
                }
            }
        }
        // Fall through to app activation.
        return activate_app("Terminal");
    }

    // Tier 3: any other terminal — best-effort app activation.
    let name = app_name_for_program(program);
    activate_app(name)
}

// ─── Tier 1: iTerm2 ───────────────────────────────────────────────────────

/// Iterates all iTerm2 windows → tabs → sessions and selects the one whose
/// `id` matches `uuid`. The UUID is pre-validated — safe to interpolate
/// into the script.
fn focus_iterm_by_session_id(uuid: &str) -> bool {
    // Safety: uuid passes is_valid_uuid() — only hex digits and hyphens.
    let script = format!(
        r#"
tell application "iTerm2"
    activate
    set found to false
    repeat with w in windows
        repeat with t in tabs of w
            repeat with s in sessions of t
                if id of s is "{uuid}" then
                    select s
                    select t
                    set index of w to 1
                    set found to true
                    exit repeat
                end if
            end repeat
            if found then exit repeat
        end repeat
        if found then exit repeat
    end repeat
    found
end tell
"#,
        uuid = uuid
    );

    run_osascript(&script)
        .map(|out| out.trim() == "true")
        .unwrap_or(false)
}

// ─── Tier 2: Apple Terminal ────────────────────────────────────────────────

/// Iterates all Terminal windows → tabs and selects the tab whose `tty` matches
/// "/dev/<tty>". The tty value is pre-validated — safe to interpolate.
fn focus_terminal_app_by_tty(tty: &str) -> bool {
    // Safety: tty passes is_valid_tty() — only alphanumeric and '/'.
    let script = format!(
        r#"
tell application "Terminal"
    activate
    set target_tty to "/dev/{tty}"
    set found to false
    repeat with w in windows
        set tidx to 1
        repeat with t in tabs of w
            if tty of t is target_tty then
                set selected tab of w to t
                set index of w to 1
                set found to true
                exit repeat
            end if
            set tidx to tidx + 1
        end repeat
        if found then exit repeat
    end repeat
    found
end tell
"#,
        tty = tty
    );

    run_osascript(&script)
        .map(|out| out.trim() == "true")
        .unwrap_or(false)
}

// ─── Tier 3: generic app activation ───────────────────────────────────────

/// Activates the named application. `name` comes from `app_name_for_program`
/// which returns only static string literals — no user data is interpolated.
fn activate_app(name: &str) -> bool {
    // Safety: name comes exclusively from the static match in app_name_for_program.
    // No user-supplied data reaches this string interpolation.
    let script = format!(
        r#"tell application "{name}" to activate"#,
        name = name
    );
    run_osascript(&script).is_some()
}

// ─── osascript runner ──────────────────────────────────────────────────────

/// Runs `osascript -e <script>` and returns stdout on success (exit 0),
/// None on error. Stderr is suppressed to avoid polluting the app log.
fn run_osascript(script: &str) -> Option<String> {
    let out = Command::new("osascript").arg("-e").arg(script).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        None
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::is_valid_focus_url;

    #[test]
    fn accepts_warp_scheme_with_hex_session() {
        assert!(is_valid_focus_url(
            "warp://session/9f6d05b9e7974a4fb0c5c489a44a3dbf"
        ));
    }

    #[test]
    fn accepts_warposs_scheme() {
        assert!(is_valid_focus_url(
            "warposs://session/9f6d05b9e7974a4fb0c5c489a44a3dbf"
        ));
    }

    #[test]
    fn rejects_http_scheme() {
        assert!(!is_valid_focus_url("http://example.com/focus"));
    }

    #[test]
    fn rejects_https_scheme() {
        assert!(!is_valid_focus_url("https://example.com/focus"));
    }

    #[test]
    fn rejects_shell_metacharacters() {
        assert!(!is_valid_focus_url("warp://session/abc;rm -rf /"));
        assert!(!is_valid_focus_url("warp://session/abc&&evil"));
        assert!(!is_valid_focus_url("warp://session/abc|cat /etc/passwd"));
        assert!(!is_valid_focus_url("warp://session/abc`whoami`"));
        assert!(!is_valid_focus_url("warp://session/abc$(id)"));
    }

    #[test]
    fn rejects_spaces() {
        assert!(!is_valid_focus_url("warp://session/abc def"));
    }

    #[test]
    fn rejects_oversized_url() {
        let long = format!("warp://session/{}", "a".repeat(256));
        assert!(!is_valid_focus_url(&long));
    }

    #[test]
    fn rejects_empty_string() {
        assert!(!is_valid_focus_url(""));
    }
}
