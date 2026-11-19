import { useState } from 'react';
import {
  Check,
  Copy,
  Download,
  Layers,
  Pencil,
  Plus,
  RefreshCw,
  Share2,
  SlidersHorizontal,
  Trash2,
} from 'lucide-react';
import { api } from '../api/tauri';
import { ExportModpackModal, ImportModpackModal } from './ModpackModals';
import { ProfileSettingsDrawer } from './ProfileSettingsDrawer';
import type {
  LocalMod,
  ModProfile,
  ModpackImportResult,
  ProfileSettings,
} from '../types';

interface ProfilesViewProps {
  profiles: ModProfile[];
  localMods: LocalMod[];
  activeProfileId: string | null;
  onSwitchProfile: (profileId: string) => void;
  onCreateProfile: (name: string, cloneCurrent: boolean) => Promise<void>;
  onRenameProfile: (profileId: string, newName: string) => Promise<void>;
  onDeleteProfile: (profileId: string) => Promise<void>;
  onUpdateProfileSettings: (
    profileId: string,
    settings: ProfileSettings
  ) => Promise<void>;
  onSnapshotProfileVersions: (profileId: string) => Promise<void>;
  onModpackImported: (result: ModpackImportResult) => Promise<void>;
  onError: (msg: string) => void;
}

