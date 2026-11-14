use crate::models::{LocalMod, ModInfo};
use crate::services::moddb::ModDbClient;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use semver::Version;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

pub fn mods_dir(data_path: &Path) -> PathBuf {
    data_path.join("Mods")
}

pub fn disabled_mods_dir(data_path: &Path) -> PathBuf {
    data_path.join("ModsDisabled")
}

pub fn scan_local_mods(data_path: &Path) -> Result<Vec<LocalMod>, String> {
    let mut mods = Vec::new();
    let enabled_dir = mods_dir(data_path);
    let disabled_dir = disabled_mods_dir(data_path);

    if enabled_dir.is_dir() {
        scan_directory(&enabled_dir, true, &mut mods)?;
    }
    if disabled_dir.is_dir() {
        scan_directory(&disabled_dir, false, &mut mods)?;
    }

    mods.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(mods)
}

fn scan_directory(dir: &Path, enabled: bool, out: &mut Vec<LocalMod>) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {}: {e}", dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_zip = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("zip"))
            .unwrap_or(false);

        if !is_zip {
            continue;
        }

        let filename = match path.file_name().and_then(|s| s.to_str()) {
            Some(f) => f.to_string(),
            None => continue,
        };

        out.push(read_mod_zip(&path, filename, enabled));
    }

    Ok(())
}

pub fn read_mod_zip(path: &Path, filename: String, enabled: bool) -> LocalMod {
    let fallback_name = filename.trim_end_matches(".zip").to_string();
    let fallback_modid = fallback_name
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>();

    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => {
            return LocalMod {
                filename,
                modid: fallback_modid,
                name: fallback_name,
                version: "0.0.0".to_string(),
                description: String::new(),
                authors: vec![],
                side: "Universal".to_string(),
                enabled,
                icon_base64: None,
                dependencies: HashMap::new(),
                latest_version: None,
                latest_download_url: None,
                latest_filename: None,
                update_available: false,
            };
        }
    };

    let mut archive = match ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => {
            return LocalMod {
                filename,
                modid: fallback_modid,
                name: fallback_name,
                version: "0.0.0".to_string(),
                description: "Invalid zip archive".to_string(),
                authors: vec![],
                side: "Universal".to_string(),
                enabled,
                icon_base64: None,
                dependencies: HashMap::new(),
                latest_version: None,
                latest_download_url: None,
                latest_filename: None,
                update_available: false,
            };
        }
    };

    let mut mod_info = ModInfo::default();
    let mut icon_base64: Option<String> = None;

    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let entry_name = entry.name().to_string();
        let lower = entry_name.to_lowercase();

        if lower == "modinfo.json" || lower.ends_with("/modinfo.json") {
            let mut raw = String::new();
            if entry.read_to_string(&mut raw).is_ok() {
                let cleaned = raw.trim_start_matches('\u{feff}');
                if let Ok(parsed) = serde_json::from_str::<ModInfo>(cleaned) {
                    mod_info = parsed;
                }
            }
        } else if (lower == "modicon.png" || lower.ends_with("/modicon.png"))
            && icon_base64.is_none()
        {
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                icon_base64 = Some(format!("data:image/png;base64,{}", BASE64.encode(buf)));
            }
        }
    }

    let name = mod_info
        .name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| fallback_name.clone());

    let modid = mod_info
        .modid
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            name.to_lowercase()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect()
        });

    LocalMod {
        filename,
        modid,
        name,
        version: mod_info.version.unwrap_or_else(|| "0.0.0".to_string()),
        description: mod_info.description.unwrap_or_default(),
        authors: mod_info.authors.unwrap_or_default(),
        side: mod_info.side.unwrap_or_else(|| "Universal".to_string()),
        enabled,
        icon_base64,
        dependencies: mod_info.dependencies.unwrap_or_default(),
        latest_version: None,
        latest_download_url: None,
        latest_filename: None,
        update_available: false,
    }
}

pub fn toggle_mod(data_path: &Path, filename: &str, enable: bool) -> Result<(), String> {
    let enabled_dir = mods_dir(data_path);
    let disabled_dir = disabled_mods_dir(data_path);

    let (src_dir, dst_dir) = if enable {
        (&disabled_dir, &enabled_dir)
    } else {
        (&enabled_dir, &disabled_dir)
    };

    let src = src_dir.join(filename);
    if !src.exists() {
        // Already in the target directory?
        if dst_dir.join(filename).exists() {
            return Ok(());
        }
        return Err(format!("Mod file not found: {}", src.display()));
    }

    fs::create_dir_all(dst_dir)
        .map_err(|e| format!("Failed to create directory {}: {e}", dst_dir.display()))?;

    let dst = dst_dir.join(filename);
    move_file_safely(&src, &dst)
}

