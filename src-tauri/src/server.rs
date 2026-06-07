//! Local HTTP server that receives Claude Code hooks.
//! Each handler: applies to the store -> emits snapshot to the webview -> responds 200.
//! Fast response is critical: a slow hook stalls the user's session.

use crate::hooks::{summarize_detail, HookPayload};
use crate::refresh;
use crate::state::{InstanceState, Store};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tauri::AppHandle;

pub const BIND_ADDR: &str = "127.0.0.1:8787";

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub app: AppHandle,
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

async fn debug_snapshot(State(s): State<AppState>) -> Json<Vec<crate::state::Instance>> {
    Json(s.store.snapshot())
}

fn sid(p: &HookPayload) -> Option<String> {
    p.session_id.clone().filter(|s| !s.is_empty())
}

async fn session_start(State(s): State<AppState>, Json(p): Json<HookPayload>) -> StatusCode {
    if let Some(id) = sid(&p) {
        s.store.apply(&id, p.cwd.as_deref(), InstanceState::Idle, None);
        emit(&s);
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
        emit(&s);
    }
    StatusCode::OK
}

async fn tool_activity(State(s): State<AppState>, Json(p): Json<HookPayload>) -> StatusCode {
    if let Some(id) = sid(&p) {
        let detail = summarize_detail(&p);
        s.store.apply(&id, p.cwd.as_deref(), InstanceState::Working, detail);
        emit(&s);
    }
    StatusCode::OK
}

async fn notif_permission(State(s): State<AppState>, Json(p): Json<HookPayload>) -> StatusCode {
    if let Some(id) = sid(&p) {
        let detail = p.message.clone();
        s.store
            .apply(&id, p.cwd.as_deref(), InstanceState::WaitingPermission, detail);
        emit(&s);
    }
    StatusCode::OK
}

async fn notif_idle(State(s): State<AppState>, Json(p): Json<HookPayload>) -> StatusCode {
    if let Some(id) = sid(&p) {
        s.store
            .apply(&id, p.cwd.as_deref(), InstanceState::WaitingInput, None);
        emit(&s);
    }
    StatusCode::OK
}

async fn stop(State(s): State<AppState>, Json(p): Json<HookPayload>) -> StatusCode {
    if let Some(id) = sid(&p) {
        s.store.apply(&id, p.cwd.as_deref(), InstanceState::Idle, None);
        emit(&s);
    }
    StatusCode::OK
}
