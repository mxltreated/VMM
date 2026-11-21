use crate::models::{
    ModDbDetail, ModDbRelease, ModDependenciesSummary, ModInfo, ModInstallPlan, RemoteModInfo,
    ResolvedDependencyItem,
};
use crate::services::local_mods::{
    is_newer_version, parse_modinfo_bytes, scan_local_mods,
};
use crate::services::moddb::ModDbClient;
use crate::services::modpack::select_release;
use crate::services::security::{is_zip_symlink, read_bounded, validate_moddb_download_url};
use flate2::read::DeflateDecoder;
use futures_util::stream::{self, StreamExt};
use reqwest::header::RANGE;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Cursor;
use std::path::Path;
use zip::ZipArchive;

const BUILTIN_GAME_MODIDS: &[&str] = &["game", "survival", "creative", "essentials"];
const MAX_DIRECT_DOWNLOAD_INSPECT_BYTES: usize = 256 * 1024;
const MAX_FALLBACK_INSPECT_BYTES: usize = 2 * 1024 * 1024;
const MAX_TAIL_RANGE_BYTES: u64 = 65_557; // 22-byte EOCD + max 65535-byte comment
const MAX_CENTRAL_DIR_BYTES: usize = 2 * 1024 * 1024;
const MAX_COMPRESSED_MODINFO_BYTES: usize = 512 * 1024;
const MAX_UNCOMPRESSED_MODINFO_BYTES: usize = 256 * 1024;
const MAX_DEPENDENCY_DEPTH: usize = 6;
const MAX_RESOLVED_DEPENDENCIES: usize = 40;
const MAX_BATCH_INSPECT_IDS: usize = 24;

pub fn is_builtin_game_modid(modid: &str) -> bool {
    let lower = modid.trim().to_ascii_lowercase();
    BUILTIN_GAME_MODIDS.iter().any(|b| *b == lower)
}

pub fn split_dependencies(
    deps: &HashMap<String, String>,
) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut mod_deps = HashMap::new();
    let mut game_reqs = HashMap::new();
    for (k, v) in deps {
        let key = k.trim();
        if key.is_empty() {
            continue;
        }
        if is_builtin_game_modid(key) {
            game_reqs.insert(key.to_string(), v.clone());
        } else {
            mod_deps.insert(key.to_string(), v.clone());
        }
    }
    (mod_deps, game_reqs)
}

fn modinfo_to_remote(info: ModInfo) -> RemoteModInfo {
    RemoteModInfo {
        modid: info.modid.filter(|s| !s.trim().is_empty()),
        version: info.version.filter(|s| !s.trim().is_empty()),
        dependencies: info.dependencies.unwrap_or_default(),
    }
}

fn extract_modinfo_from_full_zip_bytes(bytes: &[u8]) -> Result<RemoteModInfo, String> {
    let cursor = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("Failed to open mod zip archive: {e}"))?;

    let mut fallback_index: Option<usize> = None;

    if let Ok(mut entry) = archive.by_name("modinfo.json") {
        if !is_zip_symlink(entry.unix_mode()) {
            let raw = read_bounded(&mut entry, MAX_UNCOMPRESSED_MODINFO_BYTES, "modinfo.json")?;
            if let Some(parsed) = parse_modinfo_bytes(&raw) {
                return Ok(modinfo_to_remote(parsed));
            }
        }
    }

    for i in 0..archive.len() {
        let entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.is_dir() || is_zip_symlink(entry.unix_mode()) {
            continue;
        }
        let normalized = entry.name().replace('\\', "/").to_ascii_lowercase();
        if normalized == "modinfo.json" {
            fallback_index = Some(i);
            break;
        }
        if normalized.ends_with("/modinfo.json") && fallback_index.is_none() {
            fallback_index = Some(i);
        }
    }

    if let Some(idx) = fallback_index {
        let mut entry = archive
            .by_index(idx)
            .map_err(|e| format!("Failed to read modinfo.json entry: {e}"))?;
        let raw = read_bounded(&mut entry, MAX_UNCOMPRESSED_MODINFO_BYTES, "modinfo.json")?;
        if let Some(parsed) = parse_modinfo_bytes(&raw) {
            return Ok(modinfo_to_remote(parsed));
        }
    }

    Ok(RemoteModInfo::default())
}

