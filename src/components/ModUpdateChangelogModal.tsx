import { useEffect, useState } from 'react';
import { ArrowUpCircle, FileText, Loader2, X } from 'lucide-react';
import { api } from '../api/tauri';
import { htmlToPlainText } from '../utils/sanitize';
import type { LocalMod, ModDbRelease } from '../types';

interface ModUpdateChangelogModalProps {
  mod: LocalMod;
  onClose: () => void;
  onConfirmUpdate: (mod: LocalMod) => Promise<void>;
}

export function ModUpdateChangelogModal({
  mod,
  onClose,
  onConfirmUpdate,
}: ModUpdateChangelogModalProps) {
  const [releases, setReleases] = useState<ModDbRelease[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    api
      .getModDbDetail(mod.modid)
      .then((detail) => {
        if (cancelled) return;
        const newer: ModDbRelease[] = [];
        for (const rel of detail.releases) {
          if (rel.modversion === mod.version) break;
          newer.push(rel);
          if (newer.length >= 5) break;
        }
        setReleases(newer.length > 0 ? newer : detail.releases.slice(0, 1));
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [mod.modid, mod.version]);

  const handleConfirm = async () => {
    setUpdating(true);
    try {
      await onConfirmUpdate(mod);
      onClose();
    } finally {
      setUpdating(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="w-full max-w-lg rounded-2xl bg-[#141720] border border-white/[0.08] shadow-2xl overflow-hidden flex flex-col max-h-[82vh]">
        <div className="px-5 py-4 border-b border-white/[0.06] flex items-center justify-between">
          <div className="flex items-center gap-2.5 min-w-0">
            <FileText className="w-4 h-4 text-amber-400 shrink-0" />
            <div className="min-w-0">
              <h2 className="text-sm font-semibold text-zinc-100 truncate">
                Update {mod.name}
              </h2>
              <p className="text-[11px] text-zinc-400 font-mono">
                v{mod.version} → v{mod.latestVersion}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-zinc-500 hover:text-zinc-200 cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 overflow-y-auto space-y-3 text-xs">
          {loading ? (
            <div className="py-10 flex items-center justify-center gap-2 text-zinc-400">
              <Loader2 className="w-4 h-4 animate-spin text-amber-400" />
              <span>Fetching release changelog from Mod DB...</span>
            </div>
          ) : error ? (
            <div className="p-3 rounded-xl bg-rose-500/10 border border-rose-500/25 text-rose-300">
              Could not load changelog: {error}. You can still proceed with the update.
            </div>
          ) : releases.length === 0 ? (
            <div className="text-zinc-400">No release notes found.</div>
          ) : (
            releases.map((rel) => (
              <div
                key={rel.releaseid}
                className="p-3.5 rounded-xl bg-[#0e1015] border border-white/[0.06] space-y-2"
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="font-mono font-semibold text-amber-300">
                    v{rel.modversion ?? mod.latestVersion}
                  </span>
                  {rel.created && (
                    <span className="text-[11px] text-zinc-500">
                      {rel.created.split(' ')[0]}
                    </span>
                  )}
                </div>
                <div className="text-zinc-300 whitespace-pre-wrap leading-relaxed">
                  {htmlToPlainText(rel.changelog)}
                </div>
              </div>
            ))
          )}
        </div>

        <div className="px-5 py-3.5 border-t border-white/[0.06] bg-[#10131b] flex items-center justify-end gap-2.5">
          <button
            onClick={onClose}
            className="px-3.5 py-2 rounded-lg text-zinc-400 hover:text-zinc-200 text-xs cursor-pointer"
          >
            Cancel
          </button>
          <button
            onClick={handleConfirm}
            disabled={updating}
            className="flex items-center gap-1.5 px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-zinc-950 text-xs font-semibold cursor-pointer"
          >
            {updating ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <ArrowUpCircle className="w-3.5 h-3.5" />
            )}
            <span>
              {updating
                ? 'Updating...'
                : `Update to v${mod.latestVersion ?? ''}`}
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}
