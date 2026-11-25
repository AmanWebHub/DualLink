use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(crate) fn unique_profile_name(profiles: &[Profile]) -> String {
    let mut number = profiles.len() + 1;
    loop {
        let candidate = format!("My profile {number}");
        let lowered = candidate.to_lowercase();
        if !profiles.iter().any(|profile| profile.name.to_lowercase() == lowered) {
            return candidate;
        }
        number += 1;
    }
}

/// Which profile should be selected after `deleted` is removed (`new_len` is the length afterwards).
pub(crate) fn selection_after_delete(selected: usize, deleted: usize, new_len: usize) -> usize {
    if deleted < selected {
        selected - 1
    } else if deleted == selected {
        deleted.min(new_len.saturating_sub(1))
    } else {
        selected
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Profile {
    pub(crate) name: String,
    pub(crate) bindings: Vec<String>,
}

pub(crate) fn load_profiles() -> Result<Option<Vec<Profile>>, String> {
    let path = profiles_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| error.to_string())
}

pub(crate) fn profiles_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    base.map(|path| path.join("Padflow").join("profiles.json"))
        .ok_or_else(|| "Could not find a local settings folder".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_profile_names_never_collide() {
        let profile = |name: &str| Profile { name: name.into(), bindings: Vec::new() };
        let profiles = vec![profile("Everyday"), profile("my profile 3")];
        assert_eq!(unique_profile_name(&profiles), "My profile 4");
        assert_eq!(unique_profile_name(&[]), "My profile 1");
    }

    #[test]
    fn selection_follows_the_right_profile_after_a_delete() {
        assert_eq!(selection_after_delete(2, 0, 3), 1);
        assert_eq!(selection_after_delete(0, 2, 3), 0);
        assert_eq!(selection_after_delete(1, 1, 3), 1);
        assert_eq!(selection_after_delete(2, 2, 2), 1);
    }
}
