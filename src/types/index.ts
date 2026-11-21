export type ThemeId =
  | 'forge-ember'
  | 'temporal-rift'
  | 'seraph-light'
  | 'rustworks'
  | 'redwood-forest';

export interface AppSettings {
  gameExePath: string | null;
  dataPath: string | null;
  activeProfileId: string | null;
  preferredGameVersion: string | null;
  onboardingCompleted: boolean;
  launchArgs: string | null;
  autoCheckUpdates: boolean;
  githubRepo?: string | null;
  theme?: string | null;
}

export interface DetectedPaths {
  gameExePath: string | null;
  dataPath: string | null;
}

export interface LocalMod {
  filename: string;
  modid: string;
  name: string;
  version: string;
  description: string;
  authors: string[];
  side: string;
  enabled: boolean;
  iconBase64: string | null;
  dependencies: Record<string, string>;
  latestVersion: string | null;
  latestDownloadUrl: string | null;
  latestFilename: string | null;
  updateAvailable: boolean;
}

export interface ProfileSettings {
  gameVersion: string | null;
  exePathOverride: string | null;
  maxRamMb: number | null;
  launchArgsOverride: string | null;
  notes: string | null;
  pinnedModVersions: Record<string, string>;
  sourceUrl?: string | null;
}

export interface ModProfile {
  id: string;
  name: string;
  isBuiltin: boolean;
  enabledModids: string[];
  createdAt: string;
  settings: ProfileSettings;
}

export interface DownloadProgressEvent {
  modid: string;
  filename: string;
  bytesReceived: number;
  totalBytes: number | null;
  done: boolean;
  error: string | null;
}

export interface ModDbSummary {
  modid: number;
  assetid: number | null;
  downloads: number | null;
  follows: number | null;
  trendingpoints: number | null;
  comments: number | null;
  name: string;
  summary: string | null;
  modidstrs: string[];
  author: string | null;
  urlalias: string | null;
  side: string | null;
  modType: string | null;
  logo: string | null;
  tags: string[];
  lastreleased: string | null;
}

export interface ModDbRelease {
  releaseid: number;
  mainfile: string | null;
  filename: string | null;
  fileid: number | null;
  downloads: number | null;
  tags: string[];
  modidstr: string | null;
  modversion: string | null;
  created: string | null;
  changelog: string | null;
}

export interface ModDbDetail {
  modid: number;
  assetid: number | null;
  name: string;
  text: string | null;
  author: string | null;
  urlalias: string | null;
  logofile: string | null;
  homepageurl: string | null;
  sourcecodeurl: string | null;
  wikiurl: string | null;
  issuetrackerurl: string | null;
  side: string | null;
  tags: string[];
  releases: ModDbRelease[];
  lastreleased: string | null;
  downloads: number | null;
  follows: number | null;
}

export interface ModDbTag {
  tagid: number;
  name: string;
  color: string | null;
}

export interface ModDbGameVersion {
  tagid: number;
  name: string;
  color: string | null;
}

export interface ModDbFilters {
  tags: ModDbTag[];
  gameVersions: ModDbGameVersion[];
}

export interface GameStatus {
  running: boolean;
  pid: number | null;
}

export interface ModConfigFile {
  relPath: string;
  filename: string;
  sizeBytes: number;
  content: string;
}

export type NavTab =
  | 'installed'
  | 'browse'
  | 'configs'
  | 'profiles'
  | 'multiplayer'
  | 'logs'
  | 'settings';

export interface ModpackModEntry {
  modid: string;
  name: string;
  version: string | null;
  pinned: boolean;
  side: string | null;
}

export interface ModpackManifest {
  formatVersion: number;
  name: string;
  author: string | null;
  description: string | null;
  gameVersion: string | null;
  createdAt: string;
  mods: ModpackModEntry[];
  includedConfigs: string[];
}

export interface ModpackImportItem {
  modid: string;
  name: string;
  requestedVersion: string | null;
  resolvedVersion: string | null;
  downloadUrl: string | null;
  filename: string | null;
  status: 'installed' | 'downloadable' | 'missing';
}

export interface ModpackImportPreview {
  manifest: ModpackManifest;
  items: ModpackImportItem[];
  hasConfigs: boolean;
  manifestHash: string;
}

export interface ModpackImportResult {
  profile: ModProfile;
  downloadedCount: number;
  skippedMissing: string[];
  restoredConfigs: number;
}

export interface P2pStatus {
  active: boolean;
  mode: 'idle' | 'host' | 'client';
  roomCode: string | null;
  localAddress: string | null;
  connectedPeers: number;
}

export interface ServerConfigSummary {
  serverName: string;
  port: number;
  maxClients: number;
  password: string;
  allowPvp: boolean;
  whitelistMode: number;
}

export interface ServerStatus {
  running: boolean;
  pid: number | null;
  port: number;
  exePath: string | null;
  dataPath: string | null;
}

export interface ServerLogEvent {
  line: string;
  isError: boolean;
}

export interface AppUpdateInfo {
  currentVersion: string;
  latestVersion: string;
  updateAvailable: boolean;
  releaseNotes: string | null;
  htmlUrl: string;
  downloadUrl: string | null;
}

export interface ModDependenciesSummary {
  modid: number;
  releaseId: number | null;
  modidStr: string | null;
  modDependencies: Record<string, string>;
  gameRequirements: Record<string, string>;
}

export interface ResolvedDependencyItem {
  modid: string;
  name: string;
  versionRequirement: string;
  status: 'installed' | 'disabled' | 'download' | 'missing';
  installedVersion: string | null;
  installedFilename: string | null;
  resolvedVersion: string | null;
  downloadUrl: string | null;
  filename: string | null;
  replaceOldFilename: string | null;
  requiredBy: string[];
}

export interface ModInstallPlan {
  primaryModid: string;
  primaryName: string;
  primaryVersion: string | null;
  primaryUrl: string;
  primaryFilename: string;
  primaryReplaceOldFilename: string | null;
  gameRequirements: Record<string, string>;
  dependencies: ResolvedDependencyItem[];
  unresolvedWarnings: string[];
}

export interface BatchDownloadItem {
  modid: string;
  url: string;
  filename: string;
  replaceOldFilename?: string | null;
}

export interface EnableModItem {
  modid: string;
  filename: string;
}

export interface BatchDownloadError {
  modid: string;
  filename: string;
  error: string;
}

export interface BatchDownloadResult {
  succeeded: string[];
  enabled: string[];
  failed: BatchDownloadError[];
}

export interface GameLogFile {
  filename: string;
  relPath: string;
  sizeBytes: number;
  modifiedMs: number;
  isCrash: boolean;
  category: 'crash' | 'client' | 'server' | 'other' | string;
}

export interface GameLogContent {
  relPath: string;
  content: string;
  sizeBytes: number;
  truncated: boolean;
  lineCount: number;
}

export interface CrashSuspect {
  modid: string;
  modName: string;
  filename: string | null;
  enabled: boolean | null;
  confidence: 'high' | 'medium' | 'low' | string;
  reasons: string[];
  sampleLines: string[];
}

export interface CrashAnalysisReport {
  crashDetected: boolean;
  headline: string;
  exceptionSummary: string | null;
  analyzedFiles: string[];
  suspects: CrashSuspect[];
  recommendations: string[];
}
