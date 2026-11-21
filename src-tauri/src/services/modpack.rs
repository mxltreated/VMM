use crate::models::{
    ModDbDetail, ModDbRelease, ModpackImportItem, ModpackImportPreview, ModpackImportResult,
    ModpackManifest, ModpackModEntry,
};
use crate::services::downloader::download_mod_file;
use crate::services::local_mods::scan_local_mods;
use crate::services::mod_configs::{list_mod_configs, mod_config_dir};
use crate::services::moddb::ModDbClient;
use crate::services::profiles::{create_profile, load_profiles, save_profiles, switch_profile};
use crate::services::security::{
    build_hardened_http_client, checked_redirect_policy, ct_eq_hex, ensure_no_symlink_escape,
    is_private_or_loopback_host, is_windows_reserved_name, is_zip_symlink, read_bounded,
    validate_https_url, write_file_atomic,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use futures_util::StreamExt;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use tauri::AppHandle;
use url::Url;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

const SHARE_CODE_PREFIX: &str = "vmm1:";
const MAX_SHARE_CODE_CHARS: usize = 512 * 1024;
const MAX_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_MODS_IN_MANIFEST: usize = 500;
const MAX_MODPACK_ZIP_HASH_BYTES: usize = 25 * 1024 * 1024;
const MAX_CONFIG_FILES: usize = 250;
const MAX_SINGLE_CONFIG_BYTES: usize = 2 * 1024 * 1024;
const MAX_TOTAL_CONFIG_BYTES: usize = 20 * 1024 * 1024;

fn is_allowed_modpack_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && !is_private_or_loopback_host(url)
}

pub fn build_manifest_from_profile(
    config_dir: &Path,
    data_path: &Path,
    profile_id: &str,
    pack_name: Option<String>,
    author: Option<String>,
    description: Option<String>,
    include_configs: bool,
) -> Result<ModpackManifest, String> {
    let profiles = load_profiles(config_dir, Some(data_path))?;
    let profile = profiles
        .into_iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("Profile '{profile_id}' not found"))?;

    let local_mods = scan_local_mods(data_path)?;
    let local_by_id: HashMap<String, _> = local_mods
        .into_iter()
        .map(|m| (m.modid.to_lowercase(), m))
        .collect();

    let mut mods = Vec::new();
    for modid in &profile.enabled_modids {
        let lower = modid.to_lowercase();
        let pinned_ver = profile.settings.pinned_mod_versions.get(&lower).cloned();
        if let Some(local) = local_by_id.get(&lower) {
            mods.push(ModpackModEntry {
                modid: local.modid.clone(),
                name: local.name.clone(),
                version: pinned_ver.clone().or_else(|| Some(local.version.clone())),
                pinned: pinned_ver.is_some(),
                side: Some(local.side.clone()),
            });
        } else {
            mods.push(ModpackModEntry {
                modid: modid.clone(),
                name: modid.clone(),
                version: pinned_ver.clone(),
                pinned: pinned_ver.is_some(),
                side: None,
            });
        }
    }
    mods.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let included_configs = if include_configs {
        list_mod_configs(data_path)?
            .into_iter()
            .map(|c| c.rel_path)
            .collect()
    } else {
        vec![]
    };

    let name = pack_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(profile.name);

    Ok(ModpackManifest {
        format_version: 1,
        name,
        author: author.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        description: description
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        game_version: profile.settings.game_version.clone(),
        created_at: chrono_like_timestamp(),
        mods,
        included_configs,
    })
}

fn chrono_like_timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

pub fn encode_share_code(manifest: &ModpackManifest) -> Result<String, String> {
    let json = serde_json::to_vec(manifest)
        .map_err(|e| format!("Failed to serialize modpack manifest: {e}"))?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(&json)
        .map_err(|e| format!("Failed to compress modpack manifest: {e}"))?;
    let compressed = encoder
        .finish()
        .map_err(|e| format!("Failed to finish compression: {e}"))?;
    let encoded = URL_SAFE_NO_PAD.encode(compressed);
    Ok(format!("{SHARE_CODE_PREFIX}{encoded}"))
}

