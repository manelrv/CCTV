//! State of Claude Code instances: state enum, in-memory store,
//! transitions, and dead-session reaper. See docs/HOOKS.md for the mapping.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Working with no new events for longer than this -> considered Unknown.
pub const STALE_SECS: u64 = 180;
/// Any state with no events for longer than this -> removed from the store.
/// (covers the case of killing a session without a SessionEnd arriving)
pub const REMOVE_SECS: u64 = 1800;

/// Origin of the instance: supervisor files (background) or HTTP hooks
/// (foreground). See docs/DATA-SOURCES.md.
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
    /// Lower = more urgent. Defines sort order in the list.
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
    /// States that "demand" user attention (trigger auto-show of the floating window).
    pub fn needs_attention(self) -> bool {
        matches!(self, InstanceState::WaitingPermission | InstanceState::WaitingInput)
    }
    /// Terminal states: the session finished and will not transition again.
    /// For background jobs the supervisor never deletes their state.json, so
    /// terminal instances are expired by TTL instead (see reap and jobs::scan).
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            InstanceState::Completed | InstanceState::Error | InstanceState::Unknown
        )
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

/// Derives a human-readable project name from cwd: collapses $HOME to `~`
/// and, for deep paths, abbreviates the middle keeping the last 2 segments.
/// E.g.: /Users/x/dev/agent-os -> ~/dev/agent-os
///       /Users/x/a/b/CCTV/src-tauri -> ~/…/CCTV/src-tauri
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
    /// Applies a transition from HTTP hooks (foreground source): creates the
    /// instance if it does not exist, updates state, detail, and last_event_at.
    /// `detail` of None leaves the existing detail unchanged.
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

    /// Replaces the full set of background instances (Source A). "Background wins"
    /// rule: removes any foreground entry that shares a session_id with an incoming
    /// instance. See docs/DATA-SOURCES.md.
    pub fn set_background_snapshot(&self, instances: Vec<Instance>) {
        let mut map = self.inner.lock().unwrap();

        // Remove all existing background entries.
        map.retain(|_, inst| inst.source != Source::Background);

        // Remove any foreground entry whose session_id appears in the incoming set.
        let incoming_ids: std::collections::HashSet<&str> =
            instances.iter().map(|i| i.session_id.as_str()).collect();
        map.retain(|id, inst| {
            !(inst.source == Source::Foreground && incoming_ids.contains(id.as_str()))
        });

        // Insert the new background set.
        for inst in instances {
            map.insert(inst.session_id.clone(), inst);
        }
    }

    /// Transitions stale Working entries to Unknown and removes very old ones.
    /// Active Background instances are managed by the supervisor file lifecycle
    /// and never reaped — with ONE exception: the supervisor never deletes the
    /// state.json of finished jobs, so TERMINAL background instances expire by
    /// TTL here (jobs::scan applies the same filter on re-scan).
    /// Returns true if anything changed (so the caller emits a snapshot only when needed).
    pub fn reap(&self) -> bool {
        let mut map = self.inner.lock().unwrap();
        let ts = now();
        let mut changed = false;

        // Remove very old foreground entries, and expired terminal background ones.
        map.retain(|_, inst| {
            let age = ts.saturating_sub(inst.last_event_at);
            let expired_fg = inst.source == Source::Foreground && age > REMOVE_SECS;
            let expired_terminal_bg = inst.source == Source::Background
                && inst.state.is_terminal()
                && age > REMOVE_SECS;
            if expired_fg || expired_terminal_bg {
                changed = true;
                return false;
            }
            true
        });

        // Transition foreground Working entries to Unknown if they have been silent too long.
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

    /// Snapshot sorted by urgency and, within each level, by recency.
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

    /// Builds an instance with last_event_at shifted `age_secs` into the past.
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
    fn reap_never_touches_nonterminal_background() {
        let store = Store::default();
        insert(&store, mk("bg", Source::Background, InstanceState::Working, REMOVE_SECS + 999));
        assert!(!store.reap());
        assert_eq!(get_state(&store, "bg"), Some(InstanceState::Working));
    }

    #[test]
    fn reap_removes_expired_terminal_background() {
        let store = Store::default();
        insert(&store, mk("done", Source::Background, InstanceState::Completed, REMOVE_SECS + 20));
        insert(&store, mk("fail", Source::Background, InstanceState::Error, REMOVE_SECS + 20));
        insert(&store, mk("stop", Source::Background, InstanceState::Unknown, REMOVE_SECS + 20));
        assert!(store.reap());
        assert_eq!(store.snapshot().len(), 0);
    }

    #[test]
    fn reap_keeps_fresh_terminal_background() {
        let store = Store::default();
        insert(&store, mk("done", Source::Background, InstanceState::Completed, 60));
        assert!(!store.reap());
        assert_eq!(get_state(&store, "done"), Some(InstanceState::Completed));
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
