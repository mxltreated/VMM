use crate::models::{AppSettings, AppUpdateInfo, DetectedPaths};
use crate::services::security::{
    build_hardened_http_client, checked_redirect_policy, ct_eq_hex,
    is_allowed_github_download_host, is_zip_symlink, read_bounded, validate_github_release_url,
    validate_https_url, write_file_atomic,
};
use futures_util::StreamExt;
use rand::RngCore;
use semver::Version;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use url::Url;

pub const OFFICIAL_GITHUB_REPO: &str = "mxltreated/VMM";
const MAX_RELEASE_JSON_BYTES: usize = 2 * 1024 * 1024;
const MAX_CHECKSUMS_BYTES: usize = 64 * 1024;
const MAX_UPDATE_BYTES: usize = 150 * 1024 * 1024;

#[cfg(windows)]
const UPDATE_HELPER_PS1: &str = r#"$ErrorActionPreference = 'Stop'
$parentPid = [int]$env:VMM_PARENT_PID
$mode = $env:VMM_INSTALL_MODE
$staged = $env:VMM_STAGED_PATH
$target = $env:VMM_TARGET_EXE
$installDir = $env:VMM_INSTALL_DIR
$stageDir = $env:VMM_STAGE_DIR

Wait-Process -Id $parentPid -Timeout 15 -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 300

if ($mode -eq 'zip') {
    try {
        Copy-Item -LiteralPath $staged -Destination $target -Force
    } catch {
        $sq = $staged.Replace("'", "''")
        $tq = $target.Replace("'", "''")
        $cmd = "Copy-Item -LiteralPath '$sq' -Destination '$tq' -Force"
        $encoded = [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($cmd))
        Start-Process -FilePath 'powershell.exe' -Verb RunAs -WindowStyle Hidden -Wait -ArgumentList @('-NoProfile', '-EncodedCommand', $encoded)
    }
    if (Test-Path -LiteralPath $target) {
        Start-Process -FilePath $target
    }
} elseif ($mode -eq 'msi') {
    Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i', $staged, '/qn', '/norestart') -Wait -PassThru | Out-Null
    if (Test-Path -LiteralPath $target) {
        Start-Process -FilePath $target
    }
} else {
    $exeArgs = if ([string]::IsNullOrEmpty($installDir)) { @('/S') } else { @('/S', "/D=$installDir") }
    Start-Process -FilePath $staged -ArgumentList $exeArgs -Wait -PassThru | Out-Null
    if (Test-Path -LiteralPath $target) {
        Start-Process -FilePath $target
    }
}

if (-not [string]::IsNullOrEmpty($stageDir) -and (Test-Path -LiteralPath $stageDir)) {
    Remove-Item -LiteralPath $stageDir -Recurse -Force -ErrorAction SilentlyContinue
}
"#;

fn is_allowed_github_api_host(url: &Url) -> bool {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    url.host_str()
        .map(|h| h.trim_end_matches('.').eq_ignore_ascii_case("api.github.com"))
        .unwrap_or(false)
}

pub fn validate_github_repo(repo: &str) -> Result<String, String> {
    let trimmed = repo.trim().trim_matches('/');
    let mut parts = trimmed.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err("Enter GitHub repository as 'owner/repo'".to_string());
    };
    let valid_seg = |s: &str| -> bool {
        !s.is_empty()
            && s != "."
            && s != ".."
            && s.len() <= 100
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    if !valid_seg(owner) || !valid_seg(name) {
        return Err("Invalid GitHub repository identifier".to_string());
    }
    Ok(format!("{owner}/{name}"))
}

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
            candidates.push(
                local
                    .join("Programs")
                    .join("Vintagestory")
                    .join("Vintagestory.exe"),
            );
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
            github_repo: Some(OFFICIAL_GITHUB_REPO.to_string()),
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
        settings.github_repo = Some(OFFICIAL_GITHUB_REPO.to_string());
    }

    Ok(settings)
}

pub fn save_settings(config_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    let file_path = config_dir.join("settings.json");
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    write_file_atomic(&file_path, json.as_bytes())
}

#[derive(Debug, Clone)]
struct VerifiedReleaseAsset {
    asset_name: String,
    download_url: String,
    api_sha256: Option<String>,
    checksums_url: Option<String>,
    latest_version: String,
    update_available: bool,
    release_notes: Option<String>,
    html_url: String,
}

fn parse_api_sha256_digest(asset: &Value) -> Option<String> {
    let raw = asset.get("digest").and_then(|v| v.as_str())?;
    let hex_part = raw.trim().strip_prefix("sha256:")?;
    if hex_part.len() == 64 && hex_part.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(hex_part.to_ascii_lowercase())
    } else {
        None
    }
}

