import { useCallback, useEffect, useMemo, useState } from 'react';
import { AlertCircle, X } from 'lucide-react';
import { api } from './api/tauri';
import { BrowseModsView } from './components/BrowseModsView';
import { ConfigsView } from './components/ConfigsView';
import { HeaderBar } from './components/HeaderBar';
import { InstalledModsView } from './components/InstalledModsView';
import { ModDetailDrawer } from './components/ModDetailDrawer';
import { MultiplayerView } from './components/MultiplayerView';
import { OnboardingModal } from './components/OnboardingModal';
import { ProfilesView } from './components/ProfilesView';
import { SettingsView } from './components/SettingsView';
import { Sidebar } from './components/Sidebar';
import type {
  AppSettings,
  DownloadProgressEvent,
  GameStatus,
  LocalMod,
  ModConfigFile,
  ModDbDetail,
  ModDbRelease,
  ModDbSummary,
  ModProfile,
  ModpackImportResult,
  NavTab,
  ProfileSettings,
} from './types';

export function App() {
  const [activeTab, setActiveTab] = useState<NavTab>('installed');
  const [settings, setSettings] = useState<AppSettings>({
    gameExePath: null,
    dataPath: null,
    activeProfileId: 'default',
    preferredGameVersion: null,
    onboardingCompleted: true,
    launchArgs: null,
    autoCheckUpdates: false,
  });
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [localMods, setLocalMods] = useState<LocalMod[]>([]);
  const [modConfigs, setModConfigs] = useState<ModConfigFile[]>([]);
  const [selectedConfigPath, setSelectedConfigPath] = useState<string | null>(null);
  const [profiles, setProfiles] = useState<ModProfile[]>([]);
  const [loadingMods, setLoadingMods] = useState(true);
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [launching, setLaunching] = useState(false);
  const [gameStatus, setGameStatus] = useState<GameStatus>({
    running: false,
    pid: null,
  });
  const [downloads, setDownloads] = useState<Record<string, DownloadProgressEvent>>({});
  const [selectedMod, setSelectedMod] = useState<ModDbSummary | null>(null);
  const [bannerError, setBannerError] = useState<string | null>(null);

  const refreshLocalMods = useCallback(async () => {
    try {
      const mods = await api.listLocalMods();
      setLocalMods(mods);
    } catch {
      setLocalMods([]);
    } finally {
      setLoadingMods(false);
    }
  }, []);

  const refreshProfiles = useCallback(async () => {
    try {
      const list = await api.listProfiles();
      setProfiles(list);
    } catch {
      // ignore if dataPath not configured yet
    }
  }, []);

  const refreshConfigs = useCallback(async () => {
    try {
      const list = await api.listModConfigs();
      setModConfigs(list);
    } catch {
      setModConfigs([]);
    }
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const s = await api.getSettings();
        setSettings(s);
        if (!s.onboardingCompleted) {
          setShowOnboarding(true);
        }
        await Promise.all([refreshLocalMods(), refreshProfiles(), refreshConfigs()]);
        if (s.autoCheckUpdates) {
          api
            .checkModUpdates()
            .then(setLocalMods)
            .catch(() => {});
        }
      } catch (err) {
        setBannerError(err instanceof Error ? err.message : String(err));
        await Promise.all([refreshLocalMods(), refreshProfiles(), refreshConfigs()]);
      }
      const status = await api.getGameStatus().catch(() => ({ running: false, pid: null }));
      setGameStatus(status);
    })();
  }, [refreshLocalMods, refreshProfiles, refreshConfigs]);

  useEffect(() => {
    const interval = setInterval(() => {
      api
        .getGameStatus()
        .then(setGameStatus)
        .catch(() => {});
    }, 3000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    api.onDownloadProgress((ev) => {
      setDownloads((prev) => ({ ...prev, [ev.modid]: ev }));
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const enabledCount = useMemo(
    () => localMods.filter((m) => m.enabled).length,
    [localMods]
  );

  const handleToggleMod = async (mod: LocalMod) => {
    try {
      await api.toggleLocalMod(mod.filename, mod.modid, !mod.enabled);
      await Promise.all([refreshLocalMods(), refreshProfiles()]);
    } catch (err) {
      setBannerError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleDeleteMod = async (mod: LocalMod) => {
    try {
      await api.deleteLocalMod(mod.filename, mod.modid, mod.enabled);
      await Promise.all([refreshLocalMods(), refreshProfiles()]);
    } catch (err) {
      setBannerError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCheckUpdates = async () => {
    setCheckingUpdates(true);
    setBannerError(null);
    try {
      const updated = await api.checkModUpdates();
      setLocalMods(updated);
    } catch (err) {
      setBannerError(err instanceof Error ? err.message : String(err));
    } finally {
      setCheckingUpdates(false);
    }
  };

  const handleUpdateMod = async (mod: LocalMod) => {
    if (!mod.latestDownloadUrl || !mod.latestFilename) return;
    try {
      await api.downloadMod({
        modid: mod.modid,
        url: mod.latestDownloadUrl,
        filename: mod.latestFilename,
        replaceOldFilename: mod.filename,
      });
      await Promise.all([refreshLocalMods(), refreshProfiles()]);
    } catch (err) {
      setBannerError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleInstallRelease = async (detail: ModDbDetail, release: ModDbRelease) => {
    if (!release.mainfile || !release.filename) return;
    const modid = release.modidstr || String(detail.modid);
    const existing = localMods.find(
      (m) => m.modid.toLowerCase() === modid.toLowerCase()
    );
    try {
      await api.downloadMod({
        modid,
        url: release.mainfile,
        filename: release.filename,
        replaceOldFilename: existing?.filename ?? null,
      });
      const updatedSettings = await api.getSettings();
      setSettings(updatedSettings);
      await Promise.all([refreshLocalMods(), refreshProfiles()]);
    } catch (err) {
      setBannerError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleQuickInstall = async (summary: ModDbSummary) => {
    try {
      const detail = await api.getModDbDetail(String(summary.modid));
      const latest = detail.releases[0];
      if (!latest || !latest.mainfile || !latest.filename) {
        setBannerError(`No downloadable release found for ${summary.name}`);
        return;
      }
      await handleInstallRelease(detail, latest);
    } catch (err) {
      setBannerError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleSwitchProfile = async (profileId: string) => {
    try {
      await api.switchProfile(profileId);
      const nextSettings = await api.getSettings();
      setSettings(nextSettings);
      await Promise.all([refreshLocalMods(), refreshProfiles()]);
    } catch (err) {
      setBannerError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCreateProfile = async (name: string, cloneCurrent: boolean) => {
    try {
      await api.createProfile(name, cloneCurrent);
      await refreshProfiles();
    } catch (err) {
      setBannerError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleRenameProfile = async (profileId: string, newName: string) => {
    try {
      const list = await api.renameProfile(profileId, newName);
      setProfiles(list);
    } catch (err) {
      setBannerError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleDeleteProfile = async (profileId: string) => {
    try {
      const list = await api.deleteProfile(profileId);
      setProfiles(list);
      const nextSettings = await api.getSettings();
      setSettings(nextSettings);
      await refreshLocalMods();
    } catch (err) {
      setBannerError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleUpdateProfileSettings = async (
    profileId: string,
    profileSettings: ProfileSettings
  ) => {
    const list = await api.updateProfileSettings(profileId, profileSettings);
    setProfiles(list);
  };

  const handleSnapshotProfileVersions = async (profileId: string) => {
    const list = await api.snapshotProfileVersions(profileId);
    setProfiles(list);
  };

  const handleSaveSettings = async (next: AppSettings) => {
    const saved = await api.saveSettings(next);
    setSettings(saved);
    await Promise.all([refreshLocalMods(), refreshProfiles(), refreshConfigs()]);
  };

  const handleSaveConfig = async (relPath: string, content: string) => {
    const updated = await api.saveModConfig(relPath, content);
    setModConfigs((prev) =>
      prev.map((c) => (c.relPath === updated.relPath ? updated : c))
    );
  };

  const handleConfigureMod = (relPath: string) => {
    setSelectedConfigPath(relPath);
    setActiveTab('configs');
  };

  const handleCompleteOnboarding = async (
    nextSettings: AppSettings,
    selectedProfileId: string
  ) => {
    const saved = await api.saveSettings(nextSettings);
    setSettings(saved);
    if (selectedProfileId && selectedProfileId !== saved.activeProfileId) {
      await api.switchProfile(selectedProfileId);
      const refreshed = await api.getSettings();
      setSettings(refreshed);
    }
    setShowOnboarding(false);
    await Promise.all([refreshLocalMods(), refreshProfiles(), refreshConfigs()]);
  };

  const handleSkipOnboarding = async () => {
    const saved = await api.saveSettings({
      ...settings,
      onboardingCompleted: true,
    });
    setSettings(saved);
    setShowOnboarding(false);
  };

  const handleLaunchGame = async () => {
    setLaunching(true);
    setBannerError(null);
    try {
      const status = await api.launchGame();
      setGameStatus(status);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setBannerError(msg);
      if (msg.toLowerCase().includes('not configured') || msg.toLowerCase().includes('not found')) {
        setActiveTab('settings');
      }
    } finally {
      setLaunching(false);
    }
  };

  return (
    <div className="h-screen w-screen flex bg-[#0e1015] text-zinc-100 overflow-hidden">
      <Sidebar
        activeTab={activeTab}
        onSelectTab={setActiveTab}
        enabledCount={enabledCount}
        totalCount={localMods.length}
        gameStatus={gameStatus}
      />

      <div className="flex-1 flex flex-col min-w-0">
        <HeaderBar
          activeTab={activeTab}
          profiles={profiles}
          activeProfileId={settings.activeProfileId}
          onSwitchProfile={handleSwitchProfile}
          onLaunchGame={handleLaunchGame}
          launching={launching}
          gameStatus={gameStatus}
        />

        {bannerError && (
          <div className="animate-slide-up mx-6 mt-3 px-4 py-2.5 rounded-xl bg-rose-500/10 border border-rose-500/25 flex items-center justify-between gap-3 text-xs text-rose-300">
            <div className="flex items-center gap-2 min-w-0">
              <AlertCircle className="w-4 h-4 shrink-0" />
              <span className="truncate">{bannerError}</span>
            </div>
            <button
              onClick={() => setBannerError(null)}
              className="p-1 hover:bg-rose-500/20 rounded cursor-pointer"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        <main key={activeTab} className="animate-fade-in flex-1 min-h-0 flex flex-col">
          {activeTab === 'installed' && (
            <InstalledModsView
              mods={localMods}
              configs={modConfigs}
              loading={loadingMods}
              checkingUpdates={checkingUpdates}
              downloads={downloads}
              onToggleMod={handleToggleMod}
              onDeleteMod={handleDeleteMod}
              onUpdateMod={handleUpdateMod}
              onConfigureMod={handleConfigureMod}
              onCheckUpdates={handleCheckUpdates}
              onOpenFolder={() => api.openModsFolder().catch((e) => setBannerError(String(e)))}
            />
          )}

          {activeTab === 'browse' && (
            <BrowseModsView
              installedMods={localMods}
              downloads={downloads}
              onSelectMod={setSelectedMod}
              onQuickInstall={handleQuickInstall}
            />
          )}

          {activeTab === 'configs' && (
            <ConfigsView
              configs={modConfigs}
              selectedRelPath={selectedConfigPath}
              onSelectConfig={setSelectedConfigPath}
              onSaveConfig={handleSaveConfig}
              onRefresh={refreshConfigs}
              onOpenFolder={() =>
                api.openModConfigsFolder().catch((e) => setBannerError(String(e)))
              }
            />
          )}

          {activeTab === 'profiles' && (
            <ProfilesView
              profiles={profiles}
              localMods={localMods}
              activeProfileId={settings.activeProfileId}
              onSwitchProfile={handleSwitchProfile}
              onCreateProfile={handleCreateProfile}
              onRenameProfile={handleRenameProfile}
              onDeleteProfile={handleDeleteProfile}
              onUpdateProfileSettings={handleUpdateProfileSettings}
              onSnapshotProfileVersions={handleSnapshotProfileVersions}
              onModpackImported={async (res: ModpackImportResult) => {
                setSettings((s) => ({ ...s, activeProfileId: res.profile.id }));
                await Promise.all([refreshLocalMods(), refreshProfiles(), refreshConfigs()]);
              }}
              onError={(msg) => setBannerError(msg)}
            />
          )}

          {activeTab === 'multiplayer' && (
            <MultiplayerView
              activeProfile={
                profiles.find((p) => p.id === settings.activeProfileId) ?? null
              }
              onError={(msg) => setBannerError(msg)}
            />
          )}

          {activeTab === 'settings' && (
            <SettingsView
              settings={settings}
              onSaveSettings={handleSaveSettings}
              onOpenOnboarding={() => setShowOnboarding(true)}
            />
          )}
        </main>

        <footer className="h-7 shrink-0 border-t border-white/[0.04] bg-[#0e1015]/90 px-6 flex items-center justify-between text-[11px] text-zinc-600 select-none">
          <span>Vintage Mod Manager (VMM) • v1.1</span>
          <span>Modpack Sharing &amp; Mesh VPN Multiplayer</span>
        </footer>
      </div>

      <ModDetailDrawer
        modSummary={selectedMod}
        downloads={downloads}
        onClose={() => setSelectedMod(null)}
        onInstallRelease={handleInstallRelease}
      />

      {showOnboarding && (
        <OnboardingModal
          settings={settings}
          onComplete={handleCompleteOnboarding}
          onSkip={handleSkipOnboarding}
        />
      )}
    </div>
  );
}

export default App;
