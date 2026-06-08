// Suppresses the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
#[cfg(target_os = "macos")]
mod focus;
mod hooks;
mod i18n;
mod jobs;
#[cfg(target_os = "macos")]
mod macos;
mod refresh;
mod server;
mod state;
mod transcript;
mod tray;

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Cursor through the attention-needing instances for the global hotkey cycle.
struct HotkeyCursor(Mutex<usize>);

/// Returns the next focusable instance index and the updated cursor value.
///
/// - `instances`: the slice of attention-needing instances to cycle through.
/// - `cursor`: the current position (may be out of range after the slice shrinks).
///
/// Behaviour:
/// - Returns `(None, 0)` when `instances` is empty.
/// - Clamps `cursor` to `cursor % len` on entry (handles stale cursors after shrink).
/// - Skips instances with `terminal = None` (background-only; nothing to focus).
/// - Loops at most `instances.len()` times to avoid infinite loops when nothing is focusable.
/// - Returns `(None, 0)` when all instances have `terminal = None`.
/// - Returns `(Some(idx), next_cursor)` where `next_cursor = (idx + 1) % len`.
fn next_focusable(instances: &[state::Instance], cursor: usize) -> (Option<usize>, usize) {
    let len = instances.len();
    if len == 0 {
        return (None, 0);
    }
    let start = cursor % len;
    for offset in 0..len {
        let idx = (start + offset) % len;
        if instances[idx].terminal.is_some() {
            return (Some(idx), (idx + 1) % len);
        }
    }
    (None, 0)
}

#[cfg(test)]
mod hotkey_tests {
    use super::*;
    use crate::state::{Instance, InstanceState, Source, TerminalRef};

    fn make_instance(id: &str, has_terminal: bool) -> Instance {
        Instance {
            session_id: id.to_string(),
            cwd: "/tmp".to_string(),
            project: id.to_string(),
            state: InstanceState::WaitingInput,
            detail: None,
            source: Source::Foreground,
            started_at: 0,
            last_event_at: 0,
            context_tokens: None,
            in_flight_tasks: None,
            terminal: if has_terminal {
                Some(TerminalRef {
                    program: "iTerm.app".to_string(),
                    session_id: None,
                    tty: Some("ttys001".to_string()),
                    focus_url: None,
                })
            } else {
                None
            },
        }
    }

    #[test]
    fn test_empty_set() {
        let instances: Vec<Instance> = vec![];
        let (idx, cursor) = next_focusable(&instances, 0);
        assert_eq!(idx, None);
        assert_eq!(cursor, 0);
    }

    #[test]
    fn test_all_background() {
        // All instances have terminal = None (background); nothing to focus.
        let instances = vec![
            make_instance("a", false),
            make_instance("b", false),
            make_instance("c", false),
        ];
        let (idx, cursor) = next_focusable(&instances, 0);
        assert_eq!(idx, None);
        assert_eq!(cursor, 0);
    }

    #[test]
    fn test_single_foreground() {
        // One instance with a terminal; cursor at 0 → picks index 0 and advances to 1 % 1 = 0.
        let instances = vec![make_instance("a", true)];
        let (idx, cursor) = next_focusable(&instances, 0);
        assert_eq!(idx, Some(0));
        assert_eq!(cursor, 0); // (0 + 1) % 1 = 0
    }

    #[test]
    fn test_multiple_foreground_wrap() {
        // Two foreground instances; cursor at 1 → picks index 1, then wraps to 0.
        let instances = vec![make_instance("a", true), make_instance("b", true)];
        let (idx, cursor) = next_focusable(&instances, 1);
        assert_eq!(idx, Some(1));
        assert_eq!(cursor, 0); // (1 + 1) % 2 = 0
    }

