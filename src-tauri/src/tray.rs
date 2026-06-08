//! Tray icon + menu.
//! The tray process is the one that always lives and hosts the hook server.

use crate::config;
use crate::i18n;
use crate::refresh::{self, PrefsState};
use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};
use tauri_plugin_autostart::ManagerExt;

// Initial calm tray icon (embedded in the binary).
const ICON_CALM: &[u8] = include_bytes!("../../icons/tray-calm-64.png");

/// Opacity presets exposed in the tray submenu (percent values).
const OPACITY_PRESETS: &[u8] = &[100, 90, 80, 70, 60, 50];

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let prefs = config::load(app);

    // Initial icon: calm (no instances at startup).
    let calm_icon = tauri::image::Image::from_bytes(ICON_CALM)
        .unwrap_or_else(|_| app.default_window_icon().cloned().unwrap());

    let _tray = TrayIconBuilder::with_id("main")
        .menu(&build_menu(app, &prefs)?)
        .show_menu_on_left_click(true)
        .icon(calm_icon)
        .tooltip("CCTV")
        .on_menu_event(move |app, event| handle_menu(app, event.id().as_ref()))
        .build(app)?;

    Ok(())
}

/// Constructs (or rebuilds) the tray menu from current prefs.
/// Called once on startup and again after every theme/opacity change so
/// the check marks reflect the new selection.
fn build_menu(app: &AppHandle, prefs: &config::Prefs) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let s = i18n::strings(i18n::Lang::from_pref(&prefs.language));

    // --- toggles ---
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

    // --- theme submenu ---
    let theme_system =
        CheckMenuItemBuilder::with_id("theme_system", s.theme_system)
            .checked(prefs.theme == "system")
            .build(app)?;
    let theme_dark =
        CheckMenuItemBuilder::with_id("theme_dark", s.theme_dark)
            .checked(prefs.theme == "dark")
            .build(app)?;
    let theme_light =
        CheckMenuItemBuilder::with_id("theme_light", s.theme_light)
            .checked(prefs.theme == "light")
            .build(app)?;
    let theme_sub = SubmenuBuilder::new(app, s.theme)
        .item(&theme_system)
        .item(&theme_dark)
        .item(&theme_light)
        .build()?;

    // --- opacity submenu ---
    // Round prefs.opacity to the nearest preset to decide which item is checked.
    let nearest = nearest_preset(prefs.opacity);
    let opacity_items: Vec<_> = OPACITY_PRESETS
        .iter()
        .map(|&p| {
            CheckMenuItemBuilder::with_id(
                format!("opacity_{p}"),
                format!("{p}%"),
            )
            .checked(p == nearest)
            .build(app)
        })
        .collect::<Result<_, _>>()?;

    let mut opacity_builder = SubmenuBuilder::new(app, s.opacity);
    for item in &opacity_items {
        opacity_builder = opacity_builder.item(item);
    }
    let opacity_sub = opacity_builder.build()?;

    // --- language submenu ---
    // "Automatic" follows the system locale; explicit entries pin a language.
    // Each language is shown in its own native name (i18n::LANGUAGES).
    let lang_auto = CheckMenuItemBuilder::with_id("lang_auto", s.language_auto)
        .checked(prefs.language.is_empty() || prefs.language == "auto")
        .build(app)?;
    let lang_items: Vec<_> = i18n::LANGUAGES
        .iter()
        .map(|&(code, name)| {
            CheckMenuItemBuilder::with_id(format!("lang_{code}"), name)
                .checked(prefs.language == code)
                .build(app)
        })
        .collect::<Result<_, _>>()?;

    let mut language_builder = SubmenuBuilder::new(app, s.language).item(&lang_auto);
    for item in &lang_items {
        language_builder = language_builder.item(item);
    }
    let language_sub = language_builder.build()?;

    // --- quit ---
    let quit = MenuItemBuilder::with_id("quit", s.quit).build(app)?;

    MenuBuilder::new(app)
        .item(&show)
        .separator()
        .item(&floating)
        .item(&on_top)
        .item(&auto_hide)
        .item(&compact)
        .separator()
        .item(&at_login)
        .separator()
        .item(&theme_sub)
        .item(&opacity_sub)
        .item(&language_sub)
        .separator()
        .item(&quit)
        .build()
}

/// Returns the preset value (from OPACITY_PRESETS) closest to `value`.
fn nearest_preset(value: u8) -> u8 {
    *OPACITY_PRESETS
        .iter()
        .min_by_key(|&&p| (p as i16 - value as i16).unsigned_abs())
        .unwrap_or(&100)
}