pub fn decode_share_code(code: &str) -> Result<ModpackManifest, String> {
    let trimmed = code.trim();
    if trimmed.len() > MAX_SHARE_CODE_CHARS {
        return Err("Share code exceeds maximum allowed length".to_string());
    }
    let payload = trimmed
        .strip_prefix(SHARE_CODE_PREFIX)
        .ok_or_else(|| "Invalid share code: expected 'vmm1:' prefix".to_string())?;

    let compressed = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| format!("Invalid share code encoding: {e}"))?;

    let mut decoder = DeflateDecoder::new(&compressed[..]);
    let decompressed = read_bounded(&mut decoder, MAX_MANIFEST_BYTES, "Share code manifest")?;

    let manifest: ModpackManifest = serde_json::from_slice(&decompressed)
        .map_err(|e| format!("Failed to parse modpack manifest: {e}"))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &ModpackManifest) -> Result<(), String> {
    if manifest.format_version != 1 {
        return Err(format!(
            "Unsupported modpack format version: {}",
            manifest.format_version
        ));
    }
    if manifest.name.trim().is_empty() {
        return Err("Modpack manifest is missing a name".to_string());
    }
    if manifest.mods.len() > MAX_MODS_IN_MANIFEST {
        return Err(format!(
            "Modpack exceeds maximum allowed mod count ({MAX_MODS_IN_MANIFEST})"
        ));
    }
    Ok(())
}

