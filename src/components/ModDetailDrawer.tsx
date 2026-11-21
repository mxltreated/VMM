import { useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertTriangle,
  BookOpen,
  Bug,
  CheckCircle2,
  Code2,
  Download,
  ExternalLink,
  FileText,
  GitBranch,
  Globe,
  Loader2,
  Maximize2,
  Minimize2,
  Package,
  PauseCircle,
  X,
} from 'lucide-react';
import { api } from '../api/tauri';
import { DependencyTooltipBadge } from './DependencyTooltip';
import {
  evaluateDependenciesAgainstLocal,
  formatVersionRequirement,
} from '../utils/dependencies';
import { isSafeHttpUrl, sanitizeModDbHtml } from '../utils/sanitize';
import type {
  DownloadProgressEvent,
  LocalMod,
  ModDbDetail,
  ModDbRelease,
  ModDbSummary,
  ModDependenciesSummary,
} from '../types';

const MODDB_ORIGIN = 'https://mods.vintagestory.at';

interface ModDetailDrawerProps {
  modSummary: ModDbSummary | null;
  installedMods: LocalMod[];
  downloads: Record<string, DownloadProgressEvent>;
  resolvingInstall?: boolean;
  onClose: () => void;
  onInstallRelease: (detail: ModDbDetail, release: ModDbRelease) => void;
}

type DetailTab = 'description' | 'releases';

interface LightboxState {
  src: string;
  alt: string;
}

function resolveModDbUrl(rawUrl: string): string {
  try {
    return new URL(rawUrl, MODDB_ORIGIN).toString();
  } catch {
    return '';
  }
}

