//! Propagacion centralizada del estado al webview y a la bandeja.
//! UNICO punto de emision: reemplaza los emit() dispersos en server.rs, jobs.rs
//! y el reaper de main.rs. Ver docs/ARCHITECTURE.md#empuje-de-estado-al-webview.

use crate::config::Prefs;
use crate::state::Store;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, image::Image};

// Icono calm: ninguna instancia reclama atencion del usuario.
const ICON_CALM: &[u8] = include_bytes!("../../icons/tray-calm-64.png");
// Icono alert: al menos una instancia reclama atencion.
const ICON_ALERT: &[u8] = include_bytes!("../../icons/tray-alert-64.png");

/// Variante de icono para la bandeja (usada en tests unitarios).
#[derive(Debug, PartialEq, Eq)]
pub enum TrayVariant {
    Calm,
    Alert,
}

/// Devuelve la variante correcta segun el conteo de atencion.
/// Logica pura, sin runtime de Tauri — testeable de forma unitaria.
pub fn tray_variant(attention: usize) -> TrayVariant {
    if attention > 0 {
        TrayVariant::Alert
    } else {
        TrayVariant::Calm
    }
}

/// Estado de preferencias compartido entre hilos, gestionado como managed state.
/// Permite que refresh() acceda a las prefs sin leer disco en cada evento de hook.
#[derive(Default)]
pub struct PrefsState(pub Mutex<Prefs>);

/// Emite el snapshot al webview, actualiza el icono de la bandeja y aplica
/// auto-show/hide segun las preferencias actuales.
///
/// Esta funcion es el UNICO punto de emision del estado. Debe ser rapida: no
/// hace I/O de disco (las prefs vienen del managed state).
pub fn refresh(app: &AppHandle, store: &Arc<Store>) {
    let snapshot = store.snapshot();
    let attention = store.attention_count();

    // 1. Emite snapshot al webview.
    let _ = app.emit("instances", &snapshot);

    // 2. Actualiza icono y titulo de bandeja.
    if let Some(tray) = app.tray_by_id("main") {
        let icon_bytes = match tray_variant(attention) {
            TrayVariant::Alert => ICON_ALERT,
            TrayVariant::Calm => ICON_CALM,
        };
        if let Ok(img) = Image::from_bytes(icon_bytes) {
            let _ = tray.set_icon(Some(img));
        }
        // En macOS, el titulo aparece junto al icono en la barra de menu.
        // En otros SO es un no-op inofensivo.
        let title = if attention > 0 {
            attention.to_string()
        } else {
            String::new()
        };
        let _ = tray.set_title(Some(&title));
    }

    // 3. Auto-hide/show segun preferencias (managed state, sin I/O).
    if let Some(prefs_state) = app.try_state::<PrefsState>() {
        let prefs = prefs_state.0.lock().unwrap().clone();
        apply_auto_hide(app, &prefs, attention);
    }
}

/// Aplica la logica de auto-hide: oculta la ventana cuando no hay atencion y el
/// toggle esta activo; la muestra cuando hay atencion (solo si floating_window
/// esta habilitado para no violar la preferencia del usuario).
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

/// Muestra/oculta el panel (macOS) o la ventana (otros SO).
///
/// OJO threading: refresh() corre desde hilos de tokio (hooks), del watcher de
/// jobs y del reaper. Las APIs de ventana de Tauri redespachan al main thread
/// internamente, pero el handle crudo del NSPanel NO — llamar a orderOut/show
/// desde otro hilo aborta con SIGTRAP en AppKit. Por eso todo pasa por
/// run_on_main_thread y el panel se resuelve DENTRO del closure.
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
            // Fallback si el panel aun no esta convertido.
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
