import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Download,
  Flame,
  Loader2,
  Package,
  RefreshCw,
  Search,
  Sparkles,
  Tag,
  X,
} from 'lucide-react';
import { api } from '../api/tauri';
import type {
  DownloadProgressEvent,
  LocalMod,
  ModDbFilters,
  ModDbSummary,
} from '../types';

interface BrowseModsViewProps {
  installedMods: LocalMod[];
  downloads: Record<string, DownloadProgressEvent>;
  onSelectMod: (mod: ModDbSummary) => void;
  onQuickInstall: (mod: ModDbSummary) => void;
}

export function BrowseModsView({
  installedMods,
  downloads,
  onSelectMod,
  onQuickInstall,
}: BrowseModsViewProps) {
  const [query, setQuery] = useState('');
  const [selectedTags, setSelectedTags] = useState<number[]>([]);
  const [selectedGameVersion, setSelectedGameVersion] = useState<string>('');
  const [orderBy, setOrderBy] = useState<string>('trendingpoints');
  const [mods, setMods] = useState<ModDbSummary[]>([]);
  const [featuredPool, setFeaturedPool] = useState<ModDbSummary[]>([]);
  const [filters, setFilters] = useState<ModDbFilters | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [visibleLimit, setVisibleLimit] = useState(48);
  const [refreshTick, setRefreshTick] = useState(0);
  const [tagMenuOpen, setTagMenuOpen] = useState(false);
  const [featuredIndex, setFeaturedIndex] = useState(0);
  const forceRefreshRef = useRef(false);
  const tagMenuRef = useRef<HTMLDivElement>(null);

  const installedModIds = useMemo(() => {
    const set = new Set<string>();
    for (const m of installedMods) {
      if (m.modid) set.add(m.modid.toLowerCase());
    }
    return set;
  }, [installedMods]);

  useEffect(() => {
    api.getModDbFilters().then(setFilters).catch(() => {});
  }, []);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (tagMenuRef.current && !tagMenuRef.current.contains(e.target as Node)) {
        setTagMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  useEffect(() => {
    let cancelled = false;
    const timer = setTimeout(async () => {
      setLoading(true);
      setError(null);
      setVisibleLimit(48);
      try {
        const force = forceRefreshRef.current;
        forceRefreshRef.current = false;
        const res = await api.searchModDb({
          text: query,
          tagIds: selectedTags,
          gameVersionId: selectedGameVersion ? Number(selectedGameVersion) : null,
          orderBy,
          forceRefresh: force,
        });
        if (!cancelled) {
          setMods(res);
          if (
            !query.trim() &&
            selectedTags.length === 0 &&
            !selectedGameVersion &&
            orderBy === 'trendingpoints'
          ) {
            setFeaturedPool(res);
          }
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }, 280);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [query, selectedTags, selectedGameVersion, orderBy, refreshTick]);

  const featuredMods = useMemo(() => {
    if (featuredPool.length === 0) return [];
    const maxDownloads = Math.max(
      1,
      ...featuredPool.map((m) => m.downloads ?? 0)
    );
    const maxTrending = Math.max(
      1,
      ...featuredPool.map((m) => Math.max(0, m.trendingpoints ?? 0))
    );
    return [...featuredPool]
      .sort((a, b) => {
        const scoreA =
          (Math.max(0, a.trendingpoints ?? 0) / maxTrending) * 0.55 +
          ((a.downloads ?? 0) / maxDownloads) * 0.45;
        const scoreB =
          (Math.max(0, b.trendingpoints ?? 0) / maxTrending) * 0.55 +
          ((b.downloads ?? 0) / maxDownloads) * 0.45;
        return scoreB - scoreA;
      })
      .slice(0, 9);
  }, [featuredPool]);

  const pageSize = 3;
  const totalFeaturedPages = Math.max(1, Math.ceil(featuredMods.length / pageSize));
  const currentFeaturedPage = featuredIndex % totalFeaturedPages;
  const visibleFeatured = featuredMods.slice(
    currentFeaturedPage * pageSize,
    currentFeaturedPage * pageSize + pageSize
  );

  const hasActiveFilters =
    Boolean(query.trim()) ||
    selectedTags.length > 0 ||
    Boolean(selectedGameVersion);

  const toggleTag = (tagId: number) => {
    setSelectedTags((prev) =>
      prev.includes(tagId) ? prev.filter((id) => id !== tagId) : [...prev, tagId]
    );
  };

  const clearAllFilters = () => {
    setQuery('');
    setSelectedTags([]);
    setSelectedGameVersion('');
    setTagMenuOpen(false);
  };

  const visibleMods = useMemo(
    () => mods.slice(0, visibleLimit),
    [mods, visibleLimit]
  );

  return (
    <div className="flex flex-col h-full">
      <div className="px-6 py-4 border-b border-white/[0.05] space-y-2.5">
        <div className="flex flex-wrap items-center gap-3">
          <div className="relative flex-1 min-w-[220px] max-w-sm">
            <Search className="w-3.5 h-3.5 text-zinc-500 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search Vintage Story Mod DB..."
              className="w-full bg-[#161922] border border-white/[0.07] rounded-lg pl-8 pr-3 py-1.5 text-xs text-zinc-200 placeholder:text-zinc-500 focus:outline-none focus:border-amber-500/40"
            />
          </div>

          <select
            value={selectedGameVersion}
            onChange={(e) => setSelectedGameVersion(e.target.value)}
            className="bg-[#161922] border border-white/[0.07] rounded-lg px-2.5 py-1.5 text-xs text-zinc-300 focus:outline-none focus:border-amber-500/40 cursor-pointer"
          >
            <option value="">All Game Versions</option>
            {filters?.gameVersions.slice(0, 40).map((gv) => (
              <option key={gv.tagid} value={String(gv.tagid)}>
                VS {gv.name}
              </option>
            ))}
          </select>

          {/* Multi-select Tag dropdown */}
          <div className="relative" ref={tagMenuRef}>
            <button
              type="button"
              onClick={() => setTagMenuOpen((o) => !o)}
              className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border text-xs transition-colors cursor-pointer ${
                selectedTags.length > 0
                  ? 'bg-amber-500/15 border-amber-500/35 text-amber-300'
                  : 'bg-[#161922] border-white/[0.07] text-zinc-300 hover:bg-white/[0.04]'
              }`}
            >
              <Tag className="w-3.5 h-3.5" />
              <span>
                {selectedTags.length > 0
                  ? `Tags (${selectedTags.length})`
                  : 'Tags'}
              </span>
              <ChevronDown className="w-3.5 h-3.5 opacity-70" />
            </button>

            {tagMenuOpen && filters && (
              <div className="animate-scale-in origin-top-left absolute left-0 mt-1.5 w-60 max-h-64 overflow-y-auto rounded-xl bg-[#161922] border border-white/[0.1] shadow-2xl p-1.5 z-30">
                {filters.tags.map((tag) => {
                  const checked = selectedTags.includes(tag.tagid);
                  return (
                    <button
                      key={tag.tagid}
                      type="button"
                      onClick={() => toggleTag(tag.tagid)}
                      className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-xs text-left transition-colors cursor-pointer ${
                        checked
                          ? 'bg-amber-500/15 text-amber-300'
                          : 'text-zinc-300 hover:bg-white/[0.05]'
                      }`}
                    >
                      <span className="truncate">{tag.name}</span>
                      {checked && <Check className="w-3.5 h-3.5 shrink-0" />}
                    </button>
                  );
                })}
              </div>
            )}
          </div>

          <select
            value={orderBy}
            onChange={(e) => setOrderBy(e.target.value)}
            className="bg-[#161922] border border-white/[0.07] rounded-lg px-2.5 py-1.5 text-xs text-zinc-300 focus:outline-none focus:border-amber-500/40 cursor-pointer"
          >
            <option value="trendingpoints">Trending</option>
            <option value="downloads">Most Downloaded</option>
            <option value="follows">Most Followed</option>
            <option value="lastreleased">Recently Updated</option>
          </select>

          {hasActiveFilters && (
            <button
              type="button"
              onClick={clearAllFilters}
              className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-white/[0.05] hover:bg-white/[0.09] text-xs text-zinc-300 cursor-pointer"
            >
              <X className="w-3.5 h-3.5" />
              <span>Clear Filters</span>
            </button>
          )}

          <div className="ml-auto flex items-center gap-2.5">
            <span className="text-xs text-zinc-500 tabular-nums">
              {!loading && `${mods.length.toLocaleString()} mods`}
            </span>
            <button
              onClick={() => {
                forceRefreshRef.current = true;
                setRefreshTick((t) => t + 1);
              }}
              disabled={loading}
              title="Refresh Mod DB cache"
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-[#161922] hover:bg-white/[0.06] border border-white/[0.07] text-xs text-zinc-300 cursor-pointer disabled:opacity-50"
            >
              <RefreshCw
                className={`w-3.5 h-3.5 ${loading ? 'animate-spin text-amber-400' : ''}`}
              />
              <span>Refresh</span>
            </button>
          </div>
        </div>

        {/* Selected tag chips */}
        {selectedTags.length > 0 && filters && (
          <div className="flex flex-wrap items-center gap-1.5">
            {selectedTags.map((id) => {
              const tag = filters.tags.find((t) => t.tagid === id);
              if (!tag) return null;
              return (
                <button
                  key={id}
                  onClick={() => toggleTag(id)}
                  className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-amber-500/15 border border-amber-500/30 text-[11px] text-amber-300 hover:bg-amber-500/25 cursor-pointer"
                >
                  <span>{tag.name}</span>
                  <X className="w-3 h-3" />
                </button>
              );
            })}
          </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        {/* Featured Spotlight Carousel */}
        {!hasActiveFilters && visibleFeatured.length > 0 && !loading && (
          <section className="space-y-2.5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Flame className="w-4 h-4 text-amber-400" />
                <h2 className="text-xs font-semibold uppercase tracking-wider text-zinc-300">
                  Featured — Top Trending &amp; Downloaded
                </h2>
              </div>

              <div className="flex items-center gap-2">
                <div className="flex items-center gap-1">
                  {Array.from({ length: totalFeaturedPages }).map((_, idx) => (
                    <button
                      key={idx}
                      type="button"
                      onClick={() => setFeaturedIndex(idx)}
                      aria-label={`Show featured page ${idx + 1}`}
                      className={`h-1.5 rounded-full transition-all cursor-pointer ${
                        idx === currentFeaturedPage
                          ? 'w-5 bg-amber-400'
                          : 'w-1.5 bg-zinc-700 hover:bg-zinc-500'
                      }`}
                    />
                  ))}
                </div>

                <div className="flex items-center gap-1 ml-1">
                  <button
                    type="button"
                    onClick={() =>
                      setFeaturedIndex(
                        (i) => (i - 1 + totalFeaturedPages) % totalFeaturedPages
                      )
                    }
                    className="p-1 rounded-lg bg-[#161922] hover:bg-white/[0.08] border border-white/[0.07] text-zinc-300 cursor-pointer"
                  >
                    <ChevronLeft className="w-3.5 h-3.5" />
                  </button>
                  <button
                    type="button"
                    onClick={() =>
                      setFeaturedIndex((i) => (i + 1) % totalFeaturedPages)
                    }
                    className="p-1 rounded-lg bg-[#161922] hover:bg-white/[0.08] border border-white/[0.07] text-zinc-300 cursor-pointer"
                  >
                    <ChevronRight className="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>
            </div>

            <div
              key={currentFeaturedPage}
              className="animate-fade-in grid grid-cols-1 lg:grid-cols-3 gap-3.5"
            >
              {visibleFeatured.map((mod) => {
                const isInstalled = mod.modidstrs.some((s) =>
                  installedModIds.has(s.toLowerCase())
                );
                const dlKey = mod.modidstrs[0] || String(mod.modid);
                const dl = downloads[dlKey] || downloads[String(mod.modid)];
                const isDownloading = Boolean(dl && !dl.done);

                return (
                  <div
                    key={`feat-${mod.modid}`}
                    onClick={() => onSelectMod(mod)}
                    className="group p-4 rounded-2xl bg-gradient-to-br from-amber-500/[0.09] via-[#141720] to-[#12151c] border border-amber-500/25 hover:border-amber-500/50 hover:-translate-y-0.5 hover:shadow-lg hover:shadow-amber-500/5 transition-all duration-200 cursor-pointer flex flex-col justify-between gap-3"
                  >
                    <div className="space-y-2.5">
                      <div className="flex items-start gap-3">
                        {mod.logo ? (
                          <img
                            src={mod.logo}
                            alt={mod.name}
                            className="w-12 h-12 rounded-xl object-cover bg-zinc-900 shrink-0 border border-white/[0.08]"
                          />
                        ) : (
                          <div className="w-12 h-12 rounded-xl bg-amber-500/10 flex items-center justify-center text-amber-300 shrink-0">
                            <Package className="w-5 h-5" />
                          </div>
                        )}

                        <div className="min-w-0 flex-1">
                          <h3 className="text-sm font-semibold text-zinc-100 truncate group-hover:text-amber-300 transition-colors">
                            {mod.name}
                          </h3>
                          <div className="flex items-center gap-1.5 text-[11px] text-zinc-400 truncate mt-0.5">
                            <span className="truncate">by {mod.author || 'Unknown'}</span>
                            {mod.side && (
                              <span className="text-[9px] uppercase px-1 py-0.2 rounded bg-white/[0.06] text-zinc-300 shrink-0">
                                {mod.side}
                              </span>
                            )}
                          </div>
                        </div>
                      </div>

                      <div className="flex items-start gap-1.5 text-xs text-zinc-300 leading-relaxed">
                        <span className="shrink-0 text-[9px] font-semibold uppercase tracking-wider px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300 border border-amber-500/25 mt-0.5">
                          TL;DR
                        </span>
                        <p className="line-clamp-2 text-zinc-300 text-[11px]">
                          {mod.summary ||
                            'Popular community mod — click for details & releases.'}
                        </p>
                      </div>
                    </div>

                    <div className="pt-2.5 border-t border-white/[0.06] flex items-center justify-between gap-2">
                      <span className="text-[11px] text-zinc-400 tabular-nums truncate">
                        {(mod.downloads ?? 0).toLocaleString()} dl •{' '}
                        {(mod.follows ?? 0).toLocaleString()} follows
                      </span>

                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          onQuickInstall(mod);
                        }}
                        disabled={isDownloading}
                        className={`flex items-center gap-1 px-2.5 py-1 rounded-lg text-[11px] font-semibold shrink-0 transition-colors cursor-pointer ${
                          isInstalled
                            ? 'bg-emerald-500/15 text-emerald-300 border border-emerald-500/30 hover:bg-emerald-500/25'
                            : 'bg-amber-500 hover:bg-amber-400 text-zinc-950'
                        }`}
                      >
                        {isDownloading ? (
                          <>
                            <Loader2 className="w-3 h-3 animate-spin" />
                            <span>...</span>
                          </>
                        ) : isInstalled ? (
                          <>
                            <Check className="w-3 h-3" />
                            <span>Installed</span>
                          </>
                        ) : (
                          <>
                            <Download className="w-3 h-3" />
                            <span>Install</span>
                          </>
                        )}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </section>
        )}

        {loading ? (
          <div className="flex items-center justify-center h-56 gap-2 text-xs text-zinc-400">
            <Loader2 className="w-4 h-4 animate-spin text-amber-400" />
            <span>Querying mods.vintagestory.at...</span>
          </div>
        ) : error ? (
          <div className="p-4 rounded-xl bg-rose-500/10 border border-rose-500/20 text-xs text-rose-300">
            {error}
          </div>
        ) : visibleMods.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-56 text-center">
            <Sparkles className="w-7 h-7 text-zinc-600 mb-2" />
            <p className="text-sm text-zinc-300 font-medium">No mods found</p>
            <p className="text-xs text-zinc-500 mt-1">
              Try a different search term or clear your filters.
            </p>
          </div>
        ) : (
          <>
            <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3.5">
              {visibleMods.map((mod) => {
                const isInstalled = mod.modidstrs.some((s) =>
                  installedModIds.has(s.toLowerCase())
                );
                const dlKey = mod.modidstrs[0] || String(mod.modid);
                const dl = downloads[dlKey] || downloads[String(mod.modid)];
                const isDownloading = Boolean(dl && !dl.done);
                const pct =
                  dl && dl.totalBytes
                    ? Math.min(100, Math.round((dl.bytesReceived / dl.totalBytes) * 100))
                    : null;

                return (
                  <div
                    key={mod.modid}
                    onClick={() => onSelectMod(mod)}
                    className="group flex flex-col justify-between p-4 rounded-xl bg-[#141720] border border-white/[0.06] hover:border-amber-500/35 hover:-translate-y-0.5 hover:bg-[#161a24] transition-all duration-200 cursor-pointer"
                  >
                    <div>
                      <div className="flex items-start gap-3">
                        {mod.logo ? (
                          <img
                            src={mod.logo}
                            alt={mod.name}
                            loading="lazy"
                            className="w-12 h-12 rounded-lg object-cover bg-zinc-900 shrink-0 border border-white/[0.06]"
                          />
                        ) : (
                          <div className="w-12 h-12 rounded-lg bg-white/[0.04] border border-white/[0.06] flex items-center justify-center text-zinc-500 shrink-0">
                            <Package className="w-5 h-5" />
                          </div>
                        )}

                        <div className="min-w-0 flex-1">
                          <h3 className="text-sm font-semibold text-zinc-100 truncate group-hover:text-amber-300 transition-colors">
                            {mod.name}
                          </h3>
                          <p className="text-xs text-zinc-500 truncate">
                            by {mod.author || 'Unknown'}
                          </p>
                          <div className="flex items-center gap-1.5 mt-1.5 flex-wrap">
                            {mod.tags.slice(0, 2).map((tag) => (
                              <span
                                key={tag}
                                className="text-[10px] px-1.5 py-0.5 rounded bg-white/[0.04] text-zinc-400 border border-white/[0.05]"
                              >
                                {tag}
                              </span>
                            ))}
                          </div>
                        </div>
                      </div>

                      <p className="text-xs text-zinc-400 mt-3 line-clamp-2 leading-relaxed">
                        {mod.summary || 'No summary provided.'}
                      </p>
                    </div>

                    <div className="mt-4 pt-3 border-t border-white/[0.05] flex items-center justify-between gap-2">
                      <span className="text-[11px] text-zinc-500 tabular-nums">
                        {(mod.downloads ?? 0).toLocaleString()} downloads
                      </span>

                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          onQuickInstall(mod);
                        }}
                        disabled={isDownloading}
                        className={`flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
                          isInstalled
                            ? 'bg-emerald-500/12 text-emerald-300 border border-emerald-500/25 hover:bg-emerald-500/20'
                            : 'bg-amber-500/15 text-amber-300 border border-amber-500/30 hover:bg-amber-500/25'
                        }`}
                      >
                        {isDownloading ? (
                          <>
                            <Loader2 className="w-3 h-3 animate-spin" />
                            <span>{pct !== null ? `${pct}%` : 'Downloading'}</span>
                          </>
                        ) : isInstalled ? (
                          <>
                            <Check className="w-3 h-3" />
                            <span>Installed</span>
                          </>
                        ) : (
                          <>
                            <Download className="w-3 h-3" />
                            <span>Install</span>
                          </>
                        )}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>

            {visibleLimit < mods.length && (
              <div className="mt-6 flex justify-center">
                <button
                  onClick={() => setVisibleLimit((n) => n + 48)}
                  className="px-4 py-2 rounded-lg bg-[#161922] hover:bg-white/[0.06] border border-white/[0.07] text-xs font-medium text-zinc-300 cursor-pointer"
                >
                  Load More ({mods.length - visibleLimit} remaining)
                </button>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
