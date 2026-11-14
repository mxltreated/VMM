import { useEffect, useMemo, useState } from 'react';
import {
  Check,
  Code2,
  FileJson,
  FolderOpen,
  RefreshCw,
  Save,
  Search,
  SlidersHorizontal,
} from 'lucide-react';
import type { ModConfigFile } from '../types';

interface ConfigsViewProps {
  configs: ModConfigFile[];
  selectedRelPath: string | null;
  onSelectConfig: (relPath: string) => void;
  onSaveConfig: (relPath: string, content: string) => Promise<void>;
  onRefresh: () => void;
  onOpenFolder: () => void;
}

type JsonPrimitive = string | number | boolean | null;
type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };

function isPlainObject(v: JsonValue): v is { [key: string]: JsonValue } {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function updateNestedValue(
  root: JsonValue,
  path: (string | number)[],
  next: JsonValue
): JsonValue {
  if (path.length === 0) return next;
  const [head, ...rest] = path;
  if (Array.isArray(root) && typeof head === 'number') {
    const copy = [...root];
    copy[head] = updateNestedValue(copy[head], rest, next);
    return copy;
  }
  if (isPlainObject(root) && typeof head === 'string') {
    return {
      ...root,
      [head]: updateNestedValue(root[head], rest, next),
    };
  }
  return root;
}

function isReadOnlyDescriptionKey(key: string): boolean {
  const lower = key.toLowerCase();
  return (
    lower.includes('description') ||
    lower.includes('comment') ||
    lower.startsWith('_') ||
    lower.startsWith('//')
  );
}

function JsonField({
  label,
  value,
  path,
  onChange,
}: {
  label: string;
  value: JsonValue;
  path: (string | number)[];
  onChange: (path: (string | number)[], next: JsonValue) => void;
}) {
  if (typeof value === 'boolean') {
    return (
      <div className="flex items-center justify-between gap-4 py-2.5 px-3.5 rounded-xl bg-[#161922] border border-white/[0.05]">
        <span className="text-xs font-medium text-zinc-200 font-mono truncate">
          {label}
        </span>
        <button
          type="button"
          role="switch"
          aria-checked={value}
          onClick={() => onChange(path, !value)}
          className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors ${
            value ? 'bg-amber-500' : 'bg-zinc-700'
          }`}
        >
          <span
            className={`inline-block h-4 w-4 transform rounded-full bg-white shadow transition ${
              value ? 'translate-x-4' : 'translate-x-0'
            }`}
          />
        </button>
      </div>
    );
  }

  if (typeof value === 'number') {
    return (
      <div className="flex items-center justify-between gap-4 py-2 px-3.5 rounded-xl bg-[#161922] border border-white/[0.05]">
        <span className="text-xs font-medium text-zinc-200 font-mono truncate">
          {label}
        </span>
        <input
          type="number"
          step="any"
          value={Number.isNaN(value) ? '' : value}
          onChange={(e) => {
            const num = e.target.value === '' ? 0 : Number(e.target.value);
            if (!Number.isNaN(num)) onChange(path, num);
          }}
          className="w-32 bg-[#0e1015] border border-white/[0.08] rounded-lg px-2.5 py-1 text-xs text-zinc-100 font-mono text-right focus:outline-none focus:border-amber-500/40"
        />
      </div>
    );
  }

  if (typeof value === 'string' || value === null) {
    if (typeof value === 'string' && isReadOnlyDescriptionKey(label)) {
      return (
        <div className="py-2 px-3.5 rounded-xl bg-[#13161f]/60 border border-white/[0.03] space-y-0.5">
          <div className="text-[11px] font-medium text-zinc-500 font-mono">
            {label}
          </div>
          <p className="text-xs text-zinc-400 leading-relaxed select-text">
            {value}
          </p>
        </div>
      );
    }

    return (
      <div className="flex items-center justify-between gap-4 py-2 px-3.5 rounded-xl bg-[#161922] border border-white/[0.05]">
        <span className="text-xs font-medium text-zinc-200 font-mono truncate">
          {label}
        </span>
        <input
          type="text"
          value={value ?? ''}
          onChange={(e) => onChange(path, e.target.value)}
          className="w-56 bg-[#0e1015] border border-white/[0.08] rounded-lg px-2.5 py-1 text-xs text-zinc-100 font-mono focus:outline-none focus:border-amber-500/40"
        />
      </div>
    );
  }

  if (isPlainObject(value)) {
    const entries = Object.entries(value);
    return (
      <details
        open
        className="rounded-xl bg-[#13161f] border border-white/[0.06] p-3 space-y-2"
      >
        <summary className="text-xs font-semibold text-amber-300/90 font-mono cursor-pointer select-none">
          {label} ({entries.length})
        </summary>
        <div className="space-y-1.5 pt-1">
          {entries.map(([k, v]) => (
            <JsonField
              key={k}
              label={k}
              value={v}
              path={[...path, k]}
              onChange={onChange}
            />
          ))}
        </div>
      </details>
    );
  }

  // Array fallback: editable JSON snippet
  return (
    <div className="p-3 rounded-xl bg-[#161922] border border-white/[0.05] space-y-1.5">
      <div className="text-xs font-medium text-zinc-200 font-mono">
        {label} [{value.length}]
      </div>
      <textarea
        rows={Math.min(5, Math.max(2, value.length))}
        defaultValue={JSON.stringify(value, null, 2)}
        onBlur={(e) => {
          try {
            const parsed = JSON.parse(e.target.value);
            onChange(path, parsed);
          } catch {
            // keep existing on invalid array JSON
          }
        }}
        className="w-full bg-[#0e1015] border border-white/[0.08] rounded-lg p-2 text-[11px] text-zinc-300 font-mono focus:outline-none focus:border-amber-500/40"
      />
    </div>
  );
}

export function ConfigsView({
  configs,
  selectedRelPath,
  onSelectConfig,
  onSaveConfig,
  onRefresh,
  onOpenFolder,
}: ConfigsViewProps) {
  const [search, setSearch] = useState('');
  const [mode, setMode] = useState<'visual' | 'raw'>('visual');
  const [draft, setDraft] = useState('');
  const [saving, setSaving] = useState(false);
  const [savedToast, setSavedToast] = useState(false);
  const [parseError, setParseError] = useState<string | null>(null);

  const activeConfig = useMemo(
    () =>
      configs.find((c) => c.relPath === selectedRelPath) ?? configs[0] ?? null,
    [configs, selectedRelPath]
  );

  useEffect(() => {
    if (activeConfig) {
      setDraft(activeConfig.content);
      setParseError(null);
    } else {
      setDraft('');
    }
  }, [activeConfig]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return configs;
    return configs.filter((c) => c.relPath.toLowerCase().includes(q));
  }, [configs, search]);

  const parsedDraft = useMemo<{ ok: true; value: JsonValue } | { ok: false; err: string }>(() => {
    if (!draft.trim()) return { ok: false, err: 'Empty file' };
    try {
      return { ok: true, value: JSON.parse(draft) as JsonValue };
    } catch (e) {
      return { ok: false, err: e instanceof Error ? e.message : 'Invalid JSON' };
    }
  }, [draft]);

  const isDirty = activeConfig ? draft !== activeConfig.content : false;

  const handleFieldChange = (path: (string | number)[], nextVal: JsonValue) => {
    if (!parsedDraft.ok) return;
    const updated = updateNestedValue(parsedDraft.value, path, nextVal);
    setDraft(JSON.stringify(updated, null, 2));
    setParseError(null);
  };

  const handleSave = async () => {
    if (!activeConfig || saving) return;
    if (!parsedDraft.ok) {
      setParseError(parsedDraft.err);
      return;
    }
    setSaving(true);
    setParseError(null);
    try {
      await onSaveConfig(activeConfig.relPath, draft);
      setSavedToast(true);
      setTimeout(() => setSavedToast(false), 1800);
    } catch (e) {
      setParseError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex flex-1 min-h-0">
      {/* Left file list */}
      <div className="w-72 shrink-0 border-r border-white/[0.06] bg-[#11141b] flex flex-col">
        <div className="p-3 border-b border-white/[0.05] space-y-2">
          <div className="relative">
            <Search className="w-3.5 h-3.5 text-zinc-500 absolute left-2.5 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Filter ModConfig/*.json..."
              className="w-full bg-[#161922] border border-white/[0.07] rounded-lg pl-8 pr-2.5 py-1.5 text-xs text-zinc-200 placeholder:text-zinc-500 focus:outline-none focus:border-amber-500/40"
            />
          </div>
          <div className="flex items-center gap-1.5">
            <button
              onClick={onRefresh}
              className="flex-1 flex items-center justify-center gap-1.5 py-1.5 rounded-lg bg-[#161922] hover:bg-white/[0.06] border border-white/[0.06] text-[11px] text-zinc-300 cursor-pointer"
            >
              <RefreshCw className="w-3 h-3" />
              <span>Refresh</span>
            </button>
            <button
              onClick={onOpenFolder}
              className="flex-1 flex items-center justify-center gap-1.5 py-1.5 rounded-lg bg-[#161922] hover:bg-white/[0.06] border border-white/[0.06] text-[11px] text-zinc-300 cursor-pointer"
            >
              <FolderOpen className="w-3 h-3" />
              <span>Folder</span>
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {filtered.length === 0 ? (
            <div className="p-4 text-center text-xs text-zinc-500">
              No config files found in ModConfig/.
            </div>
          ) : (
            filtered.map((cfg) => {
              const selected = activeConfig?.relPath === cfg.relPath;
              return (
                <button
                  key={cfg.relPath}
                  onClick={() => onSelectConfig(cfg.relPath)}
                  className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-left transition-colors cursor-pointer ${
                    selected
                      ? 'bg-amber-500/12 text-amber-300 border border-amber-500/20'
                      : 'text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.04] border border-transparent'
                  }`}
                >
                  <FileJson className="w-3.5 h-3.5 shrink-0" />
                  <span className="text-xs font-mono truncate">{cfg.relPath}</span>
                </button>
              );
            })
          )}
        </div>
      </div>

      {/* Right editor */}
      <div className="flex-1 flex flex-col min-w-0">
        {!activeConfig ? (
          <div className="flex-1 flex flex-col items-center justify-center text-center p-8">
            <SlidersHorizontal className="w-8 h-8 text-zinc-600 mb-2" />
            <p className="text-sm font-medium text-zinc-300">
              No ModConfig file selected
            </p>
            <p className="text-xs text-zinc-500 mt-1 max-w-sm">
              Mods write their configuration files into VintagestoryData/ModConfig/ on first boot.
            </p>
          </div>
        ) : (
          <>
            <div className="px-6 py-3.5 border-b border-white/[0.05] flex items-center justify-between gap-3">
              <div className="flex items-center gap-2 min-w-0">
                <span className="text-sm font-semibold font-mono text-zinc-100 truncate">
                  {activeConfig.relPath}
                </span>
                {isDirty && (
                  <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300">
                    Unsaved
                  </span>
                )}
              </div>

              <div className="flex items-center gap-2">
                <div className="flex items-center bg-[#161922] border border-white/[0.07] rounded-lg p-0.5 text-xs">
                  <button
                    onClick={() => setMode('visual')}
                    className={`flex items-center gap-1 px-2.5 py-1 rounded-md cursor-pointer ${
                      mode === 'visual'
                        ? 'bg-white/[0.08] text-zinc-100'
                        : 'text-zinc-500 hover:text-zinc-300'
                    }`}
                  >
                    <SlidersHorizontal className="w-3 h-3" />
                    <span>Form</span>
                  </button>
                  <button
                    onClick={() => setMode('raw')}
                    className={`flex items-center gap-1 px-2.5 py-1 rounded-md cursor-pointer ${
                      mode === 'raw'
                        ? 'bg-white/[0.08] text-zinc-100'
                        : 'text-zinc-500 hover:text-zinc-300'
                    }`}
                  >
                    <Code2 className="w-3 h-3" />
                    <span>JSON</span>
                  </button>
                </div>

                <button
                  onClick={handleSave}
                  disabled={saving || !isDirty}
                  className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-40 text-zinc-950 text-xs font-semibold cursor-pointer"
                >
                  {savedToast ? (
                    <>
                      <Check className="w-3.5 h-3.5" />
                      <span>Saved</span>
                    </>
                  ) : (
                    <>
                      <Save className="w-3.5 h-3.5" />
                      <span>{saving ? 'Saving...' : 'Save'}</span>
                    </>
                  )}
                </button>
              </div>
            </div>

            {parseError && (
              <div className="mx-6 mt-3 px-3 py-2 rounded-lg bg-rose-500/10 border border-rose-500/20 text-xs text-rose-300">
                {parseError}
              </div>
            )}

            <div key={`${activeConfig.relPath}-${mode}`} className="animate-fade-in flex-1 overflow-y-auto p-6">
              {mode === 'raw' ? (
                <textarea
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  spellCheck={false}
                  className="w-full h-full min-h-[420px] bg-[#12151c] border border-white/[0.07] rounded-xl p-4 text-xs font-mono text-zinc-200 leading-relaxed focus:outline-none focus:border-amber-500/40 resize-none"
                />
              ) : !parsedDraft.ok ? (
                <div className="p-4 rounded-xl bg-rose-500/10 border border-rose-500/20 text-xs text-rose-300">
                  Cannot render visual form: {parsedDraft.err}. Switch to JSON mode to fix syntax.
                </div>
              ) : isPlainObject(parsedDraft.value) ? (
                <div className="space-y-2 max-w-2xl">
                  {Object.entries(parsedDraft.value).map(([key, val]) => (
                    <JsonField
                      key={key}
                      label={key}
                      value={val}
                      path={[key]}
                      onChange={handleFieldChange}
                    />
                  ))}
                </div>
              ) : (
                <div className="text-xs text-zinc-400">
                  Top-level value is not an object. Switch to JSON mode to edit.
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