pub fn move_file_safely(src: &Path, dst: &Path) -> Result<(), String> {
    if let Err(err) = fs::rename(src, dst) {
        // Fallback copy + delete if rename fails across volumes
        fs::copy(src, dst)
            .map_err(|e| format!("Failed to move mod ({err}); copy failed: {e}"))?;
        fs::remove_file(src)
            .map_err(|e| format!("Copied mod but failed to remove original: {e}"))?;
    }
    Ok(())
}

pub fn delete_mod(data_path: &Path, filename: &str, enabled: bool) -> Result<(), String> {
    let dir = if enabled {
        mods_dir(data_path)
    } else {
        disabled_mods_dir(data_path)
    };
    let target = dir.join(filename);
    if target.exists() {
        fs::remove_file(&target)
            .map_err(|e| format!("Failed to delete {}: {e}", target.display()))?;
    }
    Ok(())
}

pub fn is_newer_version(installed: &str, latest: &str) -> bool {
    let clean_inst = installed.trim().trim_start_matches('v');
    let clean_lat = latest.trim().trim_start_matches('v');

    if let (Ok(v_inst), Ok(v_lat)) = (Version::parse(clean_inst), Version::parse(clean_lat)) {
        return v_lat > v_inst;
    }

    // Fallback numeric dot-segment comparison (e.g., "1.2" vs "1.2.1")
    let parse_nums = |s: &str| -> Vec<u64> {
        s.split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.parse::<u64>().ok())
            .collect()
    };

    let a = parse_nums(clean_inst);
    let b = parse_nums(clean_lat);
    if !a.is_empty() && !b.is_empty() {
        return b > a;
    }

    clean_lat != clean_inst
}

pub async fn check_updates_for_mods(
    client: &ModDbClient,
    mut mods: Vec<LocalMod>,
) -> Vec<LocalMod> {
    for m in &mut mods {
        if m.modid.is_empty() {
            continue;
        }
        if let Ok(detail) = client.get_mod_detail(&m.modid).await {
            if let Some(latest_rel) = detail.releases.first() {
                if let Some(ver) = &latest_rel.modversion {
                    m.latest_version = Some(ver.clone());
                    m.latest_download_url = latest_rel.mainfile.clone();
                    m.latest_filename = latest_rel.filename.clone();
                    m.update_available = is_newer_version(&m.version, ver);
                }
            }
        }
    }
    mods
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    fn create_test_mod_zip(path: &Path, modinfo_json: &str) {
        let file = fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("modinfo.json", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(modinfo_json.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn test_read_mod_zip_and_toggle() {
        let dir = tempdir().unwrap();
        let mods_path = mods_dir(dir.path());
        fs::create_dir_all(&mods_path).unwrap();

        let zip_path = mods_path.join("carrycapacity_1.0.0.zip");
        create_test_mod_zip(
            &zip_path,
            r#"{
                "type": "code",
                "modid": "carrycapacity",
                "name": "Carry On",
                "version": "1.0.0",
                "authors": ["copygirl"],
                "description": "Pick up chests"
            }"#,
        );

        let scanned = scan_local_mods(dir.path()).unwrap();
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].modid, "carrycapacity");
        assert_eq!(scanned[0].name, "Carry On");
        assert!(scanned[0].enabled);

        // Disable -> moves to ModsDisabled
        toggle_mod(dir.path(), "carrycapacity_1.0.0.zip", false).unwrap();
        assert!(!zip_path.exists());
        assert!(disabled_mods_dir(dir.path())
            .join("carrycapacity_1.0.0.zip")
            .exists());

        let scanned_disabled = scan_local_mods(dir.path()).unwrap();
        assert_eq!(scanned_disabled.len(), 1);
        assert!(!scanned_disabled[0].enabled);

        // Re-enable -> moves back to Mods
        toggle_mod(dir.path(), "carrycapacity_1.0.0.zip", true).unwrap();
        assert!(zip_path.exists());
    }

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("1.0.0", "1.0.1"));
        assert!(is_newer_version("v1.2.0", "1.3.0"));
        assert!(!is_newer_version("2.0.0", "1.9.9"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(is_newer_version("1.2", "1.2.1"));
    }
}
