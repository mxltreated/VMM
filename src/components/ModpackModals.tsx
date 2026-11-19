import { useState } from 'react';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import {
  AlertTriangle,
  Check,
  Copy,
  Download,
  FileArchive,
  FolderOpen,
  Loader2,
  PackageCheck,
  Share2,
  X,
} from 'lucide-react';
import { api } from '../api/tauri';
import type { ModProfile, ModpackImportPreview, ModpackImportResult } from '../types';

interface ExportModpackModalProps {
  profile: ModProfile;
  onClose: () => void;
  onError: (msg: string) => void;
}

export function ExportModpackModal({
  profile,
  onClose,
  onError,
}: ExportModpackModalProps) {
  const [packName, setPackName] = useState(profile.name);
  const [author, setAuthor] = useState('');
  const [description, setDescription] = useState(profile.settings.notes ?? '');
  const [includeConfigs, setIncludeConfigs] = useState(false);
  const [shareCode, setShareCode] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [exportedPath, setExportedPath] = useState<string | null>(null);

  const handleGenerateCode = async () => {
    setBusy(true);
    try {
      const code = await api.generateModpackCode({
        profileId: profile.id,
        packName,
        author,
        description,
      });
      setShareCode(code);
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleExportFile = async () => {
    const ext = includeConfigs ? 'vmmpack.zip' : 'vmmpack.json';
    const safeBase = packName.trim().toLowerCase().replace(/[^a-z0-9-_]+/g, '-') || 'modpack';
    const dest = await saveDialog({
      defaultPath: `${safeBase}.${ext}`,
      filters: includeConfigs
        ? [{ name: 'VMM Modpack Bundle', extensions: ['zip'] }]
        : [{ name: 'VMM Modpack Manifest', extensions: ['json'] }],
    });
    if (!dest) return;

    setBusy(true);
    try {
      await api.exportModpackFile({
        profileId: profile.id,
        destPath: dest,
        packName,
        author,
        description,
        includeConfigs,
      });
      setExportedPath(dest);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="w-full max-w-lg rounded-2xl bg-[#141720] border border-white/[0.08] shadow-2xl overflow-hidden">
        <div className="px-5 py-4 border-b border-white/[0.06] flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <Share2 className="w-4 h-4 text-amber-400" />
            <h2 className="text-sm font-semibold text-zinc-100">
              Share Modpack — {profile.name}
            </h2>
          </div>
          <button
            onClick={onClose}
            className="text-zinc-500 hover:text-zinc-200 cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 space-y-4 text-xs">
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-zinc-400 mb-1">Pack Name</label>
              <input
                type="text"
                value={packName}
                onChange={(e) => setPackName(e.target.value)}
                className="w-full bg-[#0e1015] border border-white/[0.08] rounded-lg px-3 py-2 text-zinc-200 focus:outline-none focus:border-amber-500/40"
              />
            </div>
            <div>
              <label className="block text-zinc-400 mb-1">Author (optional)</label>
              <input
                type="text"
                value={author}
                onChange={(e) => setAuthor(e.target.value)}
                placeholder="Your name"
                className="w-full bg-[#0e1015] border border-white/[0.08] rounded-lg px-3 py-2 text-zinc-200 focus:outline-none focus:border-amber-500/40"
              />
            </div>
          </div>

          <div>
            <label className="block text-zinc-400 mb-1">Description (optional)</label>
            <textarea
              rows={2}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Short summary of this modpack..."
              className="w-full bg-[#0e1015] border border-white/[0.08] rounded-lg px-3 py-2 text-zinc-200 focus:outline-none focus:border-amber-500/40 resize-none"
            />
          </div>

          <label className="flex items-center gap-2.5 p-3 rounded-xl bg-white/[0.02] border border-white/[0.06] cursor-pointer">
            <input
              type="checkbox"
              checked={includeConfigs}
              onChange={(e) => setIncludeConfigs(e.target.checked)}
              className="accent-amber-500"
            />
            <div>
              <div className="text-zinc-200 font-medium">
                Include ModConfig/ files (exports .vmmpack.zip)
              </div>
              <div className="text-[11px] text-zinc-500">
                Bundles your tweaked mod configs alongside the manifest.
              </div>
            </div>
          </label>

          {shareCode && (
            <div className="p-3 rounded-xl bg-emerald-500/10 border border-emerald-500/25 space-y-1.5">
              <div className="flex items-center justify-between text-emerald-300 font-medium">
                <span>Share Code ({profile.enabledModids.length} mods)</span>
                <button
                  onClick={() => {
                    navigator.clipboard.writeText(shareCode);
                    setCopied(true);
                    setTimeout(() => setCopied(false), 1500);
                  }}
                  className="flex items-center gap-1 text-[11px] hover:underline cursor-pointer"
                >
                  {copied ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                  {copied ? 'Copied!' : 'Copy'}
                </button>
              </div>
              <div className="font-mono text-[10px] text-zinc-300 break-all max-h-20 overflow-y-auto bg-[#0e1015]/80 p-2 rounded-lg">
                {shareCode}
              </div>
            </div>
          )}

          {exportedPath && (
            <div className="p-2.5 rounded-lg bg-emerald-500/10 border border-emerald-500/25 text-emerald-300 text-[11px]">
              Exported successfully to <span className="font-mono">{exportedPath}</span>
            </div>
          )}
        </div>

        <div className="px-5 py-3.5 border-t border-white/[0.06] bg-[#10131b] flex items-center justify-end gap-2.5">
          <button
            onClick={handleGenerateCode}
            disabled={busy || !packName.trim()}
            className="flex items-center gap-1.5 px-3.5 py-2 rounded-lg bg-white/[0.06] hover:bg-white/[0.1] disabled:opacity-50 text-zinc-200 text-xs font-medium cursor-pointer"
          >
            {busy ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Copy className="w-3.5 h-3.5" />}
            <span>Copy Share Code</span>
          </button>
          <button
            onClick={handleExportFile}
            disabled={busy || !packName.trim()}
            className="flex items-center gap-1.5 px-3.5 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-zinc-950 text-xs font-semibold cursor-pointer"
          >
            <FileArchive className="w-3.5 h-3.5" />
            <span>{includeConfigs ? 'Export .vmmpack.zip' : 'Export .vmmpack.json'}</span>
          </button>
        </div>
      </div>
    </div>
  );
}

interface ImportModpackModalProps {
  onClose: () => void;
  onImported: (result: ModpackImportResult) => Promise<void>;
  onError: (msg: string) => void;
}

export function ImportModpackModal({
  onClose,
  onImported,
  onError,
}: ImportModpackModalProps) {
  const [source, setSource] = useState('');
  const [preview, setPreview] = useState<ModpackImportPreview | null>(null);
  const [profileName, setProfileName] = useState('');
  const [restoreConfigs, setRestoreConfigs] = useState(true);
  const [loadingPreview, setLoadingPreview] = useState(false);
  const [installing, setInstalling] = useState(false);

  const runPreview = async (src: string) => {
    if (!src.trim()) return;
    setLoadingPreview(true);
    try {
      const p = await api.previewModpackImport(src.trim());
      setPreview(p);
      setProfileName(p.manifest.name);
      setRestoreConfigs(p.hasConfigs);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingPreview(false);
    }
  };

  const handleBrowseFile = async () => {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'VMM Modpack', extensions: ['json', 'zip'] }],
    });
    if (typeof selected === 'string') {
      setSource(selected);
      await runPreview(selected);
    }
  };

  const handleInstall = async () => {
    if (!preview) return;
    setInstalling(true);
    try {
      const res = await api.applyModpackImport({
        source: source.trim(),
        profileName: profileName.trim() || preview.manifest.name,
        restoreConfigs,
        expectedManifestHash: preview.manifestHash,
      });
      await onImported(res);
      onClose();
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setInstalling(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="w-full max-w-xl rounded-2xl bg-[#141720] border border-white/[0.08] shadow-2xl overflow-hidden flex flex-col max-h-[85vh]">
        <div className="px-5 py-4 border-b border-white/[0.06] flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <Download className="w-4 h-4 text-amber-400" />
            <h2 className="text-sm font-semibold text-zinc-100">Import Modpack</h2>
          </div>
          <button
            onClick={onClose}
            className="text-zinc-500 hover:text-zinc-200 cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 space-y-4 text-xs overflow-y-auto">
          <div className="flex gap-2">
            <input
              type="text"
              value={source}
              onChange={(e) => setSource(e.target.value)}
              placeholder="Paste vmm1:... code, GitHub raw/Gist URL, or choose a file..."
              className="flex-1 bg-[#0e1015] border border-white/[0.08] rounded-lg px-3 py-2 text-zinc-200 placeholder:text-zinc-500 focus:outline-none focus:border-amber-500/40"
            />
            <button
              onClick={handleBrowseFile}
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-white/[0.06] hover:bg-white/[0.1] text-zinc-200 cursor-pointer"
            >
              <FolderOpen className="w-3.5 h-3.5" />
              <span>File</span>
            </button>
            <button
              onClick={() => runPreview(source)}
              disabled={!source.trim() || loadingPreview}
              className="px-3.5 py-2 rounded-lg bg-amber-500/15 hover:bg-amber-500/25 border border-amber-500/30 text-amber-300 font-medium disabled:opacity-50 cursor-pointer"
            >
              {loadingPreview ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : 'Inspect'}
            </button>
          </div>

          {preview && (
            <div className="space-y-3">
              <div className="p-3.5 rounded-xl bg-[#0e1015] border border-white/[0.06] space-y-2">
                <div className="flex items-center justify-between">
                  <span className="text-sm font-semibold text-zinc-100">
                    {preview.manifest.name}
                  </span>
                  {preview.manifest.gameVersion && (
                    <span className="px-2 py-0.5 rounded bg-white/[0.05] text-zinc-400 text-[11px]">
                      VS {preview.manifest.gameVersion}
                    </span>
                  )}
                </div>
                {preview.manifest.description && (
                  <p className="text-zinc-400">{preview.manifest.description}</p>
                )}
                <div className="pt-1">
                  <label className="block text-zinc-500 mb-1">New Profile Name</label>
                  <input
                    type="text"
                    value={profileName}
                    onChange={(e) => setProfileName(e.target.value)}
                    className="w-full bg-[#141720] border border-white/[0.08] rounded-lg px-2.5 py-1.5 text-zinc-200 focus:outline-none focus:border-amber-500/40"
                  />
                </div>
                {preview.hasConfigs && (
                  <label className="flex items-center gap-2 pt-1 text-zinc-300 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={restoreConfigs}
                      onChange={(e) => setRestoreConfigs(e.target.checked)}
                      className="accent-amber-500"
                    />
                    <span>
                      Restore {preview.manifest.includedConfigs.length} bundled ModConfig/ file(s)
                    </span>
                  </label>
                )}
              </div>

              <div className="space-y-1.5 max-h-56 overflow-y-auto pr-1">
                {preview.items.map((item) => (
                  <div
                    key={item.modid}
                    className="flex items-center justify-between px-3 py-2 rounded-lg bg-[#0e1015]/70 border border-white/[0.05]"
                  >
                    <div>
                      <div className="text-zinc-200 font-medium">{item.name}</div>
                      <div className="text-[11px] text-zinc-500 font-mono">
                        {item.modid}
                        {item.resolvedVersion ? ` • v${item.resolvedVersion}` : ''}
                      </div>
                    </div>
                    {item.status === 'installed' && (
                      <span className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-emerald-500/15 text-emerald-300 text-[11px]">
                        <PackageCheck className="w-3 h-3" /> Installed
                      </span>
                    )}
                    {item.status === 'downloadable' && (
                      <span className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-amber-500/15 text-amber-300 text-[11px]">
                        <Download className="w-3 h-3" /> Will Download
                      </span>
                    )}
                    {item.status === 'missing' && (
                      <span className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-rose-500/15 text-rose-300 text-[11px]">
                        <AlertTriangle className="w-3 h-3" /> Manual Install
                      </span>
                    )}
                  </div>
                ))}
              </div>
            </div>
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
            onClick={handleInstall}
            disabled={!preview || installing}
            className="flex items-center gap-1.5 px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-zinc-950 text-xs font-semibold cursor-pointer"
          >
            {installing ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Download className="w-3.5 h-3.5" />
            )}
            <span>{installing ? 'Installing Mods...' : 'Install & Create Profile'}</span>
          </button>
        </div>
      </div>
    </div>
  );
}
