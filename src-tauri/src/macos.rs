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

/// Resizes the monitor window to a new height, keeping its TOP edge fixed.
///
/// Why native: `WebviewWindow::set_size()` is a no-op on `decorations:false`
/// windows on macOS (tauri#11975), which is exactly our frameless monitor.
/// We set the NSWindow frame directly instead.
///
/// macOS frames are bottom-left origin: growing the height upward would move
/// the visible top edge. To keep the top fixed we shift origin.y down by the
/// height delta. Width is preserved.
///
/// Must run on the main thread (caller dispatches via run_on_main_thread).
pub fn resize_window_keep_top(window: &WebviewWindow, height: f64) {
    let raw = match window.ns_window() {
        Ok(ptr) => ptr,
        Err(e) => {
            eprintln!("[macos] ns_window() failed: {e}");
            return;
        }
    };
    // Full paths: the tauri_panel! macro already imports NSWindow/NS* at module
    // scope, so we qualify here to avoid name collisions.
    // SAFETY: Tauri guarantees ns_window() returns a live NSWindow for as long
    // as the WebviewWindow exists. Same cast pattern as tauri-runtime-wry.
    let ns_window: &objc2_app_kit::NSWindow =
        unsafe { &*raw.cast::<objc2_app_kit::NSWindow>() };

    let frame = ns_window.frame();
    let new_height = height.max(1.0);
    // Keep the top edge fixed: top_y = origin.y + height is invariant.
    let new_y = frame.origin.y + (frame.size.height - new_height);
    let new_frame = objc2_foundation::NSRect::new(
        objc2_foundation::NSPoint::new(frame.origin.x, new_y),
        objc2_foundation::NSSize::new(frame.size.width, new_height),
    );
    // display:true repaints immediately.
    unsafe { ns_window.setFrame_display(new_frame, true) };
}

/// Sets the panel's window level to reflect the "always on top" preference.
///
/// `setup_panel` pins the level to Status (above fullscreen) so the monitor
/// floats over everything. To honour the user turning "always on top" OFF we
/// drop the level to Normal at runtime, so ordinary windows can cover it again.
/// Turning it back ON restores the Status level. Dispatched to the main thread:
/// raw NSPanel calls must not run off it.
pub fn set_always_on_top(app: &tauri::AppHandle, on_top: bool) {
    use tauri_nspanel::ManagerExt;
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Ok(panel) = app2.get_webview_panel("monitor") {
            let level = if on_top {
                PanelLevel::Status
            } else {
                PanelLevel::Normal
            };
            panel.set_level(level.value());
        }
    });
}
