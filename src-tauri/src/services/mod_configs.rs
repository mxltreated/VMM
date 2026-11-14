use crate::models::ModConfigFile;
use serde_json::Value;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub fn mod_config_dir(data_path: &Path) -> PathBuf {
    data_path.join("ModConfig")
}

fn resolve_safe_config_path(data_path: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let rel = Path::new(rel_path);
    if rel.is_absolute() {
        return Err("Config path must be relative".to_string());
    }
    for comp in rel.components() {
        match comp {
            Component::Normal(_) => {}
            _ => return Err("Invalid config path".to_string()),
        }
    }
    Ok(mod_config_dir(data_path).join(rel))
}

pub fn list_mod_configs(data_path: &Path) -> Result<Vec<ModConfigFile>, String> {
    let root = mod_config_dir(data_path);
    if !root.is_dir() {
        return Ok(vec![]);
    }

    let mut out = Vec::new();
    collect_json_files(&root, &root, &mut out)?;
    out.sort_by(|a, b| a.rel_path.to_lowercase().cmp(&b.rel_path.to_lowercase()));
    Ok(out)
}

fn collect_json_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<ModConfigFile>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read ModConfig directory {}: {e}", dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(root, &path, out)?;
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
        let size_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
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
    let target = resolve_safe_config_path(data_path, rel_path)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {e}"))?;
    }

    let parsed: Value = serde_json::from_str(raw_content)
        .map_err(|e| format!("Invalid JSON syntax: {e}"))?;
    let formatted = serde_json::to_string_pretty(&parsed)
        .map_err(|e| format!("Failed to format JSON: {e}"))?;

    // Keep a one-time .bak of the original file before modifying
    if target.exists() {
        let bak_path = target.with_extension("json.bak");
        if !bak_path.exists() {
            let _ = fs::copy(&target, &bak_path);
        }
    }

    fs::write(&target, &formatted)
        .map_err(|e| format!("Failed to write {}: {e}", target.display()))?;

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

        // Reject path traversal
        assert!(save_mod_config(dir.path(), "../escape.json", "{}").is_err());

        // Valid save creates backup and formats JSON
        let saved = save_mod_config(dir.path(), "RealSmoke.json", r#"{"Enabled":false,"Max":20}"#).unwrap();
        assert!(saved.content.contains("\"Enabled\": false"));
        assert!(cfg_dir.join("RealSmoke.json.bak").exists());
    }
}
