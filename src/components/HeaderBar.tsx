import { Layers, Play, Loader2 } from 'lucide-react';
import type { GameStatus, ModProfile, NavTab } from '../types';

interface HeaderBarProps {
  activeTab: NavTab;
  profiles: ModProfile[];
  activeProfileId: string | null;
  onSwitchProfile: (profileId: string) => void;
  onLaunchGame: () => void;
  launching: boolean;
  gameStatus: GameStatus;
}

const TAB_TITLES: Record<NavTab, { title: string; subtitle: string }> = {
  installed: {
    title: 'Installed Mods',
    subtitle: 'Manage enabled mods and ModsDisabled archive',
  },
  browse: {
    title: 'Browse Mod DB',
    subtitle: 'Discover and download mods from mods.vintagestory.at',
  },
  configs: {
    title: 'Mod Configs',
    subtitle: 'Tweak mod values in ModConfig/ before launching Vintage Story',
  },
  profiles: {
    title: 'Mod Profiles',
    subtitle: 'Switch between curated mod sets or share modpacks with friends',
  },
  multiplayer: {
    title: 'Multiplayer & P2P',
    subtitle: 'Host or join a zero-download, encrypted peer-to-peer Vintage Story room',
  },
  logs: {
    title: 'Game Logs & Crash Assistant',
    subtitle: 'Inspect client/server logs and detect mods causing crashes or errors',
  },
  settings: {
    title: 'Settings',
    subtitle: 'Configure game executable and data directory',
  },
};

export function HeaderBar({
  activeTab,
  profiles,
  activeProfileId,
  onSwitchProfile,
  onLaunchGame,
  launching,
  gameStatus,
}: HeaderBarProps) {
  const info = TAB_TITLES[activeTab];

  return (
    <header className="h-16 shrink-0 border-b border-white/[0.06] bg-[#0e1015]/80 backdrop-blur-md px-6 flex items-center justify-between">
      <div key={activeTab} className="animate-fade-in">
        <h1 className="text-base font-semibold text-zinc-100 tracking-tight">
          {info.title}
        </h1>
        <p className="text-xs text-zinc-500">{info.subtitle}</p>
      </div>

      <div className="flex items-center gap-3">
        <div className="flex items-center gap-2 bg-[#161922] border border-white/[0.07] hover:border-white/[0.14] transition-colors rounded-lg px-2.5 py-1.5">
          <Layers className="w-3.5 h-3.5 text-zinc-400" />
          <select
            value={activeProfileId ?? 'default'}
            onChange={(e) => onSwitchProfile(e.target.value)}
            className="bg-transparent text-xs font-medium text-zinc-200 focus:outline-none cursor-pointer pr-1"
            title="Switch active mod profile"
          >
            {profiles.map((p) => (
              <option key={p.id} value={p.id} className="bg-[#161922] text-zinc-200">
                {p.name}
              </option>
            ))}
          </select>
        </div>

        <button
          onClick={onLaunchGame}
          disabled={launching || gameStatus.running}
          className={`flex items-center gap-2 px-4 py-2 rounded-lg text-xs font-semibold transition-all duration-150 active:scale-[0.97] cursor-pointer disabled:cursor-default ${
            gameStatus.running
              ? 'bg-emerald-500/15 text-emerald-300 border border-emerald-500/30'
              : 'bg-amber-500 hover:bg-amber-400 text-zinc-950 shadow-sm shadow-amber-500/20 hover:shadow-amber-500/35'
          }`}
        >
          {launching ? (
            <Loader2 className="w-3.5 h-3.5 animate-spin" />
          ) : (
            <Play className="w-3.5 h-3.5 fill-current" />
          )}
          <span>{gameStatus.running ? 'Game Running' : 'Play Vintage Story'}</span>
        </button>
      </div>
    </header>
  );
}
