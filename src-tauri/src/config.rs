//! Preferencias del usuario, persistidas en un JSON del config dir de la app.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Prefs {
    pub floating_window: bool,
    pub always_on_top: bool,
    pub auto_hide: bool,
    pub compact: bool,
    pub open_at_login: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            floating_window: true,
            always_on_top: true,
            auto_hide: false,
            compact: false,
            open_at_login: true,
        }
    }
}

fn prefs_path(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("prefs.json"))
}

pub fn load(app: &AppHandle) -> Prefs {
    let Some(path) = prefs_path(app) else {
        return Prefs::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, prefs: &Prefs) {
    if let Some(path) = prefs_path(app) {
        if let Ok(json) = serde_json::to_string_pretty(prefs) {
            let _ = std::fs::write(path, json);
        }
    }
}
