//! FUENTE A: lee el estado de las sesiones en segundo plano que persiste el
//! supervisor de Claude Code (~/.claude/jobs/<id>/state.json) y vigila el
//! directorio para reaccionar a los cambios. Ver docs/DATA-SOURCES.md.
//!
//! Esquema verificado empiricamente el 2026-06-06 contra sesiones reales.
//! Campos reales (todos camelCase en el JSON):
//!   state:  "working" | "blocked" | "done" | "stopped" | "failed"
//!   tempo:  "active" | "idle" | "blocked"  (ortogonal a state)
//!   detail: string de progreso (NO existe summary/title — esos no son campos)
//!   needs:  pregunta al usuario o "approve <Tool>: <path>" (permission)
//!   intent: prompt original (fallback para detail)
//!   name:   nombre corto generado por el supervisor (opcional)
//!   sessionId, daemonShort, cwd, createdAt, updatedAt: camelCase
//!
//! Combinaciones observadas empiricamente:
//!   state=working + tempo=active               → Working (procesando activamente)
//!   state=working + tempo=idle                 → Working (esperando shell task)
//!   state=working + tempo=blocked              → WaitingPermission (needs=approve …)
//!   state=blocked + tempo=blocked              → WaitingInput (needs=<pregunta>)
//!   state=done    + tempo=idle                 → Completed
//!   state=stopped + tempo=idle                 → Unknown (detenido por usuario)
//!   state=failed  + tempo=idle                 → Error

use crate::state::{project_from_cwd, Instance, InstanceState, Source, Store};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

/// Esquema defensivo (todo Option) de ~/.claude/jobs/<id>/state.json.
/// Verificado empiricamente; parsea con serde(rename_all = "camelCase") para
/// mapear sessionId, createdAt, updatedAt directamente.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct JobState {
    /// ID de sesion — campo "sessionId" en el JSON.
    session_id: Option<String>,
    /// Estado macro del trabajo: "working" | "blocked" | "done" | "stopped" | "failed".
    state: Option<String>,
    /// Ritmo ortogonal al estado: "active" | "idle" | "blocked".
    /// Cuando state=working + tempo=blocked => esperando permiso.
    /// Cuando state=blocked + tempo=blocked => esperando input del usuario.
    tempo: Option<String>,
    /// Detalle de progreso (puede ser null cuando acaba de arrancar).
    detail: Option<String>,
    /// Pregunta/accion pendiente: "approve Write: /path" (permiso) o pregunta libre (input).
    needs: Option<String>,
    /// Prompt original del usuario — fallback para detail.
    intent: Option<String>,
    /// Nombre corto generado por el supervisor (campo "name").
    name: Option<String>,
    /// Directorio de trabajo de la sesion.
    cwd: Option<String>,
    /// Timestamp de creacion — RFC3339/ISO 8601.
    created_at: Option<String>,
    /// Timestamp de ultima actualizacion — RFC3339/ISO 8601.
    updated_at: Option<String>,
}

fn jobs_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("jobs"))
}

/// Mapea los valores empíricos de state.json al InstanceState interno.
///
/// Requiere tanto `state` como `tempo` porque la distinción entre "esperando
/// permiso" y "esperando input del usuario" NO está en `state` sino en la
/// combinación state+tempo:
///   - state=working + tempo=blocked → WaitingPermission (approve <Tool>)
///   - state=blocked + tempo=blocked → WaitingInput (pregunta al usuario)
fn map_state(raw_state: &str, raw_tempo: &str) -> InstanceState {
    let state = raw_state.to_lowercase().replace([' ', '-'], "_");
    let tempo = raw_tempo.to_lowercase().replace([' ', '-'], "_");

    match state.as_str() {
        // Activo o esperando resultado de shell task en background.
        "working" | "running" | "in_progress" => {
            // Si tempo=blocked, el agente está detenido esperando aprobación
            // de un tool use (Write, Bash, etc.). El needs="approve ..." lo confirma.
            if tempo == "blocked" {
                InstanceState::WaitingPermission
            } else {
                InstanceState::Working
            }
        }
        // El agente hizo una pregunta al usuario y espera respuesta.
        // Observado: state=blocked + tempo=blocked + needs=<pregunta libre>.
        "blocked" | "needs_input" | "waiting_input" | "awaiting_input" => {
            InstanceState::WaitingInput
        }
        // "done" es el valor observado empiricamente para completado OK.
        "completed" | "done" => InstanceState::Completed,
        "idle" => InstanceState::Idle,
        "failed" | "error" => InstanceState::Error,
        // "stopped" = el usuario lo detuvo; se muestra en gris sin accion util.
        "stopped" => InstanceState::Unknown,
        _ => InstanceState::Unknown,
    }
}

