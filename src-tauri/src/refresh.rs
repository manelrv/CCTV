//! Centralised state propagation to the webview and the tray.
//! Single emission point: replaces the scattered emit() calls in server.rs, jobs.rs,
//! and the reaper in main.rs. See docs/ARCHITECTURE.md#pushing-state-to-the-webview.

use crate::config::Prefs;
use crate::i18n;
use crate::state::{Instance, InstanceState, Store};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, image::Image};
use tauri_plugin_notification::NotificationExt;

// Calm icon: no instance is demanding user attention.
const ICON_CALM: &[u8] = include_bytes!("../../icons/tray-calm-64.png");
// Alert icon: at least one instance is demanding attention.
const ICON_ALERT: &[u8] = include_bytes!("../../icons/tray-alert-64.png");

/// Tray icon variant (used in unit tests).
#[derive(Debug, PartialEq, Eq)]
pub enum TrayVariant {
    Calm,
    Alert,
}

/// Returns the correct variant based on the attention count.
/// Pure logic, no Tauri runtime — unit-testable.
pub fn tray_variant(attention: usize) -> TrayVariant {
    if attention > 0 {
        TrayVariant::Alert
    } else {
        TrayVariant::Calm
    }
}

/// Preferences state shared between threads, managed as Tauri managed state.
/// Allows refresh() to access prefs without disk I/O on every hook event.
#[derive(Default)]
pub struct PrefsState(pub Mutex<Prefs>);

/// Tracks which session_ids are currently in an attention state (WaitingPermission
/// or WaitingInput). Used by refresh() to detect transitions and fire one
/// notification per new entry.
#[derive(Default)]
pub struct AttentionState(pub Mutex<HashSet<String>>);

/// Returns session_ids that are in `current` but NOT in `prev`.
/// These are the IDs that just entered attention — each gets one notification.
///
/// Note: an id that leaves attention and then re-enters WILL re-notify
/// (it disappears from prev when it leaves, so the next entry is "new").
///
/// Design decision: instances that are ALREADY in attention when the app
/// first starts ARE notified. This is intentional — the user just launched
/// the app and needs to know what is pending immediately.
pub fn newly_attention(prev: &HashSet<String>, current: &HashSet<String>) -> Vec<String> {
    current.difference(prev).cloned().collect()
}

