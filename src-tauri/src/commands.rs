use crate::models::{
    AppSettings, AppUpdateInfo, DetectedPaths, FirewallStatus, GameStatus, LocalMod,
    ModConfigFile, ModDbDetail, ModDbFilters, ModDbSummary, ModProfile, ModpackImportPreview,
    ModpackImportResult, ModpackManifest, NetworkEndpoint, ProfileSettings, ServerConfigSummary,
    ServerLogEvent, ServerStatus,
};
use crate::services::downloader::download_mod_file;
use crate::services::launcher::GameLauncher;
use crate::services::local_mods::{
    check_updates_for_mods, delete_mod as fs_delete_mod, mods_dir, scan_local_mods,
    toggle_mod as fs_toggle_mod,
};
use crate::services::mod_configs::{
    list_mod_configs as svc_list_mod_configs, mod_config_dir,
    save_mod_config as svc_save_mod_config,
};
use crate::services::moddb::ModDbClient;
use crate::services::modpack::{
    apply_modpack_import as svc_apply_modpack_import, build_manifest_from_profile,
    encode_share_code, export_modpack_file as svc_export_modpack_file,
    preview_modpack_import as svc_preview_modpack_import,
};
use crate::services::multiplayer::{
    add_firewall_rule as svc_add_firewall_rule, check_firewall_rule as svc_check_firewall_rule,
    detect_network_endpoints as svc_detect_network_endpoints, join_server as svc_join_server,
};
use crate::services::profiles::{
    create_profile as svc_create_profile, delete_profile as svc_delete_profile, load_profiles,
    rename_profile as svc_rename_profile, snapshot_profile_versions as svc_snapshot_profile_versions,
    switch_profile as svc_switch_profile, sync_mod_in_active_profile,
    update_profile_settings as svc_update_profile_settings,
};
use crate::services::server::{
    default_server_data_path, detect_server_exe, load_server_config, save_server_config,
    ServerManager,
};
use crate::services::settings::{
    check_github_app_update, detect_vintage_story_paths, load_settings, save_settings,
};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

pub struct AppState {
    pub moddb: ModDbClient,
    pub launcher: GameLauncher,
    pub server: ServerManager,
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("Failed to resolve app config dir: {e}"))
}

fn require_data_path(cfg_dir: &PathBuf) -> Result<(AppSettings, PathBuf), String> {
    let settings = load_settings(cfg_dir)?;
    let dp = settings
        .data_path
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "Vintage Story data path is not configured".to_string())?;
    Ok((settings.clone(), PathBuf::from(dp)))
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    let cfg = config_dir(&app)?;
    load_settings(&cfg)
}

