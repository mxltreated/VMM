import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AppSettings,
  AppUpdateInfo,
  DetectedPaths,
  DownloadProgressEvent,
  GameStatus,
  LocalMod,
  ModConfigFile,
  ModDbDetail,
  ModDbFilters,
  ModDbSummary,
  ModProfile,
  ModpackImportPreview,
  ModpackImportResult,
  ModpackManifest,
  P2pStatus,
  ProfileSettings,
  ServerConfigSummary,
  ServerLogEvent,
  ServerStatus,
} from '../types';

export const api = {
  getSettings: () => invoke<AppSettings>('get_settings'),
  saveSettings: (settings: AppSettings) =>
    invoke<AppSettings>('save_app_settings', { settings }),
  detectPaths: () => invoke<DetectedPaths>('detect_paths'),

  listLocalMods: () => invoke<LocalMod[]>('list_local_mods'),
  toggleLocalMod: (filename: string, modid: string, enable: boolean) =>
    invoke<void>('toggle_local_mod', { filename, modid, enable }),
  deleteLocalMod: (filename: string, modid: string, enabled: boolean) =>
    invoke<void>('delete_local_mod', { filename, modid, enabled }),
  checkModUpdates: () => invoke<LocalMod[]>('check_mod_updates'),

  listProfiles: () => invoke<ModProfile[]>('list_profiles'),
  createProfile: (name: string, cloneCurrent: boolean) =>
    invoke<ModProfile>('create_profile', { name, cloneCurrent }),
  renameProfile: (profileId: string, newName: string) =>
    invoke<ModProfile[]>('rename_profile', { profileId, newName }),
  deleteProfile: (profileId: string) =>
    invoke<ModProfile[]>('delete_profile', { profileId }),
  switchProfile: (profileId: string) =>
    invoke<ModProfile>('switch_profile', { profileId }),
  updateProfileSettings: (profileId: string, profileSettings: ProfileSettings) =>
    invoke<ModProfile[]>('update_profile_settings', {
      profileId,
      profileSettings,
    }),
  snapshotProfileVersions: (profileId: string) =>
    invoke<ModProfile[]>('snapshot_profile_versions', { profileId }),

  searchModDb: (params: {
    text?: string;
    tagIds?: number[];
    gameVersionId?: number | null;
    orderBy?: string;
    forceRefresh?: boolean;
  }) =>
    invoke<ModDbSummary[]>('search_moddb', {
      text: params.text || null,
      tagIds: params.tagIds ?? [],
      gameVersionId: params.gameVersionId ?? null,
      orderBy: params.orderBy || null,
      forceRefresh: params.forceRefresh ?? false,
    }),
  getModDbDetail: (modid: string) =>
    invoke<ModDbDetail>('get_moddb_detail', { modid }),
  getModDbFilters: () => invoke<ModDbFilters>('get_moddb_filters'),

  downloadMod: (params: {
    modid: string;
    url: string;
    filename: string;
    replaceOldFilename?: string | null;
  }) =>
    invoke<string>('download_mod', {
      modid: params.modid,
      url: params.url,
      filename: params.filename,
      replaceOldFilename: params.replaceOldFilename ?? null,
    }),

  launchGame: () => invoke<GameStatus>('launch_game'),
  getGameStatus: () => invoke<GameStatus>('get_game_status'),
  openModsFolder: () => invoke<void>('open_mods_folder'),

  listModConfigs: () => invoke<ModConfigFile[]>('list_mod_configs'),
  saveModConfig: (relPath: string, content: string) =>
    invoke<ModConfigFile>('save_mod_config', { relPath, content }),
  openModConfigsFolder: () => invoke<void>('open_mod_configs_folder'),
  openDataFolder: () => invoke<void>('open_data_folder'),
  clearModDbCache: () => invoke<void>('clear_moddb_cache'),

  onDownloadProgress: (cb: (ev: DownloadProgressEvent) => void): Promise<UnlistenFn> =>
    listen<DownloadProgressEvent>('download-progress', (event) => cb(event.payload)),

  // Modpack Sharing
  generateModpackCode: (params: {
    profileId: string;
    packName?: string | null;
    author?: string | null;
    description?: string | null;
  }) =>
    invoke<string>('generate_modpack_code', {
      profileId: params.profileId,
      packName: params.packName ?? null,
      author: params.author ?? null,
      description: params.description ?? null,
    }),

  exportModpackFile: (params: {
    profileId: string;
    destPath: string;
    packName?: string | null;
    author?: string | null;
    description?: string | null;
    includeConfigs: boolean;
  }) =>
    invoke<ModpackManifest>('export_modpack_file', {
      profileId: params.profileId,
      destPath: params.destPath,
      packName: params.packName ?? null,
      author: params.author ?? null,
      description: params.description ?? null,
      includeConfigs: params.includeConfigs,
    }),

  previewModpackImport: (source: string) =>
    invoke<ModpackImportPreview>('preview_modpack_import', { source }),

  applyModpackImport: (params: {
    source: string;
    profileName?: string | null;
    restoreConfigs: boolean;
  }) =>
    invoke<ModpackImportResult>('apply_modpack_import', {
      source: params.source,
      profileName: params.profileName ?? null,
      restoreConfigs: params.restoreConfigs,
    }),

  // Multiplayer, P2P & Dedicated Server
  getP2pStatus: () => invoke<P2pStatus>('get_p2p_status'),
  startP2pHost: (targetPort: number) =>
    invoke<P2pStatus>('start_p2p_host', { targetPort }),
  joinP2pRoom: (params: {
    roomCode: string;
    preferredPort?: number | null;
    launchGame: boolean;
  }) =>
    invoke<P2pStatus>('join_p2p_room', {
      roomCode: params.roomCode,
      preferredPort: params.preferredPort ?? null,
      launchGame: params.launchGame,
    }),
  stopP2p: () => invoke<P2pStatus>('stop_p2p'),
  joinServer: (address: string) =>
    invoke<GameStatus>('join_server', { address }),

  getServerStatus: () => invoke<ServerStatus>('get_server_status'),
  getServerLogs: () => invoke<ServerLogEvent[]>('get_server_logs'),
  getServerConfig: (customDataPath?: string | null) =>
    invoke<ServerConfigSummary>('get_server_config', {
      customDataPath: customDataPath ?? null,
    }),
  saveServerConfig: (
    summary: ServerConfigSummary,
    customDataPath?: string | null
  ) =>
    invoke<void>('save_server_config_cmd', {
      summary,
      customDataPath: customDataPath ?? null,
    }),
  getServerConfigRaw: (customDataPath?: string | null) =>
    invoke<string>('get_server_config_raw', {
      customDataPath: customDataPath ?? null,
    }),
  saveServerConfigRaw: (raw: string, customDataPath?: string | null) =>
    invoke<ServerConfigSummary>('save_server_config_raw_cmd', {
      raw,
      customDataPath: customDataPath ?? null,
    }),
  startDedicatedServer: (params: {
    port: number;
    maxClients: number;
    customExePath?: string | null;
    customDataPath?: string | null;
  }) =>
    invoke<ServerStatus>('start_dedicated_server', {
      port: params.port,
      maxClients: params.maxClients,
      customExePath: params.customExePath ?? null,
      customDataPath: params.customDataPath ?? null,
    }),
  stopDedicatedServer: () => invoke<ServerStatus>('stop_dedicated_server'),
  sendServerCommand: (command: string) =>
    invoke<void>('send_server_command', { command }),
  onServerLog: (cb: (ev: ServerLogEvent) => void): Promise<UnlistenFn> =>
    listen<ServerLogEvent>('server-log', (event) => cb(event.payload)),

  // Remote GitHub App Updates
  checkAppUpdate: (repo: string) =>
    invoke<AppUpdateInfo>('check_app_update', { repo }),
  openExternalUrl: (url: string) =>
    invoke<void>('open_external_url', { url }),
  installAppUpdate: (url: string) =>
    invoke<void>('install_app_update', { url }),
};
