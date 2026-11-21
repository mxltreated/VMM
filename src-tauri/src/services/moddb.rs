use crate::models::{
    ModDbDetail, ModDbDetailResponse, ModDbFilters, ModDbGameVersionsResponse, ModDbListResponse,
    ModDbSummary, ModDbTagsResponse, RemoteModInfo,
};
use crate::services::security::{
    build_hardened_http_client, checked_redirect_policy, is_allowed_moddb_host, write_file_atomic,
};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};

const BASE_URL: &str = "https://mods.vintagestory.at/api";
const LIST_TTL_SECS: u64 = 15 * 60;
const DETAIL_TTL_SECS: u64 = 15 * 60;
const FILTERS_TTL_SECS: u64 = 60 * 60;
const RELEASE_MODINFO_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const MAX_CACHED_LISTS: usize = 40;
const MAX_CACHED_DETAILS: usize = 200;
const MAX_CACHED_RELEASE_MODINFOS: usize = 1000;
const MAX_DISK_CACHE_BYTES: usize = 16 * 1024 * 1024;
const MAX_API_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry<T> {
    fetched_at: u64,
    data: T,
}

impl<T> CacheEntry<T> {
    fn is_fresh(&self, ttl_secs: u64) -> bool {
        now_secs().saturating_sub(self.fetched_at) < ttl_secs
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DiskCache {
    lists: HashMap<String, CacheEntry<Vec<ModDbSummary>>>,
    details: HashMap<String, CacheEntry<ModDbDetail>>,
    filters: Option<CacheEntry<ModDbFilters>>,
    #[serde(default)]
    release_modinfo: HashMap<String, CacheEntry<RemoteModInfo>>,
}

fn prune_map<T>(map: &mut HashMap<String, CacheEntry<T>>, ttl_secs: u64, max_entries: usize) {
    map.retain(|_, entry| entry.is_fresh(ttl_secs));
    if map.len() > max_entries {
        let mut keys_by_age: Vec<(String, u64)> = map
            .iter()
            .map(|(k, v)| (k.clone(), v.fetched_at))
            .collect();
        keys_by_age.sort_by_key(|(_, ts)| *ts);
        let to_remove = map.len() - max_entries;
        for (key, _) in keys_by_age.into_iter().take(to_remove) {
            map.remove(&key);
        }
    }
}

fn prune_cache(cache: &mut DiskCache) {
    prune_map(&mut cache.lists, LIST_TTL_SECS, MAX_CACHED_LISTS);
    prune_map(&mut cache.details, DETAIL_TTL_SECS, MAX_CACHED_DETAILS);
    prune_map(
        &mut cache.release_modinfo,
        RELEASE_MODINFO_TTL_SECS,
        MAX_CACHED_RELEASE_MODINFOS,
    );
    if let Some(filters) = &cache.filters {
        if !filters.is_fresh(FILTERS_TTL_SECS) {
            cache.filters = None;
        }
    }
}

async fn read_bounded_json_response<T: for<'de> Deserialize<'de>>(
    resp: reqwest::Response,
    label: &str,
) -> Result<T, String> {
    if !resp.status().is_success() {
        return Err(format!("Mod DB returned HTTP {}", resp.status()));
    }
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_API_RESPONSE_BYTES {
            return Err(format!("{label} exceeds maximum response size"));
        }
    }
    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();
    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| format!("Failed reading {label}: {e}"))?;
        if buf.len().saturating_add(chunk.len()) > MAX_API_RESPONSE_BYTES {
            return Err(format!("{label} exceeds maximum response size"));
        }
        buf.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&buf).map_err(|e| format!("Failed to parse {label}: {e}"))
}

pub struct ModDbClient {
    client: Client,
    cache: RwLock<DiskCache>,
    cache_path: RwLock<Option<PathBuf>>,
    persist_lock: Mutex<()>,
}

impl ModDbClient {
    pub fn new() -> Self {
        let client = build_hardened_http_client(
            concat!("VintageStoryModManager/", env!("CARGO_PKG_VERSION")),
            30,
            checked_redirect_policy(5, is_allowed_moddb_host),
        )
        .unwrap_or_else(|_| Client::new());
        Self {
            client,
            cache: RwLock::new(DiskCache::default()),
            cache_path: RwLock::new(None),
            persist_lock: Mutex::new(()),
        }
    }