#[tauri::command]
pub fn save_app_settings(app: AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    let cfg = config_dir(&app)?;
    save_settings(&cfg, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn detect_paths() -> DetectedPaths {
    detect_vintage_story_paths()
}

#[tauri::command]
pub fn list_local_mods(app: AppHandle) -> Result<Vec<LocalMod>, String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    scan_local_mods(&dp)
}

#[tauri::command]
pub fn toggle_local_mod(
    app: AppHandle,
    filename: String,
    modid: String,
    enable: bool,
) -> Result<(), String> {
    let cfg = config_dir(&app)?;
    let (settings, dp) = require_data_path(&cfg)?;
    fs_toggle_mod(&dp, &filename, enable)?;
    sync_mod_in_active_profile(
        &cfg,
        &dp,
        settings.active_profile_id.as_deref(),
        &modid,
        enable,
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_local_mod(
    app: AppHandle,
    filename: String,
    modid: String,
    enabled: bool,
) -> Result<(), String> {
    let cfg = config_dir(&app)?;
    let (settings, dp) = require_data_path(&cfg)?;
    fs_delete_mod(&dp, &filename, enabled)?;
    sync_mod_in_active_profile(
        &cfg,
        &dp,
        settings.active_profile_id.as_deref(),
        &modid,
        false,
    )?;
    Ok(())
}

#[tauri::command]
pub async fn check_mod_updates(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<LocalMod>, String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    let mods = scan_local_mods(&dp)?;
    Ok(check_updates_for_mods(&state.moddb, mods).await)
}

#[tauri::command]
pub fn list_profiles(app: AppHandle) -> Result<Vec<ModProfile>, String> {
    let cfg = config_dir(&app)?;
    let settings = load_settings(&cfg)?;
    let dp = settings.data_path.as_deref().map(PathBuf::from);
    load_profiles(&cfg, dp.as_deref())
}

#[tauri::command]
pub fn create_profile(
    app: AppHandle,
    name: String,
    clone_current: bool,
) -> Result<ModProfile, String> {
    let cfg = config_dir(&app)?;
    let settings = load_settings(&cfg)?;
    let dp = settings.data_path.as_deref().map(PathBuf::from);
    svc_create_profile(&cfg, dp.as_deref(), name, clone_current)
}

#[tauri::command]
pub fn rename_profile(
    app: AppHandle,
    profile_id: String,
    new_name: String,
) -> Result<Vec<ModProfile>, String> {
    let cfg = config_dir(&app)?;
    let settings = load_settings(&cfg)?;
    let dp = settings.data_path.as_deref().map(PathBuf::from);
    svc_rename_profile(&cfg, dp.as_deref(), &profile_id, new_name)
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, profile_id: String) -> Result<Vec<ModProfile>, String> {
    let cfg = config_dir(&app)?;
    let mut settings = load_settings(&cfg)?;
    let dp = settings.data_path.as_deref().map(PathBuf::from);
    let profiles = svc_delete_profile(&cfg, dp.as_deref(), &profile_id)?;
    if settings.active_profile_id.as_deref() == Some(&profile_id) {
        settings.active_profile_id = Some("default".to_string());
        save_settings(&cfg, &settings)?;
        if let Some(data_dir) = dp {
            let _ = svc_switch_profile(&cfg, &data_dir, "default");
        }
    }
    Ok(profiles)
}

#[tauri::command]
pub fn switch_profile(app: AppHandle, profile_id: String) -> Result<ModProfile, String> {
    let cfg = config_dir(&app)?;
    let (mut settings, dp) = require_data_path(&cfg)?;
    let profile = svc_switch_profile(&cfg, &dp, &profile_id)?;
    settings.active_profile_id = Some(profile.id.clone());
    save_settings(&cfg, &settings)?;
    Ok(profile)
}

#[tauri::command]
pub fn update_profile_settings(
    app: AppHandle,
    profile_id: String,
    profile_settings: ProfileSettings,
) -> Result<Vec<ModProfile>, String> {
    let cfg = config_dir(&app)?;
    let settings = load_settings(&cfg)?;
    let dp = settings.data_path.as_deref().map(PathBuf::from);
    svc_update_profile_settings(&cfg, dp.as_deref(), &profile_id, profile_settings)
}

#[tauri::command]
pub fn snapshot_profile_versions(
    app: AppHandle,
    profile_id: String,
) -> Result<Vec<ModProfile>, String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    svc_snapshot_profile_versions(&cfg, &dp, &profile_id)
}

#[tauri::command]
pub async fn search_moddb(
    state: State<'_, AppState>,
    text: Option<String>,
    tag_ids: Vec<i64>,
    game_version_id: Option<i64>,
    order_by: Option<String>,
    force_refresh: Option<bool>,
) -> Result<Vec<ModDbSummary>, String> {
    state
        .moddb
        .search_mods(
            text.as_deref(),
            &tag_ids,
            game_version_id,
            order_by.as_deref(),
            force_refresh.unwrap_or(false),
        )
        .await
}

#[tauri::command]
pub async fn get_moddb_detail(
    state: State<'_, AppState>,
    modid: String,
) -> Result<ModDbDetail, String> {
    state.moddb.get_mod_detail(&modid).await
}

#[tauri::command]
pub async fn get_moddb_filters(state: State<'_, AppState>) -> Result<ModDbFilters, String> {
    state.moddb.get_filters().await
}

#[tauri::command]
pub async fn download_mod(
    app: AppHandle,
    modid: String,
    url: String,
    filename: String,
    replace_old_filename: Option<String>,
) -> Result<String, String> {
    let cfg = config_dir(&app)?;
    let (mut settings, dp) = require_data_path(&cfg)?;

    let saved_name = download_mod_file(
        &app,
        &modid,
        &url,
        &filename,
        &dp,
        replace_old_filename.as_deref(),
    )
    .await?;

    // If user is on Vanilla, switch active profile to Default or keep Vanilla?
    // Adding to active profile if not Vanilla ensures it stays enabled on switch.
    if settings.active_profile_id.as_deref() == Some("vanilla") {
        settings.active_profile_id = Some("default".to_string());
        let _ = save_settings(&cfg, &settings);
    }
    sync_mod_in_active_profile(
        &cfg,
        &dp,
        settings.active_profile_id.as_deref(),
        &modid,
        true,
    )?;

    Ok(saved_name)
}

#[tauri::command]
pub fn launch_game(app: AppHandle, state: State<'_, AppState>) -> Result<GameStatus, String> {
    let cfg = config_dir(&app)?;
    let settings = load_settings(&cfg)?;

    let dp = settings.data_path.as_deref().map(PathBuf::from);
    let active_profile = load_profiles(&cfg, dp.as_deref())
        .ok()
        .and_then(|list| {
            list.into_iter()
                .find(|p| Some(p.id.as_str()) == settings.active_profile_id.as_deref())
        });

    let profile_exe = active_profile
        .as_ref()
        .and_then(|p| p.settings.exe_path_override.as_deref())
        .filter(|s| !s.trim().is_empty());

    let exe_path = profile_exe
        .or(settings.game_exe_path.as_deref())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            "Vintagestory.exe path is not configured. Please set it in Settings.".to_string()
        })?;

    let profile_args = active_profile
        .as_ref()
        .and_then(|p| p.settings.launch_args_override.as_deref())
        .filter(|s| !s.trim().is_empty());

    let launch_args = profile_args.or(settings.launch_args.as_deref());
    let max_ram_mb = active_profile.as_ref().and_then(|p| p.settings.max_ram_mb);

    state.launcher.launch(
        exe_path,
        settings.data_path.as_deref(),
        launch_args,
        max_ram_mb,
    )
}

