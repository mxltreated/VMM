import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { AlertTriangle, CheckCircle2, GitBranch, PauseCircle } from 'lucide-react';
import {
  formatVersionRequirement,
  type EvaluatedDependencies,
} from '../utils/dependencies';

interface DependencyTooltipBadgeProps {
  evaluated: EvaluatedDependencies;
  /** 'local' warns in rose/amber for missing/disabled deps; 'remote' uses neutral/amber styling for Browse & Detail views. */
  variant?: 'local' | 'remote';
  compact?: boolean;
}

export function DependencyTooltipBadge({
  evaluated,
  variant = 'local',
  compact = false,
}: DependencyTooltipBadgeProps) {
  const { modDeps, gameRequirements, missingCount, disabledCount } = evaluated;
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<{
    top: number;
    left: number;
    placeAbove: boolean;
  }>({ top: 0, left: 0, placeAbove: false });

  const updatePosition = useCallback(() => {
    const el = triggerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const tooltipWidth = 288;
    const estimatedHeight = Math.min(260, 74 + modDeps.length * 36);
    const spaceBelow = window.innerHeight - rect.bottom;
    const placeAbove = spaceBelow < estimatedHeight + 16 && rect.top > estimatedHeight + 16;

    const top = placeAbove ? rect.top - 8 : rect.bottom + 8;
    const clampedLeft = Math.max(
      12,
      Math.min(rect.left, window.innerWidth - tooltipWidth - 12)
    );
    setCoords({ top, left: clampedLeft, placeAbove });
  }, [modDeps.length]);

  useEffect(() => {
    if (!open) return;
    updatePosition();
    const onScrollOrResize = () => updatePosition();
    window.addEventListener('scroll', onScrollOrResize, true);
    window.addEventListener('resize', onScrollOrResize);
    return () => {
      window.removeEventListener('scroll', onScrollOrResize, true);
      window.removeEventListener('resize', onScrollOrResize);
    };
  }, [open, updatePosition]);

  if (modDeps.length === 0) {
    return null;
  }

  let badgeClass =
    'bg-white/[0.05] text-zinc-300 border-white/[0.08] hover:bg-white/[0.09]';
  let Icon = GitBranch;
  let label = `${modDeps.length} ${modDeps.length === 1 ? 'dep' : 'deps'}`;

  if (variant === 'local') {
    if (missingCount > 0) {
      badgeClass =
        'bg-rose-500/15 text-rose-300 border-rose-500/30 hover:bg-rose-500/25';
      Icon = AlertTriangle;
      label = `${missingCount} missing ${missingCount === 1 ? 'dep' : 'deps'}`;
    } else if (disabledCount > 0) {
      badgeClass =
        'bg-amber-500/15 text-amber-300 border-amber-500/30 hover:bg-amber-500/25';
      Icon = PauseCircle;
      label = `${disabledCount} disabled ${disabledCount === 1 ? 'dep' : 'deps'}`;
    }
  } else if (missingCount > 0 || disabledCount > 0) {
    badgeClass =
      'bg-amber-500/12 text-amber-300 border-amber-500/25 hover:bg-amber-500/20';
  } else {
    badgeClass =
      'bg-emerald-500/12 text-emerald-300 border-emerald-500/25 hover:bg-emerald-500/20';
  }

  const gameVersion = gameRequirements.game || gameRequirements.Game;

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          setOpen((prev) => !prev);
        }}
        onMouseEnter={() => {
          updatePosition();
          setOpen(true);
        }}
        onMouseLeave={() => setOpen(false)}
        onFocus={() => {
          updatePosition();
          setOpen(true);
        }}
        onBlur={() => setOpen(false)}
        aria-label={`Mod dependencies (${modDeps.length})`}
        className={`inline-flex items-center gap-1 rounded-full border font-medium transition-colors cursor-default shrink-0 ${
          compact ? 'text-[10px] px-1.5 py-0.5' : 'text-[11px] px-2 py-0.5'
        } ${badgeClass}`}
      >
        <Icon className={compact ? 'w-2.5 h-2.5 shrink-0' : 'w-3 h-3 shrink-0'} />
        <span>{label}</span>
      </button>

      {open &&
        createPortal(
          <div
            role="tooltip"
            style={{
              position: 'fixed',
              top: coords.top,
              left: coords.left,
              transform: coords.placeAbove ? 'translateY(-100%)' : 'none',
              zIndex: 9999,
            }}
            className="pointer-events-none w-72 rounded-xl bg-[#161922]/98 backdrop-blur-md border border-white/[0.12] shadow-2xl p-3 text-xs text-zinc-200 animate-fade-in"
          >
            <div className="flex items-center justify-between gap-2 pb-2 mb-2 border-b border-white/[0.07]">
              <div className="flex items-center gap-1.5 font-semibold text-zinc-100">
                <GitBranch className="w-3.5 h-3.5 text-amber-400" />
                <span>Mod Dependencies ({modDeps.length})</span>
              </div>
              {gameVersion && (
                <span className="text-[10px] font-mono text-zinc-400">
                  VS {gameVersion}
                </span>
              )}
            </div>

            <div className="space-y-1.5 max-h-52 overflow-y-auto pr-0.5">
              {modDeps.map((dep) => {
                const statusBadge =
                  dep.status === 'installed' ? (
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
                      <span>{variant === 'local' ? 'Missing' : 'Needed'}</span>
                    </span>
                  );

                return (
                  <div
                    key={dep.modid}
                    className="flex items-center justify-between gap-2 rounded-lg bg-[#0e1015]/80 border border-white/[0.05] px-2 py-1.5"
                  >
                    <div className="min-w-0">
                      <div className="font-medium text-zinc-100 truncate">
                        {dep.name}
                      </div>
                      <div className="text-[10px] text-zinc-400 font-mono truncate">
                        {dep.modid} • {formatVersionRequirement(dep.requiredVersion)}
                      </div>
                    </div>
                    {statusBadge}
                  </div>
                );
              })}
            </div>
          </div>,
          document.body
        )}
    </>
  );
}