    pub async fn init_disk_cache(&self, cache_dir: PathBuf) {
        let file_path = cache_dir.join("moddb_cache.json");
        {
            let mut path_guard = self.cache_path.write().await;
            *path_guard = Some(file_path.clone());
        }
        let loaded = tokio::task::spawn_blocking(move || -> Option<DiskCache> {
            let meta = fs::metadata(&file_path).ok()?;
            if meta.len() > MAX_DISK_CACHE_BYTES as u64 {
                return None;
            }
            let raw = fs::read(&file_path).ok()?;
            let mut parsed = serde_json::from_slice::<DiskCache>(&raw).ok()?;
            prune_cache(&mut parsed);
            Some(parsed)
        })
        .await
        .ok()
        .flatten();

        if let Some(parsed) = loaded {
            let mut cache_guard = self.cache.write().await;
            *cache_guard = parsed;
        }
    }

    async fn persist_cache(&self) {
        let _persist_guard = self.persist_lock.lock().await;
        let path_opt = self.cache_path.read().await.clone();
        let Some(path) = path_opt else {
            return;
        };

        let json = {
            let mut cache_guard = self.cache.write().await;
            prune_cache(&mut cache_guard);
            serde_json::to_vec(&*cache_guard).ok()
        };

        let Some(bytes) = json else {
            return;
        };
        if bytes.len() > MAX_DISK_CACHE_BYTES {
            return;
        }

        let _ = tokio::task::spawn_blocking(move || {
            let _ = write_file_atomic(&path, &bytes);
        })
        .await;
    }

    pub async fn clear_cache(&self) -> Result<(), String> {
        let _persist_guard = self.persist_lock.lock().await;
        {
            let mut guard = self.cache.write().await;
            *guard = DiskCache::default();
        }
        if let Some(path) = self.cache_path.read().await.as_ref().cloned() {
            tokio::task::spawn_blocking(move || {
                if path.exists() {
                    fs::remove_file(path)
                        .map_err(|e| format!("Failed to delete cache file: {e}"))
                } else {
                    Ok(())
                }
            })
            .await
            .map_err(|e| format!("Failed waiting for cache clear: {e}"))??;
        }
        Ok(())
    }

    fn make_list_key(
        text: Option<&str>,
        tag_ids: &[i64],
        game_version_id: Option<i64>,
        order_by: Option<&str>,
    ) -> String {
        let q = text.unwrap_or("").trim().to_lowercase();
        let mut tags = tag_ids.to_vec();
        tags.sort_unstable();
        let gv = game_version_id.map(|v| v.to_string()).unwrap_or_default();
        let order = order_by.unwrap_or("").trim().to_lowercase();
        format!("q={q}|tags={tags:?}|gv={gv}|order={order}")
    }

    pub async fn search_mods(
        &self,
        text: Option<&str>,
        tag_ids: &[i64],
        game_version_id: Option<i64>,
        order_by: Option<&str>,
        force_refresh: bool,
    ) -> Result<Vec<ModDbSummary>, String> {
        let query_text = text.unwrap_or("").trim();
        // Fetch and cache the full list for (tag_ids, game_version_id, order_by) without `text`
        // so local fuzzy/relevance search works across non-identical text and avoids per-keystroke
        // network requests.
        let key = Self::make_list_key(None, tag_ids, game_version_id, order_by);

        let base_mods = if !force_refresh {
            let guard = self.cache.read().await;
            guard
                .lists
                .get(&key)
                .filter(|entry| entry.is_fresh(LIST_TTL_SECS))
                .map(|entry| entry.data.clone())
        } else {
            None
        };

        let all_mods = match base_mods {
            Some(cached) => cached,
            None => {
                let mut params: Vec<(String, String)> = Vec::new();
                for tid in tag_ids {
                    params.push(("tagids[]".to_string(), tid.to_string()));
                }
                if let Some(gv) = game_version_id {
                    params.push(("gameversions[]".to_string(), gv.to_string()));
                }
                if let Some(order) = order_by {
                    if !order.trim().is_empty() {
                        params.push(("orderby".to_string(), order.to_string()));
                    }
                }

                let resp = self
                    .client
                    .get(format!("{BASE_URL}/mods"))
                    .query(&params)
                    .send()
                    .await
                    .map_err(|e| format!("Mod DB request failed: {e}"))?;

                let parsed: ModDbListResponse =
                    read_bounded_json_response(resp, "Mod DB response").await?;

                {
                    let mut guard = self.cache.write().await;
                    guard.lists.insert(
                        key,
                        CacheEntry {
                            fetched_at: now_secs(),
                            data: parsed.mods.clone(),
                        },
                    );
                    prune_cache(&mut guard);
                }
                self.persist_cache().await;

                parsed.mods
            }
        };

        if query_text.is_empty() {
            Ok(all_mods)
        } else {
            Ok(crate::services::mod_search::filter_and_rank_mods(
                &all_mods,
                query_text,
                order_by,
            ))
        }
    }