pub fn parse_sha256_from_sums(sums_text: &str, asset_name: &str) -> Option<String> {
    let normalize_name = |s: &str| {
        s.trim()
            .trim_start_matches('*')
            .trim_start_matches("./")
            .replace(' ', ".")
            .to_ascii_lowercase()
    };
    let target = normalize_name(asset_name);

    for line in sums_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(ws_idx) = trimmed.find(char::is_whitespace) else {
            continue;
        };
        let digest = &trimmed[..ws_idx];
        let rest = trimmed[ws_idx..].trim_start();
        if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        if normalize_name(rest) == target {
            return Some(digest.to_ascii_lowercase());
        }
    }
    None
}

async fn fetch_verified_release_asset(repo: &str) -> Result<VerifiedReleaseAsset, String> {
    let validated_repo = validate_github_repo(repo)?;
    let url = format!("https://api.github.com/repos/{validated_repo}/releases/latest");
    let client = build_hardened_http_client(
        concat!("VintageStoryModManager/", env!("CARGO_PKG_VERSION")),
        20,
        checked_redirect_policy(5, is_allowed_github_api_host),
    )?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("GitHub request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API returned HTTP {}", resp.status()));
    }

    if let Some(len) = resp.content_length() {
        if len as usize > MAX_RELEASE_JSON_BYTES {
            return Err("GitHub release metadata exceeds size limit".to_string());
        }
    }

    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();
    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| format!("Failed to read GitHub release response: {e}"))?;
        if buf.len().saturating_add(chunk.len()) > MAX_RELEASE_JSON_BYTES {
            return Err("GitHub release metadata exceeds size limit".to_string());
        }
        buf.extend_from_slice(&chunk);
    }

    let json: Value = serde_json::from_slice(&buf)
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

    let expected_html_prefix = format!("https://github.com/{}/", validated_repo.to_ascii_lowercase());
    let fallback_html_url = format!("https://github.com/{validated_repo}/releases");
    let html_url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .and_then(|raw| validate_https_url(raw).ok())
        .map(|u| u.to_string())
        .filter(|s| s.to_ascii_lowercase().starts_with(&expected_html_prefix))
        .unwrap_or(fallback_html_url);

    let release_notes = json
        .get("body")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut preferred_zip: Option<(String, String, Option<String>)> = None;
    let mut fallback_zip: Option<(String, String, Option<String>)> = None;
    let mut exe_asset: Option<(String, String, Option<String>)> = None;
    let mut msi_asset: Option<(String, String, Option<String>)> = None;
    let mut checksums_url: Option<String> = None;

    if let Some(assets) = json.get("assets").and_then(|v| v.as_array()) {
        for asset in assets {
            let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let Some(raw_dl) = asset.get("browser_download_url").and_then(|v| v.as_str()) else {
                continue;
            };
            let Ok(valid_dl) = validate_github_release_url(raw_dl, &validated_repo) else {
                continue;
            };
            let dl_str = valid_dl.to_string();
            let lower_name = name.to_ascii_lowercase();

            if lower_name == "sha256sums.txt" {
                checksums_url = Some(dl_str);
                continue;
            }

            let api_sha256 = parse_api_sha256_digest(asset);
            if lower_name == "vmm-windows-x64.zip" && preferred_zip.is_none() {
                preferred_zip = Some((name.to_string(), dl_str, api_sha256));
            } else if lower_name.ends_with(".zip") && fallback_zip.is_none() {
                fallback_zip = Some((name.to_string(), dl_str, api_sha256));
            } else if lower_name.ends_with(".exe") && exe_asset.is_none() {
                exe_asset = Some((name.to_string(), dl_str, api_sha256));
            } else if lower_name.ends_with(".msi") && msi_asset.is_none() {
                msi_asset = Some((name.to_string(), dl_str, api_sha256));
            }
        }
    }

    let chosen = preferred_zip
        .or(fallback_zip)
        .or(exe_asset)
        .or(msi_asset);

    let (asset_name, download_url, api_sha256) = match chosen {
        Some((n, u, d)) => (n, u, d),
        None => (String::new(), String::new(), None),
    };

    Ok(VerifiedReleaseAsset {
        asset_name,
        download_url,
        api_sha256,
        checksums_url,
        latest_version: tag,
        update_available,
        release_notes,
        html_url,
    })
}

pub async fn check_github_app_update(repo: &str) -> Result<AppUpdateInfo, String> {
    let release = fetch_verified_release_asset(repo).await?;
    Ok(AppUpdateInfo {
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        latest_version: release.latest_version,
        update_available: release.update_available,
        release_notes: release.release_notes,
        html_url: release.html_url,
        download_url: if release.download_url.is_empty() {
            None
        } else {
            Some(release.download_url)
        },
    })
}

