use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::profiles::profiles_path;

#[derive(Serialize, Deserialize)]
pub(crate) struct Settings {
    /// Turn Xbox output on by itself whenever a controller is connected.
    #[serde(default = "default_true")]
    pub(crate) auto_enable: bool,
}

pub(crate) fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self { auto_enable: true }
    }
}

pub(crate) fn settings_path() -> Result<PathBuf, String> {
    profiles_path().map(|path| path.with_file_name("settings.json"))
}

pub(crate) fn load_settings() -> Settings {
    settings_path()
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub(crate) fn save_settings(settings: &Settings) -> Result<(), String> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    std::fs::write(path, json).map_err(|error| error.to_string())
}

/// Whether Xbox output should start by itself right now.
pub(crate) fn should_auto_enable(
    auto_enable: bool,
    controller_connected: bool,
    already_attempted: bool,
    turned_off_by_user: bool,
    output_active: bool,
) -> bool {
    auto_enable && controller_connected && !already_attempted && !turned_off_by_user && !output_active
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xbox_output_starts_automatically_only_when_it_should() {
        // normal case: setting on, controller connected, nothing tried yet
        assert!(should_auto_enable(true, true, false, false, false));
        // setting off, no controller, already tried, switched off by the user, already on
        assert!(!should_auto_enable(false, true, false, false, false));
        assert!(!should_auto_enable(true, false, false, false, false));
        assert!(!should_auto_enable(true, true, true, false, false));
        assert!(!should_auto_enable(true, true, false, true, false));
        assert!(!should_auto_enable(true, true, false, false, true));
    }
}
