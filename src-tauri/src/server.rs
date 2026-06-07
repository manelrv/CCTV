//! Local HTTP server that receives Claude Code hooks.
//! Each handler: applies to the store -> emits snapshot to the webview -> responds 200.
//! Fast response is critical: a slow hook stalls the user's session.

use crate::hooks::{summarize_detail, HookPayload};
use crate::refresh;
use crate::state::{InstanceState, Store, TerminalRef};
use crate::transcript;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

pub const BIND_ADDR: &str = "127.0.0.1:8787";

/// Minimum seconds between transcript reads per session.
/// Avoids reading potentially large files on every hook event.
const TRANSCRIPT_THROTTLE_SECS: u64 = 10;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub app: AppHandle,
    /// Throttle map: session_id -> epoch secs of last transcript read.
    pub transcript_last_read: Arc<Mutex<HashMap<String, u64>>>,
}

pub async fn serve(state: AppState) {
    let app = Router::new()
        .route("/hooks/session-start", post(session_start))
        .route("/hooks/session-end", post(session_end))
        .route("/hooks/user-prompt", post(user_prompt))
        .route("/hooks/pre-tool", post(tool_activity))
        .route("/hooks/post-tool", post(tool_activity))
        .route("/hooks/notification/permission", post(notif_permission))
        .route("/hooks/notification/idle", post(notif_idle))
        .route("/hooks/stop", post(stop))
        .route("/health", post(|| async { StatusCode::OK }))
        // Read-only, for debugging the store externally (curl). Bound to loopback only;
        // no sensitive data beyond cwd/detail.
        .route("/debug/snapshot", get(debug_snapshot))
        .with_state(state);

    match tokio::net::TcpListener::bind(BIND_ADDR).await {
        Ok(listener) => {
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("[server] error: {e}");
            }
        }
        // TODO(claude-code): if the port is busy, try the next one or
        // notify the user via the tray.
        Err(e) => eprintln!("[server] could not bind {BIND_ADDR}: {e}"),
    }
}

/// Propagates state to the webview and the tray. Delegates to refresh::refresh().
fn emit(state: &AppState) {
    refresh::refresh(&state.app, &state.store);
}

/// Spawns a background task that reads context tokens from a transcript.
/// Throttled per session to at most once every TRANSCRIPT_THROTTLE_SECS.
/// The handler has already responded 200 before this is called — no latency added.
fn spawn_transcript_read(s: AppState, session_id: String, transcript_path: Option<String>) {
    let Some(path) = transcript_path else { return };
    tauri::async_runtime::spawn(async move {
        // Throttle check
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        {
            let mut map = s.transcript_last_read.lock().unwrap();
            if let Some(&last) = map.get(&session_id) {
                if now.saturating_sub(last) < TRANSCRIPT_THROTTLE_SECS {
                    return;
                }
            }
            map.insert(session_id.clone(), now);
        }

        // Read tokens from transcript (blocking I/O in async context is acceptable here:
        // the file is read once every 10s per session and it is < 256 KiB of tail).
        if let Some(tokens) = transcript::read_context_tokens(std::path::Path::new(&path)) {
            if s.store.set_context_tokens(&session_id, tokens) {
                refresh::refresh(&s.app, &s.store);
            }
        }
    });
}

async fn debug_snapshot(State(s): State<AppState>) -> Json<Vec<crate::state::Instance>> {
    Json(s.store.snapshot())
}

fn sid(p: &HookPayload) -> Option<String> {
    p.session_id.clone().filter(|s| !s.is_empty())
}

/// Extracts terminal info from a hook payload, if present and non-empty.
/// Returns None if term_program is absent or blank (no point storing a ref
/// without at least knowing which terminal app we are dealing with).
fn terminal_ref(p: &HookPayload) -> Option<TerminalRef> {
    let program = p.term_program.as_deref().filter(|s| !s.is_empty())?.to_string();
    Some(TerminalRef {
        program,
        session_id: p.term_session_id.clone().filter(|s| !s.is_empty()),
        tty: p.tty.clone().filter(|s| !s.is_empty()),
        focus_url: p.focus_url.clone().filter(|s| !s.is_empty()),
    })
}

async fn session_start(State(s): State<AppState>, Json(p): Json<HookPayload>) -> StatusCode {
    if let Some(id) = sid(&p) {
        s.store.apply(&id, p.cwd.as_deref(), InstanceState::Idle, None);
        if let Some(term) = terminal_ref(&p) {
            s.store.set_terminal(&id, term);
        }
        emit(&s);
        spawn_transcript_read(s, id, p.transcript_path);
    }
    StatusCode::OK
}

async fn session_end(State(s): State<AppState>, Json(p): Json<HookPayload>) -> StatusCode {
    if let Some(id) = sid(&p) {
        s.store.remove(&id);
        emit(&s);
    }
    StatusCode::OK
}

async fn user_prompt(State(s): State<AppState>, Json(p): Json<HookPayload>) -> StatusCode {
    if let Some(id) = sid(&p) {
        s.store.apply(&id, p.cwd.as_deref(), InstanceState::Working, None);
        if let Some(term) = terminal_ref(&p) {
            s.store.set_terminal(&id, term);
        }
        emit(&s);
        spawn_transcript_read(s, id, p.transcript_path);
    }
    StatusCode::OK
}

async fn tool_activity(State(s): State<AppState>, Json(p): Json<HookPayload>) -> StatusCode {
    if let Some(id) = sid(&p) {
        let detail = summarize_detail(&p);
        s.store.apply(&id, p.cwd.as_deref(), InstanceState::Working, detail);
        emit(&s);
        spawn_transcript_read(s, id, p.transcript_path);
    }
    StatusCode::OK
}

async fn notif_permission(State(s): State<AppState>, Json(p): Json<HookPayload>) -> StatusCode {
    if let Some(id) = sid(&p) {
        let detail = p.message.clone();
        s.store
            .apply(&id, p.cwd.as_deref(), InstanceState::WaitingPermission, detail);
        emit(&s);
        spawn_transcript_read(s, id, p.transcript_path);
    }
    StatusCode::OK
}

async fn notif_idle(State(s): State<AppState>, Json(p): Json<HookPayload>) -> StatusCode {
    if let Some(id) = sid(&p) {
        s.store
            .apply(&id, p.cwd.as_deref(), InstanceState::WaitingInput, None);
        emit(&s);
        spawn_transcript_read(s, id, p.transcript_path);
    }
    StatusCode::OK
}

async fn stop(State(s): State<AppState>, Json(p): Json<HookPayload>) -> StatusCode {
    if let Some(id) = sid(&p) {
        s.store.apply(&id, p.cwd.as_deref(), InstanceState::Idle, None);
        emit(&s);
        spawn_transcript_read(s, id, p.transcript_path);
    }
    StatusCode::OK
}
