use crate::models::{ModProfile, ProfileSettings};
use crate::services::local_mods::{scan_local_mods, toggle_mod};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

pub fn load_profiles(config_dir: &Path, data_path: Option<&Path>) -> Result<Vec<ModProfile>, String> {
    let file_path = config_dir.join("profiles.json");
    if !file_path.exists() {
        let mut initial_enabled: Vec<String> = Vec::new();
        if let Some(dp) = data_path {
            if let Ok(mods) = scan_local_mods(dp) {
                initial_enabled = mods
                    .into_iter()
                    .filter(|m| m.enabled)
                    .map(|m| m.modid)
                    .collect();
            }
        }

        let defaults = vec![
            ModProfile {
                id: "default".to_string(),
                name: "Default".to_string(),
                is_builtin: true,
                enabled_modids: initial_enabled,
                created_at: "Built-in".to_string(),
                settings: ProfileSettings::default(),
            },
            ModProfile {
                id: "vanilla".to_string(),
                name: "Vanilla".to_string(),
                is_builtin: true,
                enabled_modids: vec![],
                created_at: "Built-in".to_string(),
                settings: ProfileSettings::default(),
            },
        ];
        save_profiles(config_dir, &defaults)?;
        return Ok(defaults);
    }

    let raw = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read profiles.json: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("Failed to parse profiles.json: {e}"))
}

pub fn save_profiles(config_dir: &Path, profiles: &[ModProfile]) -> Result<(), String> {
    fs::create_dir_all(config_dir)
        .map_err(|e| format!("Failed to create config directory: {e}"))?;
    let file_path = config_dir.join("profiles.json");
    let json = serde_json::to_string_pretty(profiles)
        .map_err(|e| format!("Failed to serialize profiles: {e}"))?;
    fs::write(&file_path, json).map_err(|e| format!("Failed to write profiles.json: {e}"))
}

pub fn create_profile(
    config_dir: &Path,
    data_path: Option<&Path>,
    name: String,
    clone_current: bool,
) -> Result<ModProfile, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Profile name cannot be empty".to_string());
    }

    let mut profiles = load_profiles(config_dir, data_path)?;
    let slug: String = trimmed
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let mut id = slug.trim_matches('-').to_string();
    if id.is_empty() {
        id = "profile".to_string();
    }
    let base_id = id.clone();
    let mut counter = 2;
    while profiles.iter().any(|p| p.id == id) {
        id = format!("{base_id}-{counter}");
        counter += 1;
    }

    let enabled_modids = if clone_current {
        if let Some(dp) = data_path {
            scan_local_mods(dp)?
                .into_iter()
                .filter(|m| m.enabled)
                .map(|m| m.modid)
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let new_profile = ModProfile {
        id,
        name: trimmed.to_string(),
        is_builtin: false,
        enabled_modids,
        created_at: "Custom".to_string(),
        settings: ProfileSettings::default(),
    };

    profiles.push(new_profile.clone());
    save_profiles(config_dir, &profiles)?;
    Ok(new_profile)
}

pub fn rename_profile(
    config_dir: &Path,
    data_path: Option<&Path>,
    profile_id: &str,
    new_name: String,
) -> Result<Vec<ModProfile>, String> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err("Profile name cannot be empty".to_string());
    }
    let mut profiles = load_profiles(config_dir, data_path)?;
    let target = profiles
        .iter_mut()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("Profile '{profile_id}' not found"))?;
    target.name = trimmed.to_string();
    save_profiles(config_dir, &profiles)?;
    Ok(profiles)
}

pub fn delete_profile(
    config_dir: &Path,
    data_path: Option<&Path>,
    profile_id: &str,
) -> Result<Vec<ModProfile>, String> {
    let mut profiles = load_profiles(config_dir, data_path)?;
    if let Some(p) = profiles.iter().find(|p| p.id == profile_id) {
        if p.is_builtin {
            return Err("Built-in profiles cannot be deleted".to_string());
        }
    }
    profiles.retain(|p| p.id != profile_id);
    save_profiles(config_dir, &profiles)?;
    Ok(profiles)
}

