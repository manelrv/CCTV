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

/// Deriva un nombre legible del proyecto a partir del cwd: colapsa $HOME a `~`
/// y, si el path es profundo, abrevia el medio dejando los 2 ultimos segmentos.
/// Ej: /Users/x/dev/agent-os -> ~/dev/agent-os
///     /Users/x/a/b/CCTV/src-tauri -> ~/…/CCTV/src-tauri
pub(crate) fn project_from_cwd(cwd: &str) -> String {
    let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned());
    project_from_cwd_with_home(cwd, home.as_deref())
}

fn project_from_cwd_with_home(cwd: &str, home: Option<&str>) -> String {
    let cwd = cwd.trim_end_matches('/');
    if cwd.is_empty() {
        return String::new();
    }
    let (prefix, rest) = match home.map(|h| h.trim_end_matches('/')) {
        Some(h) if cwd == h => return "~".to_string(),
        Some(h) if cwd.starts_with(&format!("{h}/")) => ("~", &cwd[h.len() + 1..]),
        _ => ("", cwd.trim_start_matches('/')),
    };
    let segments: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    match segments.len() {
        0 => prefix.to_string(),
        1 | 2 if !prefix.is_empty() => format!("{}/{}", prefix, segments.join("/")),
        1 => segments[0].to_string(),
        2 => segments.join("/"),
        n if !prefix.is_empty() => format!("{}/…/{}/{}", prefix, segments[n - 2], segments[n - 1]),
        n => format!("…/{}/{}", segments[n - 2], segments[n - 1]),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: Option<&str> = Some("/Users/x");

    /// Construye una instancia con last_event_at desplazado `age_secs` al pasado.
    fn mk(id: &str, source: Source, state: InstanceState, age_secs: u64) -> Instance {
        let ts = now().saturating_sub(age_secs);
        Instance {
            session_id: id.to_string(),
            cwd: format!("/tmp/{id}"),
            project: id.to_string(),
            state,
            detail: None,
            source,
            started_at: ts,
            last_event_at: ts,
        }
    }

    fn insert(store: &Store, inst: Instance) {
        store.inner.lock().unwrap().insert(inst.session_id.clone(), inst);
    }

    fn get_state(store: &Store, id: &str) -> Option<InstanceState> {
        store.inner.lock().unwrap().get(id).map(|i| i.state)
    }

    #[test]
    fn reap_marks_stale_foreground_working_as_unknown() {
        let store = Store::default();
        insert(&store, mk("fg", Source::Foreground, InstanceState::Working, STALE_SECS + 20));
        assert!(store.reap());
        assert_eq!(get_state(&store, "fg"), Some(InstanceState::Unknown));
    }

    #[test]
    fn reap_removes_very_old_foreground() {
        let store = Store::default();
        insert(&store, mk("fg", Source::Foreground, InstanceState::Idle, REMOVE_SECS + 20));
        assert!(store.reap());
        assert_eq!(get_state(&store, "fg"), None);
    }

    #[test]
    fn reap_never_touches_background() {
        let store = Store::default();
        insert(&store, mk("bg", Source::Background, InstanceState::Working, REMOVE_SECS + 999));
        assert!(!store.reap());
        assert_eq!(get_state(&store, "bg"), Some(InstanceState::Working));
    }

    #[test]
    fn reap_keeps_fresh_foreground_untouched() {
        let store = Store::default();
        insert(&store, mk("fg", Source::Foreground, InstanceState::Working, 10));
        assert!(!store.reap());
        assert_eq!(get_state(&store, "fg"), Some(InstanceState::Working));
    }

    #[test]
    fn background_snapshot_wins_over_foreground_with_same_id() {
        let store = Store::default();
        insert(&store, mk("x", Source::Foreground, InstanceState::Working, 0));
        store.set_background_snapshot(vec![mk("x", Source::Background, InstanceState::Completed, 0)]);
        let snap = store.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].source, Source::Background);
        assert_eq!(snap[0].state, InstanceState::Completed);
    }

    #[test]
    fn background_snapshot_replaces_previous_background_set() {
        let store = Store::default();
        store.set_background_snapshot(vec![mk("a", Source::Background, InstanceState::Working, 0)]);
        store.set_background_snapshot(vec![mk("b", Source::Background, InstanceState::Working, 0)]);
        let snap = store.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].session_id, "b");
    }

    #[test]
    fn background_snapshot_leaves_unrelated_foreground_alone() {
        let store = Store::default();
        insert(&store, mk("fg", Source::Foreground, InstanceState::Working, 0));
        store.set_background_snapshot(vec![mk("bg", Source::Background, InstanceState::Working, 0)]);
        assert_eq!(store.snapshot().len(), 2);
        assert_eq!(get_state(&store, "fg"), Some(InstanceState::Working));
    }

    #[test]
    fn collapses_home_to_tilde() {
        assert_eq!(project_from_cwd_with_home("/Users/x/dev/agent-os", HOME), "~/dev/agent-os");
        assert_eq!(project_from_cwd_with_home("/Users/x/CCTV", HOME), "~/CCTV");
        assert_eq!(project_from_cwd_with_home("/Users/x", HOME), "~");
    }

    #[test]
    fn abbreviates_deep_paths_keeping_last_two_segments() {
        assert_eq!(
            project_from_cwd_with_home("/Users/x/a/b/CCTV/src-tauri", HOME),
            "~/…/CCTV/src-tauri"
        );
        assert_eq!(project_from_cwd_with_home("/opt/srv/apps/web", None), "…/apps/web");
    }

    #[test]
    fn handles_paths_outside_home() {
        assert_eq!(project_from_cwd_with_home("/opt/web", None), "opt/web");
        assert_eq!(project_from_cwd_with_home("/srv", None), "srv");
        assert_eq!(project_from_cwd_with_home("/Users/other/app", HOME), "…/other/app");
    }

    #[test]
    fn handles_edge_cases() {
        assert_eq!(project_from_cwd_with_home("", HOME), "");
        assert_eq!(project_from_cwd_with_home("/Users/x/", HOME), "~");
        assert_eq!(project_from_cwd_with_home("relative/path", None), "relative/path");
    }
}
