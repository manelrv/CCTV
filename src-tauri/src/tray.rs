//! Icono de bandeja + menu de preferencias.
//! El proceso de bandeja es el que vive siempre y hostea el servidor de hooks.

use crate::config;
use crate::i18n;
use crate::refresh::{self, PrefsState};
use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};
use tauri_plugin_autostart::ManagerExt;

// Icono inicial de bandeja en calma (embed en el binario).
const ICON_CALM: &[u8] = include_bytes!("../../icons/tray-calm-64.png");

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let prefs = config::load(app);
    let s = i18n::strings(i18n::Lang::detect());

    let show = MenuItemBuilder::with_id("show", s.show_window).build(app)?;
    let floating = CheckMenuItemBuilder::with_id("toggle_floating", s.floating_window)
        .checked(prefs.floating_window)
        .build(app)?;
    let on_top = CheckMenuItemBuilder::with_id("toggle_on_top", s.always_on_top)
        .checked(prefs.always_on_top)
        .build(app)?;
    let auto_hide = CheckMenuItemBuilder::with_id("toggle_auto_hide", s.auto_hide)
        .checked(prefs.auto_hide)
        .build(app)?;
    let compact = CheckMenuItemBuilder::with_id("toggle_compact", s.compact_mode)
        .checked(prefs.compact)
        .build(app)?;
    let at_login = CheckMenuItemBuilder::with_id("toggle_at_login", s.open_at_login)
        .checked(prefs.open_at_login)
        .build(app)?;
    let quit = MenuItemBuilder::with_id("quit", s.quit).build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show)
        .separator()
        .item(&floating)
        .item(&on_top)
        .item(&auto_hide)
        .item(&compact)
        .separator()
        .item(&at_login)
        .separator()
        .item(&quit)
        .build()?;

    // Icono inicial: calm (no hay instancias al arrancar).
    let calm_icon = tauri::image::Image::from_bytes(ICON_CALM)
        .unwrap_or_else(|_| app.default_window_icon().cloned().unwrap());

    let _tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .icon(calm_icon)
        .tooltip("CCTV")
        .on_menu_event(move |app, event| handle_menu(app, event.id().as_ref()))
        .build(app)?;

    Ok(())
}

fn handle_menu(app: &AppHandle, id: &str) {
    let mut prefs = config::load(app);
    match id {
        "show" => toggle_window(app, true),
        "quit" => app.exit(0),

        "toggle_floating" => {
            prefs.floating_window = !prefs.floating_window;
            toggle_window(app, prefs.floating_window);
            persist_and_sync(app, &prefs);
        }

        "toggle_on_top" => {
            prefs.always_on_top = !prefs.always_on_top;
            if let Some(w) = app.get_webview_window("monitor") {
                let _ = w.set_always_on_top(prefs.always_on_top);
            }
            persist_and_sync(app, &prefs);
        }

        "toggle_auto_hide" => {
            prefs.auto_hide = !prefs.auto_hide;
            // Aplica la nueva preferencia de inmediato segun el estado actual.
            if let Some(store) = app.try_state::<std::sync::Arc<crate::state::Store>>() {
                let attention = store.attention_count();
                refresh::apply_auto_hide(app, &prefs, attention);
            }
            persist_and_sync(app, &prefs);
        }

        "toggle_compact" => {
            prefs.compact = !prefs.compact;
            // Notifica al frontend para que aplique/quite la clase .compact.
            let _ = app.emit("prefs", &prefs);
            persist_and_sync(app, &prefs);
        }

        "toggle_at_login" => {
            prefs.open_at_login = !prefs.open_at_login;
            // Delega en el plugin de autostart (ManagerExt::autolaunch()).
            let manager = app.autolaunch();
            if prefs.open_at_login {
                let _ = manager.enable();
            } else {
                let _ = manager.disable();
            }
            persist_and_sync(app, &prefs);
        }

        _ => {}
    }
}

/// Persiste las prefs en disco y actualiza el managed state (PrefsState).
fn persist_and_sync(app: &AppHandle, prefs: &config::Prefs) {
    config::save(app, prefs);
    if let Some(state) = app.try_state::<PrefsState>() {
        *state.0.lock().unwrap() = prefs.clone();
    }
}

fn toggle_window(app: &AppHandle, show: bool) {
    if let Some(w) = app.get_webview_window("monitor") {
        if show {
            let _ = w.show();
            let _ = w.set_focus();
        } else {
            let _ = w.hide();
        }
    }
}
