//! Servidor HTTP local que recibe los hooks de Claude Code.
//! Cada handler: aplica al store -> emite snapshot al webview -> responde 200.
//! Responder rapido es critico: un hook lento ralentiza la sesion del usuario.

use crate::hooks::{summarize_detail, HookPayload};
use crate::state::{InstanceState, Store};
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

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
        .with_state(state);

    match tokio::net::TcpListener::bind(BIND_ADDR).await {
        Ok(listener) => {
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("[server] error: {e}");
            }
        }
        // TODO(claude-code): si el puerto esta ocupado, probar el siguiente o
        // avisar al usuario por la bandeja.
        Err(e) => eprintln!("[server] no se pudo bindear {BIND_ADDR}: {e}"),
    }
}

/// Empuja el snapshot completo al webview. El frontend escucha "instances".
fn emit(state: &AppState) {
    let snapshot = state.store.snapshot();
    let _ = state.app.emit("instances", &snapshot);
    // TODO(claude-code): aqui tambien actualizar el icono de bandeja
    // (color/contador) y aplicar auto-show/hide segun attention_count().
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