fn read_u16_le(buf: &[u8], offset: usize) -> Option<u16> {
    let slice = buf.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32_le(buf: &[u8], offset: usize) -> Option<u32> {
    let slice = buf.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64_le(buf: &[u8], offset: usize) -> Option<u64> {
    let slice = buf.get(offset..offset + 8)?;
    Some(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

#[derive(Debug)]
struct ZipCdEntry {
    method: u16,
    compressed_size: u64,
    uncompressed_size: u64,
    local_header_offset: u64,
}

fn locate_eocd(tail: &[u8]) -> Option<(usize, u32, u32)> {
    if tail.len() < 22 {
        return None;
    }
    let mut pos = tail.len() - 22;
    loop {
        if tail.get(pos..pos + 4) == Some(&[0x50, 0x4b, 0x05, 0x06]) {
            let comment_len = read_u16_le(tail, pos + 20)? as usize;
            if pos + 22 + comment_len == tail.len() {
                let cd_size = read_u32_le(tail, pos + 12)?;
                let cd_offset = read_u32_le(tail, pos + 16)?;
                return Some((pos, cd_size, cd_offset));
            }
        }
        if pos == 0 {
            break;
        }
        pos -= 1;
    }
    None
}

fn find_modinfo_in_central_directory(cd: &[u8]) -> Option<ZipCdEntry> {
    let mut pos = 0usize;
    let mut nested_candidate: Option<ZipCdEntry> = None;

    while pos + 46 <= cd.len() {
        if cd.get(pos..pos + 4) != Some(&[0x50, 0x4b, 0x01, 0x02]) {
            break;
        }
        let method = read_u16_le(cd, pos + 10)?;
        let comp_32 = read_u32_le(cd, pos + 20)? as u64;
        let uncomp_32 = read_u32_le(cd, pos + 24)? as u64;
        let fname_len = read_u16_le(cd, pos + 28)? as usize;
        let extra_len = read_u16_le(cd, pos + 30)? as usize;
        let comment_len = read_u16_le(cd, pos + 32)? as usize;
        let ext_attrs = read_u32_le(cd, pos + 38)?;
        let lfh_32 = read_u32_le(cd, pos + 42)? as u64;

        let fname_start = pos + 46;
        let extra_start = fname_start + fname_len;
        let next_pos = extra_start + extra_len + comment_len;
        if next_pos > cd.len() {
            break;
        }

        let unix_mode = ext_attrs >> 16;
        let is_symlink = unix_mode != 0 && is_zip_symlink(Some(unix_mode));
        let is_dir = (ext_attrs & 0x10) != 0 || ((unix_mode & 0o170000) == 0o040000);

        if !is_symlink && !is_dir {
            let fname_bytes = &cd[fname_start..extra_start];
            let fname = String::from_utf8_lossy(fname_bytes)
                .replace('\\', "/")
                .to_ascii_lowercase();
            let is_exact = fname == "modinfo.json";
            let is_nested = fname.ends_with("/modinfo.json");

            if is_exact || (is_nested && nested_candidate.is_none()) {
                let mut compressed_size = comp_32;
                let mut uncompressed_size = uncomp_32;
                let mut local_header_offset = lfh_32;

                // Parse ZIP64 extended information extra field (0x0001) if needed
                if comp_32 == 0xFFFF_FFFF || uncomp_32 == 0xFFFF_FFFF || lfh_32 == 0xFFFF_FFFF {
                    let extra = &cd[extra_start..extra_start + extra_len];
                    let mut epos = 0usize;
                    while epos + 4 <= extra.len() {
                        let tag = read_u16_le(extra, epos).unwrap_or(0);
                        let sz = read_u16_le(extra, epos + 2).unwrap_or(0) as usize;
                        epos += 4;
                        if epos + sz > extra.len() {
                            break;
                        }
                        if tag == 0x0001 {
                            let mut zpos = 0usize;
                            if uncomp_32 == 0xFFFF_FFFF && zpos + 8 <= sz {
                                uncompressed_size =
                                    read_u64_le(&extra[epos..epos + sz], zpos).unwrap_or(0);
                                zpos += 8;
                            }
                            if comp_32 == 0xFFFF_FFFF && zpos + 8 <= sz {
                                compressed_size =
                                    read_u64_le(&extra[epos..epos + sz], zpos).unwrap_or(0);
                                zpos += 8;
                            }
                            if lfh_32 == 0xFFFF_FFFF && zpos + 8 <= sz {
                                local_header_offset =
                                    read_u64_le(&extra[epos..epos + sz], zpos).unwrap_or(0);
                            }
                            break;
                        }
                        epos += sz;
                    }
                }

                let entry = ZipCdEntry {
                    method,
                    compressed_size,
                    uncompressed_size,
                    local_header_offset,
                };
                if is_exact {
                    return Some(entry);
                }
                nested_candidate = Some(entry);
            }
        }

        pos = next_pos;
    }

    nested_candidate
}

fn decode_modinfo_from_lfh_payload(
    lfh_bytes: &[u8],
    method: u16,
    compressed_size: usize,
) -> Result<RemoteModInfo, String> {
    if lfh_bytes.len() < 30 || lfh_bytes.get(0..4) != Some(&[0x50, 0x4b, 0x03, 0x04]) {
        return Err("Invalid ZIP local file header".to_string());
    }
    let fname_len = read_u16_le(lfh_bytes, 26).ok_or("Invalid LFH filename len")? as usize;
    let extra_len = read_u16_le(lfh_bytes, 28).ok_or("Invalid LFH extra len")? as usize;
    let data_start = 30usize
        .checked_add(fname_len)
        .and_then(|v| v.checked_add(extra_len))
        .ok_or("LFH header overflow")?;
    let data_end = data_start
        .checked_add(compressed_size)
        .ok_or("LFH payload overflow")?;
    let payload = lfh_bytes
        .get(data_start..data_end)
        .ok_or("Incomplete compressed modinfo.json slice")?;

    let raw_bytes = match method {
        0 => {
            if payload.len() > MAX_UNCOMPRESSED_MODINFO_BYTES {
                return Err("modinfo.json exceeds size limit".to_string());
            }
            payload.to_vec()
        }
        8 => {
            let mut decoder = DeflateDecoder::new(payload);
            read_bounded(&mut decoder, MAX_UNCOMPRESSED_MODINFO_BYTES, "modinfo.json")?
        }
        other => {
            return Err(format!("Unsupported ZIP compression method {other} for modinfo.json"));
        }
    };

    Ok(parse_modinfo_bytes(&raw_bytes)
        .map(modinfo_to_remote)
        .unwrap_or_default())
}

async fn read_bounded_bytes(resp: reqwest::Response, max_bytes: usize, label: &str) -> Result<Vec<u8>, String> {
    if let Some(len) = resp.content_length() {
        if len as usize > max_bytes {
            return Err(format!("{label} exceeds size limit"));
        }
    }
    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();
    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| format!("Failed reading {label}: {e}"))?;
        if buf.len().saturating_add(chunk.len()) > max_bytes {
            return Err(format!("{label} exceeds size limit"));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

async fn fetch_range(
    client: &reqwest::Client,
    url: &url::Url,
    start: u64,
    end_inclusive: u64,
    max_bytes: usize,
    label: &str,
) -> Result<(u16, Vec<u8>), String> {
    let range_hdr = format!("bytes={start}-{end_inclusive}");
    let resp = client
        .get(url.clone())
        .header(RANGE, range_hdr)
        .send()
        .await
        .map_err(|e| format!("Failed range request for {label}: {e}"))?;

    let status = resp.status().as_u16();
    if status != 206 && status != 200 {
        return Err(format!("Unexpected HTTP {status} reading {label}"));
    }
    let limit = if status == 200 {
        MAX_FALLBACK_INSPECT_BYTES
    } else {
        max_bytes
    };
    let bytes = read_bounded_bytes(resp, limit, label).await?;
    Ok((status, bytes))
}

async fn inspect_remote_zip_range(
    client: &reqwest::Client,
    raw_url: &str,
) -> Result<RemoteModInfo, String> {
    let url = validate_moddb_download_url(raw_url)?;

    let head_resp = client
        .head(url.clone())
        .send()
        .await
        .map_err(|e| format!("HEAD request failed: {e}"))?;

    let content_len = if head_resp.status().is_success() {
        head_resp.content_length()
    } else {
        None
    };
    let accepts_ranges = head_resp
        .headers()
        .get(reqwest::header::ACCEPT_RANGES)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_ascii_lowercase().contains("bytes"))
        .unwrap_or(false);

    if let Some(len) = content_len {
        if len <= MAX_DIRECT_DOWNLOAD_INSPECT_BYTES as u64 || (!accepts_ranges && len <= MAX_FALLBACK_INSPECT_BYTES as u64) {
            let resp = client
                .get(url)
                .send()
                .await
                .map_err(|e| format!("GET request failed: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("GET failed with HTTP {}", resp.status()));
            }
            let bytes = read_bounded_bytes(resp, MAX_FALLBACK_INSPECT_BYTES, "mod archive").await?;
            return extract_modinfo_from_full_zip_bytes(&bytes);
        }
    }

    let Some(total_len) = content_len else {
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("GET request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET failed with HTTP {}", resp.status()));
        }
        let bytes = read_bounded_bytes(resp, MAX_FALLBACK_INSPECT_BYTES, "mod archive").await?;
        return extract_modinfo_from_full_zip_bytes(&bytes);
    };

    if total_len < 22 {
        return Err("Remote file is too small to be a zip archive".to_string());
    }

    let tail_len = total_len.min(MAX_TAIL_RANGE_BYTES);
    let tail_start = total_len - tail_len;
    let (status, tail) = fetch_range(
        client,
        &url,
        tail_start,
        total_len - 1,
        MAX_TAIL_RANGE_BYTES as usize + 64,
        "ZIP EOCD",
    )
    .await?;

    if status == 200 {
        return extract_modinfo_from_full_zip_bytes(&tail);
    }

    let (eocd_pos, cd_size_32, cd_offset_32) =
        locate_eocd(&tail).ok_or("Could not locate ZIP End of Central Directory")?;

    let (cd_offset, cd_size) = if cd_size_32 == 0xFFFF_FFFF || cd_offset_32 == 0xFFFF_FFFF {
        // ZIP64 EOCD Locator sits 20 bytes before EOCD
        let mut z64_offset = None;
        if eocd_pos >= 20 {
            let loc = &tail[eocd_pos - 20..eocd_pos];
            if loc.get(0..4) == Some(&[0x50, 0x4b, 0x06, 0x07]) {
                z64_offset = read_u64_le(loc, 8);
            }
        }
        let z64_off = z64_offset.ok_or("ZIP64 EOCD locator missing")?;
        let (st, z64_bytes) =
            fetch_range(client, &url, z64_off, z64_off + 55, 128, "ZIP64 EOCD").await?;
        if st == 200 {
            return extract_modinfo_from_full_zip_bytes(&z64_bytes);
        }
        if z64_bytes.len() < 56 || z64_bytes.get(0..4) != Some(&[0x50, 0x4b, 0x06, 0x06]) {
            return Err("Invalid ZIP64 EOCD".to_string());
        }
        let size64 = read_u64_le(&z64_bytes, 40).ok_or("Missing ZIP64 CD size")?;
        let off64 = read_u64_le(&z64_bytes, 48).ok_or("Missing ZIP64 CD offset")?;
        (off64, size64)
    } else {
        (cd_offset_32 as u64, cd_size_32 as u64)
    };

    if cd_size == 0 || cd_size as usize > MAX_CENTRAL_DIR_BYTES {
        return Err("ZIP Central Directory exceeds inspection limit".to_string());
    }
    let cd_end = cd_offset
        .checked_add(cd_size)
        .and_then(|end| end.checked_sub(1))
        .filter(|end| *end < total_len)
        .ok_or("ZIP Central Directory range out of bounds")?;

    let (cd_status, cd_bytes) = fetch_range(
        client,
        &url,
        cd_offset,
        cd_end,
        MAX_CENTRAL_DIR_BYTES + 64,
        "ZIP Central Directory",
    )
    .await?;

    if cd_status == 200 {
        return extract_modinfo_from_full_zip_bytes(&cd_bytes);
    }

    let Some(entry) = find_modinfo_in_central_directory(&cd_bytes) else {
        return Ok(RemoteModInfo::default());
    };

    if entry.compressed_size > MAX_COMPRESSED_MODINFO_BYTES as u64
        || entry.uncompressed_size > MAX_UNCOMPRESSED_MODINFO_BYTES as u64
    {
        return Err("modinfo.json exceeds maximum allowed size".to_string());
    }

    // Read Local File Header + up to 4 KB slack for extra fields + compressed payload
    let optimistic_end = entry
        .local_header_offset
        .saturating_add(30 + 512 + 4096)
        .saturating_add(entry.compressed_size)
        .min(total_len - 1);

    let (lfh_status, mut lfh_bytes) = fetch_range(
        client,
        &url,
        entry.local_header_offset,
        optimistic_end,
        MAX_COMPRESSED_MODINFO_BYTES + 8192,
        "modinfo.json local file header",
    )
    .await?;

    if lfh_status == 200 {
        return extract_modinfo_from_full_zip_bytes(&lfh_bytes);
    }

    if lfh_bytes.len() < 30 {
        return Err("Truncated local file header".to_string());
    }
    let fname_len = read_u16_le(&lfh_bytes, 26).unwrap_or(0) as u64;
    let extra_len = read_u16_le(&lfh_bytes, 28).unwrap_or(0) as u64;
    let needed_len = 30 + fname_len + extra_len + entry.compressed_size;
    if needed_len > (MAX_COMPRESSED_MODINFO_BYTES + 16_384) as u64 {
        return Err("modinfo.json local entry exceeds size limit".to_string());
    }
    if (lfh_bytes.len() as u64) < needed_len {
        let exact_end = (entry.local_header_offset + needed_len - 1).min(total_len - 1);
        let (_, exact_bytes) = fetch_range(
            client,
            &url,
            entry.local_header_offset,
            exact_end,
            MAX_COMPRESSED_MODINFO_BYTES + 16_384,
            "modinfo.json payload",
        )
        .await?;
        lfh_bytes = exact_bytes;
    }

    decode_modinfo_from_lfh_payload(&lfh_bytes, entry.method, entry.compressed_size as usize)
}

pub async fn inspect_release_modinfo(
    moddb: &ModDbClient,
    release: &ModDbRelease,
) -> Result<RemoteModInfo, String> {
    let url = release
        .mainfile
        .as_deref()
        .ok_or_else(|| "Release has no download URL".to_string())?;
    let cache_key = if release.releaseid > 0 {
        format!("rel:{}", release.releaseid)
    } else {
        format!("url:{url}")
    };

    if let Some(cached) = moddb.get_cached_release_modinfo(&cache_key).await {
        return Ok(cached);
    }

    let info = inspect_remote_zip_range(moddb.http_client(), url).await?;
    moddb.put_cached_release_modinfo(cache_key, info.clone()).await;
    Ok(info)
}

pub async fn inspect_mod_dependencies_summary(
    moddb: &ModDbClient,
    modid: &str,
) -> Result<ModDependenciesSummary, String> {
    let detail = moddb.get_mod_detail(modid).await?;
    let Some(latest) = detail.releases.iter().find(|r| r.mainfile.is_some()) else {
        return Ok(ModDependenciesSummary {
            modid: detail.modid,
            release_id: None,
            modid_str: None,
            mod_dependencies: HashMap::new(),
            game_requirements: HashMap::new(),
        });
    };

    let remote_info = inspect_release_modinfo(moddb, latest).await?;
    let (mod_dependencies, game_requirements) = split_dependencies(&remote_info.dependencies);
    Ok(ModDependenciesSummary {
        modid: detail.modid,
        release_id: Some(latest.releaseid),
        modid_str: remote_info.modid.or_else(|| latest.modidstr.clone()),
        mod_dependencies,
        game_requirements,
    })
}

pub async fn inspect_mod_dependencies_batch(
    moddb: &ModDbClient,
    modids: Vec<String>,
) -> Vec<ModDependenciesSummary> {
    let unique: Vec<String> = {
        let mut seen = HashSet::new();
        modids
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && seen.insert(s.to_ascii_lowercase()))
            .take(MAX_BATCH_INSPECT_IDS)
            .collect()
    };

    stream::iter(unique)
        .map(|id| async move { inspect_mod_dependencies_summary(moddb, &id).await.ok() })
        .buffer_unordered(4)
        .filter_map(|res| async move { res })
        .collect()
        .await
}

fn clean_version_req(req: &str) -> &str {
    req.trim()
        .trim_start_matches(">=")
        .trim_start_matches('>')
        .trim_start_matches('^')
        .trim_start_matches('~')
        .trim()
}

fn is_wildcard_version(req: &str) -> bool {
    let t = req.trim();
    t.is_empty() || t == "*" || t.eq_ignore_ascii_case("any")
}

fn version_satisfies(installed: &str, req: &str) -> bool {
    if is_wildcard_version(req) {
        return true;
    }
    let clean_req = clean_version_req(req);
    if is_wildcard_version(clean_req) {
        return true;
    }
    // Satisfied unless required version is strictly newer than installed version
    !is_newer_version(installed, clean_req)
}

fn pick_best_release<'a>(
    detail: &'a ModDbDetail,
    preferred_game_version: Option<&str>,
) -> Option<&'a ModDbRelease> {
    select_release(detail, None, false, preferred_game_version)
}

