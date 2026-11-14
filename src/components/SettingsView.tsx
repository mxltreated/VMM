import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  Check,
  Download,
  FolderOpen,
  GitBranch,
  Loader2,
  Sparkles,
  Terminal,
  Trash2,
  Wand2,
} from 'lucide-react';
import { api } from '../api/tauri';
import type { AppSettings, AppUpdateInfo } from '../types';

interface SettingsViewProps {
  settings: AppSettings;
  onSaveSettings: (next: AppSettings) => Promise<void>;
  onOpenOnboarding: () => void;
}

export function SettingsView({
  settings,
  onSaveSettings,
  onOpenOnboarding,
}: SettingsViewProps) {
  const [gameExePath, setGameExePath] = useState(settings.gameExePath ?? '');
  const [dataPath, setDataPath] = useState(settings.dataPath ?? '');
  const [launchArgs, setLaunchArgs] = useState(settings.launchArgs ?? '');
  const [autoCheckUpdates, setAutoCheckUpdates] = useState(
    settings.autoCheckUpdates ?? false
  );
  const [saving, setSaving] = useState(false);
  const [savedToast, setSavedToast] = useState(false);
  const [cacheCleared, setCacheCleared] = useState(false);
  const [githubRepo, setGithubRepo] = useState(
    settings.githubRepo ?? 'mxltreated/VMM'
  );
  const [checkingAppUpdate, setCheckingAppUpdate] = useState(false);
  const [installingAppUpdate, setInstallingAppUpdate] = useState(false);
  const [appUpdateInfo, setAppUpdateInfo] = useState<AppUpdateInfo | null>(null);
  const [appUpdateErr, setAppUpdateErr] = useState<string | null>(null);

  const pickExe = async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      title: 'Select Vintagestory.exe',
      filters: [{ name: 'Executable', extensions: ['exe'] }],
    });
    if (typeof selected === 'string') {
      setGameExePath(selected);
    }
  };

  const pickDataDir = async () => {
    const selected = await open({
      multiple: false,
      directory: true,
      title: 'Select VintagestoryData folder',
    });
    if (typeof selected === 'string') {
      setDataPath(selected);
    }
  };

  const handleAutoDetect = async () => {
    const detected = await api.detectPaths();
    if (detected.gameExePath) setGameExePath(detected.gameExePath);
    if (detected.dataPath) setDataPath(detected.dataPath);
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      await onSaveSettings({
        ...settings,
        gameExePath: gameExePath.trim() || null,
        dataPath: dataPath.trim() || null,
        launchArgs: launchArgs.trim() || null,
        autoCheckUpdates,
        githubRepo: githubRepo.trim() || null,
      });
      setSavedToast(true);
      setTimeout(() => setSavedToast(false), 2000);
    } finally {
      setSaving(false);
    }
  };

  const handleClearCache = async () => {
    await api.clearModDbCache();
    setCacheCleared(true);
    setTimeout(() => setCacheCleared(false), 2000);
  };

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-5 max-w-2xl">
      <form
        onSubmit={handleSave}
        className="p-6 rounded-2xl bg-[#141720] border border-white/[0.06] space-y-6"
      >
        <div>
          <h2 className="text-sm font-semibold text-zinc-100">
            Game, Data &amp; Launch Options
          </h2>
          <p className="text-xs text-zinc-500 mt-1">
            Configure where Vintage Story lives and how it launches.
          </p>
        </div>

        <div className="space-y-4">
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-zinc-300">
              Vintage Story Executable (Vintagestory.exe)
            </label>
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={gameExePath}
                onChange={(e) => setGameExePath(e.target.value)}
                placeholder="C:\...\Vintagestory\Vintagestory.exe"
                className="flex-1 bg-[#0e1015] border border-white/[0.07] rounded-lg px-3 py-2 text-xs text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-amber-500/40"
              />
              <button
                type="button"
                onClick={pickExe}
                className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-[#1c202c] hover:bg-white/[0.08] border border-white/[0.07] text-xs text-zinc-200 cursor-pointer"
              >
                <FolderOpen className="w-3.5 h-3.5" />
                <span>Browse</span>
              </button>
            </div>
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-zinc-300">
              VintagestoryData Folder
            </label>
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={dataPath}
                onChange={(e) => setDataPath(e.target.value)}
                placeholder="%APPDATA%\VintagestoryData"
                className="flex-1 bg-[#0e1015] border border-white/[0.07] rounded-lg px-3 py-2 text-xs text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-amber-500/40"
              />
              <button
                type="button"
                onClick={pickDataDir}
                className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-[#1c202c] hover:bg-white/[0.08] border border-white/[0.07] text-xs text-zinc-200 cursor-pointer"
              >
                <FolderOpen className="w-3.5 h-3.5" />
                <span>Browse</span>
              </button>
            </div>
          </div>

          <div className="space-y-1.5">
            <label className="flex items-center gap-1.5 text-xs font-medium text-zinc-300">
              <Terminal className="w-3.5 h-3.5 text-zinc-500" />
              <span>Custom Launch Arguments</span>
            </label>
            <input
              type="text"
              value={launchArgs}
              onChange={(e) => setLaunchArgs(e.target.value)}
              placeholder='--openWorld "My World" --tracelog'
              className="w-full bg-[#0e1015] border border-white/[0.07] rounded-lg px-3 py-2 text-xs font-mono text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-amber-500/40"
            />
          </div>

          <div className="flex items-center justify-between gap-4 pt-1">
            <div>
              <div className="text-xs font-medium text-zinc-200">
                Auto-check for mod updates on startup
              </div>
              <div className="text-[11px] text-zinc-500">
                Checks installed mods against Mod DB cache when the app opens.
              </div>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={autoCheckUpdates}
              onClick={() => setAutoCheckUpdates((v) => !v)}
              className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors ${
                autoCheckUpdates ? 'bg-amber-500' : 'bg-zinc-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white shadow transition ${
                  autoCheckUpdates ? 'translate-x-4' : 'translate-x-0'
                }`}
              />
            </button>
          </div>
        </div>

        <div className="pt-4 border-t border-white/[0.06] flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={onOpenOnboarding}
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-white/[0.05] hover:bg-white/[0.09] text-xs font-medium text-zinc-300 transition-colors active:scale-[0.98] cursor-pointer"
            >
              <Wand2 className="w-3.5 h-3.5 text-amber-400" />
              <span>Run Setup Wizard</span>
            </button>
            <button
              type="button"
              onClick={handleAutoDetect}
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-white/[0.05] hover:bg-white/[0.09] text-xs font-medium text-zinc-300 transition-colors active:scale-[0.98] cursor-pointer"
            >
              <Sparkles className="w-3.5 h-3.5 text-amber-400" />
              <span>Auto-Detect</span>
            </button>
          </div>

          <div className="flex items-center gap-3">
            {savedToast && (
              <span className="animate-scale-in flex items-center gap-1 text-xs text-emerald-400">
                <Check className="w-3.5 h-3.5" />
                Saved
              </span>
            )}
            <button
              type="submit"
              disabled={saving}
              className="px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 text-zinc-950 text-xs font-semibold transition-all active:scale-[0.98] cursor-pointer disabled:opacity-50"
            >
              {saving ? 'Saving...' : 'Save Settings'}
            </button>
          </div>
        </div>
      </form>

      {/* Storage & Folder Shortcuts */}
      <div className="p-6 rounded-2xl bg-[#141720] border border-white/[0.06] space-y-4">
        <div>
          <h2 className="text-sm font-semibold text-zinc-100">
            Folders &amp; Cache
          </h2>
          <p className="text-xs text-zinc-500 mt-1">
            Quick shortcuts to your Vintage Story directories and Mod DB cache.
          </p>
        </div>

        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={() => api.openModsFolder()}
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-[#1c202c] hover:bg-white/[0.08] border border-white/[0.07] text-xs text-zinc-200 active:scale-[0.98] cursor-pointer"
            >
              <FolderOpen className="w-3.5 h-3.5 text-zinc-400" />
              <span>Mods</span>
            </button>
            <button
              type="button"
              onClick={() => api.openModConfigsFolder()}
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-[#1c202c] hover:bg-white/[0.08] border border-white/[0.07] text-xs text-zinc-200 active:scale-[0.98] cursor-pointer"
            >
              <FolderOpen className="w-3.5 h-3.5 text-zinc-400" />
              <span>ModConfig</span>
            </button>
            <button
              type="button"
              onClick={() => api.openDataFolder()}
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-[#1c202c] hover:bg-white/[0.08] border border-white/[0.07] text-xs text-zinc-200 active:scale-[0.98] cursor-pointer"
            >
              <FolderOpen className="w-3.5 h-3.5 text-zinc-400" />
              <span>Data Folder</span>
            </button>
          </div>

          <button
            type="button"
            onClick={handleClearCache}
            className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-white/[0.05] hover:bg-rose-500/15 hover:text-rose-300 border border-white/[0.07] text-xs font-medium text-zinc-300 transition-colors active:scale-[0.98] cursor-pointer"
          >
            {cacheCleared ? (
              <>
                <Check className="w-3.5 h-3.5 text-emerald-400" />
                <span className="text-emerald-400">Cache Cleared</span>
              </>
            ) : (
              <>
                <Trash2 className="w-3.5 h-3.5" />
                <span>Clear Mod DB Cache</span>
              </>
            )}
          </button>
        </div>
      </div>

      {/* Remote App Updates (GitHub Releases) */}
      <div className="p-6 rounded-2xl bg-[#141720] border border-white/[0.06] space-y-4">
        <div className="flex items-center gap-2">
          <GitBranch className="w-4 h-4 text-amber-400" />
          <div>
            <h2 className="text-sm font-semibold text-zinc-100">
              VMM Updates (GitHub Releases)
            </h2>
            <p className="text-xs text-zinc-500">
              Point VMM to your GitHub repository to check for new app releases.
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <input
            type="text"
            value={githubRepo}
            onChange={(e) => setGithubRepo(e.target.value)}
            placeholder="owner/vintage-story-mod-manager"
            className="flex-1 bg-[#0e1015] border border-white/[0.07] rounded-lg px-3 py-2 text-xs font-mono text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-amber-500/40"
          />
          <button
            type="button"
            disabled={!githubRepo.trim() || checkingAppUpdate}
            onClick={async () => {
              setCheckingAppUpdate(true);
              setAppUpdateErr(null);
              try {
                await onSaveSettings({
                  ...settings,
                  githubRepo: githubRepo.trim() || null,
                });
                const info = await api.checkAppUpdate(githubRepo.trim());
                setAppUpdateInfo(info);
              } catch (err) {
                setAppUpdateErr(err instanceof Error ? err.message : String(err));
              } finally {
                setCheckingAppUpdate(false);
              }
            }}
            className="flex items-center gap-1.5 px-3.5 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-zinc-950 text-xs font-semibold cursor-pointer"
          >
            {checkingAppUpdate && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
            <span>Check Updates</span>
          </button>
        </div>

        {appUpdateErr && (
          <div className="p-3 rounded-xl bg-rose-500/10 border border-rose-500/25 text-xs text-rose-300">
            {appUpdateErr}
          </div>
        )}

        {appUpdateInfo && (
          <div className="p-3.5 rounded-xl bg-[#0e1015] border border-white/[0.06] space-y-2 text-xs">
            <div className="flex items-center justify-between">
              <span className="text-zinc-200 font-medium">
                {appUpdateInfo.updateAvailable
                  ? `Update available: v${appUpdateInfo.latestVersion} (current v${appUpdateInfo.currentVersion})`
                  : `You are up to date (v${appUpdateInfo.currentVersion})`}
              </span>
              {appUpdateInfo.updateAvailable && (
                <button
                  type="button"
                  disabled={installingAppUpdate}
                  onClick={async () => {
                    if (!appUpdateInfo.downloadUrl) {
                      await api.openExternalUrl(appUpdateInfo.htmlUrl);
                      return;
                    }
                    setInstallingAppUpdate(true);
                    setAppUpdateErr(null);
                    try {
                      await api.installAppUpdate(appUpdateInfo.downloadUrl);
                    } catch (err) {
                      setAppUpdateErr(
                        err instanceof Error ? err.message : String(err)
                      );
                      setInstallingAppUpdate(false);
                    }
                  }}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-emerald-300 font-medium cursor-pointer disabled:opacity-50"
                >
                  {installingAppUpdate ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <Download className="w-3.5 h-3.5" />
                  )}
                  <span>
                    {installingAppUpdate
                      ? 'Downloading Installer...'
                      : appUpdateInfo.downloadUrl
                        ? `Install v${appUpdateInfo.latestVersion}`
                        : `Download v${appUpdateInfo.latestVersion}`}
                  </span>
                </button>
              )}
            </div>
            {appUpdateInfo.releaseNotes && (
              <div className="text-[11px] text-zinc-400 whitespace-pre-wrap max-h-28 overflow-y-auto">
                {appUpdateInfo.releaseNotes}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