fn compute_manifest_hash(manifest: &ModpackManifest, extra_zip_bytes: Option<&[u8]>) -> Result<String, String> {
    let canonical = serde_json::to_vec(manifest)
        .map_err(|e| format!("Failed to serialize manifest for hashing: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    if let Some(zip_bytes) = extra_zip_bytes {
        hasher.update([0x00]);
        hasher.update(zip_bytes);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn export_modpack_file(
    config_dir: &Path,
    data_path: &Path,
    profile_id: &str,
    dest_path: &Path,
    pack_name: Option<String>,
    author: Option<String>,
    description: Option<String>,
    include_configs: bool,
) -> Result<ModpackManifest, String> {
    let manifest = build_manifest_from_profile(
        config_dir,
        data_path,
        profile_id,
        pack_name,
        author,
        description,
        include_configs,
    )?;

    let is_zip = dest_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
        || include_configs;

    if is_zip {
        let file = File::create(dest_path)
            .map_err(|e| format!("Failed to create modpack archive: {e}"))?;
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("Failed to serialize manifest: {e}"))?;
        zip.start_file("manifest.json", options)
            .map_err(|e| format!("Failed to write manifest to zip: {e}"))?;
        zip.write_all(manifest_json.as_bytes())
            .map_err(|e| format!("Failed to write manifest bytes: {e}"))?;

        if include_configs {
            let cfg_root = mod_config_dir(data_path);
            for rel in &manifest.included_configs {
                let full_path = cfg_root.join(rel);
                if ensure_no_symlink_escape(&cfg_root, &full_path).is_ok() && full_path.is_file() {
                    if let Ok(bytes) = fs::read(&full_path) {
                        let archive_path = format!("ModConfig/{}", rel.replace('\\', "/"));
                        if zip.start_file(&archive_path, options).is_ok() {
                            let _ = zip.write_all(&bytes);
                        }
                    }
                }
            }
        }

        zip.finish()
            .map_err(|e| format!("Failed to finalize modpack zip: {e}"))?;
    } else {
        let json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("Failed to serialize manifest: {e}"))?;
        write_file_atomic(dest_path, json.as_bytes())?;
    }

    Ok(manifest)
}

pub async fn load_manifest_from_source(source: &str) -> Result<(ModpackManifest, bool, String), String> {
    let trimmed = source.trim();
    if trimmed.starts_with(SHARE_CODE_PREFIX) {
        let manifest = decode_share_code(trimmed)?;
        let hash = compute_manifest_hash(&manifest, None)?;
        return Ok((manifest, false, hash));
    }

    if trimmed.starts_with("http://") {
        return Err("Remote modpack URLs must use https://".to_string());
    }

    if trimmed.starts_with("https://") {
        let parsed_url = validate_https_url(trimmed)?;
        if is_private_or_loopback_host(&parsed_url) {
            return Err("Remote modpack URLs cannot target local or private network hosts".to_string());
        }
        let client = build_hardened_http_client(
            concat!("VintageStoryModManager/", env!("CARGO_PKG_VERSION")),
            30,
            checked_redirect_policy(5, is_allowed_modpack_url),
        )?;
        let resp = client
            .get(parsed_url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch remote modpack manifest: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("Remote manifest returned HTTP {}", resp.status()));
        }
        if let Some(len) = resp.content_length() {
            if len as usize > MAX_MANIFEST_BYTES {
                return Err("Remote modpack manifest exceeds size limit".to_string());
            }
        }
        let mut stream = resp.bytes_stream();
        let mut buf = Vec::new();
        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res
                .map_err(|e| format!("Failed to read remote manifest body: {e}"))?;
            if buf.len().saturating_add(chunk.len()) > MAX_MANIFEST_BYTES {
                return Err("Remote modpack manifest exceeds size limit".to_string());
            }
            buf.extend_from_slice(&chunk);
        }
        let manifest: ModpackManifest = serde_json::from_slice(&buf)
            .map_err(|e| format!("Failed to parse remote manifest JSON: {e}"))?;
        validate_manifest(&manifest)?;
        let hash = compute_manifest_hash(&manifest, None)?;
        return Ok((manifest, false, hash));
    }

    let path = Path::new(trimmed);
    if !path.is_file() {
        return Err("Provide a valid .vmmpack file, https:// URL, or vmm1: share code".to_string());
    }

    let is_zip = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);

    if is_zip {
        let mut zip_bytes_file =
            File::open(path).map_err(|e| format!("Failed to open zip: {e}"))?;
        let zip_bytes = read_bounded(
            &mut zip_bytes_file,
            MAX_MODPACK_ZIP_HASH_BYTES,
            "Modpack zip archive",
        )?;

        let mut archive = ZipArchive::new(std::io::Cursor::new(&zip_bytes))
            .map_err(|e| format!("Invalid modpack zip archive: {e}"))?;
        let has_configs = (0..archive.len()).any(|i| {
            archive
                .by_index(i)
                .map(|f| f.name().starts_with("ModConfig/") && !f.is_dir())
                .unwrap_or(false)
        });
        let manifest_bytes = {
            let mut manifest_file = archive
                .by_name("manifest.json")
                .map_err(|_| "Zip archive is missing manifest.json".to_string())?;
            if is_zip_symlink(manifest_file.unix_mode()) {
                return Err("manifest.json cannot be a symlink".to_string());
            }
            read_bounded(&mut manifest_file, MAX_MANIFEST_BYTES, "manifest.json")?
        };
        let manifest: ModpackManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|e| format!("Failed to parse manifest.json: {e}"))?;
        validate_manifest(&manifest)?;
        let hash = compute_manifest_hash(&manifest, Some(&zip_bytes))?;
        Ok((manifest, has_configs, hash))
    } else {
        let mut file = File::open(path)
            .map_err(|e| format!("Failed to open manifest file: {e}"))?;
        let raw_bytes = read_bounded(&mut file, MAX_MANIFEST_BYTES, "Modpack manifest file")?;
        let manifest: ModpackManifest = serde_json::from_slice(&raw_bytes)
            .map_err(|e| format!("Failed to parse manifest file: {e}"))?;
        validate_manifest(&manifest)?;
        let hash = compute_manifest_hash(&manifest, None)?;
        Ok((manifest, false, hash))
    }
}

pub(crate) fn select_release<'a>(
    detail: &'a ModDbDetail,
    requested_version: Option<&str>,
    pinned: bool,
    game_version: Option<&str>,
) -> Option<&'a ModDbRelease> {
    if detail.releases.is_empty() {
        return None;
    }

    if pinned {
        if let Some(req) = requested_version {
            if let Some(exact) = detail
                .releases
                .iter()
                .find(|r| r.modversion.as_deref() == Some(req) && r.mainfile.is_some())
            {
                return Some(exact);
            }
        }
    }

    if let Some(gv) = game_version {
        if let Some(compat) = detail.releases.iter().find(|r| {
            r.mainfile.is_some()
                && r.tags
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case(gv) || t.contains(gv))
        }) {
            return Some(compat);
        }
    }

    detail.releases.iter().find(|r| r.mainfile.is_some())
}

