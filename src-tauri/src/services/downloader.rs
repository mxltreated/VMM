use crate::models::DownloadProgressEvent;
use crate::services::local_mods::{disabled_mods_dir, mods_dir};
use crate::services::security::{
    build_hardened_http_client, checked_redirect_policy, ensure_no_symlink_escape,
    is_allowed_moddb_host, validate_mod_filename, validate_moddb_download_url,
};
use futures_util::StreamExt;
use rand::RngCore;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use tauri::{AppHandle, Emitter};
use zip::ZipArchive;

const MAX_MOD_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;

pub async fn download_mod_file(
    app: &AppHandle,
    modid: &str,
    url: &str,
    filename: &str,
    data_path: &Path,
    replace_old_filename: Option<&str>,
) -> Result<String, String> {
    let validated_url = validate_moddb_download_url(url)?;
    let safe_filename = validate_mod_filename(filename)?;
    let validated_old = match replace_old_filename {
        Some(old) => Some(validate_mod_filename(old)?),
        None => None,
    };

    let target_dir = mods_dir(data_path);
    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create Mods directory: {e}"))?;

    let final_path = target_dir.join(&safe_filename);
    ensure_no_symlink_escape(&target_dir, &final_path)?;

    let mut rand_bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut rand_bytes);
    let tmp_path = target_dir.join(format!(
        ".{safe_filename}.{}.{}.part",
        std::process::id(),
        hex::encode(rand_bytes)
    ));

    let client = build_hardened_http_client(
        concat!("VintageStoryModManager/", env!("CARGO_PKG_VERSION")),
        180,
        checked_redirect_policy(5, is_allowed_moddb_host),
    )?;

    let resp = client
        .get(validated_url)
        .send()
        .await
        .map_err(|e| format!("Download request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed with HTTP {}", resp.status()));
    }

    let total_bytes = resp.content_length();
    if let Some(len) = total_bytes {
        if len > MAX_MOD_DOWNLOAD_BYTES {
            return Err(format!(
                "Mod archive exceeds maximum allowed size ({MAX_MOD_DOWNLOAD_BYTES} bytes)"
            ));
        }
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp_path)
        .map_err(|e| format!("Failed to create temp file: {e}"))?;

    let mut stream = resp.bytes_stream();
    let mut received: u64 = 0;

    while let Some(chunk_res) = stream.next().await {
        let chunk = match chunk_res {
            Ok(c) => c,
            Err(e) => {
                drop(file);
                let _ = fs::remove_file(&tmp_path);
                return Err(format!("Error reading download stream: {e}"));
            }
        };
        received = received.saturating_add(chunk.len() as u64);
        if received > MAX_MOD_DOWNLOAD_BYTES {
            drop(file);
            let _ = fs::remove_file(&tmp_path);
            return Err(format!(
                "Mod archive exceeds maximum allowed size ({MAX_MOD_DOWNLOAD_BYTES} bytes)"
            ));
        }
        if let Err(e) = file.write_all(&chunk) {
            drop(file);
            let _ = fs::remove_file(&tmp_path);
            return Err(format!("Failed to write to temp file: {e}"));
        }

        let _ = app.emit(
            "download-progress",
            DownloadProgressEvent {
                modid: modid.to_string(),
                filename: safe_filename.clone(),
                bytes_received: received,
                total_bytes,
                done: false,
                error: None,
            },
        );
    }

    if let Err(e) = file.sync_all() {
        drop(file);
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Failed to flush downloaded file: {e}"));
    }
    drop(file);

    // Verify archive integrity before replacing any installed mod file
    {
        let check_file = fs::File::open(&tmp_path).map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            format!("Failed to open downloaded mod archive: {e}")
        })?;
        if let Err(e) = ZipArchive::new(check_file) {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!("Downloaded file is not a valid zip archive: {e}"));
        }
    }

    if let Err(e) = fs::rename(&tmp_path, &final_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Failed to finalize mod file: {e}"));
    }

    if let Some(old_name) = validated_old {
        if old_name != safe_filename {
            let old_enabled = target_dir.join(&old_name);
            if ensure_no_symlink_escape(&target_dir, &old_enabled).is_ok() && old_enabled.exists() {
                let _ = fs::remove_file(old_enabled);
            }
            let disabled_dir = disabled_mods_dir(data_path);
            if disabled_dir.exists() {
                let old_disabled = disabled_dir.join(&old_name);
                if ensure_no_symlink_escape(&disabled_dir, &old_disabled).is_ok()
                    && old_disabled.exists()
                {
                    let _ = fs::remove_file(old_disabled);
                }
            }
        }
    }

    let _ = app.emit(
        "download-progress",
        DownloadProgressEvent {
            modid: modid.to_string(),
            filename: safe_filename.clone(),
            bytes_received: received,
            total_bytes: total_bytes.or(Some(received)),
            done: true,
            error: None,
        },
    );

    Ok(safe_filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_validation_rules() {
        assert!(validate_moddb_download_url("https://moddbcdn.vintagestory.at/file.zip").is_ok());
        assert!(validate_moddb_download_url("http://moddbcdn.vintagestory.at/file.zip").is_err());
        assert!(validate_moddb_download_url("https://127.0.0.1/file.zip").is_err());
        assert!(validate_mod_filename("../mod.zip").is_err());
        assert!(validate_mod_filename("valid_mod.zip").is_ok());
    }
}
