mod commands;
mod models;
mod services;

use commands::AppState;
use services::launcher::GameLauncher;
use services::moddb::ModDbClient;
use services::server::ServerManager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            moddb: ModDbClient::new(),
            launcher: GameLauncher::new(),
            server: ServerManager::new(),
        })
        .setup(|app| {
            if let Ok(cache_dir) = app.path().app_cache_dir() {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = handle.state::<AppState>();
                    state.moddb.init_disk_cache(cache_dir).await;
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_app_settings,
            commands::detect_paths,
            commands::list_local_mods,
            commands::toggle_local_mod,
            commands::delete_local_mod,
            commands::check_mod_updates,
            commands::list_profiles,
            commands::create_profile,
            commands::rename_profile,
            commands::delete_profile,
            commands::switch_profile,
            commands::update_profile_settings,
            commands::snapshot_profile_versions,
            commands::search_moddb,
            commands::get_moddb_detail,
            commands::get_moddb_filters,
            commands::download_mod,
            commands::launch_game,
            commands::get_game_status,
            commands::open_mods_folder,
            commands::list_mod_configs,
            commands::save_mod_config,
            commands::open_mod_configs_folder,
            commands::open_data_folder,
            commands::clear_moddb_cache,
            commands::generate_modpack_code,
            commands::export_modpack_file,
            commands::preview_modpack_import,
            commands::apply_modpack_import,
            commands::detect_network_endpoints,
            commands::check_firewall_rule,
            commands::add_firewall_rule,
            commands::join_server,
            commands::get_server_status,
            commands::get_server_logs,
            commands::get_server_config,
            commands::save_server_config_cmd,
            commands::start_dedicated_server,
            commands::stop_dedicated_server,
            commands::send_server_command,
            commands::check_app_update,
            commands::open_external_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