pub async fn preview_modpack_import(
    source: &str,
    moddb: &ModDbClient,
    data_path: &Path,
) -> Result<ModpackImportPreview, String> {
    let (manifest, has_configs, manifest_hash) = load_manifest_from_source(source).await?;
    let local_mods = scan_local_mods(data_path)?;
    let local_by_id: HashMap<String, _> = local_mods
        .into_iter()
        .map(|m| (m.modid.to_lowercase(), m))
        .collect();

    let mut items = Vec::with_capacity(manifest.mods.len());
    for entry in &manifest.mods {
        let lower = entry.modid.to_lowercase();
        if let Some(local) = local_by_id.get(&lower) {
            let version_matches = !entry.pinned
                || entry.version.as_deref() == Some(local.version.as_str());
            if version_matches {
                items.push(ModpackImportItem {
                    modid: entry.modid.clone(),
                    name: entry.name.clone(),
                    requested_version: entry.version.clone(),
                    resolved_version: Some(local.version.clone()),
                    download_url: None,
                    filename: Some(local.filename.clone()),
                    status: "installed".to_string(),
                });
                continue;
            }
        }

        match moddb.get_mod_detail(&entry.modid).await {
            Ok(detail) => {
                if let Some(rel) = select_release(
                    &detail,
                    entry.version.as_deref(),
                    entry.pinned,
                    manifest.game_version.as_deref(),
                ) {
                    items.push(ModpackImportItem {
                        modid: entry.modid.clone(),
                        name: detail.name.clone(),
                        requested_version: entry.version.clone(),
                        resolved_version: rel.modversion.clone(),
                        download_url: rel.mainfile.clone(),
                        filename: rel.filename.clone(),
                        status: "downloadable".to_string(),
                    });
                } else {
                    items.push(ModpackImportItem {
                        modid: entry.modid.clone(),
                        name: entry.name.clone(),
                        requested_version: entry.version.clone(),
                        resolved_version: None,
                        download_url: None,
                        filename: None,
                        status: "missing".to_string(),
                    });
                }
            }
            Err(_) => {
                items.push(ModpackImportItem {
                    modid: entry.modid.clone(),
                    name: entry.name.clone(),
                    requested_version: entry.version.clone(),
                    resolved_version: None,
                    download_url: None,
                    filename: None,
                    status: "missing".to_string(),
                });
            }
        }
    }

    Ok(ModpackImportPreview {
        manifest,
        items,
        has_configs,
        manifest_hash,
    })
}