export function ProfilesView({
  profiles,
  localMods,
  activeProfileId,
  onSwitchProfile,
  onCreateProfile,
  onRenameProfile,
  onDeleteProfile,
  onUpdateProfileSettings,
  onSnapshotProfileVersions,
  onModpackImported,
  onError,
}: ProfilesViewProps) {
  const [newName, setNewName] = useState('');
  const [cloneCurrent, setCloneCurrent] = useState(true);
  const [creating, setCreating] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');
  const [configuringId, setConfiguringId] = useState<string | null>(null);
  const [sharingProfile, setSharingProfile] = useState<ModProfile | null>(null);
  const [showImport, setShowImport] = useState(false);
  const [syncingId, setSyncingId] = useState<string | null>(null);

  const handleSyncFromUrl = async (profile: ModProfile) => {
    const url = profile.settings.sourceUrl;
    if (!url || syncingId) return;
    setSyncingId(profile.id);
    try {
      const res = await api.applyModpackImport({
        source: url,
        profileName: `${profile.name} (Synced)`,
        restoreConfigs: false,
        expectedManifestHash: null,
      });
      await onModpackImported(res);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setSyncingId(null);
    }
  };

  const configuringProfile =
    profiles.find((p) => p.id === configuringId) ?? null;

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newName.trim() || creating) return;
    setCreating(true);
    try {
      await onCreateProfile(newName.trim(), cloneCurrent);
      setNewName('');
    } finally {
      setCreating(false);
    }
  };

  const handleRenameSubmit = async (id: string) => {
    if (!editName.trim()) {
      setEditingId(null);
      return;
    }
    await onRenameProfile(id, editName.trim());
    setEditingId(null);
  };

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-6 max-w-4xl">
      <form
        onSubmit={handleCreate}
        className="p-4 rounded-2xl bg-[#141720] border border-white/[0.06] flex flex-wrap items-center gap-3"
      >
        <input
          type="text"
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          placeholder="New profile name (e.g. Homestead Server)..."
          className="flex-1 min-w-[200px] bg-[#0e1015] border border-white/[0.07] rounded-lg px-3 py-2 text-xs text-zinc-200 placeholder:text-zinc-500 focus:outline-none focus:border-amber-500/40"
        />

        <label className="flex items-center gap-2 text-xs text-zinc-400 cursor-pointer select-none">
          <input
            type="checkbox"
            checked={cloneCurrent}
            onChange={(e) => setCloneCurrent(e.target.checked)}
            className="accent-amber-500 rounded"
          />
          <Copy className="w-3.5 h-3.5 text-zinc-500" />
          <span>Snapshot enabled mods</span>
        </label>

        <button
          type="submit"
          disabled={!newName.trim() || creating}
          className="flex items-center gap-1.5 px-3.5 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-zinc-950 text-xs font-semibold cursor-pointer"
        >
          <Plus className="w-3.5 h-3.5" />
          <span>Create Profile</span>
        </button>

        <button
          type="button"
          onClick={() => setShowImport(true)}
          className="flex items-center gap-1.5 px-3.5 py-2 rounded-lg bg-white/[0.06] hover:bg-white/[0.1] text-zinc-200 text-xs font-medium cursor-pointer"
        >
          <Download className="w-3.5 h-3.5" />
          <span>Import Modpack</span>
        </button>
      </form>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-3.5">
        {profiles.map((profile) => {
          const isActive = profile.id === activeProfileId;
          const isEditing = editingId === profile.id;

          return (
            <div
              key={profile.id}
              className={`p-4 rounded-2xl border transition-all duration-200 hover:-translate-y-0.5 flex flex-col justify-between gap-4 ${
                isActive
                  ? 'bg-amber-500/[0.06] border-amber-500/35 shadow-sm shadow-amber-500/10'
                  : 'bg-[#141720] border-white/[0.06] hover:border-white/[0.14]'
              }`}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-center gap-3 min-w-0">
                  <div
                    className={`w-9 h-9 rounded-xl flex items-center justify-center shrink-0 ${
                      isActive
                        ? 'bg-amber-500/20 text-amber-300'
                        : 'bg-white/[0.04] text-zinc-400'
                    }`}
                  >
                    <Layers className="w-4 h-4" />
                  </div>

                  <div className="min-w-0">
                    {isEditing ? (
                      <input
                        autoFocus
                        type="text"
                        value={editName}
                        onChange={(e) => setEditName(e.target.value)}
                        onBlur={() => handleRenameSubmit(profile.id)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') handleRenameSubmit(profile.id);
                          if (e.key === 'Escape') setEditingId(null);
                        }}
                        className="bg-[#0e1015] border border-amber-500/40 rounded px-2 py-0.5 text-sm text-zinc-100 focus:outline-none"
                      />
                    ) : (
                      <div className="flex items-center gap-2">
                        <h3 className="text-sm font-semibold text-zinc-100 truncate">
                          {profile.name}
                        </h3>
                        {profile.isBuiltin && (
                          <span className="text-[10px] px-1.5 py-0.5 rounded bg-white/[0.05] text-zinc-400">
                            Built-in
                          </span>
                        )}
                      </div>
                    )}
                    <p className="text-xs text-zinc-500 mt-0.5">
                      {profile.id === 'vanilla'
                        ? 'All mods moved to ModsDisabled'
                        : `${profile.enabledModids.length} enabled mod${
                            profile.enabledModids.length === 1 ? '' : 's'
                          }`}
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-1">
                  {profile.settings.sourceUrl && (
                    <button
                      onClick={() => handleSyncFromUrl(profile)}
                      disabled={syncingId === profile.id}
                      title="Sync latest modpack from GitHub URL"
                      className="p-1.5 rounded-lg text-emerald-400 hover:bg-emerald-500/10 cursor-pointer disabled:opacity-50"
                    >
                      <RefreshCw
                        className={`w-3.5 h-3.5 ${
                          syncingId === profile.id ? 'animate-spin' : ''
                        }`}
                      />
                    </button>
                  )}
                  <button
                    onClick={() => setSharingProfile(profile)}
                    title="Share / Export Modpack"
                    className="p-1.5 rounded-lg text-zinc-400 hover:text-amber-300 hover:bg-amber-500/10 cursor-pointer"
                  >
                    <Share2 className="w-3.5 h-3.5" />
                  </button>
                  <button
                    onClick={() => setConfiguringId(profile.id)}
                    title="Profile settings (RAM, version control, launch overrides)"
                    className="p-1.5 rounded-lg text-zinc-400 hover:text-amber-300 hover:bg-amber-500/10 cursor-pointer"
                  >
                    <SlidersHorizontal className="w-3.5 h-3.5" />
                  </button>
                  {!profile.isBuiltin && (
                    <>
                      <button
                        onClick={() => {
                          setEditingId(profile.id);
                          setEditName(profile.name);
                        }}
                        title="Rename profile"
                        className="p-1.5 rounded-lg text-zinc-500 hover:text-zinc-200 hover:bg-white/[0.06] cursor-pointer"
                      >
                        <Pencil className="w-3.5 h-3.5" />
                      </button>
                      <button
                        onClick={() => onDeleteProfile(profile.id)}
                        title="Delete profile"
                        className="p-1.5 rounded-lg text-zinc-500 hover:text-rose-400 hover:bg-rose-500/10 cursor-pointer"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </>
                  )}
                </div>
              </div>

              {(profile.settings.gameVersion ||
                profile.settings.maxRamMb ||
                Object.keys(profile.settings.pinnedModVersions ?? {}).length > 0) && (
                <div className="flex flex-wrap items-center gap-1.5">
                  {profile.settings.gameVersion && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-white/[0.05] text-zinc-300 font-mono">
                      VS {profile.settings.gameVersion}
                    </span>
                  )}
                  {profile.settings.maxRamMb && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-white/[0.05] text-zinc-300 font-mono">
                      {Math.round(profile.settings.maxRamMb / 1024)} GB RAM
                    </span>
                  )}
                  {Object.keys(profile.settings.pinnedModVersions ?? {}).length > 0 && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/12 text-amber-300">
                      {Object.keys(profile.settings.pinnedModVersions).length} pinned
                    </span>
                  )}
                </div>
              )}

              <div className="flex items-center justify-end">
                {isActive ? (
                  <span className="flex items-center gap-1.5 text-xs font-medium text-amber-300">
                    <Check className="w-3.5 h-3.5" />
                    Active Profile
                  </span>
                ) : (
                  <button
                    onClick={() => onSwitchProfile(profile.id)}
                    className="px-3 py-1.5 rounded-lg bg-white/[0.06] hover:bg-white/[0.1] text-xs font-medium text-zinc-200 transition-colors cursor-pointer"
                  >
                    Activate
                  </button>
                )}
              </div>
            </div>
          );
        })}
      </div>

      <ProfileSettingsDrawer
        profile={configuringProfile}
        localMods={localMods}
        onClose={() => setConfiguringId(null)}
        onSave={onUpdateProfileSettings}
        onSnapshotVersions={onSnapshotProfileVersions}
      />

      {sharingProfile && (
        <ExportModpackModal
          profile={sharingProfile}
          onClose={() => setSharingProfile(null)}
          onError={onError}
        />
      )}

      {showImport && (
        <ImportModpackModal
          onClose={() => setShowImport(false)}
          onImported={onModpackImported}
          onError={onError}
        />
      )}
    </div>
  );
}
