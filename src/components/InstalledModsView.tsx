import { useMemo, useState } from 'react';
import {
  ArrowUpCircle,
  FolderOpen,
  Package,
  RefreshCw,
  Search,
  SlidersHorizontal,
  Trash2,
} from 'lucide-react';
import { DependencyTooltipBadge } from './DependencyTooltip';
import { ModUpdateChangelogModal } from './ModUpdateChangelogModal';
import {
  evaluateDependenciesAgainstLocal,
  splitDependenciesMap,
} from '../utils/dependencies';
import { scoreSearchFields } from '../utils/search';
import type { DownloadProgressEvent, LocalMod, ModConfigFile } from '../types';

interface InstalledModsViewProps {
  mods: LocalMod[];
  configs: ModConfigFile[];
  loading: boolean;
  checkingUpdates: boolean;
  downloads: Record<string, DownloadProgressEvent>;
  onToggleMod: (mod: LocalMod) => void;
  onDeleteMod: (mod: LocalMod) => void;
  onUpdateMod: (mod: LocalMod) => Promise<void>;
  onConfigureMod: (relPath: string) => void;
  onSelectMod: (mod: LocalMod) => void;
  onCheckUpdates: () => void;
  onOpenFolder: () => void;
}

type FilterMode = 'all' | 'enabled' | 'disabled' | 'updates';