pub async fn apply_modpack_import(
    app: &AppHandle,
    config_dir: &Path,
    data_path: &Path,
    moddb: &ModDbClient,
    source: &str,
    profile_name: Option<String>,
    restore_configs: bool,
    expected_manifest_hash: Option<&str>,
) -> Result<ModpackImportResult, String> {
    let preview = preview_modpack_import(source, moddb, data_path).await?;
    if let Some(expected) = expected_manifest_hash {
        if !ct_eq_hex(&preview.manifest_hash, expected) {
            return Err("Modpack content changed since preview. Please inspect again.".to_string());
        }
    }

    let mut downloaded_count = 0usize;
    let mut skipped_missing = Vec::new();
    let mut enabled_modids = Vec::new();
    let mut pinned_versions = HashMap::new();

    for (idx, item) in preview.items.iter().enumerate() {
        let entry = &preview.manifest.mods[idx];
        match item.status.as_str() {
            "installed" => {
                enabled_modids.push(item.modid.clone());
                if entry.pinned {
                    if let Some(ver) = &item.resolved_version {
                        pinned_versions.insert(item.modid.to_lowercase(), ver.clone());
                    }
                }
            }
            "downloadable" => {
                if let (Some(url), Some(filename)) = (&item.download_url, &item.filename) {
                    download_mod_file(app, &item.modid, url, filename, data_path, None).await?;
                    downloaded_count += 1;
                    enabled_modids.push(item.modid.clone());
                    if entry.pinned {
                        if let Some(ver) = &item.resolved_version {
                            pinned_versions.insert(item.modid.to_lowercase(), ver.clone());
                        }
                    }
                } else {
                    skipped_missing.push(item.name.clone());
                }
            }
            _ => {
                skipped_missing.push(item.name.clone());
            }
        }
    }

    let restored_configs = if restore_configs && preview.has_configs {
        extract_configs_from_zip(Path::new(source.trim()), data_path)?
    } else {
        0
    };

    let target_name = profile_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| preview.manifest.name.clone());

    let new_profile = create_profile(config_dir, Some(data_path), target_name, false)?;
    let mut profiles = load_profiles(config_dir, Some(data_path))?;
    let mut deduped = HashSet::new();
    let unique_enabled: Vec<String> = enabled_modids
        .into_iter()
        .filter(|id| deduped.insert(id.to_lowercase()))
        .collect();

    if let Some(p) = profiles.iter_mut().find(|p| p.id == new_profile.id) {
        p.enabled_modids = unique_enabled;
        p.settings.game_version = preview.manifest.game_version.clone();
        p.settings.notes = preview.manifest.description.clone();
        p.settings.pinned_mod_versions = pinned_versions;
        let trimmed_src = source.trim();
        if trimmed_src.starts_with("https://") {
            p.settings.source_url = Some(trimmed_src.to_string());
        }
    }
    save_profiles(config_dir, &profiles)?;
    let switched = switch_profile(config_dir, data_path, &new_profile.id)?;

    Ok(ModpackImportResult {
        profile: switched,
        downloaded_count,
        skipped_missing,
        restored_configs,
    })
}