/// Parsea un timestamp RFC3339 (e.g. "2026-06-06T15:55:52.871Z") a epoch secs.
/// No depende de chrono: parsea manualmente los campos numericos del string.
/// Devuelve None si el formato no es reconocible.
fn parse_rfc3339(s: &str) -> Option<u64> {
    // Formato esperado: YYYY-MM-DDTHH:MM:SS[.mmm]Z
    // Tomamos solo la parte hasta la Z o el offset, ignoramos fraccion de segundos.
    let s = s.trim();
    // Debe tener al menos "YYYY-MM-DDTHH:MM:SSZ" = 20 chars
    if s.len() < 20 {
        return None;
    }
    let year: u64 = s[0..4].parse().ok()?;
    let month: u64 = s[5..7].parse().ok()?;
    let day: u64 = s[8..10].parse().ok()?;
    let hour: u64 = s[11..13].parse().ok()?;
    let min: u64 = s[14..16].parse().ok()?;
    let sec: u64 = s[17..19].parse().ok()?;

    // Conversion aproximada a epoch (sin ajuste de leap seconds, suficiente para UI).
    // Dias desde 1970-01-01.
    if year < 1970 || month < 1 || month > 12 || day < 1 || day > 31 {
        return None;
    }
    let days_per_month = [0u64, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let mut days: u64 = 0;
    for y in 1970..year {
        days += if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 366 } else { 365 };
    }
    for m in 1..month {
        let d = if m == 2 && leap { 29 } else { days_per_month[m as usize] };
        days += d;
    }
    days += day - 1;
    Some(days * 86400 + hour * 3600 + min * 60 + sec)
}

/// Lee todos los state.json y construye el set de instancias background.
fn scan() -> Vec<Instance> {
    let Some(dir) = jobs_dir() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let state_path = entry.path().join("state.json");
        let Ok(text) = std::fs::read_to_string(&state_path) else {
            continue;
        };
        let Ok(js) = serde_json::from_str::<JobState>(&text) else {
            continue;
        };

        // session_id: preferimos sessionId del JSON; fallback al nombre del dir.
        let id = js
            .session_id
            .or_else(|| entry.file_name().to_str().map(String::from))
            .unwrap_or_default();
        if id.is_empty() {
            continue;
        }

        let raw_state = js.state.unwrap_or_default();
        let raw_tempo = js.tempo.unwrap_or_default();
        let cwd = js.cwd.unwrap_or_default();

        // detail: si hay accion pendiente ("needs": pregunta o "approve ..."),
        // es lo mas util de mostrar; si no, "detail", luego "intent" (prompt
        // original), luego "name" (generado por el supervisor).
        let detail = js
            .needs
            .filter(|s| !s.is_empty())
            .or_else(|| js.detail.filter(|s| !s.is_empty()))
            .or_else(|| js.intent.filter(|s| !s.is_empty()))
            .or(js.name);

        // Timestamps: RFC3339 → epoch secs; fallback a mtime del fichero.
        let file_ts = mtime(&state_path);
        let started_at = js
            .created_at
            .as_deref()
            .and_then(parse_rfc3339)
            .unwrap_or(file_ts);
        let last_event_at = js
            .updated_at
            .as_deref()
            .and_then(parse_rfc3339)
            .unwrap_or(file_ts);

        out.push(Instance {
            session_id: id,
            project: project_from_cwd(&cwd),
            cwd,
            state: map_state(&raw_state, &raw_tempo),
            detail,
            source: Source::Background,
            started_at,
            last_event_at,
        });
    }
    out
}

fn mtime(p: &PathBuf) -> u64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or_else(|| {
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
        })
}

fn rescan_and_emit(store: &Arc<Store>, app: &AppHandle) {
    store.set_background_snapshot(scan());
    let _ = app.emit("instances", &store.snapshot());
}

/// Arranca el watcher en un hilo propio (notify es sincrono). Hace un scan
/// inicial y luego re-escanea (con debounce simple) ante cualquier cambio.
pub fn start(store: Arc<Store>, app: AppHandle) {
    std::thread::spawn(move || {
        rescan_and_emit(&store, &app);

        let Some(dir) = jobs_dir() else { return };
        let _ = std::fs::create_dir_all(&dir);

        let (tx, rx) = channel();
        let mut watcher: RecommendedWatcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[jobs] no se pudo crear el watcher: {e}");
                return;
            }
        };
        if let Err(e) = watcher.watch(&dir, RecursiveMode::Recursive) {
            eprintln!("[jobs] no se pudo vigilar {dir:?}: {e}");
            return;
        }

        // Debounce: agrupa rafagas de eventos en un solo rescan.
        loop {
            match rx.recv() {
                Ok(_) => {
                    while rx.recv_timeout(Duration::from_millis(150)).is_ok() {}
                    rescan_and_emit(&store, &app);
                }
                Err(_) => break,
            }
        }
    });
}
