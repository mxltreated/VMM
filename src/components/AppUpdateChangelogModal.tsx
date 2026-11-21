import { Download, ExternalLink, FileText, Loader2, X } from 'lucide-react';
import { api } from '../api/tauri';
import type { AppUpdateInfo } from '../types';

interface AppUpdateChangelogModalProps {
  updateInfo: AppUpdateInfo;
  installing: boolean;
  onClose: () => void;
  onInstall: () => void;
}

export function AppUpdateChangelogModal({
  updateInfo,
  installing,
  onClose,
  onInstall,
}: AppUpdateChangelogModalProps) {
  return (
    <div className="animate-fade-in fixed inset-0 z-50 flex items-center justify-center bg-black/65 backdrop-blur-sm p-4">
      <div className="animate-scale-in w-full max-w-lg rounded-2xl bg-[#12151c] border border-white/[0.1] shadow-2xl overflow-hidden flex flex-col max-h-[80vh]">
        <div className="px-5 py-4 border-b border-white/[0.06] flex items-center justify-between gap-3">
          <div className="flex items-center gap-2.5 min-w-0">
            <div className="w-8 h-8 rounded-lg bg-emerald-500/15 border border-emerald-500/30 flex items-center justify-center text-emerald-400 shrink-0">
              <FileText className="w-4 h-4" />
            </div>
            <div className="min-w-0">
              <h3 className="text-sm font-semibold text-zinc-100 truncate">
                What&apos;s New in VMM v{updateInfo.latestVersion}
              </h3>
              <p className="text-[11px] text-zinc-400">
                Current version: v{updateInfo.currentVersion}
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={onClose}
            className="p-1.5 rounded-lg text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.05] cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 overflow-y-auto flex-1">
          {updateInfo.releaseNotes && updateInfo.releaseNotes.trim() ? (
            <pre className="whitespace-pre-wrap font-sans text-xs leading-relaxed text-zinc-300 bg-[#0e1015] border border-white/[0.06] rounded-xl p-4">
              {updateInfo.releaseNotes.trim()}
            </pre>
          ) : (
            <div className="text-xs text-zinc-400 bg-[#0e1015] border border-white/[0.06] rounded-xl p-4">
              No release notes were included for v{updateInfo.latestVersion}. You can view the release page on GitHub for details.
            </div>
          )}
        </div>

        <div className="px-5 py-3.5 border-t border-white/[0.06] bg-[#0e1015]/60 flex items-center justify-between gap-3">
          <button
            type="button"
            onClick={() => void api.openExternalUrl(updateInfo.htmlUrl)}
            className="flex items-center gap-1.5 text-xs text-zinc-400 hover:text-zinc-200 transition-colors cursor-pointer"
          >
            <ExternalLink className="w-3.5 h-3.5" />
            <span>View on GitHub</span>
          </button>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={onClose}
              className="px-3 py-1.5 rounded-lg bg-white/[0.05] hover:bg-white/[0.09] text-xs font-medium text-zinc-300 cursor-pointer"
            >
              Close
            </button>
            <button
              type="button"
              disabled={installing}
              onClick={onInstall}
              className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg bg-emerald-500 hover:bg-emerald-400 disabled:opacity-50 text-zinc-950 text-xs font-semibold cursor-pointer"
            >
              {installing ? (
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <Download className="w-3.5 h-3.5" />
              )}
              <span>
                {installing
                  ? 'Installing...'
                  : updateInfo.downloadUrl
                    ? 'Update & Restart'
                    : `Download v${updateInfo.latestVersion}`}
              </span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
