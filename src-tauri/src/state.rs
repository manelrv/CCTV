//! Estado de las instancias de Claude Code: enum de estados, store en memoria,
//! transiciones y reaper de sesiones muertas. Ver docs/HOOKS.md para el mapeo.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Working sin nuevos eventos durante mas de esto -> se considera Unknown.
pub const STALE_SECS: u64 = 180;
/// Cualquier estado sin eventos durante mas de esto -> se elimina del store.
/// (cubre el caso de matar la sesion sin que llegue SessionEnd)
pub const REMOVE_SECS: u64 = 1800;

/// Origen de la instancia: ficheros del supervisor (segundo plano) o hooks HTTP
/// (primer plano). Ver docs/DATA-SOURCES.md.
#[derive(Clone, Copy, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Background,
    Foreground,
}

#[derive(Clone, Copy, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum InstanceState {
    Working,
    WaitingPermission,
    WaitingInput,
    Error,
    Unknown,
    Idle,
    Completed,
}

impl InstanceState {
    /// Menor = mas urgente. Define el orden en la lista.
    fn urgency(self) -> u8 {
        match self {
            InstanceState::WaitingPermission => 0,
            InstanceState::WaitingInput => 1,
            InstanceState::Error => 2,
            InstanceState::Working => 3,
            InstanceState::Unknown => 4,
            InstanceState::Idle => 5,
            InstanceState::Completed => 6,
        }
    }
    /// Estados que "reclaman" al usuario (disparan auto-show de la ventana).
    pub fn needs_attention(self) -> bool {
        matches!(self, InstanceState::WaitingPermission | InstanceState::WaitingInput)
    }
}

#[derive(Clone, Serialize)]
pub struct Instance {
    pub session_id: String,
    pub cwd: String,
    pub project: String,
    pub state: InstanceState,
    pub detail: Option<String>,
    pub source: Source,
    pub started_at: u64,
    pub last_event_at: u64,
}

#[derive(Default)]
pub struct Store {
    inner: Mutex<HashMap<String, Instance>>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// Deriva un nombre legible del proyecto a partir del cwd.
/// TODO(claude-code): colapsar $HOME a ~ y quizas mostrar 2 ultimos segmentos.
pub(crate) fn project_from_cwd(cwd: &str) -> String {
    cwd.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(cwd)
        .to_string()
}

impl Store {
    /// Aplica una transicion desde los hooks HTTP (fuente foreground): crea la
    /// instancia si no existe, actualiza estado, detalle y last_event_at.
    /// `detail` None deja el detalle como estaba.
    pub fn apply(
        &self,
        session_id: &str,
        cwd: Option<&str>,
        state: InstanceState,
        detail: Option<String>,
    ) {
        let mut map = self.inner.lock().unwrap();
        let ts = now();
        let cwd = cwd.unwrap_or("").to_string();
        let entry = map.entry(session_id.to_string()).or_insert_with(|| Instance {
            session_id: session_id.to_string(),
            cwd: cwd.clone(),
            project: project_from_cwd(&cwd),
            state,
            detail: None,
            source: Source::Foreground,
            started_at: ts,
            last_event_at: ts,
        });
        if !cwd.is_empty() {
            entry.cwd = cwd.clone();
            entry.project = project_from_cwd(&cwd);
        }
        entry.state = state;
        if detail.is_some() {
            entry.detail = detail;
        }
        entry.last_event_at = ts;
    }

    pub fn remove(&self, session_id: &str) {
        self.inner.lock().unwrap().remove(session_id);
    }

    /// Reemplaza el set completo de instancias background (Fuente A). Regla
    /// "background manda": elimina cualquier foreground que comparta session_id
    /// con una instancia incoming. Ver docs/DATA-SOURCES.md.
    pub fn set_background_snapshot(&self, instances: Vec<Instance>) {
        let mut map = self.inner.lock().unwrap();

        // Elimina todas las entradas background anteriores.
        map.retain(|_, inst| inst.source != Source::Background);

        // Elimina cualquier foreground cuyo session_id aparece en el nuevo set.
        let incoming_ids: std::collections::HashSet<&str> =
            instances.iter().map(|i| i.session_id.as_str()).collect();
        map.retain(|id, inst| {
            !(inst.source == Source::Foreground && incoming_ids.contains(id.as_str()))
        });

        // Inserta el nuevo set background.
        for inst in instances {
            map.insert(inst.session_id.clone(), inst);
        }
    }

    /// Pasa Working viejos a Unknown y elimina los muy viejos.
    /// Solo actua sobre instancias Foreground: las Background las gestiona el
    /// ciclo de vida de los ficheros del supervisor. Ver docs/DATA-SOURCES.md.
    /// Devuelve true si algo cambio (para emitir snapshot solo cuando toca).
    pub fn reap(&self) -> bool {
        let mut map = self.inner.lock().unwrap();
        let ts = now();
        let mut changed = false;

        // Elimina foreground muy viejos.
        map.retain(|_, inst| {
            if inst.source == Source::Foreground
                && ts.saturating_sub(inst.last_event_at) > REMOVE_SECS
            {
                changed = true;
                return false;
            }
            true
        });

        // Pasa foreground Working a Unknown si llevan mucho tiempo sin eventos.
        for inst in map.values_mut() {
            if inst.source == Source::Foreground
                && inst.state == InstanceState::Working
                && ts.saturating_sub(inst.last_event_at) > STALE_SECS
            {
                inst.state = InstanceState::Unknown;
                changed = true;
            }
        }
        changed
    }

    /// Snapshot ordenado por urgencia y, dentro de cada nivel, por actividad.
    pub fn snapshot(&self) -> Vec<Instance> {
        let map = self.inner.lock().unwrap();
        let mut v: Vec<Instance> = map.values().cloned().collect();
        v.sort_by(|a, b| {
            a.state
                .urgency()
                .cmp(&b.state.urgency())
                .then(b.last_event_at.cmp(&a.last_event_at))
        });
        v
    }

    pub fn attention_count(&self) -> usize {
        self.inner
            .lock()
            .unwrap()
            .values()
            .filter(|i| i.state.needs_attention())
            .count()
    }
}