/// Emits the snapshot to the webview, updates the tray icon, applies
/// auto-show/hide according to current preferences, and fires one desktop
/// notification per session that newly enters an attention state.
///
/// This function is the ONLY state emission point. It must be fast: no
/// disk I/O (prefs come from managed state).
pub fn refresh(app: &AppHandle, store: &Arc<Store>) {
    let snapshot = store.snapshot();
    let attention = store.attention_count();

    // 1. Emit snapshot to the webview.
    let _ = app.emit("instances", &snapshot);

    // 2. Update tray icon and title.
    if let Some(tray) = app.tray_by_id("main") {
        let icon_bytes = match tray_variant(attention) {
            TrayVariant::Alert => ICON_ALERT,
            TrayVariant::Calm => ICON_CALM,
        };
        if let Ok(img) = Image::from_bytes(icon_bytes) {
            let _ = tray.set_icon(Some(img));
        }
        // On macOS the title appears next to the icon in the menu bar.
        // On other OSes this is a harmless no-op.
        let title = if attention > 0 {
            attention.to_string()
        } else {
            String::new()
        };
        let _ = tray.set_title(Some(&title));
    }

    // 3. Auto-hide/show according to preferences (managed state, no I/O).
    if let Some(prefs_state) = app.try_state::<PrefsState>() {
        let prefs = prefs_state.0.lock().unwrap().clone();
        apply_auto_hide(app, &prefs, attention);
    }

    // 4. Desktop notifications for newly-entered attention instances.
    if let Some(attn_state) = app.try_state::<AttentionState>() {
        let current: HashSet<String> = snapshot
            .iter()
            .filter(|i| i.state.needs_attention())
            .map(|i| i.session_id.clone())
            .collect();

        let to_notify: Vec<(String, Instance)> = {
            let prev = attn_state.0.lock().unwrap();
            newly_attention(&prev, &current)
                .into_iter()
                .filter_map(|id| {
                    snapshot.iter().find(|i| i.session_id == id).map(|i| (id, i.clone()))
                })
                .collect()
        };

        // Update the stored set after releasing the lock above.
        *attn_state.0.lock().unwrap() = current;

        if !to_notify.is_empty() {
            // Honour the language preference; fall back to system locale ("auto").
            let lang = app
                .try_state::<PrefsState>()
                .map(|p| i18n::Lang::from_pref(&p.0.lock().unwrap().language))
                .unwrap_or_else(i18n::Lang::detect);
            let strings = i18n::strings(lang);
            let app2 = app.clone();
            // Threading note: tauri-plugin-notification uses notify-rust on macOS,
            // which calls UNUserNotificationCenter. Apple recommends main-thread calls
            // for UNUserNotificationCenter. We dispatch via run_on_main_thread as the
            // same precaution we already apply for NSPanel (SIGTRAP lesson from phase 5).
            let _ = app.run_on_main_thread(move || {
                for (_, inst) in to_notify {
                    let body = inst.detail.as_deref().unwrap_or(match inst.state {
                        InstanceState::WaitingPermission => strings.notif_permission,
                        _ => strings.notif_input,
                    });
                    if let Err(e) =
                        app2.notification().builder().title(&inst.project).body(body).show()
                    {
                        eprintln!("[notify] failed to show notification: {e}");
                    }
                }
            });
        }
    }
}

/// Applies auto-hide logic: hides the floating window when nothing needs attention
/// and the toggle is active; shows it when attention is needed (only if floating_window
/// is enabled, to avoid overriding the user's preference).
/// Whether the floating panel should be shown immediately on startup.
///
/// The window starts hidden (`visible: false` in tauri.conf.json). In auto-hide
/// mode it stays hidden until something needs attention — the first `refresh()`
/// reveals it via `apply_auto_hide`. With auto-hide OFF the window is meant to be
/// always visible, so it must be shown explicitly at startup (nothing else does).
pub fn show_on_startup(prefs: &Prefs) -> bool {
    prefs.floating_window && !prefs.auto_hide
}

pub fn apply_auto_hide(app: &AppHandle, prefs: &Prefs, attention: usize) {
    if !prefs.auto_hide {
        return;
    }
    if attention > 0 && prefs.floating_window {
        set_panel_visible(app, true);
    } else if attention == 0 {
        set_panel_visible(app, false);
    }
}

