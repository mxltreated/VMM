import { useEffect, useState } from 'react';
import { Download, ExternalLink, Loader2, Package, X } from 'lucide-react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { api } from '../api/tauri';
import type { DownloadProgressEvent, ModDbDetail, ModDbRelease, ModDbSummary } from '../types';

interface ModDetailDrawerProps {
  modSummary: ModDbSummary | null;
  downloads: Record<string, DownloadProgressEvent>;
  onClose: () => void;
  onInstallRelease: (detail: ModDbDetail, release: ModDbRelease) => void;
}

export function ModDetailDrawer({
  modSummary,
  downloads,
  onClose,
  onInstallRelease,
}: ModDetailDrawerProps) {
  const [detail, setDetail] = useState<ModDbDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!modSummary) {
      setDetail(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    api
      .getModDbDetail(String(modSummary.modid))
      .then((d) => {
        if (!cancelled) setDetail(d);
      })
      .catch((err) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [modSummary]);

  if (!modSummary) return null;

  const dlKey = modSummary.modidstrs[0] || String(modSummary.modid);
  const dl = downloads[dlKey] || downloads[String(modSummary.modid)];
  const isDownloading = Boolean(dl && !dl.done);

  return (
    <div className="animate-fade-in fixed inset-0 z-40 flex justify-end bg-black/50 backdrop-blur-xs">
      <div className="animate-slide-right w-full max-w-xl bg-[#12151c] border-l border-white/[0.08] h-full flex flex-col shadow-2xl">
        <div className="px-6 py-4 border-b border-white/[0.06] flex items-center justify-between">
          <div className="flex items-center gap-3 min-w-0">
            {modSummary.logo ? (
              <img
                src={modSummary.logo}
                alt={modSummary.name}
                className="w-10 h-10 rounded-lg object-cover bg-zinc-900 border border-white/[0.06]"
              />
            ) : (
              <div className="w-10 h-10 rounded-lg bg-white/[0.04] flex items-center justify-center text-zinc-500">
                <Package className="w-5 h-5" />
              </div>
            )}
            <div className="min-w-0">
              <h2 className="text-base font-semibold text-zinc-100 truncate">
                {modSummary.name}
              </h2>
              <p className="text-xs text-zinc-400">
                by {modSummary.author || 'Unknown'} • {modSummary.side || 'both'}
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

        <div className="flex-1 overflow-y-auto p-6 space-y-6">
          {loading ? (
            <div className="flex items-center justify-center h-40 gap-2 text-xs text-zinc-400">
              <Loader2 className="w-4 h-4 animate-spin text-amber-400" />
              <span>Loading mod details...</span>
            </div>
          ) : error ? (
            <div className="p-3 rounded-lg bg-rose-500/10 border border-rose-500/20 text-xs text-rose-300">
              {error}
            </div>
          ) : detail ? (
            <>
              <div className="flex flex-wrap items-center gap-2">
                {detail.tags.map((t) => (
                  <span
                    key={t}
                    className="text-[11px] px-2 py-0.5 rounded-md bg-white/[0.05] text-zinc-300"
                  >
                    {t}
                  </span>
                ))}
                <button
                  onClick={() =>
                    openUrl(
                      `https://mods.vintagestory.at/${detail.urlalias || `show/mod/${detail.assetid ?? detail.modid}`}`
                    )
                  }
                  className="ml-auto flex items-center gap-1 text-xs text-amber-400 hover:text-amber-300 cursor-pointer"
                >
                  <span>Open on Mod DB</span>
                  <ExternalLink className="w-3 h-3" />
                </button>
              </div>

              {detail.text && (
                <div className="text-xs text-zinc-300 leading-relaxed space-y-2 bg-[#161922] p-4 rounded-xl border border-white/[0.05] max-h-60 overflow-y-auto">
                  <div
                    className="prose prose-invert prose-xs max-w-none [&_img]:max-w-full [&_img]:rounded-lg [&_a]:text-amber-400"
                    dangerouslySetInnerHTML={{ __html: detail.text }}
                  />
                </div>
              )}

              <div>
                <h3 className="text-xs font-semibold uppercase tracking-wider text-zinc-400 mb-3">
                  Releases ({detail.releases.length})
                </h3>
                <div className="space-y-2">
                  {detail.releases.map((rel) => (
                    <div
                      key={rel.releaseid}
                      className="p-3.5 rounded-xl bg-[#161922] border border-white/[0.06] flex items-center justify-between gap-3"
                    >
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="text-sm font-semibold text-zinc-100 font-mono">
                            v{rel.modversion || '?'}
                          </span>
                          {rel.tags.slice(0, 4).map((gv) => (
                            <span
                              key={gv}
                              className="text-[10px] px-1.5 py-0.5 rounded bg-white/[0.05] text-zinc-400"
                            >
                              {gv}
                            </span>
                          ))}
                          {rel.tags.length > 4 && (
                            <span className="text-[10px] text-zinc-500">
                              +{rel.tags.length - 4}
                            </span>
                          )}
                        </div>
                        <p className="text-[11px] text-zinc-500 mt-1 truncate">
                          {rel.filename} • {(rel.downloads ?? 0).toLocaleString()} downloads
                        </p>
                      </div>

                      <button
                        onClick={() => onInstallRelease(detail, rel)}
                        disabled={!rel.mainfile || isDownloading}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-zinc-950 text-xs font-semibold shrink-0 cursor-pointer"
                      >
                        {isDownloading ? (
                          <Loader2 className="w-3.5 h-3.5 animate-spin" />
                        ) : (
                          <Download className="w-3.5 h-3.5" />
                        )}
                        <span>Install</span>
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            </>
          ) : null}
        </div>
      </div>
    </div>
  );
}