pub fn switch_profile(
    config_dir: &Path,
    data_path: &Path,
    profile_id: &str,
) -> Result<ModProfile, String> {
    let profiles = load_profiles(config_dir, Some(data_path))?;
    let target = profiles
        .into_iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("Profile '{profile_id}' not found"))?;

    let enabled_set: HashSet<String> = target
        .enabled_modids
        .iter()
        .map(|s| s.to_lowercase())
        .collect();

    let local_mods = scan_local_mods(data_path)?;
    for m in local_mods {
        let should_enable = enabled_set.contains(&m.modid.to_lowercase());
        if m.enabled != should_enable {
            toggle_mod(data_path, &m.filename, should_enable)?;
        }
    }

    Ok(target)
}

pub fn sync_mod_in_active_profile(
    config_dir: &Path,
    data_path: &Path,
    active_profile_id: Option<&str>,
    modid: &str,
    enabled: bool,
) -> Result<(), String> {
    let Some(pid) = active_profile_id else {
        return Ok(());
    };
    if pid == "vanilla" {
        return Ok(());
    }
    let mut profiles = load_profiles(config_dir, Some(data_path))?;
    if let Some(profile) = profiles.iter_mut().find(|p| p.id == pid) {
        let lower = modid.to_lowercase();
        profile.enabled_modids.retain(|id| id.to_lowercase() != lower);
        if enabled {
            profile.enabled_modids.push(modid.to_string());
        }
        save_profiles(config_dir, &profiles)?;
    }
    Ok(())
}

pub fn update_profile_settings(
    config_dir: &Path,
    data_path: Option<&Path>,
    profile_id: &str,
    settings: ProfileSettings,
) -> Result<Vec<ModProfile>, String> {
    let mut profiles = load_profiles(config_dir, data_path)?;
    let target = profiles
        .iter_mut()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("Profile '{profile_id}' not found"))?;
    target.settings = settings;
    save_profiles(config_dir, &profiles)?;
    Ok(profiles)
}

pub fn snapshot_profile_versions(
    config_dir: &Path,
    data_path: &Path,
    profile_id: &str,
) -> Result<Vec<ModProfile>, String> {
    let mut profiles = load_profiles(config_dir, Some(data_path))?;
    let local_mods = scan_local_mods(data_path)?;
    let target = profiles
        .iter_mut()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("Profile '{profile_id}' not found"))?;

    let enabled_set: HashSet<String> = target
        .enabled_modids
        .iter()
        .map(|s| s.to_lowercase())
        .collect();

    let mut pinned = HashMap::new();
    for m in local_mods {
        if enabled_set.contains(&m.modid.to_lowercase()) {
            pinned.insert(m.modid, m.version);
        }
    }
    target.settings.pinned_mod_versions = pinned;
    save_profiles(config_dir, &profiles)?;
    Ok(profiles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::local_mods::mods_dir;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    fn write_dummy_mod(dir: &Path, filename: &str, modid: &str) {
        let file = fs::File::create(dir.join(filename)).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("modinfo.json", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(format!(r#"{{"modid":"{modid}","name":"{modid}","version":"1.0.0"}}"#).as_bytes())
            .unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn test_profile_switching_moves_mods() {
        let cfg = tempdir().unwrap();
        let data = tempdir().unwrap();
        let mods_path = mods_dir(data.path());
        fs::create_dir_all(&mods_path).unwrap();

        write_dummy_mod(&mods_path, "a.zip", "moda");
        write_dummy_mod(&mods_path, "b.zip", "modb");

        let profiles = load_profiles(cfg.path(), Some(data.path())).unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].enabled_modids.len(), 2);

        // Switch to Vanilla -> both mods move to ModsDisabled
        switch_profile(cfg.path(), data.path(), "vanilla").unwrap();
        let after_vanilla = scan_local_mods(data.path()).unwrap();
        assert!(after_vanilla.iter().all(|m| !m.enabled));

        // Switch back to Default -> both mods move back to Mods
        switch_profile(cfg.path(), data.path(), "default").unwrap();
        let after_default = scan_local_mods(data.path()).unwrap();
        assert!(after_default.iter().all(|m| m.enabled));
    }
}
