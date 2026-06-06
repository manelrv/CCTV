// Evita la consola en Windows release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod hooks;
mod i18n;
mod jobs;
mod server;
mod state;
mod tray;

use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager};

fn main() {
    let store = Arc::new(state::Store::default());

    tauri::Builder::default()
        .manage(store.clone())
        .setup({
            let store = store.clone();
            move |app| {
                let handle = app.handle().clone();

                // Ventana flotante: arranca oculta, visible en todos los espacios.
                if let Some(w) = app.get_webview_window("monitor") {
                    // TODO(claude-code): en macOS, para flotar sobre apps en
                    // fullscreen puede hacer falta subir el NSWindow level via
                    // objc2/cocoa. Ver docs/ARCHITECTURE.md#macos.
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
                            let _ = reaper_handle.emit("instances", &reaper_store.snapshot());
                        }
                    }
                });

                // Bandeja.
                tray::build(&handle)?;

                Ok(())
            }
        })
        // Comando para que el frontend pida el snapshot inicial al montar.
        .invoke_handler(tauri::generate_handler![get_instances])
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