pub async fn resolve_install_plan(
    moddb: &ModDbClient,
    data_path: &Path,
    preferred_game_version: Option<&str>,
    primary_modid: &str,
    primary_name: &str,
    primary_release: &ModDbRelease,
) -> Result<ModInstallPlan, String> {
    let primary_url = primary_release
        .mainfile
        .clone()
        .ok_or_else(|| "Selected release has no download URL".to_string())?;
    let primary_filename = primary_release
        .filename
        .clone()
        .ok_or_else(|| "Selected release has no filename".to_string())?;

    let local_mods = scan_local_mods(data_path)?;
    let local_by_id: HashMap<String, _> = local_mods
        .into_iter()
        .map(|m| (m.modid.to_ascii_lowercase(), m))
        .collect();

    let primary_remote = inspect_release_modinfo(moddb, primary_release)
        .await
        .unwrap_or_default();

    let resolved_primary_modid = primary_release
        .modidstr
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| primary_remote.modid.clone())
        .unwrap_or_else(|| primary_modid.to_string());

    let primary_existing = local_by_id.get(&resolved_primary_modid.to_ascii_lowercase());

    let (direct_mod_deps, mut game_requirements) =
        split_dependencies(&primary_remote.dependencies);

    struct QueueEntry {
        modid: String,
        version_req: String,
        required_by: String,
        depth: usize,
    }

    let mut queue: VecDeque<QueueEntry> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(resolved_primary_modid.to_ascii_lowercase());
    visited.insert(primary_modid.to_ascii_lowercase());

    for (dep_id, ver_req) in direct_mod_deps {
        queue.push_back(QueueEntry {
            modid: dep_id,
            version_req: ver_req,
            required_by: primary_name.to_string(),
            depth: 1,
        });
    }

    let mut resolved_map: HashMap<String, ResolvedDependencyItem> = HashMap::new();
    let mut resolution_order: Vec<String> = Vec::new();
    let mut unresolved_warnings: Vec<String> = Vec::new();

    while let Some(item) = queue.pop_front() {
        let lower_id = item.modid.trim().to_ascii_lowercase();
        if lower_id.is_empty() || is_builtin_game_modid(&lower_id) {
            continue;
        }

        if let Some(existing) = resolved_map.get_mut(&lower_id) {
            if !existing.required_by.contains(&item.required_by) {
                existing.required_by.push(item.required_by);
            }
            continue;
        }

        if !visited.insert(lower_id.clone()) {
            continue;
        }
        if resolved_map.len() >= MAX_RESOLVED_DEPENDENCIES {
            unresolved_warnings.push(format!(
                "Stopped resolving dependencies after {MAX_RESOLVED_DEPENDENCIES} items"
            ));
            break;
        }

        if let Some(local) = local_by_id.get(&lower_id) {
            let satisfies = version_satisfies(&local.version, &item.version_req);
            if satisfies {
                let status = if local.enabled {
                    "installed".to_string()
                } else {
                    "disabled".to_string()
                };
                resolved_map.insert(
                    lower_id.clone(),
                    ResolvedDependencyItem {
                        modid: local.modid.clone(),
                        name: local.name.clone(),
                        version_requirement: item.version_req.clone(),
                        status: status.clone(),
                        installed_version: Some(local.version.clone()),
                        installed_filename: Some(local.filename.clone()),
                        resolved_version: Some(local.version.clone()),
                        download_url: None,
                        filename: None,
                        replace_old_filename: None,
                        required_by: vec![item.required_by],
                    },
                );
                resolution_order.push(lower_id.clone());

                // If disabled locally, also check its own dependencies before enabling it
                if !local.enabled && item.depth < MAX_DEPENDENCY_DEPTH {
                    let (sub_deps, sub_game) = split_dependencies(&local.dependencies);
                    for (gk, gv) in sub_game {
                        game_requirements.entry(gk).or_insert(gv);
                    }
                    for (sub_id, sub_req) in sub_deps {
                        queue.push_back(QueueEntry {
                            modid: sub_id,
                            version_req: sub_req,
                            required_by: local.name.clone(),
                            depth: item.depth + 1,
                        });
                    }
                }
                continue;
            }
        }

        // Not installed locally, or installed version is older than required -> look up on Mod DB
        match moddb.get_mod_detail(&item.modid).await {
            Ok(detail) => {
                if let Some(rel) = pick_best_release(&detail, preferred_game_version) {
                    let dl_url = rel.mainfile.clone();
                    let fname = rel.filename.clone();
                    if let (Some(url), Some(filename)) = (dl_url, fname) {
                        let sub_info = inspect_release_modinfo(moddb, rel)
                            .await
                            .unwrap_or_default();
                        let (sub_deps, sub_game) = split_dependencies(&sub_info.dependencies);
                        for (gk, gv) in sub_game {
                            game_requirements.entry(gk).or_insert(gv);
                        }

                        let local_match = local_by_id.get(&lower_id);
                        resolved_map.insert(
                            lower_id.clone(),
                            ResolvedDependencyItem {
                                modid: rel
                                    .modidstr
                                    .clone()
                                    .filter(|s| !s.trim().is_empty())
                                    .or(sub_info.modid)
                                    .unwrap_or_else(|| item.modid.clone()),
                                name: detail.name.clone(),
                                version_requirement: item.version_req.clone(),
                                status: "download".to_string(),
                                installed_version: local_match.map(|m| m.version.clone()),
                                installed_filename: local_match.map(|m| m.filename.clone()),
                                resolved_version: rel.modversion.clone(),
                                download_url: Some(url),
                                filename: Some(filename),
                                replace_old_filename: local_match.map(|m| m.filename.clone()),
                                required_by: vec![item.required_by],
                            },
                        );
                        resolution_order.push(lower_id.clone());

                        if item.depth < MAX_DEPENDENCY_DEPTH {
                            for (sub_id, sub_req) in sub_deps {
                                queue.push_back(QueueEntry {
                                    modid: sub_id,
                                    version_req: sub_req,
                                    required_by: detail.name.clone(),
                                    depth: item.depth + 1,
                                });
                            }
                        }
                        continue;
                    }
                }

                unresolved_warnings.push(format!(
                    "Dependency '{}' has no downloadable release on Mod DB",
                    detail.name
                ));
                resolved_map.insert(
                    lower_id.clone(),
                    ResolvedDependencyItem {
                        modid: item.modid.clone(),
                        name: detail.name,
                        version_requirement: item.version_req,
                        status: "missing".to_string(),
                        installed_version: None,
                        installed_filename: None,
                        resolved_version: None,
                        download_url: None,
                        filename: None,
                        replace_old_filename: None,
                        required_by: vec![item.required_by],
                    },
                );
                resolution_order.push(lower_id);
            }
            Err(_) => {
                unresolved_warnings.push(format!(
                    "Could not find dependency '{}' on Vintage Story Mod DB",
                    item.modid
                ));
                resolved_map.insert(
                    lower_id.clone(),
                    ResolvedDependencyItem {
                        modid: item.modid.clone(),
                        name: item.modid.clone(),
                        version_requirement: item.version_req,
                        status: "missing".to_string(),
                        installed_version: None,
                        installed_filename: None,
                        resolved_version: None,
                        download_url: None,
                        filename: None,
                        replace_old_filename: None,
                        required_by: vec![item.required_by],
                    },
                );
                resolution_order.push(lower_id);
            }
        }
    }

    let dependencies = resolution_order
        .into_iter()
        .filter_map(|k| resolved_map.remove(&k))
        .collect();

    Ok(ModInstallPlan {
        primary_modid: resolved_primary_modid,
        primary_name: primary_name.to_string(),
        primary_version: primary_release.modversion.clone(),
        primary_url,
        primary_filename,
        primary_replace_old_filename: primary_existing.map(|m| m.filename.clone()),
        game_requirements,
        dependencies,
        unresolved_warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn make_zip(entries: &[(&str, &[u8], CompressionMethod)]) -> Vec<u8> {
        let buf = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(buf);
        for (name, data, method) in entries {
            let opts = SimpleFileOptions::default().compression_method(*method);
            writer.start_file(*name, opts).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn test_split_dependencies_excludes_builtin_game_ids() {
        let mut deps = HashMap::new();
        deps.insert("game".to_string(), "1.22.0".to_string());
        deps.insert("Survival".to_string(), "*".to_string());
        deps.insert("creative".to_string(), "1.22.0".to_string());
        deps.insert("essentials".to_string(), "*".to_string());
        deps.insert("overhaullib".to_string(), "1.0.0".to_string());
        deps.insert("configlib".to_string(), "1.3.0".to_string());

        let (mod_deps, game_reqs) = split_dependencies(&deps);
        assert_eq!(mod_deps.len(), 2);
        assert_eq!(mod_deps.get("overhaullib").map(String::as_str), Some("1.0.0"));
        assert_eq!(mod_deps.get("configlib").map(String::as_str), Some("1.3.0"));
        assert_eq!(game_reqs.len(), 4);
    }

    #[test]
    fn test_range_zip_parser_deflate_and_stored_and_nested() {
        let mut bom_modinfo = vec![0xEF, 0xBB, 0xBF];
        bom_modinfo.extend_from_slice(
            br#"{"modid":"testmod","version":"1.2.3","dependencies":{"game":"1.22.0","overhaullib":"1.0.0"}}"#,
        );

        for (entry_name, method) in [
            ("modinfo.json", CompressionMethod::Deflated),
            ("modinfo.json", CompressionMethod::Stored),
            ("subfolder/modinfo.json", CompressionMethod::Deflated),
        ] {
            let zip_bytes = make_zip(&[
                ("assets/big.txt", &vec![b'A'; 4096], CompressionMethod::Deflated),
                (entry_name, &bom_modinfo, method),
            ]);

            let tail_start = zip_bytes.len().saturating_sub(MAX_TAIL_RANGE_BYTES as usize);
            let (_, cd_size, cd_offset) = locate_eocd(&zip_bytes[tail_start..]).unwrap();
            let cd = &zip_bytes[cd_offset as usize..(cd_offset + cd_size) as usize];
            let entry = find_modinfo_in_central_directory(cd).unwrap();
            let lfh_slice = &zip_bytes[entry.local_header_offset as usize..];
            let parsed = decode_modinfo_from_lfh_payload(
                lfh_slice,
                entry.method,
                entry.compressed_size as usize,
            )
            .unwrap();

            assert_eq!(parsed.modid.as_deref(), Some("testmod"));
            assert_eq!(parsed.version.as_deref(), Some("1.2.3"));
            assert_eq!(
                parsed.dependencies.get("overhaullib").map(String::as_str),
                Some("1.0.0")
            );
        }
    }

    #[test]
    fn test_version_satisfies() {
        assert!(version_satisfies("1.2.0", "*"));
        assert!(version_satisfies("1.2.0", ""));
        assert!(version_satisfies("1.2.0", "1.2.0"));
        assert!(version_satisfies("1.5.0", ">=1.2.0"));
        assert!(!version_satisfies("1.1.0", "1.2.0"));
    }
}
