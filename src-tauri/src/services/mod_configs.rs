use crate::models::ModConfigFile;
use crate::services::security::{
    ensure_no_symlink_escape, is_windows_reserved_name, write_file_atomic,
};
use serde_json::Value;
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_CONFIG_DEPTH: usize = 8;
const MAX_CONFIG_FILES: usize = 500;
const MAX_SINGLE_CONFIG_BYTES: usize = 2 * 1024 * 1024;

pub fn mod_config_dir(data_path: &Path) -> PathBuf {
    data_path.join("ModConfig")
}

fn resolve_safe_config_path(data_path: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let trimmed = rel_path.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return Err("Config path cannot be empty".to_string());
    }
    let rel = Path::new(trimmed);
    if rel.is_absolute() {
        return Err("Config path must be relative".to_string());
    }
    let mut has_normal = false;
    for comp in rel.components() {
        match comp {
            Component::Normal(part) => {
                has_normal = true;
                let s = part
                    .to_str()
                    .ok_or_else(|| "Invalid UTF-8 in config path".to_string())?;
                if is_windows_reserved_name(s) {
                    return Err("Config path uses a reserved system name".to_string());
                }
            }
            _ => return Err("Invalid config path".to_string()),
        }
    }
    if !has_normal {
        return Err("Invalid config path".to_string());
    }
    let is_json = rel
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("json"))
        .unwrap_or(false);
    if !is_json {
        return Err("Config file must have a .json extension".to_string());
    }
    Ok(mod_config_dir(data_path).join(rel))
}

pub fn list_mod_configs(data_path: &Path) -> Result<Vec<ModConfigFile>, String> {
    let root = mod_config_dir(data_path);
    if !root.is_dir() {
        return Ok(vec![]);
    }

    let mut out = Vec::new();
    collect_json_files(&root, &root, 0, &mut out)?;
    out.sort_by(|a, b| a.rel_path.to_lowercase().cmp(&b.rel_path.to_lowercase()));
    Ok(out)
}

fn collect_json_files(
    root: &Path,
    dir: &Path,
    depth: usize,
    out: &mut Vec<ModConfigFile>,
) -> Result<(), String> {
    if depth > MAX_CONFIG_DEPTH || out.len() >= MAX_CONFIG_FILES {
        return Ok(());
    }

    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read ModConfig directory {}: {e}", dir.display()))?;

    for entry in entries.flatten() {
        if out.len() >= MAX_CONFIG_FILES {
            break;
        }
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_symlink() {
            continue;
        }

        let path = entry.path();
        if ft.is_dir() {
            collect_json_files(root, &path, depth + 1, out)?;
            continue;
        }
        if !ft.is_file() {
            continue;
        }

        let is_json = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        if !is_json {
            continue;
        }

        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if size_bytes > MAX_SINGLE_CONFIG_BYTES as u64 {
            continue;
        }

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&rel_path)
            .to_string();
        let content = fs::read_to_string(&path).unwrap_or_default();

        out.push(ModConfigFile {
            rel_path,
            filename,
            size_bytes,
            content,
        });
    }
    Ok(())
}

pub fn save_mod_config(
    data_path: &Path,
    rel_path: &str,
    raw_content: &str,
) -> Result<ModConfigFile, String> {
    if raw_content.len() > MAX_SINGLE_CONFIG_BYTES {
        return Err(format!(
            "Config file exceeds maximum allowed size ({MAX_SINGLE_CONFIG_BYTES} bytes)"
        ));
    }

    let root = mod_config_dir(data_path);
    fs::create_dir_all(&root)
        .map_err(|e| format!("Failed to create ModConfig directory: {e}"))?;

    let target = resolve_safe_config_path(data_path, rel_path)?;
    ensure_no_symlink_escape(&root, &target)?;

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {e}"))?;
        ensure_no_symlink_escape(&root, &target)?;
    }

    let parsed: Value = serde_json::from_str(raw_content)
        .map_err(|e| format!("Invalid JSON syntax: {e}"))?;
    let formatted = serde_json::to_string_pretty(&parsed)
        .map_err(|e| format!("Failed to format JSON: {e}"))?;

    // Keep a one-time .bak of the original file before modifying
    if target.exists() {
        let bak_path = target.with_extension("json.bak");
        if !bak_path.exists() {
            if let Ok(existing_bytes) = fs::read(&target) {
                let _ = write_file_atomic(&bak_path, &existing_bytes);
            }
        }
    }

    write_file_atomic(&target, formatted.as_bytes())?;

    let filename = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(rel_path)
        .to_string();
    let size_bytes = formatted.len() as u64;

    Ok(ModConfigFile {
        rel_path: rel_path.replace('\\', "/"),
        filename,
        size_bytes,
        content: formatted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_list_and_save_mod_config() {
        let dir = tempdir().unwrap();
        let cfg_dir = mod_config_dir(dir.path());
        fs::create_dir_all(&cfg_dir).unwrap();
        fs::write(cfg_dir.join("RealSmoke.json"), r#"{"Enabled":true,"Max":10}"#).unwrap();

        let list = list_mod_configs(dir.path()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].filename, "RealSmoke.json");

        // Reject invalid JSON
        assert!(save_mod_config(dir.path(), "RealSmoke.json", "{bad json").is_err());

        // Reject path traversal & non-json extensions & reserved names
        assert!(save_mod_config(dir.path(), "../escape.json", "{}").is_err());
        assert!(save_mod_config(dir.path(), "bad.exe", "{}").is_err());
        assert!(save_mod_config(dir.path(), "CON.json", "{}").is_err());

        // Valid save creates backup and formats JSON
        let saved =
            save_mod_config(dir.path(), "RealSmoke.json", r#"{"Enabled":false,"Max":20}"#).unwrap();
        assert!(saved.content.contains("\"Enabled\": false"));
        assert!(cfg_dir.join("RealSmoke.json.bak").exists());
    }
}
