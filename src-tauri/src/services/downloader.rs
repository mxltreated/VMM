use crate::models::DownloadProgressEvent;
use crate::services::local_mods::{disabled_mods_dir, mods_dir};
use futures_util::StreamExt;
use reqwest::Client;
use std::fs;
use std::io::Write;
use std::path::Path;
use tauri::{AppHandle, Emitter};

pub async fn download_mod_file(
    app: &AppHandle,
    modid: &str,
    url: &str,
    filename: &str,
    data_path: &Path,
    replace_old_filename: Option<&str>,
) -> Result<String, String> {
    let target_dir = mods_dir(data_path);
    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create Mods directory: {e}"))?;

    let safe_filename = Path::new(filename)
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Invalid filename".to_string())?
        .to_string();

    let tmp_path = target_dir.join(format!("{safe_filename}.part"));
    let final_path = target_dir.join(&safe_filename);

    let client = Client::builder()
        .user_agent("VintageStoryModManager/0.1.0")
        .build()
        .unwrap_or_else(|_| Client::new());

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed with HTTP {}", resp.status()));
    }

    let total_bytes = resp.content_length();
    let mut file = fs::File::create(&tmp_path)
        .map_err(|e| format!("Failed to create temp file: {e}"))?;

    let mut stream = resp.bytes_stream();
    let mut received: u64 = 0;

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            format!("Error reading download stream: {e}")
        })?;
        file.write_all(&chunk).map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            format!("Failed to write to temp file: {e}")
        })?;
        received += chunk.len() as u64;

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

    file.flush()
        .map_err(|e| format!("Failed to flush downloaded file: {e}"))?;
    drop(file);

    if final_path.exists() {
        let _ = fs::remove_file(&final_path);
    }
    fs::rename(&tmp_path, &final_path)
        .map_err(|e| format!("Failed to finalize mod file: {e}"))?;

    if let Some(old_name) = replace_old_filename {
        if old_name != safe_filename {
            let old_enabled = target_dir.join(old_name);
            let old_disabled = disabled_mods_dir(data_path).join(old_name);
            if old_enabled.exists() {
                let _ = fs::remove_file(old_enabled);
            }
            if old_disabled.exists() {
                let _ = fs::remove_file(old_disabled);
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
