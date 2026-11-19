use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub game_exe_path: Option<String>,
    pub data_path: Option<String>,
    pub active_profile_id: Option<String>,
    pub preferred_game_version: Option<String>,
    #[serde(default)]
    pub onboarding_completed: bool,
    #[serde(default)]
    pub launch_args: Option<String>,
    #[serde(default)]
    pub auto_check_updates: bool,
    #[serde(default)]
    pub github_repo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DetectedPaths {
    pub game_exe_path: Option<String>,
    pub data_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ModInfo {
    pub modid: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Option<Vec<String>>,
    pub side: Option<String>,
    #[serde(rename = "type")]
    pub mod_type: Option<String>,
    pub dependencies: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalMod {
    pub filename: String,
    pub modid: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub side: String,
    pub enabled: bool,
    pub icon_base64: Option<String>,
    pub dependencies: HashMap<String, String>,
    pub latest_version: Option<String>,
    pub latest_download_url: Option<String>,
    pub latest_filename: Option<String>,
    pub update_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSettings {
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub exe_path_override: Option<String>,
    #[serde(default)]
    pub max_ram_mb: Option<u32>,
    #[serde(default)]
    pub launch_args_override: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub pinned_mod_versions: HashMap<String, String>,
    #[serde(default)]
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModProfile {
    pub id: String,
    pub name: String,
    pub is_builtin: bool,
    pub enabled_modids: Vec<String>,
    pub created_at: String,
    #[serde(default)]
    pub settings: ProfileSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressEvent {
    pub modid: String,
    pub filename: String,
    pub bytes_received: u64,
    pub total_bytes: Option<u64>,
    pub done: bool,
    pub error: Option<String>,
}

// --- Mod DB API Models ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDbListResponse {
    pub statuscode: String,
    #[serde(default)]
    pub mods: Vec<ModDbSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDbSummary {
    pub modid: i64,
    pub assetid: Option<i64>,
    pub downloads: Option<i64>,
    pub follows: Option<i64>,
    pub trendingpoints: Option<i64>,
    pub comments: Option<i64>,
    pub name: String,
    pub summary: Option<String>,
    #[serde(default)]
    pub modidstrs: Vec<String>,
    pub author: Option<String>,
    pub urlalias: Option<String>,
    pub side: Option<String>,
    #[serde(rename = "type")]
    pub mod_type: Option<String>,
    pub logo: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub lastreleased: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDbDetailResponse {
    pub statuscode: String,
    #[serde(rename = "mod")]
    pub mod_detail: Option<ModDbDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDbDetail {
    pub modid: i64,
    pub assetid: Option<i64>,
    pub name: String,
    pub text: Option<String>,
    pub author: Option<String>,
    pub urlalias: Option<String>,
    pub logofilename: Option<String>,
    pub logofile: Option<String>,
    pub homepageurl: Option<String>,
    pub sourcecodeurl: Option<String>,
    pub wikiurl: Option<String>,
    pub issuetrackerurl: Option<String>,
    pub side: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub releases: Vec<ModDbRelease>,
    pub lastreleased: Option<String>,
    pub downloads: Option<i64>,
    pub follows: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDbRelease {
    pub releaseid: i64,
    pub mainfile: Option<String>,
    pub filename: Option<String>,
    pub fileid: Option<i64>,
    pub downloads: Option<i64>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub modidstr: Option<String>,
    pub modversion: Option<String>,
    pub created: Option<String>,
    pub changelog: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDbTagsResponse {
    pub statuscode: String,
    #[serde(default)]
    pub tags: Vec<ModDbTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDbTag {
    pub tagid: i64,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDbGameVersionsResponse {
    pub statuscode: String,
    #[serde(default)]
    pub gameversions: Vec<ModDbGameVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDbGameVersion {
    pub tagid: i64,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDbFilters {
    pub tags: Vec<ModDbTag>,
    pub game_versions: Vec<ModDbGameVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStatus {
    pub running: bool,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModConfigFile {
    pub rel_path: String,
    pub filename: String,
    pub size_bytes: u64,
    pub content: String,
}

// --- Modpack Sharing Models ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackModEntry {
    pub modid: String,
    pub name: String,
    pub version: Option<String>,
    pub pinned: bool,
    pub side: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackManifest {
    pub format_version: u32,
    pub name: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub game_version: Option<String>,
    pub created_at: String,
    pub mods: Vec<ModpackModEntry>,
    #[serde(default)]
    pub included_configs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackImportItem {
    pub modid: String,
    pub name: String,
    pub requested_version: Option<String>,
    pub resolved_version: Option<String>,
    pub download_url: Option<String>,
    pub filename: Option<String>,
    pub status: String, // "installed" | "downloadable" | "missing"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackImportPreview {
    pub manifest: ModpackManifest,
    pub items: Vec<ModpackImportItem>,
    pub has_configs: bool,
    pub manifest_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackImportResult {
    pub profile: ModProfile,
    pub downloaded_count: usize,
    pub skipped_missing: Vec<String>,
    pub restored_configs: usize,
}

// --- Multiplayer & Server Models ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct P2pStatus {
    pub active: bool,
    pub mode: String, // "idle" | "host" | "client"
    pub room_code: Option<String>,
    pub local_address: Option<String>,
    pub connected_peers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigSummary {
    pub server_name: String,
    pub port: u16,
    pub max_clients: u32,
    pub password: String,
    pub allow_pvp: bool,
    pub whitelist_mode: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: u16,
    pub exe_path: Option<String>,
    pub data_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerLogEvent {
    pub line: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_notes: Option<String>,
    pub html_url: String,
    pub download_url: Option<String>,
}
