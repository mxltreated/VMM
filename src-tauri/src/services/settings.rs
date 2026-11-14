use crate::models::{AppSettings, AppUpdateInfo, DetectedPaths};
use reqwest::Client;
use semver::Version;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub fn detect_vintage_story_paths() -> DetectedPaths {
    let mut detected = DetectedPaths::default();

    if let Some(roaming) = dirs::data_dir() {
        let data_candidate = roaming.join("VintagestoryData");
        if data_candidate.is_dir() {
            detected.data_path = Some(data_candidate.to_string_lossy().to_string());
        }

        let exe_roaming = roaming.join("Vintagestory").join("Vintagestory.exe");
        if exe_roaming.is_file() {
            detected.game_exe_path = Some(exe_roaming.to_string_lossy().to_string());
        }
    }

    if detected.game_exe_path.is_none() {
        let mut candidates: Vec<PathBuf> = vec![
            PathBuf::from(r"C:\Program Files\Vintagestory\Vintagestory.exe"),
            PathBuf::from(r"C:\Program Files (x86)\Vintagestory\Vintagestory.exe"),
        ];
        if let Some(local) = dirs::data_local_dir() {
            candidates.push(local.join("Vintagestory").join("Vintagestory.exe"));
            candidates.push(local.join("Programs").join("Vintagestory").join("Vintagestory.exe"));
        }

        for candidate in candidates {
            if candidate.is_file() {
                detected.game_exe_path = Some(candidate.to_string_lossy().to_string());
                break;
            }
        }
    }

    detected
}

pub fn load_settings(config_dir: &Path) -> Result<AppSettings, String> {
    let file_path = config_dir.join("settings.json");
    if !file_path.exists() {
        let detected = detect_vintage_story_paths();
        let defaults = AppSettings {
            game_exe_path: detected.game_exe_path,
            data_path: detected.data_path,
            active_profile_id: Some("default".to_string()),
            preferred_game_version: None,
            onboarding_completed: false,
            launch_args: None,
            auto_check_updates: false,
            github_repo: Some("mxltreated/VMM".to_string()),
        };
        save_settings(config_dir, &defaults)?;
        return Ok(defaults);
    }

    let raw = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read settings.json: {e}"))?;
    let mut settings: AppSettings = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse settings.json: {e}"))?;

    if settings.data_path.is_none() || settings.game_exe_path.is_none() {
        let detected = detect_vintage_story_paths();
        if settings.data_path.is_none() {
            settings.data_path = detected.data_path;
        }
        if settings.game_exe_path.is_none() {
            settings.game_exe_path = detected.game_exe_path;
        }
    }
    if settings.github_repo.is_none() {
        settings.github_repo = Some("mxltreated/VMM".to_string());
    }

    Ok(settings)
}

pub fn save_settings(config_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    fs::create_dir_all(config_dir)
        .map_err(|e| format!("Failed to create config directory: {e}"))?;
    let file_path = config_dir.join("settings.json");
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    fs::write(&file_path, json).map_err(|e| format!("Failed to write settings.json: {e}"))
}

pub async fn check_github_app_update(repo: &str) -> Result<AppUpdateInfo, String> {
    let trimmed = repo.trim().trim_matches('/');
    if !trimmed.contains('/') || trimmed.split('/').count() != 2 {
        return Err("Enter GitHub repository as 'owner/repo'".to_string());
    }

    let url = format!("https://api.github.com/repos/{trimmed}/releases/latest");
    let client = Client::builder()
        .user_agent("VintageStoryModManager/1.1.0")
        .build()
        .unwrap_or_else(|_| Client::new());

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("GitHub request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API returned HTTP {}", resp.status()));
    }

    let json: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub release JSON: {e}"))?;

    let tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .trim_start_matches('v')
        .to_string();

    let current = env!("CARGO_PKG_VERSION").to_string();
    let update_available = match (Version::parse(&tag), Version::parse(&current)) {
        (Ok(lat), Ok(cur)) => lat > cur,
        _ => tag != current,
    };

    let html_url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let release_notes = json
        .get("body")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut download_url = None;
    if let Some(assets) = json.get("assets").and_then(|v| v.as_array()) {
        for asset in assets {
            let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.ends_with(".exe") || name.ends_with(".msi") {
                download_url = asset
                    .get("browser_download_url")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                break;
            }
        }
    }

    Ok(AppUpdateInfo {
        current_version: current,
        latest_version: tag,
        update_available,
        release_notes,
        html_url,
        download_url,
    })
}

pub async fn download_and_launch_installer(url: &str) -> Result<(), String> {
    let trimmed = url.trim();
    if !trimmed.starts_with("https://") {
        return Err("Installer URL must use HTTPS".to_string());
    }

    let filename = trimmed
        .rsplit('/')
        .next()
        .filter(|s| s.ends_with(".exe") || s.ends_with(".msi"))
        .unwrap_or("VMM-Setup.exe");

    let dest = std::env::temp_dir().join(filename);
    let client = Client::builder()
        .user_agent("VintageStoryModManager/1.1.0")
        .build()
        .unwrap_or_else(|_| Client::new());

    let resp = client
        .get(trimmed)
        .send()
        .await
        .map_err(|e| format!("Failed to download installer: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Installer download failed with HTTP {}", resp.status()));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read installer bytes: {e}"))?;

    fs::write(&dest, &bytes)
        .map_err(|e| format!("Failed to save installer to {}: {e}", dest.display()))?;

    #[cfg(windows)]
    {
        if filename.ends_with(".msi") {
            std::process::Command::new("msiexec")
                .args(["/i", &dest.to_string_lossy()])
                .spawn()
                .map_err(|e| format!("Failed to launch MSI installer: {e}"))?;
        } else {
            std::process::Command::new(&dest)
                .spawn()
                .map_err(|e| format!("Failed to launch installer: {e}"))?;
        }
    }

    Ok(())
}
