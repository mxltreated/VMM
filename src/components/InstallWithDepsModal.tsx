import { useEffect, useMemo, useState } from 'react';
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  Download,
  GitBranch,
  Loader2,
  Package,
  PauseCircle,
  X,
} from 'lucide-react';
import { formatVersionRequirement } from '../utils/dependencies';
import type {
  BatchDownloadItem,
  DownloadProgressEvent,
  EnableModItem,
  ModInstallPlan,
} from '../types';

interface InstallWithDepsModalProps {
  plan: ModInstallPlan;
  downloads: Record<string, DownloadProgressEvent>;
  onClose: () => void;
  onConfirmInstall: (
    items: BatchDownloadItem[],
    enableMods: EnableModItem[]
  ) => Promise<void>;
}

export function InstallWithDepsModal({
  plan,
  downloads,
  onClose,
  onConfirmInstall,
}: InstallWithDepsModalProps) {
  const [selectedDownloadIds, setSelectedDownloadIds] = useState<Set<string>>(
    () =>
      new Set(
        plan.dependencies
          .filter((d) => d.status === 'download')
          .map((d) => d.modid.toLowerCase())
      )
  );
  const [selectedEnableIds, setSelectedEnableIds] = useState<Set<string>>(
    () =>
      new Set(
        plan.dependencies
          .filter((d) => d.status === 'disabled')
          .map((d) => d.modid.toLowerCase())
      )
  );
  const [installing, setInstalling] = useState(false);

  useEffect(() => {
    setSelectedDownloadIds(
      new Set(
        plan.dependencies
          .filter((d) => d.status === 'download')
          .map((d) => d.modid.toLowerCase())
      )
    );
    setSelectedEnableIds(
      new Set(
        plan.dependencies
          .filter((d) => d.status === 'disabled')
          .map((d) => d.modid.toLowerCase())
      )
    );
  }, [plan]);

  const toggleDownload = (modid: string) => {
    if (installing) return;
    const key = modid.toLowerCase();
    setSelectedDownloadIds((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const toggleEnable = (modid: string) => {
    if (installing) return;
    const key = modid.toLowerCase();
    setSelectedEnableIds((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const downloadItems = useMemo<BatchDownloadItem[]>(() => {
    const items: BatchDownloadItem[] = [
      {
        modid: plan.primaryModid,
        url: plan.primaryUrl,
        filename: plan.primaryFilename,
        replaceOldFilename: plan.primaryReplaceOldFilename,
      },
    ];
    for (const dep of plan.dependencies) {
      if (
        dep.status === 'download' &&
        selectedDownloadIds.has(dep.modid.toLowerCase()) &&
        dep.downloadUrl &&
        dep.filename
      ) {
        items.push({
          modid: dep.modid,
          url: dep.downloadUrl,
          filename: dep.filename,
          replaceOldFilename: dep.replaceOldFilename,
        });
      }
    }
    return items;
  }, [plan, selectedDownloadIds]);

  const enableMods = useMemo<EnableModItem[]>(() => {
    const list: EnableModItem[] = [];
    for (const dep of plan.dependencies) {
      if (
        dep.status === 'disabled' &&
        selectedEnableIds.has(dep.modid.toLowerCase()) &&
        dep.installedFilename
      ) {
        list.push({
          modid: dep.modid,
          filename: dep.installedFilename,
        });
      }
    }
    return list;
  }, [plan, selectedEnableIds]);

  const gameVersion = plan.gameRequirements.game || plan.gameRequirements.Game;

  const handleConfirm = async () => {
    setInstalling(true);
    try {
      await onConfirmInstall(downloadItems, enableMods);
      onClose();
    } finally {
      setInstalling(false);
    }
  };

  const renderProgressBar = (modid: string) => {
    const dl = downloads[modid];
    if (!dl) return null;
    const pct = dl.totalBytes
      ? Math.min(100, Math.round((dl.bytesReceived / dl.totalBytes) * 100))
      : null;

    return (
      <div className="mt-2 space-y-1">
        <div className="h-1.5 w-full rounded-full bg-white/[0.06] overflow-hidden">
          <div
            className={`h-full transition-all duration-150 ${
              dl.error
                ? 'bg-rose-500'
                : dl.done
                  ? 'bg-emerald-500'
                  : 'bg-amber-400'
            }`}
            style={{ width: `${pct ?? (dl.done ? 100 : 35)}%` }}
          />
        </div>
        <div className="flex items-center justify-between text-[10px] text-zinc-400">
          <span className="truncate">
            {dl.error
              ? `Error: ${dl.error}`
              : dl.done
                ? 'Downloaded & verified'
                : 'Downloading...'}
          </span>
          {pct !== null && <span className="font-mono tabular-nums">{pct}%</span>}
        </div>
      </div>
    );
  };

  const totalActions = downloadItems.length + enableMods.length;

  return (
    <div
      onClick={() => {
        if (!installing) onClose();
      }}
      className="animate-fade-in fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="animate-scale-in w-full max-w-lg rounded-2xl bg-[#141720] border border-white/[0.08] shadow-2xl overflow-hidden flex flex-col max-h-[85vh]"
      >
        {/* Header */}
        <div className="px-5 py-4 border-b border-white/[0.06] flex items-center justify-between gap-3">
          <div className="flex items-center gap-2.5 min-w-0">
            <GitBranch className="w-4 h-4 text-amber-400 shrink-0" />
            <div className="min-w-0">
              <h2 className="text-sm font-semibold text-zinc-100 truncate">
                Dependencies Detected
              </h2>
              <p className="text-[11px] text-zinc-400 truncate">
                {plan.primaryName} requires additional mod(s) to run properly
              </p>
            </div>
          </div>
          <button
            type="button"
            disabled={installing}
            onClick={onClose}
            className="text-zinc-500 hover:text-zinc-200 disabled:opacity-40 cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body */}
        <div className="p-5 overflow-y-auto space-y-4 text-xs">
          {/* Primary Mod Card */}
          <div className="p-3.5 rounded-xl bg-[#0e1015] border border-amber-500/25">
            <div className="flex items-center justify-between gap-2">
              <div className="flex items-center gap-2.5 min-w-0">
                <div className="w-8 h-8 rounded-lg bg-amber-500/15 border border-amber-500/30 flex items-center justify-center text-amber-300 shrink-0">
                  <Package className="w-4 h-4" />
                </div>
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-semibold text-zinc-100 truncate">
                      {plan.primaryName}
                    </span>
                    {plan.primaryVersion && (
                      <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-white/[0.06] text-zinc-300">
                        v{plan.primaryVersion}
                      </span>
                    )}
                  </div>
                  <p className="text-[11px] text-zinc-400 truncate">
                    Primary mod • {plan.primaryFilename}
                  </p>
                </div>
              </div>

              {gameVersion && (
                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-white/[0.05] border border-white/[0.07] text-zinc-300 shrink-0">
                  VS {gameVersion}
                </span>
              )}
            </div>
            {installing && renderProgressBar(plan.primaryModid)}
          </div>

          {/* Unresolved Warnings */}
          {plan.unresolvedWarnings.length > 0 && (
            <div className="p-3 rounded-xl bg-rose-500/10 border border-rose-500/25 space-y-1 text-rose-300">
              <div className="flex items-center gap-1.5 font-semibold">
                <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
                <span>Some dependencies could not be auto-resolved</span>
              </div>
              <ul className="list-disc pl-5 space-y-0.5 text-[11px]">
                {plan.unresolvedWarnings.map((w, i) => (
                  <li key={i}>{w}</li>
                ))}
              </ul>
            </div>
          )}

          {/* Dependencies List */}
          <div className="space-y-2">
            <div className="flex items-center justify-between text-[11px] font-medium text-zinc-400 uppercase tracking-wider">
              <span>Resolved Dependencies ({plan.dependencies.length})</span>
              <span>Simultaneous Install</span>
            </div>

            <div className="space-y-2">
              {plan.dependencies.map((dep) => {
                const lower = dep.modid.toLowerCase();
                const isDownloadable = dep.status === 'download';
                const isDisabled = dep.status === 'disabled';
                const isChecked = isDownloadable
                  ? selectedDownloadIds.has(lower)
                  : isDisabled
                    ? selectedEnableIds.has(lower)
                    : false;

                const requiredByExtra = dep.requiredBy.filter(
                  (r) => r !== plan.primaryName
                );

                return (
                  <div
                    key={dep.modid}
                    onClick={() => {
                      if (isDownloadable) toggleDownload(dep.modid);
                      else if (isDisabled) toggleEnable(dep.modid);
                    }}
                    className={`p-3 rounded-xl border transition-colors ${
                      isDownloadable || isDisabled
                        ? installing
                          ? 'bg-[#0e1015] border-white/[0.06]'
                          : 'bg-[#0e1015] border-white/[0.06] hover:border-amber-500/35 cursor-pointer'
                        : 'bg-[#0e1015]/50 border-white/[0.04]'
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="flex items-start gap-2.5 min-w-0">
                        {(isDownloadable || isDisabled) && (
                          <span
                            className={`mt-0.5 w-4 h-4 rounded border flex items-center justify-center shrink-0 transition-colors ${
                              isChecked
                                ? 'bg-amber-500 border-amber-500 text-zinc-950'
                                : 'bg-transparent border-zinc-600'
                            }`}
                          >
                            {isChecked && <Check className="w-3 h-3 stroke-[3]" />}
                          </span>
                        )}

                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-1.5">
                            <span className="font-medium text-zinc-100 truncate">
                              {dep.name}
                            </span>
                            <span className="text-[10px] font-mono text-zinc-400">
                              {dep.modid}
                            </span>
                          </div>
                          <div className="text-[11px] text-zinc-400 mt-0.5">
                            Requires {formatVersionRequirement(dep.versionRequirement)}
                            {dep.resolvedVersion && dep.status === 'download' && (
                              <> → installing v{dep.resolvedVersion}</>
                            )}
                            {dep.installedVersion && dep.status !== 'download' && (
                              <> • local v{dep.installedVersion}</>
                            )}
                            {dep.installedVersion && dep.status === 'download' && (
                              <> (updating from v{dep.installedVersion})</>
                            )}
                            {requiredByExtra.length > 0 && (
                              <span className="text-zinc-500">
                                {' '}
                                • via {requiredByExtra.join(', ')}
                              </span>
                            )}
                          </div>
                        </div>
                      </div>

                      {dep.status === 'download' ? (
                        <span className="inline-flex items-center gap-1 text-[10px] font-medium px-2 py-0.5 rounded-full bg-amber-500/15 text-amber-300 border border-amber-500/25 shrink-0">
                          <Download className="w-2.5 h-2.5" />
                          <span>Download</span>
                        </span>
                      ) : dep.status === 'disabled' ? (
                        <span className="inline-flex items-center gap-1 text-[10px] font-medium px-2 py-0.5 rounded-full bg-sky-500/15 text-sky-300 border border-sky-500/25 shrink-0">
                          <PauseCircle className="w-2.5 h-2.5" />
                          <span>Enable</span>
                        </span>
                      ) : dep.status === 'installed' ? (
                        <span className="inline-flex items-center gap-1 text-[10px] font-medium px-2 py-0.5 rounded-full bg-emerald-500/15 text-emerald-300 border border-emerald-500/25 shrink-0">
                          <CheckCircle2 className="w-2.5 h-2.5" />
                          <span>Installed</span>
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 text-[10px] font-medium px-2 py-0.5 rounded-full bg-rose-500/15 text-rose-300 border border-rose-500/25 shrink-0">
                          <AlertTriangle className="w-2.5 h-2.5" />
                          <span>Not Found</span>
                        </span>
                      )}
                    </div>

                    {installing &&
                      isDownloadable &&
                      isChecked &&
                      renderProgressBar(dep.modid)}
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="px-5 py-3.5 border-t border-white/[0.06] bg-[#10131b] flex items-center justify-between gap-2.5">
          <span className="text-[11px] text-zinc-400">
            {downloadItems.length} download{downloadItems.length === 1 ? '' : 's'}
            {enableMods.length > 0 && ` • ${enableMods.length} to enable`}
          </span>

          <div className="flex items-center gap-2">
            <button
              type="button"
              disabled={installing}
              onClick={onClose}
              className="px-3.5 py-2 rounded-lg text-zinc-400 hover:text-zinc-200 disabled:opacity-40 text-xs cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="button"
              disabled={installing}
              onClick={handleConfirm}
              className="flex items-center gap-1.5 px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-zinc-950 text-xs font-semibold cursor-pointer transition-colors"
            >
              {installing ? (
                <>
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  <span>Installing All...</span>
                </>
              ) : (
                <>
                  <Download className="w-3.5 h-3.5" />
                  <span>Install All ({totalActions})</span>
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