fn handle_menu(app: &AppHandle, id: &str) {
    let mut prefs = config::load(app);
    match id {
        "show" => toggle_window(app, true),
        "quit" => app.exit(0),

        "theme_system" => {
            prefs.theme = "system".to_string();
            let _ = app.emit("prefs", &prefs);
            persist_and_sync(app, &prefs);
            rebuild_menu(app, &prefs);
        }
        "theme_dark" => {
            prefs.theme = "dark".to_string();
            let _ = app.emit("prefs", &prefs);
            persist_and_sync(app, &prefs);
            rebuild_menu(app, &prefs);
        }
        "theme_light" => {
            prefs.theme = "light".to_string();
            let _ = app.emit("prefs", &prefs);
            persist_and_sync(app, &prefs);
            rebuild_menu(app, &prefs);
        }

        "lang_auto" => {
            prefs.language = "auto".to_string();
            let _ = app.emit("prefs", &prefs);
            persist_and_sync(app, &prefs);
            rebuild_menu(app, &prefs);
        }

        id if id.starts_with("lang_") => {
            prefs.language = id.trim_start_matches("lang_").to_string();
            let _ = app.emit("prefs", &prefs);
            persist_and_sync(app, &prefs);
            rebuild_menu(app, &prefs);
        }

        id if id.starts_with("opacity_") => {
            if let Ok(pct) = id.trim_start_matches("opacity_").parse::<u8>() {
                prefs.opacity = pct;
                let _ = app.emit("prefs", &prefs);
                persist_and_sync(app, &prefs);
                rebuild_menu(app, &prefs);
            }
        }

        "toggle_floating" => {
            prefs.floating_window = !prefs.floating_window;
            toggle_window(app, prefs.floating_window);
            persist_and_sync(app, &prefs);
            rebuild_menu(app, &prefs);
        }

        "toggle_on_top" => {
            prefs.always_on_top = !prefs.always_on_top;
            // On macOS the panel's level is fixed at NSStatus (25) by setup_panel()
            // and is managed by the NSPanel subclass — set_always_on_top is a no-op
            // for the fullscreen guarantee. On other platforms it works normally.
            #[cfg(not(target_os = "macos"))]
            if let Some(w) = app.get_webview_window("monitor") {
                let _ = w.set_always_on_top(prefs.always_on_top);
            }
            persist_and_sync(app, &prefs);
            rebuild_menu(app, &prefs);
        }

        "toggle_auto_hide" => {
            prefs.auto_hide = !prefs.auto_hide;
            // Apply the new preference immediately based on the current state.
            if let Some(store) = app.try_state::<std::sync::Arc<crate::state::Store>>() {
                let attention = store.attention_count();
                refresh::apply_auto_hide(app, &prefs, attention);
            }
            persist_and_sync(app, &prefs);
            rebuild_menu(app, &prefs);
        }

        "toggle_compact" => {
            prefs.compact = !prefs.compact;
            // Notify the frontend so it applies or removes the .compact class.
            let _ = app.emit("prefs", &prefs);
            persist_and_sync(app, &prefs);
            rebuild_menu(app, &prefs);
        }

        "toggle_at_login" => {
            prefs.open_at_login = !prefs.open_at_login;
            // Delegate to the autostart plugin (ManagerExt::autolaunch()).
            let manager = app.autolaunch();
            if prefs.open_at_login {
                let _ = manager.enable();
            } else {
                let _ = manager.disable();
            }
            persist_and_sync(app, &prefs);
            rebuild_menu(app, &prefs);
        }

        _ => {}
    }
}

/// Rebuilds the tray menu and attaches it to the tray icon so check marks
/// reflect the updated prefs. A no-op if the tray icon is not found.
fn rebuild_menu(app: &AppHandle, prefs: &config::Prefs) {
    if let Ok(menu) = build_menu(app, prefs) {
        if let Some(tray) = app.tray_by_id("main") {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Persists prefs to disk and updates the managed state (PrefsState).
fn persist_and_sync(app: &AppHandle, prefs: &config::Prefs) {
    config::save(app, prefs);
    if let Some(state) = app.try_state::<PrefsState>() {
        *state.0.lock().unwrap() = prefs.clone();
    }
}

fn toggle_window(app: &AppHandle, show: bool) {
    // Delegate to the thread-safe helper: NSPanel operations can only run on the
    // main thread (see refresh::set_panel_visible).
    refresh::set_panel_visible(app, show);
}
