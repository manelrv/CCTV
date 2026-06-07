// Evita la consola en Windows release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod hooks;
mod i18n;
mod jobs;
#[cfg(target_os = "macos")]
mod macos;
mod refresh;
mod server;
mod state;
mod tray;

use std::sync::Arc;
use std::time::Duration;
use tauri::Manager;

fn main() {
    let store = Arc::new(state::Store::default());

    // Carga las prefs en managed state para que refresh() las lea sin I/O de disco.
    let initial_prefs = {
        // Necesitamos las prefs antes de setup() para inicializar el managed state.
        // Usamos el path de config estandar de la plataforma directamente.
        config::load_from_path(config::default_prefs_path())
    };
    let prefs_state = refresh::PrefsState(std::sync::Mutex::new(initial_prefs));

    // Build the plugin chain. tauri-nspanel must be registered before setup()
    // so its WebviewPanelManager is available when to_panel() is called.
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ));

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .manage(store.clone())
        .manage(prefs_state)
        .setup({
            let store = store.clone();
            move |app| {
                let handle = app.handle().clone();

                // En macOS la app debe ser Accessory (utilidad de menubar, sin
                // icono en el Dock): una app Regular no puede meter ventanas en
                // el Space fullscreen de otra app por alto que sea su level.
                #[cfg(target_os = "macos")]
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);

                // Ventana flotante: convierte a NSPanel (macOS) para poder flotar
                // sobre apps en fullscreen. En otras plataformas, set_visible_on_all_workspaces
                // es suficiente.
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

                // Servidor de hooks (vive en una task de tokio).
                let app_state = server::AppState {
                    store: store.clone(),
                    app: handle.clone(),
                };
                tauri::async_runtime::spawn(server::serve(app_state));

                // Watcher de ficheros del supervisor (~/.claude/jobs/).
                jobs::start(store.clone(), handle.clone());

                // Reaper de sesiones muertas.
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

                // Bandeja.
                tray::build(&handle)?;

                Ok(())
            }
        })
        // Comandos para que el frontend pida el snapshot y las prefs al montar.
        .invoke_handler(tauri::generate_handler![get_instances, get_prefs])
        .build(tauri::generate_context!())
        .expect("error al construir la app Tauri")
        .run(|_app, event| {
            // Mantener la app viva en bandeja aunque se cierre la ventana.
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
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