    #[test]
    fn test_foreground_background_mix() {
        // index 0 = background, index 1 = foreground, index 2 = background.
        // cursor = 0 → skips index 0, lands on index 1.
        let instances = vec![
            make_instance("a", false),
            make_instance("b", true),
            make_instance("c", false),
        ];
        let (idx, cursor) = next_focusable(&instances, 0);
        assert_eq!(idx, Some(1));
        assert_eq!(cursor, 2); // (1 + 1) % 3 = 2
    }

    #[test]
    fn test_cursor_past_end_after_shrink() {
        // cursor = 5 on a 2-element slice → clamp-on-read: 5 % 2 = 1.
        // Index 1 has a terminal → returns (Some(1), (1+1)%2 = 0).
        let instances = vec![make_instance("a", true), make_instance("b", true)];
        let (idx, cursor) = next_focusable(&instances, 5);
        assert_eq!(idx, Some(1));
        assert_eq!(cursor, 0); // (1 + 1) % 2 = 0
    }
}

fn main() {
    let store = Arc::new(state::Store::default());

    // Load prefs into managed state so refresh() can read them without disk I/O.
    let initial_prefs = {
        // We need prefs before setup() to initialize managed state.
        // Use the platform-standard config path directly.
        config::load_from_path(config::default_prefs_path())
    };
    let prefs_state = refresh::PrefsState(std::sync::Mutex::new(initial_prefs));

    // Build the plugin chain. tauri-nspanel must be registered before setup()
    // so its WebviewPanelManager is available when to_panel() is called.
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build());

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .manage(store.clone())
        .manage(prefs_state)
        .manage(refresh::AttentionState::default())
        .manage(HotkeyCursor(Mutex::new(0usize)))
        .setup({
            let store = store.clone();
            move |app| {
                let handle = app.handle().clone();

                // On macOS the app must be Accessory (menu-bar utility, no Dock icon):
                // a Regular-policy app cannot place windows in another app's fullscreen
                // Space regardless of window level.
                #[cfg(target_os = "macos")]
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);

                // Floating window: convert to NSPanel (macOS) to float above fullscreen apps.
                // On other platforms, set_visible_on_all_workspaces is sufficient.
                if let Some(w) = app.get_webview_window("monitor") {
                    // macOS: convert to NSPanel — required to float above fullscreen Spaces.
                    // The panel setup handles collectionBehavior + level internally.
                    // set_visible_on_all_workspaces is a no-op after panel conversion on macOS
                    // (the panel's own collectionBehavior takes precedence), but harmless.
                    #[cfg(target_os = "macos")]
                    macos::setup_panel(&w);
                    #[cfg(not(target_os = "macos"))]
                    let _ = w.set_visible_on_all_workspaces(true);
                }

                // Hook server (runs in a tokio task).
                let app_state = server::AppState {
                    store: store.clone(),
                    app: handle.clone(),
                    transcript_last_read: std::sync::Arc::new(std::sync::Mutex::new(
                        std::collections::HashMap::new(),
                    )),
                };
                tauri::async_runtime::spawn(server::serve(app_state));

                // Supervisor file watcher (~/.claude/jobs/).
                jobs::start(store.clone(), handle.clone());

                // Dead-session reaper.
                let reaper_store = store.clone();
                let reaper_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    let mut tick = tokio::time::interval(Duration::from_secs(30));
                    loop {
                        tick.tick().await;
                        if reaper_store.reap() {
                            refresh::refresh(&reaper_handle, &reaper_store);
                        }
                    }
                });

                // Tray.
                tray::build(&handle)?;

                // Global hotkey: CmdOrCtrl+Shift+Space — cycle through attention-needing instances.
                // The handler is registered here and runs on the global-shortcut event thread.
                // All async work is dispatched via tauri::async_runtime::spawn to avoid
                // AppKit main-thread deadlocks (NSPanel/focus calls must not run on the
                // event thread directly).
                {
                    let store_hk = store.clone();
                    let handle_hk = handle.clone();
                    app.global_shortcut().on_shortcut(
                        "CmdOrCtrl+Shift+Space",
                        move |_app, _shortcut, event| {
                            // Ignore key-release; act only on the initial press.
                            if event.state == ShortcutState::Released {
                                return;
                            }

                            let store_inner = store_hk.clone();
                            let handle_inner = handle_hk.clone();
                            tauri::async_runtime::spawn(async move {
                                // Collect attention-needing instances from the store.
                                let attention: Vec<state::Instance> = store_inner
                                    .snapshot()
                                    .into_iter()
                                    .filter(|i| i.state.needs_attention())
                                    .collect();

                                // Advance cursor and find the next focusable instance.
                                let chosen_terminal = {
                                    let cursor_state =
                                        handle_inner.state::<HotkeyCursor>();
                                    let mut cursor =
                                        cursor_state.0.lock().unwrap();
                                    let (maybe_idx, new_cursor) =
                                        next_focusable(&attention, *cursor);
                                    *cursor = new_cursor;
                                    maybe_idx.and_then(|idx| attention[idx].terminal.clone())
                                };

                                match chosen_terminal {
                                    Some(term) => {
                                        // macOS: focus the terminal hosting the session.
                                        #[cfg(target_os = "macos")]
                                        {
                                            focus::focus_terminal(&term);
                                        }
                                        // Non-macOS: no terminal focus API; raise the CCTV window.
                                        #[cfg(not(target_os = "macos"))]
                                        {
                                            let _ = term; // term is unused on non-macOS
                                            refresh::set_panel_visible(&handle_inner, true);
                                        }
                                    }
                                    None => {
                                        // Nothing focusable (empty set or all background):
                                        // raise the CCTV window so the user can see the panel.
                                        refresh::set_panel_visible(&handle_inner, true);
                                    }
                                }
                            });
                        },
                    )?;
                }

                Ok(())
            }
        })
        // Commands the frontend uses to fetch the snapshot and prefs on mount.
        .invoke_handler(tauri::generate_handler![
            get_instances,
            get_prefs,
            focus_session,
            resize_monitor
        ])
        .build(tauri::generate_context!())
        .expect("error building the Tauri app")
        .run(|_app, event| {
            // Keep the app alive in the tray when windows close (code: None),
            // but let explicit exits through — the tray Quit item calls
            // app.exit(0), which arrives here with code: Some(0).
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}

#[tauri::command]
fn get_instances(state: tauri::State<'_, Arc<state::Store>>) -> Vec<state::Instance> {
    state.snapshot()
}

#[tauri::command]
fn get_prefs(state: tauri::State<'_, refresh::PrefsState>) -> config::Prefs {
    state.0.lock().unwrap().clone()
}

/// Resizes the monitor window to `height` logical px, keeping the top edge fixed.
/// Needed because set_size() is a no-op on frameless (decorations:false) macOS
/// windows (tauri#11975). The actual resize runs on the main thread (AppKit).
#[tauri::command]
fn resize_monitor(height: f64, app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(w) = app2.get_webview_window("monitor") {
                macos::resize_window_keep_top(&w, height);
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Other platforms: set_size works on frameless windows.
        if let Some(w) = app.get_webview_window("monitor") {
            let _ = w.set_size(tauri::LogicalSize::new(360.0, height));
        }
    }
}

/// Attempts to focus the terminal window/tab hosting the given session.
/// Returns true if focus was achieved (or best-effort app activation succeeded).
/// Returns false on non-macOS platforms or when the session has no terminal info.
#[tauri::command]
fn focus_session(session_id: String, store: tauri::State<'_, std::sync::Arc<state::Store>>) -> bool {
    #[cfg(target_os = "macos")]
    {
        let terminal = {
            // Snapshot a clone of the terminal ref so we release the lock immediately.
            store.inner_snapshot_terminal(&session_id)
        };
        if let Some(term) = terminal {
            return focus::focus_terminal(&term);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (session_id, store);
    false
}