#[tauri::command]
pub fn get_game_status(state: State<'_, AppState>) -> GameStatus {
    state.launcher.status()
}

#[tauri::command]
pub fn open_mods_folder(app: AppHandle) -> Result<(), String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    let dir = mods_dir(&dp);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create Mods directory: {e}"))?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("Failed to open Mods folder: {e}"))
}

#[tauri::command]
pub fn list_mod_configs(app: AppHandle) -> Result<Vec<ModConfigFile>, String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    svc_list_mod_configs(&dp)
}

#[tauri::command]
pub fn save_mod_config(
    app: AppHandle,
    rel_path: String,
    content: String,
) -> Result<ModConfigFile, String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    svc_save_mod_config(&dp, &rel_path, &content)
}

#[tauri::command]
pub fn open_mod_configs_folder(app: AppHandle) -> Result<(), String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    let dir = mod_config_dir(&dp);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create ModConfig directory: {e}"))?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("Failed to open ModConfig folder: {e}"))
}

#[tauri::command]
pub fn open_data_folder(app: AppHandle) -> Result<(), String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    fs::create_dir_all(&dp).map_err(|e| format!("Failed to create Data directory: {e}"))?;
    app.opener()
        .open_path(dp.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("Failed to open Data folder: {e}"))
}

#[tauri::command]
pub async fn clear_moddb_cache(state: State<'_, AppState>) -> Result<(), String> {
    state.moddb.clear_cache().await
}

// --- Modpack Sharing Commands ---

#[tauri::command]
pub fn generate_modpack_code(
    app: AppHandle,
    profile_id: String,
    pack_name: Option<String>,
    author: Option<String>,
    description: Option<String>,
) -> Result<String, String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    let manifest = build_manifest_from_profile(
        &cfg,
        &dp,
        &profile_id,
        pack_name,
        author,
        description,
        false,
    )?;
    encode_share_code(&manifest)
}

#[tauri::command]
pub fn export_modpack_file(
    app: AppHandle,
    profile_id: String,
    dest_path: String,
    pack_name: Option<String>,
    author: Option<String>,
    description: Option<String>,
    include_configs: bool,
) -> Result<ModpackManifest, String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    svc_export_modpack_file(
        &cfg,
        &dp,
        &profile_id,
        PathBuf::from(&dest_path).as_path(),
        pack_name,
        author,
        description,
        include_configs,
    )
}

#[tauri::command]
pub async fn preview_modpack_import(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
) -> Result<ModpackImportPreview, String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    svc_preview_modpack_import(&source, &state.moddb, &dp).await
}

#[tauri::command]
pub async fn apply_modpack_import(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
    profile_name: Option<String>,
    restore_configs: bool,
) -> Result<ModpackImportResult, String> {
    let cfg = config_dir(&app)?;
    let (mut settings, dp) = require_data_path(&cfg)?;
    let res = svc_apply_modpack_import(
        &app,
        &cfg,
        &dp,
        &state.moddb,
        &source,
        profile_name,
        restore_configs,
    )
    .await?;
    settings.active_profile_id = Some(res.profile.id.clone());
    save_settings(&cfg, &settings)?;
    Ok(res)
}