export function InstalledModsView({
  mods,
  configs,
  loading,
  checkingUpdates,
  downloads,
  onToggleMod,
  onDeleteMod,
  onUpdateMod,
  onConfigureMod,
  onSelectMod,
  onCheckUpdates,
  onOpenFolder,
}: InstalledModsViewProps) {
  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState<FilterMode>('all');
  const [pendingUpdateMod, setPendingUpdateMod] = useState<LocalMod | null>(null);

  const updatesCount = useMemo(
    () => mods.filter((m) => m.updateAvailable).length,
    [mods]
  );

  const localModsById = useMemo(() => {
    const map = new Map<string, LocalMod>();
    for (const m of mods) {
      if (m.modid) {
        map.set(m.modid.toLowerCase(), m);
      }
    }
    return map;
  }, [mods]);

  const filtered = useMemo(() => {
    const q = search.trim();
    const base = mods.filter((m) => {
      if (filter === 'enabled' && !m.enabled) return false;
      if (filter === 'disabled' && m.enabled) return false;
      if (filter === 'updates' && !m.updateAvailable) return false;
      return true;
    });
    if (!q) return base;

    return base
      .map((m) => ({
        mod: m,
        score: scoreSearchFields(q, [
          { text: m.name, weight: 3 },
          { text: m.modid, weight: 2 },
          { text: m.authors.join(' '), weight: 1 },
          { text: m.description ?? '', weight: 0.5, allowFuzzy: false },
        ]),
      }))
      .filter((entry) => entry.score > 0)
      .sort((a, b) => b.score - a.score)
      .map((entry) => entry.mod);
  }, [mods, search, filter]);

  return (
    <div className="flex flex-col h-full">
      <div className="px-6 py-4 border-b border-white/[0.05] flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3 flex-1 min-w-[240px]">
          <div className="relative flex-1 max-w-xs">
            <Search className="w-3.5 h-3.5 text-zinc-500 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Filter installed mods..."
              className="w-full bg-[#161922] border border-white/[0.07] rounded-lg pl-8 pr-3 py-1.5 text-xs text-zinc-200 placeholder:text-zinc-500 focus:outline-none focus:border-amber-500/40"
            />
          </div>

          <div className="flex items-center bg-[#161922] border border-white/[0.07] rounded-lg p-0.5 text-xs">
            {(
              [
                ['all', 'All'],
                ['enabled', 'Enabled'],
                ['disabled', 'Disabled'],
                ['updates', `Updates${updatesCount ? ` (${updatesCount})` : ''}`],
              ] as [FilterMode, string][]
            ).map(([key, label]) => (
              <button
                key={key}
                onClick={() => setFilter(key)}
                className={`px-2.5 py-1 rounded-md font-medium transition-colors cursor-pointer ${
                  filter === key
                    ? 'bg-white/[0.08] text-zinc-100'
                    : 'text-zinc-500 hover:text-zinc-300'
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={onCheckUpdates}
            disabled={checkingUpdates || mods.length === 0}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[#161922] hover:bg-white/[0.06] border border-white/[0.07] text-xs font-medium text-zinc-300 transition-colors cursor-pointer disabled:opacity-50"
          >
            <RefreshCw
              className={`w-3.5 h-3.5 ${checkingUpdates ? 'animate-spin text-amber-400' : ''}`}
            />
            <span>{checkingUpdates ? 'Checking...' : 'Check Updates'}</span>
          </button>

          <button
            onClick={onOpenFolder}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[#161922] hover:bg-white/[0.06] border border-white/[0.07] text-xs font-medium text-zinc-300 transition-colors cursor-pointer"
          >
            <FolderOpen className="w-3.5 h-3.5" />
            <span>Mods Folder</span>
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-6">
        {loading ? (
          <div className="flex items-center justify-center h-48 text-xs text-zinc-500">
            Scanning Mods directory...
          </div>
        ) : filtered.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-56 text-center border border-dashed border-white/[0.07] rounded-2xl bg-white/[0.01]">
            <Package className="w-8 h-8 text-zinc-600 mb-2" />
            <p className="text-sm font-medium text-zinc-300">
              {mods.length === 0 ? 'No mods installed yet' : 'No matching mods'}
            </p>
            <p className="text-xs text-zinc-500 mt-1">
              {mods.length === 0
                ? 'Head over to Mod DB to download your first mod.'
                : 'Try adjusting your search or filter.'}
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            {filtered.map((mod, idx) => {
              const dl = downloads[mod.modid];
              const cleanModId = mod.modid.toLowerCase().replace(/[^a-z0-9]/g, '');
              const matchedConfig = configs.find((c) => {
                const base = c.filename.toLowerCase().replace(/[^a-z0-9]/g, '');
                return cleanModId.length >= 3 && base.includes(cleanModId);
              });
              const { modDependencies, gameRequirements } = splitDependenciesMap(
                mod.dependencies
              );
              const evaluatedDeps = evaluateDependenciesAgainstLocal(
                modDependencies,
                gameRequirements,
                localModsById
              );
              return (
                <div
                  key={`${mod.enabled ? 'e' : 'd'}-${mod.filename}`}
                  onClick={() => onSelectMod(mod)}
                  style={{ animationDelay: `${Math.min(idx * 20, 220)}ms` }}
                  className={`animate-slide-up group flex items-center justify-between gap-4 p-3.5 rounded-xl border transition-all duration-150 cursor-pointer ${
                    mod.enabled
                      ? 'bg-[#141720] border-white/[0.06] hover:border-white/[0.14] hover:bg-[#171b26]'
                      : 'bg-[#11131a]/60 border-white/[0.04] opacity-70 hover:opacity-100'
                  }`}
                >
                  <div className="flex items-center gap-3.5 min-w-0">
                    {mod.iconBase64 ? (
                      <img
                        src={mod.iconBase64}
                        alt={mod.name}
                        className="w-10 h-10 rounded-lg object-cover bg-zinc-900 shrink-0 border border-white/[0.06]"
                      />
                    ) : (
                      <div className="w-10 h-10 rounded-lg bg-white/[0.04] border border-white/[0.06] flex items-center justify-center text-zinc-500 shrink-0">
                        <Package className="w-4 h-4" />
                      </div>
                    )}

                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-zinc-100 group-hover:text-amber-200 transition-colors truncate">
                          {mod.name}
                        </span>
                        <span className="text-[11px] font-mono px-1.5 py-0.5 rounded bg-white/[0.05] text-zinc-400">
                          v{mod.version}
                        </span>
                        <DependencyTooltipBadge
                          evaluated={evaluatedDeps}
                          variant="local"
                        />
                        {mod.updateAvailable && mod.latestVersion && (
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              setPendingUpdateMod(mod);
                            }}
                            disabled={Boolean(dl && !dl.done)}
                            title="View changelog and update"
                            className="flex items-center gap-1 text-[11px] font-medium px-2 py-0.5 rounded-full bg-amber-500/15 text-amber-300 border border-amber-500/30 hover:bg-amber-500/25 transition-colors cursor-pointer"
                          >
                            <ArrowUpCircle className="w-3 h-3" />
                            <span>
                              {dl && !dl.done
                                ? 'Updating...'
                                : `Update to v${mod.latestVersion}`}
                            </span>
                          </button>
                        )}
                      </div>

                      <div className="flex items-center gap-2 text-xs text-zinc-500 mt-0.5 truncate">
                        {mod.authors.length > 0 && (
                          <span>by {mod.authors.join(', ')}</span>
                        )}
                        {mod.authors.length > 0 && mod.description && <span>•</span>}
                        <span className="truncate">{mod.description || mod.filename}</span>
                      </div>
                    </div>
                  </div>

                  <div
                    className="flex items-center gap-3 shrink-0"
                    onClick={(e) => e.stopPropagation()}
                  >
                    {matchedConfig && (
                      <button
                        onClick={() => onConfigureMod(matchedConfig.relPath)}
                        title={`Configure ${matchedConfig.filename}`}
                        className="flex items-center gap-1 px-2 py-1 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.06] text-[11px] font-medium text-zinc-300 transition-colors cursor-pointer"
                      >
                        <SlidersHorizontal className="w-3 h-3 text-amber-400" />
                        <span>Config</span>
                      </button>
                    )}

                    <button
                      onClick={() => onToggleMod(mod)}
                      role="switch"
                      aria-checked={mod.enabled}
                      title={
                        mod.enabled
                          ? 'Disable mod (move to ModsDisabled)'
                          : 'Enable mod (move to Mods)'
                      }
                      className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ${
                        mod.enabled ? 'bg-amber-500' : 'bg-zinc-700'
                      }`}
                    >
                      <span
                        className={`pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow transition duration-200 ${
                          mod.enabled ? 'translate-x-4' : 'translate-x-0'
                        }`}
                      />
                    </button>

                    <button
                      onClick={() => onDeleteMod(mod)}
                      title="Delete mod zip"
                      className="p-1.5 rounded-lg text-zinc-500 hover:text-rose-400 hover:bg-rose-500/10 transition-colors cursor-pointer opacity-0 group-hover:opacity-100"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {pendingUpdateMod && (
        <ModUpdateChangelogModal
          mod={pendingUpdateMod}
          onClose={() => setPendingUpdateMod(null)}
          onConfirmUpdate={onUpdateMod}
        />
      )}
    </div>
  );
}