function normalizeExternalUrl(rawUrl: string | null | undefined): string | null {
  const trimmed = rawUrl?.trim();
  if (!trimmed) return null;
  if (/^https?:\/\//i.test(trimmed)) return isSafeHttpUrl(trimmed);
  if (trimmed.startsWith('//')) return isSafeHttpUrl(`https:${trimmed}`);
  if (trimmed.startsWith('/')) return isSafeHttpUrl(`${MODDB_ORIGIN}${trimmed}`);
  if (/^[a-z0-9.-]+\.[a-z]{2,}(\/|$)/i.test(trimmed)) {
    return isSafeHttpUrl(`https://${trimmed}`);
  }
  return null;
}

function openExternalSafely(url: string | null) {
  const safe = isSafeHttpUrl(url);
  if (!safe) return;
  void api.openExternalUrl(safe).catch(() => {});
}

export function ModDetailDrawer({
  modSummary,
  installedMods,
  downloads,
  resolvingInstall = false,
  onClose,
  onInstallRelease,
}: ModDetailDrawerProps) {
  const [detail, setDetail] = useState<ModDbDetail | null>(null);
  const [depSummary, setDepSummary] = useState<ModDependenciesSummary | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isExpanded, setIsExpanded] = useState(false);
  const [activeTab, setActiveTab] = useState<DetailTab>('description');
  const [lightboxImage, setLightboxImage] = useState<LightboxState | null>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const installedModsById = useMemo(() => {
    const map = new Map<string, LocalMod>();
    for (const m of installedMods) {
      if (m.modid) map.set(m.modid.toLowerCase(), m);
    }
    return map;
  }, [installedMods]);

  useEffect(() => {
    setActiveTab('description');
    setLightboxImage(null);
    setDepSummary(null);
    if (scrollContainerRef.current) {
      scrollContainerRef.current.scrollTop = 0;
    }

    if (!modSummary) {
      setDetail(null);
      return;
    }

    const lookupKey =
      modSummary.modid > 0
        ? String(modSummary.modid)
        : (modSummary.modidstrs[0] ?? '').trim();

    if (!lookupKey) {
      setDetail(null);
      setLoading(false);
      setError('This mod does not have a Mod DB ID.');
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);
    api
      .getModDbDetail(lookupKey)
      .then((d) => {
        if (cancelled) return;
        setDetail(d);
        api
          .inspectModDependenciesBatch([String(d.modid)])
          .then((res) => {
            if (!cancelled && res.length > 0) {
              setDepSummary(res[0]);
            }
          })
          .catch(() => {});
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

  useEffect(() => {
    if (scrollContainerRef.current) {
      scrollContainerRef.current.scrollTop = 0;
    }
  }, [activeTab]);

  useEffect(() => {
    if (!modSummary) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (lightboxImage) {
        setLightboxImage(null);
      } else {
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [modSummary, lightboxImage, onClose]);

  const normalizedDescriptionHtml = useMemo(
    () => (detail?.text ? sanitizeModDbHtml(detail.text) : ''),
    [detail?.text]
  );

  const resourceLinks = useMemo(() => {
    if (!detail) return [];
    const candidates: Array<{
      label: string;
      url: string | null;
      icon: typeof Globe;
    }> = [
      { label: 'Homepage', url: normalizeExternalUrl(detail.homepageurl), icon: Globe },
      { label: 'Source', url: normalizeExternalUrl(detail.sourcecodeurl), icon: Code2 },
      { label: 'Wiki', url: normalizeExternalUrl(detail.wikiurl), icon: BookOpen },
      { label: 'Issues', url: normalizeExternalUrl(detail.issuetrackerurl), icon: Bug },
    ];
    return candidates.filter((item): item is { label: string; url: string; icon: typeof Globe } =>
      Boolean(item.url)
    );
  }, [detail]);

  const evaluatedDeps = useMemo(
    () =>
      depSummary
        ? evaluateDependenciesAgainstLocal(
            depSummary.modDependencies,
            depSummary.gameRequirements,
            installedModsById
          )
        : null,
    [depSummary, installedModsById]
  );

  if (!modSummary) return null;

  const dlKey = modSummary.modidstrs[0] || String(modSummary.modid);
  const dl = downloads[dlKey] || downloads[String(modSummary.modid)];
  const isDownloading = Boolean(dl && !dl.done) || resolvingInstall;
  const pct =
    dl && dl.totalBytes
      ? Math.min(100, Math.round((dl.bytesReceived / dl.totalBytes) * 100))
      : null;

  const latestRelease = detail?.releases[0] ?? null;
  const totalDownloads = detail?.downloads ?? modSummary.downloads;
  const totalFollows = detail?.follows ?? modSummary.follows;
  const safeSummaryLogo = isSafeHttpUrl(modSummary.logo);
  const fallbackAsset =
    detail?.assetid ??
    modSummary.assetid ??
    (modSummary.modid > 0 ? modSummary.modid : null);
  const rawAlias = (
    detail?.urlalias ||
    modSummary.urlalias ||
    (fallbackAsset !== null ? `show/mod/${fallbackAsset}` : '')
  ).replace(/^\/+/, '');
  const candidateModDbUrl = rawAlias
    ? isSafeHttpUrl(`${MODDB_ORIGIN}/${rawAlias}`)
    : null;
  const modDbUrl =
    candidateModDbUrl && candidateModDbUrl.startsWith(`${MODDB_ORIGIN}/`)
      ? candidateModDbUrl
      : MODDB_ORIGIN;

  const handleRichHtmlClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement | null;
    if (!target) return;

    const spoilerTitle = target.closest('.spoiler-title');
    if (spoilerTitle) {
      const spoiler = spoilerTitle.closest('.spoiler');
      if (spoiler) {
        spoiler.classList.toggle('is-open');
        return;
      }
    }

    const anchor = target.closest('a');
    if (anchor) {
      e.preventDefault();
      const rawHref = anchor.getAttribute('href')?.trim();
      if (!rawHref || rawHref.startsWith('#')) return;

      const resolvedHref = isSafeHttpUrl(resolveModDbUrl(rawHref));
      if (!resolvedHref) return;
      const wrappedImg = anchor.querySelector('img');
      if (
        wrappedImg &&
        /\.(png|jpe?g|gif|webp|bmp|svg)(\?.*)?$/i.test(resolvedHref)
      ) {
        setLightboxImage({
          src: resolvedHref,
          alt: wrappedImg.getAttribute('alt') || modSummary.name,
        });
        return;
      }

      openExternalSafely(resolvedHref);
      return;
    }

    if (target instanceof HTMLImageElement) {
      const src = target.currentSrc || target.getAttribute('src');
      if (src) {
        const safeImg = isSafeHttpUrl(resolveModDbUrl(src));
        if (safeImg) {
          setLightboxImage({
            src: safeImg,
            alt: target.getAttribute('alt') || modSummary.name,
          });
        }
      }
    }
  };

  return (
    <>
      <div
        onClick={onClose}
        className={`animate-fade-in fixed inset-0 z-40 flex bg-black/60 backdrop-blur-xs ${
          isExpanded ? 'items-center justify-center p-4 md:p-8' : 'justify-end'
        }`}
      >
        <div
          onClick={(e) => e.stopPropagation()}
          className={
            isExpanded
              ? 'animate-scale-in w-full max-w-5xl h-[88vh] bg-[#12151c] border border-white/[0.1] rounded-2xl flex flex-col shadow-2xl overflow-hidden'
              : 'animate-slide-right w-full max-w-2xl lg:max-w-3xl bg-[#12151c] border-l border-white/[0.08] h-full flex flex-col shadow-2xl'
          }
        >
          {/* Header */}
          <div className="px-6 py-4 border-b border-white/[0.06] flex items-center justify-between gap-4 shrink-0">
            <div className="flex items-center gap-3.5 min-w-0">
              {safeSummaryLogo ? (
                <img
                  src={safeSummaryLogo}
                  alt={modSummary.name}
                  className="w-11 h-11 rounded-xl object-cover bg-zinc-900 border border-white/[0.08] shrink-0"
                />
              ) : (
                <div className="w-11 h-11 rounded-xl bg-white/[0.04] border border-white/[0.06] flex items-center justify-center text-zinc-500 shrink-0">
                  <Package className="w-5 h-5" />
                </div>
              )}
              <div className="min-w-0">
                <div className="flex items-center gap-2 min-w-0">
                  <h2 className="text-base font-semibold text-zinc-100 truncate">
                    {modSummary.name}
                  </h2>
                  {modSummary.side && (
                    <span className="text-[10px] uppercase tracking-wide px-1.5 py-0.5 rounded bg-white/[0.06] text-zinc-300 shrink-0">
                      {modSummary.side}
                    </span>
                  )}
                  {evaluatedDeps && evaluatedDeps.modDeps.length > 0 && (
                    <DependencyTooltipBadge
                      evaluated={evaluatedDeps}
                      variant="remote"
                    />
                  )}
                </div>
                <p className="text-xs text-zinc-400 truncate mt-0.5">
                  by {modSummary.author || 'Unknown'}
                  {totalDownloads !== null && totalDownloads !== undefined && (
                    <> • {totalDownloads.toLocaleString()} downloads</>
                  )}
                  {totalFollows !== null && totalFollows !== undefined && (
                    <> • {totalFollows.toLocaleString()} follows</>
                  )}
                </p>
              </div>
            </div>

            <div className="flex items-center gap-2 shrink-0">
              {detail && latestRelease && latestRelease.mainfile && (
                <button
                  type="button"
                  onClick={() => onInstallRelease(detail, latestRelease)}
                  disabled={isDownloading}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-zinc-950 text-xs font-semibold cursor-pointer transition-colors"
                >
                  {isDownloading ? (
                    <>
                      <Loader2 className="w-3.5 h-3.5 animate-spin" />
                      <span>{pct !== null ? `${pct}%` : 'Installing...'}</span>
                    </>
                  ) : (
                    <>
                      <Download className="w-3.5 h-3.5" />
                      <span>Install v{latestRelease.modversion || 'Latest'}</span>
                    </>
                  )}
                </button>
              )}

              <button
                type="button"
                onClick={() => setIsExpanded((prev) => !prev)}
                title={isExpanded ? 'Dock to side panel' : 'Expand reader view'}
                aria-label={isExpanded ? 'Dock to side panel' : 'Expand reader view'}
                className="p-1.5 rounded-lg text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.06] cursor-pointer transition-colors"
              >
                {isExpanded ? (
                  <Minimize2 className="w-4 h-4" />
                ) : (
                  <Maximize2 className="w-4 h-4" />
                )}
              </button>

              <button
                type="button"
                onClick={onClose}
                title="Close (Esc)"
                aria-label="Close"
                className="p-1.5 rounded-lg text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.06] cursor-pointer transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
          </div>

          {/* Sub-header Navigation & External Links */}
          <div className="px-6 border-b border-white/[0.06] bg-[#10131a] flex flex-wrap items-center justify-between gap-2 shrink-0">
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => setActiveTab('description')}
                className={`flex items-center gap-1.5 px-3 py-2.5 text-xs font-medium border-b-2 transition-colors cursor-pointer ${
                  activeTab === 'description'
                    ? 'border-amber-400 text-amber-300'
                    : 'border-transparent text-zinc-400 hover:text-zinc-200'
                }`}
              >
                <FileText className="w-3.5 h-3.5" />
                <span>Description</span>
              </button>
              <button
                type="button"
                onClick={() => setActiveTab('releases')}
                className={`flex items-center gap-1.5 px-3 py-2.5 text-xs font-medium border-b-2 transition-colors cursor-pointer ${
                  activeTab === 'releases'
                    ? 'border-amber-400 text-amber-300'
                    : 'border-transparent text-zinc-400 hover:text-zinc-200'
                }`}
              >
                <Package className="w-3.5 h-3.5" />
                <span>Releases ({detail?.releases.length ?? 0})</span>
              </button>
            </div>

            <div className="flex flex-wrap items-center gap-2 py-1.5">
              {resourceLinks.map((link) => {
                const Icon = link.icon;
                return (
                  <button
                    key={link.label}
                    type="button"
                    onClick={() => openExternalSafely(link.url)}
                    className="flex items-center gap-1 px-2 py-1 rounded-md bg-white/[0.04] hover:bg-white/[0.08] text-[11px] text-zinc-300 hover:text-zinc-100 cursor-pointer transition-colors"
                  >
                    <Icon className="w-3 h-3 text-zinc-400" />
                    <span>{link.label}</span>
                  </button>
                );
              })}

              <button
                type="button"
                onClick={() => openExternalSafely(modDbUrl)}
                className="flex items-center gap-1 px-2.5 py-1 rounded-md bg-amber-500/10 hover:bg-amber-500/20 border border-amber-500/25 text-[11px] font-medium text-amber-300 cursor-pointer transition-colors"
              >
                <span>Open on Mod DB</span>
                <ExternalLink className="w-3 h-3" />
              </button>
            </div>
          </div>

          {/* Scrollable Body */}
          <div ref={scrollContainerRef} className="flex-1 overflow-y-auto p-6">
            {loading ? (
              <div className="flex items-center justify-center h-48 gap-2 text-xs text-zinc-400">
                <Loader2 className="w-4 h-4 animate-spin text-amber-400" />
                <span>Loading mod details...</span>
              </div>
            ) : error ? (
              <div className="p-4 rounded-xl bg-rose-500/10 border border-rose-500/20 text-xs text-rose-300">
                {error}
              </div>
            ) : detail ? (
              <div className={isExpanded ? 'max-w-3xl mx-auto space-y-5' : 'space-y-5'}>
                {activeTab === 'description' ? (
                  <>
                    {detail.tags.length > 0 && (
                      <div className="flex flex-wrap items-center gap-1.5">
                        {detail.tags.map((t) => (
                          <span
                            key={t}
                            className="text-[11px] px-2.5 py-0.5 rounded-md bg-white/[0.05] border border-white/[0.05] text-zinc-300"
                          >
                            {t}
                          </span>
                        ))}
                      </div>
                    )}

                    {evaluatedDeps && evaluatedDeps.modDeps.length > 0 && (
                      <div className="p-4 rounded-xl bg-[#161922] border border-amber-500/20 space-y-2.5">
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex items-center gap-2 text-xs font-semibold text-zinc-100">
                            <GitBranch className="w-3.5 h-3.5 text-amber-400" />
                            <span>
                              Mod Dependencies ({evaluatedDeps.modDeps.length})
                            </span>
                          </div>
                          {(evaluatedDeps.gameRequirements.game ||
                            evaluatedDeps.gameRequirements.Game) && (
                            <span className="text-[11px] font-mono text-zinc-400">
                              Requires VS{' '}
                              {evaluatedDeps.gameRequirements.game ||
                                evaluatedDeps.gameRequirements.Game}
                            </span>
                          )}
                        </div>
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                          {evaluatedDeps.modDeps.map((dep) => (
                            <div
                              key={dep.modid}
                              className="flex items-center justify-between gap-2 rounded-lg bg-[#0e1015] border border-white/[0.06] px-3 py-2 text-xs"
                            >
                              <div className="min-w-0">
                                <div className="font-medium text-zinc-100 truncate">
                                  {dep.name}
                                </div>
                                <div className="text-[10px] font-mono text-zinc-400 truncate">
                                  {dep.modid} •{' '}
                                  {formatVersionRequirement(dep.requiredVersion)}
                                </div>
                              </div>
                              {dep.status === 'installed' ? (
                                <span className="inline-flex items-center gap-1 text-[10px] font-medium px-1.5 py-0.5 rounded bg-emerald-500/15 text-emerald-300 border border-emerald-500/25 shrink-0">
                                  <CheckCircle2 className="w-2.5 h-2.5" />
                                  <span>Installed</span>
                                </span>
                              ) : dep.status === 'disabled' ? (
                                <span className="inline-flex items-center gap-1 text-[10px] font-medium px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300 border border-amber-500/25 shrink-0">
                                  <PauseCircle className="w-2.5 h-2.5" />
                                  <span>Disabled</span>
                                </span>
                              ) : (
                                <span className="inline-flex items-center gap-1 text-[10px] font-medium px-1.5 py-0.5 rounded bg-rose-500/15 text-rose-300 border border-rose-500/25 shrink-0">
                                  <AlertTriangle className="w-2.5 h-2.5" />
                                  <span>Not Installed</span>
                                </span>
                              )}
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                    {normalizedDescriptionHtml.trim() ? (
                      <div className="bg-[#161922] px-6 py-5 rounded-xl border border-white/[0.06]">
                        <div
                          onClick={handleRichHtmlClick}
                          className={`mod-description-content ${
                            isExpanded ? 'mod-description-expanded' : ''
                          }`}
                          dangerouslySetInnerHTML={{ __html: normalizedDescriptionHtml }}
                        />
                      </div>
                    ) : (
                      <div className="bg-[#161922] px-6 py-5 rounded-xl border border-white/[0.06] text-sm text-zinc-400 leading-relaxed">
                        {modSummary.summary || 'No detailed description provided for this mod.'}
                      </div>
                    )}
                  </>
                ) : detail.releases.length === 0 ? (
                  <div className="p-6 rounded-xl bg-[#161922] border border-white/[0.06] text-center text-xs text-zinc-400">
                    No releases available for this mod.
                  </div>
                ) : (
                  <div className="space-y-3">
                    {detail.releases.map((rel) => {
                      const normalizedChangelog = rel.changelog?.trim()
                        ? sanitizeModDbHtml(rel.changelog)
                        : '';
                      return (
                        <div
                          key={rel.releaseid}
                          className="p-4 rounded-xl bg-[#161922] border border-white/[0.06] space-y-3"
                        >
                          <div className="flex items-start justify-between gap-3">
                            <div className="min-w-0 space-y-1.5">
                              <div className="flex flex-wrap items-center gap-2">
                                <span className="text-sm font-semibold text-zinc-100 font-mono">
                                  v{rel.modversion || '?'}
                                </span>
                                {rel.created && (
                                  <span className="text-[11px] text-zinc-500">
                                    {rel.created.split(' ')[0]}
                                  </span>
                                )}
                                {rel.tags.map((gv) => (
                                  <span
                                    key={gv}
                                    className="text-[10px] px-1.5 py-0.5 rounded bg-white/[0.05] text-zinc-300 border border-white/[0.05]"
                                  >
                                    {gv}
                                  </span>
                                ))}
                              </div>
                              <p className="text-xs text-zinc-400 truncate">
                                {rel.filename || 'Unknown file'} •{' '}
                                {(rel.downloads ?? 0).toLocaleString()} downloads
                              </p>
                            </div>

                            <button
                              type="button"
                              onClick={() => onInstallRelease(detail, rel)}
                              disabled={!rel.mainfile || isDownloading}
                              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-zinc-950 text-xs font-semibold shrink-0 cursor-pointer transition-colors"
                            >
                              {isDownloading ? (
                                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                              ) : (
                                <Download className="w-3.5 h-3.5" />
                              )}
                              <span>Install</span>
                            </button>
                          </div>

                          {normalizedChangelog && (
                            <div className="pt-3 border-t border-white/[0.05]">
                              <div
                                onClick={handleRichHtmlClick}
                                className="mod-description-content mod-description-compact"
                                dangerouslySetInnerHTML={{ __html: normalizedChangelog }}
                              />
                            </div>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            ) : null}
          </div>
        </div>
      </div>

      {/* Image Lightbox Overlay */}
      {lightboxImage && (
        <div
          onClick={() => setLightboxImage(null)}
          className="animate-fade-in fixed inset-0 z-50 flex items-center justify-center bg-black/85 backdrop-blur-md p-6"
        >
          <button
            type="button"
            onClick={() => setLightboxImage(null)}
            aria-label="Close image preview"
            className="fixed top-5 right-5 p-2 rounded-full bg-white/10 hover:bg-white/20 text-zinc-100 cursor-pointer transition-colors"
          >
            <X className="w-5 h-5" />
          </button>

          <div
            onClick={(e) => e.stopPropagation()}
            className="flex flex-col items-center gap-3 max-w-[92vw] max-h-[90vh]"
          >
            <img
              src={lightboxImage.src}
              alt={lightboxImage.alt}
              className="max-w-[92vw] max-h-[84vh] object-contain rounded-xl shadow-2xl border border-white/[0.1]"
            />
            {lightboxImage.alt && (
              <p className="text-xs text-zinc-400 text-center max-w-xl truncate">
                {lightboxImage.alt}
              </p>
            )}
          </div>
        </div>
      )}
    </>
  );
}
