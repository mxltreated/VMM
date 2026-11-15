import { Compass, Globe, Layers, Package, Settings, SlidersHorizontal, Flame } from 'lucide-react';
import type { GameStatus, NavTab } from '../types';

interface SidebarProps {
  activeTab: NavTab;
  onSelectTab: (tab: NavTab) => void;
  enabledCount: number;
  totalCount: number;
  gameStatus: GameStatus;
}

const NAV_ITEMS: { id: NavTab; label: string; icon: typeof Package }[] = [
  { id: 'installed', label: 'Installed', icon: Package },
  { id: 'browse', label: 'Mod DB', icon: Compass },
  { id: 'configs', label: 'Configs', icon: SlidersHorizontal },
  { id: 'profiles', label: 'Profiles', icon: Layers },
  { id: 'multiplayer', label: 'Multiplayer', icon: Globe },
  { id: 'settings', label: 'Settings', icon: Settings },
];

export function Sidebar({
  activeTab,
  onSelectTab,
  enabledCount,
  totalCount,
  gameStatus,
}: SidebarProps) {
  return (
    <aside className="w-56 shrink-0 border-r border-white/[0.06] bg-[#12151c] flex flex-col justify-between select-none">
      <div>
        <div className="px-5 py-5 flex items-center gap-3">
          <div className="w-8 h-8 rounded-lg bg-amber-500/15 border border-amber-500/30 flex items-center justify-center text-amber-400">
            <Flame className="w-4 h-4" />
          </div>
          <div>
            <div className="text-sm font-semibold tracking-tight text-zinc-100">
              Vintage Mod Manager
            </div>
            <div className="text-[11px] text-zinc-500 font-medium">
              VMM • v1.1.7
            </div>
          </div>
        </div>

        <nav className="px-3 mt-2 space-y-1">
          {NAV_ITEMS.map((item) => {
            const Icon = item.icon;
            const active = activeTab === item.id;
            return (
              <button
                key={item.id}
                onClick={() => onSelectTab(item.id)}
                className={`group w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-all duration-150 active:scale-[0.98] cursor-pointer ${
                  active
                    ? 'bg-amber-500/12 text-amber-300 border border-amber-500/20 shadow-sm shadow-amber-500/5'
                    : 'text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.04] border border-transparent'
                }`}
              >
                <Icon
                  className={`w-4 h-4 shrink-0 transition-transform duration-150 ${
                    active ? 'scale-105' : 'group-hover:scale-110'
                  }`}
                />
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>
      </div>

      <div className="p-4 m-3 rounded-xl bg-white/[0.025] border border-white/[0.05] space-y-2.5">
        <div className="flex items-center justify-between text-xs">
          <span className="text-zinc-500">Active Mods</span>
          <span className="text-zinc-200 font-medium tabular-nums">
            {enabledCount} / {totalCount}
          </span>
        </div>
        <div className="flex items-center justify-between text-xs">
          <span className="text-zinc-500">Game</span>
          <span className="flex items-center gap-1.5 font-medium">
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                gameStatus.running ? 'bg-emerald-400 animate-pulse' : 'bg-zinc-600'
              }`}
            />
            <span className={gameStatus.running ? 'text-emerald-400' : 'text-zinc-400'}>
              {gameStatus.running ? 'Running' : 'Idle'}
            </span>
          </span>
        </div>
      </div>
    </aside>
  );
}
