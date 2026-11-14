use crate::models::{
    ModDbDetail, ModDbDetailResponse, ModDbFilters, ModDbGameVersionsResponse, ModDbListResponse,
    ModDbSummary, ModDbTagsResponse,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

const BASE_URL: &str = "https://mods.vintagestory.at/api";
const LIST_TTL_SECS: u64 = 15 * 60;
const DETAIL_TTL_SECS: u64 = 15 * 60;
const FILTERS_TTL_SECS: u64 = 60 * 60;

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
}

pub struct ModDbClient {
    client: Client,
    cache: RwLock<DiskCache>,
    cache_path: RwLock<Option<PathBuf>>,
}

impl ModDbClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("VintageStoryModManager/0.1.0")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            cache: RwLock::new(DiskCache::default()),
            cache_path: RwLock::new(None),
        }
    }

    pub async fn init_disk_cache(&self, cache_dir: PathBuf) {
        let file_path = cache_dir.join("moddb_cache.json");
        {
            let mut path_guard = self.cache_path.write().await;
            *path_guard = Some(file_path.clone());
        }
        if let Ok(raw) = fs::read_to_string(&file_path) {
            if let Ok(parsed) = serde_json::from_str::<DiskCache>(&raw) {
                let mut cache_guard = self.cache.write().await;
                *cache_guard = parsed;
            }
        }
    }

    async fn persist_cache(&self) {
        let path_opt = self.cache_path.read().await.clone();
        let Some(path) = path_opt else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let cache_guard = self.cache.read().await;
        if let Ok(json) = serde_json::to_string(&*cache_guard) {
            let _ = fs::write(path, json);
        }
    }

    pub async fn clear_cache(&self) -> Result<(), String> {
        {
            let mut guard = self.cache.write().await;
            *guard = DiskCache::default();
        }
        if let Some(path) = self.cache_path.read().await.as_ref() {
            if path.exists() {
                fs::remove_file(path)
                    .map_err(|e| format!("Failed to delete cache file: {e}"))?;
            }
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
        let key = Self::make_list_key(text, tag_ids, game_version_id, order_by);

        if !force_refresh {
            let guard = self.cache.read().await;
            if let Some(entry) = guard.lists.get(&key) {
                if entry.is_fresh(LIST_TTL_SECS) {
                    return Ok(entry.data.clone());
                }
            }
        }

        let mut params: Vec<(String, String)> = Vec::new();
        if let Some(q) = text {
            let trimmed = q.trim();
            if !trimmed.is_empty() {
                params.push(("text".to_string(), trimmed.to_string()));
            }
        }
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

        if !resp.status().is_success() {
            return Err(format!("Mod DB returned HTTP {}", resp.status()));
        }

        let parsed: ModDbListResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Mod DB response: {e}"))?;

        {
            let mut guard = self.cache.write().await;
            guard.lists.insert(
                key,
                CacheEntry {
                    fetched_at: now_secs(),
                    data: parsed.mods.clone(),
                },
            );
        }
        self.persist_cache().await;

        Ok(parsed.mods)
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

        if !resp.status().is_success() {
            return Err(format!("Mod DB returned HTTP {}", resp.status()));
        }

        let parsed: ModDbDetailResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse mod detail: {e}"))?;

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

        let tags_parsed: ModDbTagsResponse = tags_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse tags: {e}"))?;
        let mut gv_parsed: ModDbGameVersionsResponse = gv_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse game versions: {e}"))?;

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
        }
        self.persist_cache().await;

        Ok(filters)
    }
}