// --- Multiplayer & Server Commands ---

#[tauri::command]
pub fn detect_network_endpoints() -> Vec<NetworkEndpoint> {
    svc_detect_network_endpoints()
}

#[tauri::command]
pub fn check_firewall_rule(port: u16) -> FirewallStatus {
    svc_check_firewall_rule(port)
}

#[tauri::command]
pub fn add_firewall_rule(port: u16) -> Result<FirewallStatus, String> {
    svc_add_firewall_rule(port)
}

#[tauri::command]
pub fn join_server(
    app: AppHandle,
    state: State<'_, AppState>,
    address: String,
) -> Result<GameStatus, String> {
    let cfg = config_dir(&app)?;
    let settings = load_settings(&cfg)?;
    let profiles = load_profiles(&cfg, settings.data_path.as_deref().map(PathBuf::from).as_deref())
        .unwrap_or_default();
    let active_profile = settings
        .active_profile_id
        .as_deref()
        .and_then(|id| profiles.into_iter().find(|p| p.id == id));

    let exe_path = active_profile
        .as_ref()
        .and_then(|p| p.settings.exe_path_override.clone())
        .or(settings.game_exe_path.clone())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "Vintage Story executable path is not configured".to_string())?;

    let launch_args = active_profile
        .as_ref()
        .and_then(|p| p.settings.launch_args_override.clone())
        .or(settings.launch_args.clone());
    let max_ram = active_profile.as_ref().and_then(|p| p.settings.max_ram_mb);

    svc_join_server(
        &state.launcher,
        &exe_path,
        settings.data_path.as_deref(),
        &address,
        launch_args.as_deref(),
        max_ram,
    )
}

#[tauri::command]
pub fn get_server_status(app: AppHandle, state: State<'_, AppState>) -> Result<ServerStatus, String> {
    let cfg = config_dir(&app)?;
    let settings = load_settings(&cfg)?;
    let default_exe = detect_server_exe(settings.game_exe_path.as_deref());
    let default_data = settings
        .data_path
        .as_deref()
        .map(|dp| default_server_data_path(PathBuf::from(dp).as_path()).to_string_lossy().to_string());
    Ok(state.server.status(default_exe, default_data))
}

#[tauri::command]
pub fn get_server_logs(state: State<'_, AppState>) -> Vec<ServerLogEvent> {
    state.server.recent_logs()
}

#[tauri::command]
pub fn get_server_config(
    app: AppHandle,
    custom_data_path: Option<String>,
) -> Result<ServerConfigSummary, String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    let srv_dp = custom_data_path
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_server_data_path(&dp));
    load_server_config(&srv_dp)
}

#[tauri::command]
pub fn save_server_config_cmd(
    app: AppHandle,
    summary: ServerConfigSummary,
    custom_data_path: Option<String>,
) -> Result<(), String> {
    let cfg = config_dir(&app)?;
    let (_, dp) = require_data_path(&cfg)?;
    let srv_dp = custom_data_path
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_server_data_path(&dp));
    save_server_config(&srv_dp, &summary)
}

#[tauri::command]
pub fn start_dedicated_server(
    app: AppHandle,
    state: State<'_, AppState>,
    port: u16,
    max_clients: u32,
    custom_exe_path: Option<String>,
    custom_data_path: Option<String>,
) -> Result<ServerStatus, String> {
    let cfg = config_dir(&app)?;
    let (settings, dp) = require_data_path(&cfg)?;
    let exe_path = custom_exe_path
        .filter(|s| !s.trim().is_empty())
        .or_else(|| detect_server_exe(settings.game_exe_path.as_deref()))
        .ok_or_else(|| "VintagestoryServer.exe could not be found. Please specify its path.".to_string())?;
    let srv_dp = custom_data_path
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_server_data_path(&dp));
    let mods = mods_dir(&dp);
    state
        .server
        .start(&app, &exe_path, &srv_dp, Some(&mods), port, max_clients)
}

#[tauri::command]
pub fn stop_dedicated_server(state: State<'_, AppState>) -> Result<ServerStatus, String> {
    state.server.stop()
}

#[tauri::command]
pub fn send_server_command(state: State<'_, AppState>, command: String) -> Result<(), String> {
    state.server.send_command(&command)
}

#[tauri::command]
pub async fn check_app_update(repo: String) -> Result<AppUpdateInfo, String> {
    check_github_app_update(&repo).await
}

#[tauri::command]
pub fn open_external_url(app: AppHandle, url: String) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("Failed to open URL: {e}"))
}