fn extract_configs_from_zip(zip_path: &Path, data_path: &Path) -> Result<usize, String> {
    let file = File::open(zip_path).map_err(|e| format!("Failed to open zip: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Invalid zip: {e}"))?;
    let cfg_dir = mod_config_dir(data_path);
    fs::create_dir_all(&cfg_dir)
        .map_err(|e| format!("Failed to create ModConfig directory: {e}"))?;

    let mut count = 0usize;
    let mut total_bytes = 0usize;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {e}"))?;
        if entry.is_dir() || is_zip_symlink(entry.unix_mode()) {
            continue;
        }

        let Some(rel_str) = entry.name().strip_prefix("ModConfig/").map(str::to_string) else {
            continue;
        };
        if rel_str.is_empty() || rel_str.contains('\0') {
            continue;
        }

        let rel_path = PathBuf::from(&rel_str);
        if rel_path.is_absolute() {
            continue;
        }
        let mut valid_components = true;
        let mut has_normal = false;
        for comp in rel_path.components() {
            match comp {
                Component::Normal(part) => {
                    has_normal = true;
                    if part.to_str().map(is_windows_reserved_name).unwrap_or(true) {
                        valid_components = false;
                        break;
                    }
                }
                _ => {
                    valid_components = false;
                    break;
                }
            }
        }
        if !valid_components || !has_normal {
            continue;
        }

        let is_json = rel_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        if !is_json {
            continue;
        }

        if count >= MAX_CONFIG_FILES {
            return Err(format!(
                "Modpack exceeds maximum allowed bundled config files ({MAX_CONFIG_FILES})"
            ));
        }

        let raw_bytes = read_bounded(&mut entry, MAX_SINGLE_CONFIG_BYTES, "Bundled config")?;
        total_bytes = total_bytes.saturating_add(raw_bytes.len());
        if total_bytes > MAX_TOTAL_CONFIG_BYTES {
            return Err(format!(
                "Bundled configs exceed maximum total size ({MAX_TOTAL_CONFIG_BYTES} bytes)"
            ));
        }

        let parsed: Value = serde_json::from_slice(&raw_bytes)
            .map_err(|e| format!("Bundled config '{}' is not valid JSON: {e}", rel_str))?;
        let formatted = serde_json::to_string_pretty(&parsed)
            .map_err(|e| format!("Failed to format bundled config JSON: {e}"))?;

        let dest = cfg_dir.join(&rel_path);
        ensure_no_symlink_escape(&cfg_dir, &dest)?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config subdirectory: {e}"))?;
            ensure_no_symlink_escape(&cfg_dir, &dest)?;
        }

        if dest.exists() {
            let bak_path = dest.with_extension("json.bak");
            if !bak_path.exists() {
                if let Ok(existing) = fs::read(&dest) {
                    let _ = write_file_atomic(&bak_path, &existing);
                }
            }
        }

        write_file_atomic(&dest, formatted.as_bytes())?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn share_code_roundtrip() {
        let manifest = ModpackManifest {
            format_version: 1,
            name: "Homestead Coop".to_string(),
            author: Some("Tester".to_string()),
            description: Some("Cozy survival pack".to_string()),
            game_version: Some("1.20.4".to_string()),
            created_at: "unix:1700000000".to_string(),
            mods: vec![ModpackModEntry {
                modid: "carryon".to_string(),
                name: "Carry On".to_string(),
                version: Some("1.8.0".to_string()),
                pinned: true,
                side: Some("Both".to_string()),
            }],
            included_configs: vec![],
        };

        let code = encode_share_code(&manifest).expect("encode should succeed");
        assert!(code.starts_with("vmm1:"));
        let decoded = decode_share_code(&code).expect("decode should succeed");
        assert_eq!(decoded.name, "Homestead Coop");
        assert_eq!(decoded.mods.len(), 1);
        assert_eq!(decoded.mods[0].modid, "carryon");
        assert!(decoded.mods[0].pinned);
    }

    #[test]
    fn reject_invalid_share_code_and_deflate_bomb() {
        assert!(decode_share_code("not-a-code").is_err());
        assert!(decode_share_code("vmm1:!!!invalid!!!").is_err());

        // Construct a deflate bomb (>2 MB uncompressed zeros)
        let huge = vec![b' '; (2 * 1024 * 1024) + 512];
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&huge).unwrap();
        let compressed = encoder.finish().unwrap();
        let bomb_code = format!("vmm1:{}", URL_SAFE_NO_PAD.encode(compressed));
        assert!(decode_share_code(&bomb_code).is_err());
    }

    #[tokio::test]
    async fn rejects_http_and_private_modpack_urls() {
        assert!(load_manifest_from_source("http://example.com/pack.json")
            .await
            .is_err());
        assert!(load_manifest_from_source("https://127.0.0.1/pack.json")
            .await
            .is_err());
        assert!(load_manifest_from_source("https://localhost/pack.json")
            .await
            .is_err());
    }

    #[test]
    fn extract_configs_skips_non_json_and_traversal_and_backs_up() {
        let dir = tempdir().unwrap();
        let zip_path = dir.path().join("pack.zip");
        let cfg_dir = mod_config_dir(dir.path());
        fs::create_dir_all(&cfg_dir).unwrap();
        fs::write(cfg_dir.join("Existing.json"), br#"{"old":true}"#).unwrap();

        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let opts = SimpleFileOptions::default();

            zip.start_file("ModConfig/Existing.json", opts).unwrap();
            zip.write_all(br#"{"old":false}"#).unwrap();

            zip.start_file("ModConfig/evil.dll", opts).unwrap();
            zip.write_all(b"MZ").unwrap();

            zip.start_file("ModConfig/../escape.json", opts).unwrap();
            zip.write_all(br#"{"evil":true}"#).unwrap();

            zip.finish().unwrap();
        }

        let extracted = extract_configs_from_zip(&zip_path, dir.path()).unwrap();
        assert_eq!(extracted, 1);
        assert!(!cfg_dir.join("evil.dll").exists());
        assert!(!dir.path().join("escape.json").exists());
        assert!(cfg_dir.join("Existing.json.bak").exists());
        assert_eq!(
            fs::read_to_string(cfg_dir.join("Existing.json.bak")).unwrap(),
            r#"{"old":true}"#
        );
        assert!(fs::read_to_string(cfg_dir.join("Existing.json"))
            .unwrap()
            .contains("\"old\": false"));
    }
}