/// Shows or hides the panel (macOS) or the window (other OSes).
///
/// Threading note: refresh() is called from tokio threads (hooks), the jobs
/// watcher thread, and the reaper. Tauri's window APIs internally redispatch to
/// the main thread, but the raw NSPanel handle does NOT — calling orderOut/show
/// from another thread aborts with SIGTRAP in AppKit. Everything therefore goes
/// through run_on_main_thread and the panel is resolved INSIDE the closure.
pub fn set_panel_visible(app: &AppHandle, visible: bool) {
    #[cfg(target_os = "macos")]
    {
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            use tauri_nspanel::ManagerExt;
            if let Ok(panel) = app2.get_webview_panel("monitor") {
                if visible {
                    panel.show();
                } else {
                    panel.hide();
                }
                return;
            }
            // Fallback if the panel has not been converted yet.
            if let Some(w) = app2.get_webview_window("monitor") {
                let _ = if visible { w.show() } else { w.hide() };
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = app.get_webview_window("monitor") {
        if visible {
            let _ = w.show();
            let _ = w.set_focus();
        } else {
            let _ = w.hide();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&str]) -> HashSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn sorted(mut v: Vec<String>) -> Vec<String> {
        v.sort();
        v
    }

    #[test]
    fn newly_attention_detects_new_id() {
        let prev = ids(&["a"]);
        let current = ids(&["a", "b"]);
        assert_eq!(sorted(newly_attention(&prev, &current)), vec!["b"]);
    }

    #[test]
    fn newly_attention_does_not_re_notify_persisting_id() {
        let prev = ids(&["a"]);
        let current = ids(&["a"]);
        assert!(newly_attention(&prev, &current).is_empty());
    }

    #[test]
    fn newly_attention_re_notifies_id_that_left_and_returned() {
        // First cycle: "a" enters — notified.
        let prev = ids(&[]);
        let current1 = ids(&["a"]);
        assert_eq!(newly_attention(&prev, &current1), vec!["a"]);

        // Second cycle: "a" leaves attention.
        let current2 = ids(&[]);
        assert!(newly_attention(&current1, &current2).is_empty());

        // Third cycle: "a" re-enters — must be notified again.
        let current3 = ids(&["a"]);
        assert_eq!(newly_attention(&current2, &current3), vec!["a"]);
    }

    #[test]
    fn newly_attention_empty_prev_notifies_all_current() {
        let prev = ids(&[]);
        let current = ids(&["x", "y"]);
        assert_eq!(sorted(newly_attention(&prev, &current)), vec!["x", "y"]);
    }

    #[test]
    fn tray_variant_calm_when_zero() {
        assert_eq!(tray_variant(0), TrayVariant::Calm);
    }

    #[test]
    fn tray_variant_alert_when_nonzero() {
        assert_eq!(tray_variant(1), TrayVariant::Alert);
        assert_eq!(tray_variant(5), TrayVariant::Alert);
    }

    #[test]
    fn show_on_startup_when_floating_and_not_auto_hide() {
        let prefs = Prefs { floating_window: true, auto_hide: false, ..Prefs::default() };
        assert!(show_on_startup(&prefs));
    }

    #[test]
    fn no_show_on_startup_when_auto_hide_on() {
        // auto-hide mode keeps the window hidden until attention arrives.
        let prefs = Prefs { floating_window: true, auto_hide: true, ..Prefs::default() };
        assert!(!show_on_startup(&prefs));
    }

    #[test]
    fn no_show_on_startup_when_floating_disabled() {
        let prefs = Prefs { floating_window: false, auto_hide: false, ..Prefs::default() };
        assert!(!show_on_startup(&prefs));
    }

    #[test]
    fn prefs_serde_roundtrip() {
        let prefs = Prefs {
            floating_window: false,
            always_on_top: true,
            auto_hide: true,
            compact: true,
            open_at_login: false,
            opacity: 75,
            theme: "dark".to_string(),
            language: "es".to_string(),
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let back: Prefs = serde_json::from_str(&json).unwrap();
        assert!(!back.floating_window);
        assert!(back.always_on_top);
        assert!(back.auto_hide);
        assert!(back.compact);
        assert!(!back.open_at_login);
        assert_eq!(back.opacity, 75);
        assert_eq!(back.theme, "dark");
        assert_eq!(back.language, "es");
    }

    #[test]
    fn prefs_default_values() {
        let prefs = Prefs::default();
        assert!(prefs.floating_window);
        assert!(prefs.always_on_top);
        assert!(!prefs.auto_hide);
        assert!(!prefs.compact);
        assert!(prefs.open_at_login);
        assert_eq!(prefs.opacity, 92);
        assert_eq!(prefs.theme, "system");
    }

    #[test]
    fn prefs_serde_defaults_for_missing_fields() {
        // Old prefs.json without opacity/theme: new fields must deserialize to defaults.
        let json = r#"{"floating_window":true,"always_on_top":true,"auto_hide":false,"compact":false,"open_at_login":true}"#;
        let back: Prefs = serde_json::from_str(json).unwrap();
        assert_eq!(back.opacity, 92);
        assert_eq!(back.theme, "system");
    }
}
