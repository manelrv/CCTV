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

use std::sync::Arc;
use std::time::Duration;
use tauri::Manager;

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
        ));

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .manage(store.clone())
        .manage(prefs_state)
        .manage(refresh::AttentionState::default())
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
