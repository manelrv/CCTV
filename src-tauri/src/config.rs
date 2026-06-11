//! User preferences, persisted as JSON in the app's config directory.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn default_opacity() -> u8 {
    92
}

fn default_theme() -> String {
    "system".to_string()
}

fn default_language() -> String {
    "auto".to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Prefs {
    pub floating_window: bool,
    pub always_on_top: bool,
    pub auto_hide: bool,
    pub compact: bool,
    pub open_at_login: bool,
    #[serde(default = "default_opacity")]
    pub opacity: u8,
    #[serde(default = "default_theme")]
    pub theme: String,
    /// UI language: "auto" (follow system locale) or a supported code
    /// ("en", "es", "pt", "de", "fr", "it", "ca", "ru"). Default "auto".
    #[serde(default = "default_language")]
    pub language: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            floating_window: true,
            always_on_top: true,
            auto_hide: false,
            compact: false,
            open_at_login: true,
            opacity: default_opacity(),
            theme: default_theme(),
            language: default_language(),
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
    load_from_path(Some(path))
}

pub fn save(app: &AppHandle, prefs: &Prefs) {
    if let Some(path) = prefs_path(app) {
        if let Ok(json) = serde_json::to_string_pretty(prefs) {
            let _ = std::fs::write(path, json);
        }
    }
}

/// Loads prefs from an explicit path (useful for initializing managed state
/// before the AppHandle is available in setup()).
pub fn load_from_path(path: Option<PathBuf>) -> Prefs {
    let Some(p) = path else {
        return Prefs::default();
    };
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Returns the platform-standard path for prefs.json without requiring an
/// AppHandle. Mirrors Tauri's logic: config_dir + app identifier.
///
/// macOS:   ~/Library/Application Support/com.manelrv.cctv/prefs.json
/// Linux:   ~/.config/com.manelrv.cctv/prefs.json
/// Windows: %APPDATA%\com.manelrv.cctv\prefs.json
pub fn default_prefs_path() -> Option<PathBuf> {
    // The identifier comes from tauri.conf.json -> "identifier".
    let identifier = "com.manelrv.cctv";
    let dir = dirs::config_dir()?.join(identifier);
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("prefs.json"))
}