async fn fetch_checksum_from_sums_asset(sums_url: &str, asset_name: &str) -> Result<Option<String>, String> {
    validate_github_release_url(sums_url, OFFICIAL_GITHUB_REPO)?;
    let client = build_hardened_http_client(
        concat!("VintageStoryModManager/", env!("CARGO_PKG_VERSION")),
        30,
        checked_redirect_policy(5, is_allowed_github_download_host),
    )?;
    let resp = client
        .get(sums_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download SHA256SUMS.txt: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("SHA256SUMS.txt download failed with HTTP {}", resp.status()));
    }
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_CHECKSUMS_BYTES {
            return Err("SHA256SUMS.txt exceeds size limit".to_string());
        }
    }
    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();
    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| format!("Failed reading SHA256SUMS.txt: {e}"))?;
        if buf.len().saturating_add(chunk.len()) > MAX_CHECKSUMS_BYTES {
            return Err("SHA256SUMS.txt exceeds size limit".to_string());
        }
        buf.extend_from_slice(&chunk);
    }
    let text = String::from_utf8(buf)
        .map_err(|_| "SHA256SUMS.txt is not valid UTF-8".to_string())?;
    Ok(parse_sha256_from_sums(&text, asset_name))
}

pub async fn download_and_launch_installer(
    staging_root: &Path,
    expected_version: Option<&str>,
) -> Result<(), String> {
    let release = fetch_verified_release_asset(OFFICIAL_GITHUB_REPO).await?;

    if let Some(exp) = expected_version {
        let clean_exp = exp.trim().trim_start_matches('v');
        if !clean_exp.is_empty() && clean_exp != release.latest_version {
            return Err(format!(
                "Update version mismatch (expected v{clean_exp}, found v{})",
                release.latest_version
            ));
        }
    }

    if release.download_url.is_empty() || release.asset_name.is_empty() {
        return Err("Latest release does not contain a supported Windows update asset".to_string());
    }

    let sums_sha256 = match release.checksums_url.as_deref() {
        Some(sums_url) => fetch_checksum_from_sums_asset(sums_url, &release.asset_name).await?,
        None => None,
    };

    let expected_sha256 = match (&release.api_sha256, &sums_sha256) {
        (Some(a), Some(b)) => {
            if !ct_eq_hex(a, b) {
                return Err("Release SHA-256 metadata mismatch between GitHub API and SHA256SUMS.txt".to_string());
            }
            a.clone()
        }
        (Some(a), None) => a.clone(),
        (None, Some(b)) => b.clone(),
        (None, None) => {
            return Err("Release is missing SHA-256 checksum metadata; refusing unverified update".to_string());
        }
    };

    let lower_asset = release.asset_name.to_ascii_lowercase();
    let (mode, safe_asset_file) = if lower_asset.ends_with(".zip") {
        ("zip", "update.zip")
    } else if lower_asset.ends_with(".msi") {
        ("msi", "update-setup.msi")
    } else if lower_asset.ends_with(".exe") {
        ("exe", "update-setup.exe")
    } else {
        return Err("Unsupported update asset type".to_string());
    };

    let mut rand_bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut rand_bytes);
    let stage_dir = staging_root.join(format!(
        "vmm-update-{}-{}",
        std::process::id(),
        hex::encode(rand_bytes)
    ));
    fs::create_dir_all(&stage_dir)
        .map_err(|e| format!("Failed to create staging directory: {e}"))?;

    let result: Result<(), String> = async {
        let dest = stage_dir.join(safe_asset_file);
        let client = build_hardened_http_client(
            concat!("VintageStoryModManager/", env!("CARGO_PKG_VERSION")),
            180,
            checked_redirect_policy(5, is_allowed_github_download_host),
        )?;

        let resp = client
            .get(&release.download_url)
            .send()
            .await
            .map_err(|e| format!("Failed to download update: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Update download failed with HTTP {}", resp.status()));
        }

        if let Some(len) = resp.content_length() {
            if len as usize > MAX_UPDATE_BYTES {
                return Err("Update archive exceeds maximum allowed size".to_string());
            }
        }

        let mut file = fs::File::create(&dest)
            .map_err(|e| format!("Failed to create staged update file: {e}"))?;
        let mut hasher = Sha256::new();
        let mut received: usize = 0;
        let mut stream = resp.bytes_stream();

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res.map_err(|e| format!("Failed reading update stream: {e}"))?;
            received = received.saturating_add(chunk.len());
            if received > MAX_UPDATE_BYTES {
                return Err("Update download exceeded maximum allowed size".to_string());
            }
            hasher.update(&chunk);
            file.write_all(&chunk)
                .map_err(|e| format!("Failed writing staged update: {e}"))?;
        }

        file.sync_all()
            .map_err(|e| format!("Failed syncing staged update: {e}"))?;
        drop(file);

        let computed_sha256 = hex::encode(hasher.finalize());
        if !ct_eq_hex(&computed_sha256, &expected_sha256) {
            return Err("Update SHA-256 verification failed; aborting installation".to_string());
        }

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;

            let pid = std::process::id();
            let current_exe = std::env::current_exe()
                .map_err(|e| format!("Failed to locate current executable: {e}"))?;

            let staged_path = if mode == "zip" {
                let staged_exe = stage_dir.join("VMM-update.exe");
                {
                    let file = fs::File::open(&dest)
                        .map_err(|e| format!("Failed to open update archive: {e}"))?;
                    let mut archive = zip::ZipArchive::new(file)
                        .map_err(|e| format!("Failed to read update archive: {e}"))?;
                    let mut extracted = false;
                    for i in 0..archive.len() {
                        let mut entry = archive
                            .by_index(i)
                            .map_err(|e| format!("Failed to read archive entry: {e}"))?;
                        if entry.is_dir() || is_zip_symlink(entry.unix_mode()) {
                            continue;
                        }
                        if entry.name().to_ascii_lowercase().ends_with(".exe") {
                            let bytes = read_bounded(
                                &mut entry,
                                MAX_UPDATE_BYTES,
                                "Update executable",
                            )?;
                            if bytes.len() < 2 || &bytes[..2] != b"MZ" {
                                return Err("Extracted update binary is not a valid Windows executable".to_string());
                            }
                            write_file_atomic(&staged_exe, &bytes)?;
                            extracted = true;
                            break;
                        }
                    }
                    if !extracted {
                        return Err("Update archive did not contain an executable".to_string());
                    }
                }
                let _ = fs::remove_file(&dest);
                staged_exe
            } else {
                dest
            };

            let install_dir = if mode == "exe" {
                current_exe
                    .parent()
                    .filter(|dir| {
                        let s = dir.to_string_lossy().to_ascii_lowercase();
                        !s.ends_with(r"\target\debug") && !s.ends_with(r"\target\release")
                    })
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default()
            } else {
                String::new()
            };

            let script_path = stage_dir.join("apply-update.ps1");
            write_file_atomic(&script_path, UPDATE_HELPER_PS1.as_bytes())?;

            std::process::Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-WindowStyle",
                    "Hidden",
                    "-File",
                    &script_path.to_string_lossy(),
                ])
                .env("VMM_PARENT_PID", pid.to_string())
                .env("VMM_INSTALL_MODE", mode)
                .env("VMM_STAGED_PATH", staged_path.to_string_lossy().as_ref())
                .env("VMM_TARGET_EXE", current_exe.to_string_lossy().as_ref())
                .env("VMM_INSTALL_DIR", install_dir)
                .env("VMM_STAGE_DIR", stage_dir.to_string_lossy().as_ref())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|e| format!("Failed to launch update helper: {e}"))?;
        }

        #[cfg(not(windows))]
        {
            let _ = (mode, &stage_dir);
        }

        Ok(())
    }
    .await;

    if result.is_err() {
        let _ = fs::remove_dir_all(&stage_dir);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_github_repo_strictly() {
        assert_eq!(
            validate_github_repo("mxltreated/VMM").unwrap(),
            "mxltreated/VMM"
        );
        assert!(validate_github_repo("mxltreated/VMM?x=1").is_err());
        assert!(validate_github_repo("mxltreated/../other").is_err());
        assert!(validate_github_repo("onlyone").is_err());
        assert!(validate_github_repo("a/b/c").is_err());
    }

    #[test]
    fn parses_sha256sums_correctly() {
        let hash_zip = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let hash_exe = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
        let sums = format!(
            "# comment\n{hash_zip}  VMM-windows-x64.zip\n{hash_exe} *Vintage Mod Manager_1.2.0_x64-setup.exe\n"
        );
        assert_eq!(
            parse_sha256_from_sums(&sums, "VMM-windows-x64.zip").as_deref(),
            Some(hash_zip)
        );
        assert_eq!(
            parse_sha256_from_sums(&sums, "Vintage.Mod.Manager_1.2.0_x64-setup.exe").as_deref(),
            Some(hash_exe)
        );
        assert_eq!(parse_sha256_from_sums(&sums, "missing.zip"), None);
    }
}