    pub async fn get_mod_detail(&self, modid_or_str: &str) -> Result<ModDbDetail, String> {
        let key = modid_or_str.trim().to_lowercase();
        {
            let guard = self.cache.read().await;
            if let Some(entry) = guard.details.get(&key) {
                if entry.is_fresh(DETAIL_TTL_SECS) {
                    return Ok(entry.data.clone());
                }
            }
        }

        let encoded = urlencoding::encode(modid_or_str);
        let url = format!("{BASE_URL}/mod/{encoded}");
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch mod detail: {e}"))?;

        let parsed: ModDbDetailResponse = read_bounded_json_response(resp, "mod detail").await?;

        let detail = parsed
            .mod_detail
            .ok_or_else(|| format!("Mod '{modid_or_str}' not found on Mod DB"))?;

        {
            let mut guard = self.cache.write().await;
            guard.details.insert(
                key,
                CacheEntry {
                    fetched_at: now_secs(),
                    data: detail.clone(),
                },
            );
            prune_cache(&mut guard);
        }
        self.persist_cache().await;

        Ok(detail)
    }

    pub async fn get_filters(&self) -> Result<ModDbFilters, String> {
        {
            let guard = self.cache.read().await;
            if let Some(entry) = &guard.filters {
                if entry.is_fresh(FILTERS_TTL_SECS) {
                    return Ok(entry.data.clone());
                }
            }
        }

        let tags_fut = self.client.get(format!("{BASE_URL}/tags")).send();
        let gv_fut = self.client.get(format!("{BASE_URL}/gameversions")).send();
        let (tags_res, gv_res) = tokio::join!(tags_fut, gv_fut);

        let tags_resp = tags_res.map_err(|e| format!("Failed to fetch tags: {e}"))?;
        let gv_resp = gv_res.map_err(|e| format!("Failed to fetch game versions: {e}"))?;

        let tags_parsed: ModDbTagsResponse = read_bounded_json_response(tags_resp, "tags").await?;
        let mut gv_parsed: ModDbGameVersionsResponse =
            read_bounded_json_response(gv_resp, "game versions").await?;

        gv_parsed.gameversions.reverse();

        let filters = ModDbFilters {
            tags: tags_parsed.tags,
            game_versions: gv_parsed.gameversions,
        };

        {
            let mut guard = self.cache.write().await;
            guard.filters = Some(CacheEntry {
                fetched_at: now_secs(),
                data: filters.clone(),
            });
            prune_cache(&mut guard);
        }
        self.persist_cache().await;

        Ok(filters)
    }

    pub fn http_client(&self) -> &Client {
        &self.client
    }

    pub async fn get_cached_release_modinfo(&self, key: &str) -> Option<RemoteModInfo> {
        let guard = self.cache.read().await;
        guard
            .release_modinfo
            .get(key)
            .filter(|entry| entry.is_fresh(RELEASE_MODINFO_TTL_SECS))
            .map(|entry| entry.data.clone())
    }

    pub async fn put_cached_release_modinfo(&self, key: String, info: RemoteModInfo) {
        {
            let mut guard = self.cache.write().await;
            guard.release_modinfo.insert(
                key,
                CacheEntry {
                    fetched_at: now_secs(),
                    data: info,
                },
            );
            prune_cache(&mut guard);
        }
        self.persist_cache().await;
    }
}
