// macOS-specific setup: converts the monitor window into an NSPanel so it can
// float above fullscreen spaces on any Space.
//
// WHY NSPanel IS REQUIRED
// =======================
// A plain Tauri NSWindow cannot reliably float above fullscreen apps on macOS,
// even with:
//   - collectionBehavior = CanJoinAllSpaces | FullScreenAuxiliary (0x101)
//   - level = NSPopUpMenuWindowLevel (101)
//   - ActivationPolicy::Accessory
//
// All of those bits were verified applied (confirmed via logs) and the window
// STILL disappeared when another app entered fullscreen. The root cause is that
// macOS gates fullscreen-space visibility on the window being an NSPanel
// subclass, not just on the collection behavior bits. Converting via
// tauri-nspanel resolves this empirically.
//
// APPROACH
// ========
// We use the tauri-nspanel plugin (branch v2.1) which converts a WebviewWindow
// into an NSPanel using the `tauri_panel!` macro + `WebviewWindowExt::to_panel`.
// The panel is then configured with:
//   - NonactivatingPanel style mask (doesn't steal focus from the fullscreen app)
//   - Status level (25) — same as macOS activity monitors; penetrates fullscreen
//   - CollectionBehavior: CanJoinAllSpaces | FullScreenAuxiliary | Stationary
//
// The panel handle is stored in the plugin's internal ManagerExt store so that
// tray.rs and refresh.rs can retrieve it later via app.get_webview_panel("monitor").
#![cfg(target_os = "macos")]

use tauri::{Manager, WebviewWindow};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, PanelLevel, StyleMask, WebviewWindowExt,
};

// Define our monitor panel class.
// can_become_key_window: false — a monitoring utility must never steal focus
// from the user's active app (especially when that app is fullscreen).
// is_floating_panel: true — floating NSPanel subtype.
tauri_panel! {
    panel!(MonitorPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true
        }
    })
}

/// Converts the "monitor" WebviewWindow into an NSPanel and configures it to
/// float above fullscreen apps on all Spaces without stealing focus.
///
/// Must be called from the main thread (Tauri setup() runs on main thread).
/// After this call, show/hide the panel via `app.get_webview_panel("monitor")`.
pub fn setup_panel(window: &WebviewWindow) {
    let panel = match window.to_panel::<MonitorPanel>() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[macos] to_panel() failed: {e:?}");
            return;
        }
    };

    // NonactivatingPanel: clicking the monitor overlay does NOT activate this
    // app or steal focus from whatever app the user is working in (fullscreen or not).
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());

    // Status level (25): same level used by macOS system status-bar monitors
    // (Activity Monitor widget, etc.). Penetrates fullscreen Spaces.
    // NSPopUpMenuWindowLevel (101) was tried but is unnecessarily aggressive.
    panel.set_level(PanelLevel::Status.value());

    // CanJoinAllSpaces: panel follows the user across all Spaces.
    // FullScreenAuxiliary: panel is allowed inside a fullscreen Space.
    // Stationary: panel doesn't move with Exposé/Mission Control gestures —
    //   appropriate for a passive monitor that always stays put.
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .stationary()
            .into(),
    );
}
