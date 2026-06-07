//! Centralised state propagation to the webview and the tray.
//! Single emission point: replaces the scattered emit() calls in server.rs, jobs.rs,
//! and the reaper in main.rs. See docs/ARCHITECTURE.md#pushing-state-to-the-webview.

use crate::config::Prefs;
use crate::state::Store;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, image::Image};

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

/// Emits the snapshot to the webview, updates the tray icon, and applies
/// auto-show/hide according to current preferences.
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
}

/// Applies auto-hide logic: hides the floating window when nothing needs attention
/// and the toggle is active; shows it when attention is needed (only if floating_window
/// is enabled, to avoid overriding the user's preference).
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
    fn prefs_serde_roundtrip() {
        let prefs = Prefs {
            floating_window: false,
            always_on_top: true,
            auto_hide: true,
            compact: true,
            open_at_login: false,
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let back: Prefs = serde_json::from_str(&json).unwrap();
        assert!(!back.floating_window);
        assert!(back.always_on_top);
        assert!(back.auto_hide);
        assert!(back.compact);
        assert!(!back.open_at_login);
    }

    #[test]
    fn prefs_default_values() {
        let prefs = Prefs::default();
        assert!(prefs.floating_window);
        assert!(prefs.always_on_top);
        assert!(!prefs.auto_hide);
        assert!(!prefs.compact);
        assert!(prefs.open_at_login);
    }
}
