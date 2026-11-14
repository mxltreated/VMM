import { useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  Camera,
  Check,
  Cpu,
  FolderOpen,
  GitBranch,
  Layers,
  Terminal,
  X,
} from 'lucide-react';
import type { LocalMod, ModProfile, ProfileSettings } from '../types';

interface ProfileSettingsDrawerProps {
  profile: ModProfile | null;
  localMods: LocalMod[];
  onClose: () => void;
  onSave: (profileId: string, settings: ProfileSettings) => Promise<void>;
  onSnapshotVersions: (profileId: string) => Promise<void>;
}

const RAM_PRESETS: { label: string; mb: number | null }[] = [
  { label: 'Auto', mb: null },
  { label: '4 GB', mb: 4096 },
  { label: '6 GB', mb: 6144 },
  { label: '8 GB', mb: 8192 },
  { label: '12 GB', mb: 12288 },
  { label: '16 GB', mb: 16384 },
];

export function ProfileSettingsDrawer({
  profile,
  localMods,
  onClose,
  onSave,
  onSnapshotVersions,
}: ProfileSettingsDrawerProps) {
  const [gameVersion, setGameVersion] = useState('');
  const [exePathOverride, setExePathOverride] = useState('');
  const [maxRamMb, setMaxRamMb] = useState<number | null>(null);
  const [launchArgsOverride, setLaunchArgsOverride] = useState('');
  const [notes, setNotes] = useState('');
  const [saving, setSaving] = useState(false);
  const [savedToast, setSavedToast] = useState(false);
  const [snapshotting, setSnapshotting] = useState(false);

  useEffect(() => {
    if (!profile) return;
    setGameVersion(profile.settings.gameVersion ?? '');
    setExePathOverride(profile.settings.exePathOverride ?? '');
    setMaxRamMb(profile.settings.maxRamMb ?? null);
    setLaunchArgsOverride(profile.settings.launchArgsOverride ?? '');
    setNotes(profile.settings.notes ?? '');
  }, [profile]);

  if (!profile) return null;

  const pickExe = async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      title: `Select Vintagestory.exe for ${profile.name}`,
      filters: [{ name: 'Executable', extensions: ['exe'] }],
    });
    if (typeof selected === 'string') {
      setExePathOverride(selected);
    }
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      await onSave(profile.id, {
        gameVersion: gameVersion.trim() || null,
        exePathOverride: exePathOverride.trim() || null,
        maxRamMb: maxRamMb && maxRamMb > 0 ? maxRamMb : null,
        launchArgsOverride: launchArgsOverride.trim() || null,
        notes: notes.trim() || null,
        pinnedModVersions: profile.settings.pinnedModVersions ?? {},
      });
      setSavedToast(true);
      setTimeout(() => setSavedToast(false), 1800);
    } finally {
      setSaving(false);
    }
  };

  const handleSnapshot = async () => {
    setSnapshotting(true);
    try {
      await onSnapshotVersions(profile.id);
    } finally {
      setSnapshotting(false);
    }
  };

  const pinnedEntries = Object.entries(profile.settings.pinnedModVersions ?? {});

  return (
    <div className="animate-fade-in fixed inset-0 z-40 flex justify-end bg-black/55 backdrop-blur-xs">
      <div className="animate-slide-right w-full max-w-lg bg-[#12151c] border-l border-white/[0.08] h-full flex flex-col shadow-2xl">
        <div className="px-6 py-4 border-b border-white/[0.06] flex items-center justify-between">
          <div className="flex items-center gap-3 min-w-0">
            <div className="w-9 h-9 rounded-xl bg-amber-500/15 text-amber-300 flex items-center justify-center shrink-0">
              <Layers className="w-4 h-4" />
            </div>
            <div className="min-w-0">
              <h2 className="text-sm font-semibold text-zinc-100 truncate">
                {profile.name} — Profile Settings
              </h2>
              <p className="text-xs text-zinc-500">
                Bespoke version control, RAM cap &amp; launch overrides
              </p>
            </div>
          </div>

          <button
            onClick={onClose}
            className="p-1.5 rounded-lg text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.06] cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <form onSubmit={handleSave} className="flex-1 overflow-y-auto p-6 space-y-6">
          {/* RAM Allocation */}
          <section className="space-y-3">
            <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-zinc-400">
              <Cpu className="w-3.5 h-3.5 text-amber-400" />
              <span>RAM Allocation (.NET Heap Limit)</span>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {RAM_PRESETS.map((preset) => {
                const active = maxRamMb === preset.mb;
                return (
                  <button
                    key={preset.label}
                    type="button"
                    onClick={() => setMaxRamMb(preset.mb)}
                    className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
                      active
                        ? 'bg-amber-500 text-zinc-950 font-semibold'
                        : 'bg-[#161922] text-zinc-300 border border-white/[0.07] hover:bg-white/[0.06]'
                    }`}
                  >
                    {preset.label}
                  </button>
                );
              })}
            </div>
            <div className="flex items-center gap-2">
              <input
                type="number"
                min={512}
                step={512}
                value={maxRamMb ?? ''}
                onChange={(e) => {
                  const v = e.target.value;
                  setMaxRamMb(v ? Number(v) : null);
                }}
                placeholder="Auto (system managed)"
                className="w-48 bg-[#0e1015] border border-white/[0.07] rounded-lg px-3 py-1.5 text-xs font-mono text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-amber-500/40"
              />
              <span className="text-xs text-zinc-500">MB (DOTNET_GCHeapHardLimit)</span>
            </div>
          </section>

          {/* Version Control */}
          <section className="space-y-3 pt-4 border-t border-white/[0.06]">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-zinc-400">
                <GitBranch className="w-3.5 h-3.5 text-amber-400" />
                <span>Version Control</span>
              </div>
              <button
                type="button"
                onClick={handleSnapshot}
                disabled={snapshotting}
                className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white/[0.05] hover:bg-white/[0.09] text-[11px] font-medium text-zinc-200 cursor-pointer"
              >
                <Camera className="w-3 h-3 text-amber-400" />
                <span>{snapshotting ? 'Snapshotting...' : 'Snapshot Mod Versions'}</span>
              </button>
            </div>

            <div className="space-y-1.5">
              <label className="text-xs font-medium text-zinc-300">
                Target Game Version
              </label>
              <input
                type="text"
                value={gameVersion}
                onChange={(e) => setGameVersion(e.target.value)}
                placeholder="e.g. 1.22.7"
                className="w-full bg-[#0e1015] border border-white/[0.07] rounded-lg px-3 py-2 text-xs font-mono text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-amber-500/40"
              />
            </div>

            <div className="space-y-1.5">
              <label className="text-xs font-medium text-zinc-300">
                Profile Notes / Pack Version
              </label>
              <textarea
                rows={2}
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                placeholder="e.g. v1.2 — Added Rivers + RealSmoke tweaks"
                className="w-full bg-[#0e1015] border border-white/[0.07] rounded-lg px-3 py-2 text-xs text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-amber-500/40 resize-none"
              />
            </div>

            {pinnedEntries.length > 0 && (
              <div className="rounded-xl bg-[#161922] border border-white/[0.06] p-3 space-y-1.5 max-h-40 overflow-y-auto">
                <div className="text-[11px] font-medium text-zinc-400">
                  Pinned Mod Versions ({pinnedEntries.length})
                </div>
                {pinnedEntries.map(([modid, pinnedVer]) => {
                  const installed = localMods.find(
                    (m) => m.modid.toLowerCase() === modid.toLowerCase()
                  );
                  const drifted = installed && installed.version !== pinnedVer;
                  return (
                    <div
                      key={modid}
                      className="flex items-center justify-between text-[11px] font-mono"
                    >
                      <span className="text-zinc-300 truncate">{modid}</span>
                      <span className={drifted ? 'text-amber-300' : 'text-zinc-500'}>
                        v{pinnedVer}
                        {drifted ? ` (now v${installed.version})` : ''}
                      </span>
                    </div>
                  );
                })}
              </div>
            )}
          </section>

          {/* Executable & Launch Overrides */}
          <section className="space-y-3 pt-4 border-t border-white/[0.06]">
            <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-zinc-400">
              <Terminal className="w-3.5 h-3.5 text-amber-400" />
              <span>Executable &amp; Launch Overrides</span>
            </div>

            <div className="space-y-1.5">
              <label className="text-xs font-medium text-zinc-300">
                Custom Vintagestory.exe (optional)
              </label>
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  value={exePathOverride}
                  onChange={(e) => setExePathOverride(e.target.value)}
                  placeholder="Uses global Settings executable if blank"
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
                Profile Launch Arguments (optional)
              </label>
              <input
                type="text"
                value={launchArgsOverride}
                onChange={(e) => setLaunchArgsOverride(e.target.value)}
                placeholder='--openWorld "Homestead"'
                className="w-full bg-[#0e1015] border border-white/[0.07] rounded-lg px-3 py-2 text-xs font-mono text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-amber-500/40"
              />
            </div>
          </section>

          <div className="pt-4 border-t border-white/[0.06] flex items-center justify-end gap-3">
            {savedToast && (
              <span className="animate-scale-in flex items-center gap-1 text-xs text-emerald-400">
                <Check className="w-3.5 h-3.5" />
                Saved
              </span>
            )}
            <button
              type="submit"
              disabled={saving}
              className="px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 text-zinc-950 text-xs font-semibold active:scale-[0.98] cursor-pointer disabled:opacity-50"
            >
              {saving ? 'Saving...' : 'Save Profile Settings'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
