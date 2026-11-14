import { useCallback, useEffect, useRef, useState } from 'react';
import {
  Check,
  Copy,
  Loader2,
  Lock,
  Play,
  Send,
  Server,
  ShieldCheck,
  Square,
  Terminal,
  Users,
  Wifi,
} from 'lucide-react';
import { api } from '../api/tauri';
import type {
  ModProfile,
  P2pStatus,
  ServerConfigSummary,
  ServerLogEvent,
  ServerStatus,
} from '../types';

interface MultiplayerViewProps {
  activeProfile: ModProfile | null;
  onError: (msg: string) => void;
}

export function MultiplayerView({ activeProfile, onError }: MultiplayerViewProps) {
  const [hostMode, setHostMode] = useState<'dedicated' | 'lan'>('dedicated');
  const [p2pStatus, setP2pStatus] = useState<P2pStatus>({
    active: false,
    mode: 'idle',
    roomCode: null,
    localAddress: null,
    connectedPeers: 0,
  });
  const [joinRoomCode, setJoinRoomCode] = useState('');
  const [busyP2p, setBusyP2p] = useState(false);

  const [serverStatus, setServerStatus] = useState<ServerStatus>({
    running: false,
    pid: null,
    port: 42420,
    exePath: null,
    dataPath: null,
  });
  const [serverConfig, setServerConfig] = useState<ServerConfigSummary>({
    serverName: 'Vintage Story Server',
    port: 42420,
    maxClients: 16,
    password: '',
    allowPvp: true,
    whitelistMode: 0,
  });
  const [logs, setLogs] = useState<ServerLogEvent[]>([]);
  const [commandInput, setCommandInput] = useState('');
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [busyServer, setBusyServer] = useState(false);
  const [savingConfig, setSavingConfig] = useState(false);
  const logEndRef = useRef<HTMLDivElement | null>(null);

  const refreshAll = useCallback(async () => {
    try {
      const [p2p, srv, cfg, recentLogs] = await Promise.all([
        api.getP2pStatus(),
        api.getServerStatus(),
        api.getServerConfig().catch(() => null),
        api.getServerLogs(),
      ]);
      setP2pStatus(p2p);
      setServerStatus(srv);
      if (cfg) {
        setServerConfig(cfg);
      }
      setLogs(recentLogs);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  }, [onError]);

  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    api.onServerLog((ev) => {
      setLogs((prev) => [...prev.slice(-399), ev]);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  useEffect(() => {
    if (!serverStatus.running && !p2pStatus.active) return;
    const timer = setInterval(() => {
      if (serverStatus.running) {
        api.getServerStatus().then(setServerStatus).catch(() => {});
      }
      if (p2pStatus.active) {
        api.getP2pStatus().then(setP2pStatus).catch(() => {});
      }
    }, 3000);
    return () => clearInterval(timer);
  }, [serverStatus.running, p2pStatus.active]);

  const copyText = async (key: string, text: string) => {
    await navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => setCopiedKey(null), 1800);
  };

  const handleStartP2pHost = async () => {
    setBusyP2p(true);
    try {
      const st = await api.startP2pHost(serverConfig.port);
      setP2pStatus(st);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyP2p(false);
    }
  };

  const handleStopP2p = async () => {
    setBusyP2p(true);
    try {
      const st = await api.stopP2p();
      setP2pStatus(st);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyP2p(false);
    }
  };

  const handleJoinP2pRoom = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!joinRoomCode.trim()) return;
    setBusyP2p(true);
    try {
      const st = await api.joinP2pRoom({
        roomCode: joinRoomCode.trim(),
        preferredPort: 42420,
        launchGame: true,
      });
      setP2pStatus(st);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyP2p(false);
    }
  };

  const handleCopyFullInvite = async () => {
    if (!p2pStatus.roomCode) return;
    try {
      let packCode = '';
      if (activeProfile) {
        packCode = await api.generateModpackCode({
          profileId: activeProfile.id,
          packName: activeProfile.name,
        });
      }
      const lines = [
        `Join my Vintage Story P2P Room!`,
        `Room Code (VMM -> Multiplayer -> Join): ${p2pStatus.roomCode}`,
        serverConfig.password ? `Server Password: ${serverConfig.password}` : null,
        packCode ? `Modpack Code (VMM -> Profiles -> Import): ${packCode}` : null,
      ].filter(Boolean);
      await copyText('full-invite', lines.join('\n'));
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleStartServer = async () => {
    setBusyServer(true);
    try {
      await api.saveServerConfig(serverConfig);
      const st = await api.startDedicatedServer({
        port: serverConfig.port,
        maxClients: serverConfig.maxClients,
      });
      setServerStatus(st);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyServer(false);
    }
  };

  const handleStopServer = async () => {
    setBusyServer(true);
    try {
      const st = await api.stopDedicatedServer();
      setServerStatus(st);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyServer(false);
    }
  };

  const handleSaveConfig = async () => {
    setSavingConfig(true);
    try {
      await api.saveServerConfig(serverConfig);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setSavingConfig(false);
    }
  };

  const handleSendCommand = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!commandInput.trim() || !serverStatus.running) return;
    const cmd = commandInput.trim();
    setCommandInput('');
    try {
      await api.sendServerCommand(cmd);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-6 max-w-5xl">
      {/* Primary P2P Room Panel */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Host P2P Room */}
        <div className="p-5 rounded-2xl bg-[#141720] border border-white/[0.06] space-y-3.5 text-xs">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <ShieldCheck className="w-4 h-4 text-emerald-400" />
              <h3 className="text-sm font-semibold text-zinc-100">
                Host Encrypted P2P Room
              </h3>
            </div>
            {p2pStatus.active && p2pStatus.mode === 'host' && (
              <span className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-emerald-500/15 text-emerald-300 text-[11px] font-medium">
                <Users className="w-3 h-3" />
                {p2pStatus.connectedPeers} peer{p2pStatus.connectedPeers === 1 ? '' : 's'}
              </span>
            )}
          </div>

          <p className="text-zinc-400 leading-relaxed">
            Creates a zero-download, TLS 1.3 encrypted peer-to-peer tunnel to port{' '}
            <span className="font-mono text-zinc-200">{serverConfig.port}</span>. No VPN or router port-forwarding needed.
          </p>

          {p2pStatus.active && p2pStatus.mode === 'host' && p2pStatus.roomCode ? (
            <div className="space-y-2.5">
              <div className="p-2.5 rounded-xl bg-[#0e1015] border border-emerald-500/25 font-mono text-[10px] text-emerald-300 break-all max-h-20 overflow-y-auto">
                {p2pStatus.roomCode}
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <button
                  onClick={() => copyText('room-code', p2pStatus.roomCode!)}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-white/[0.06] hover:bg-white/[0.1] text-zinc-200 font-medium cursor-pointer"
                >
                  {copiedKey === 'room-code' ? (
                    <Check className="w-3.5 h-3.5 text-emerald-400" />
                  ) : (
                    <Copy className="w-3.5 h-3.5" />
                  )}
                  <span>{copiedKey === 'room-code' ? 'Copied!' : 'Copy Room Code'}</span>
                </button>
                <button
                  onClick={handleCopyFullInvite}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-amber-500/15 hover:bg-amber-500/25 border border-amber-500/30 text-amber-300 font-medium cursor-pointer"
                >
                  <span>{copiedKey === 'full-invite' ? 'Copied Invite!' : 'Copy Full Invite'}</span>
                </button>
                <button
                  onClick={handleStopP2p}
                  disabled={busyP2p}
                  className="ml-auto flex items-center gap-1 px-3 py-1.5 rounded-lg bg-rose-500/15 hover:bg-rose-500/25 border border-rose-500/30 text-rose-300 font-medium cursor-pointer"
                >
                  <Square className="w-3 h-3 fill-current" />
                  <span>Close Room</span>
                </button>
              </div>
            </div>
          ) : (
            <button
              onClick={handleStartP2pHost}
              disabled={busyP2p}
              className="flex items-center gap-1.5 px-4 py-2 rounded-lg bg-emerald-500 hover:bg-emerald-400 disabled:opacity-50 text-zinc-950 font-semibold cursor-pointer"
            >
              {busyP2p ? (
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <Lock className="w-3.5 h-3.5" />
              )}
              <span>{busyP2p ? 'Creating Room...' : 'Start P2P Room'}</span>
            </button>
          )}
        </div>

        {/* Join P2P Room */}
        <div className="p-5 rounded-2xl bg-[#141720] border border-white/[0.06] space-y-3.5 text-xs">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Play className="w-4 h-4 text-amber-400" />
              <h3 className="text-sm font-semibold text-zinc-100">
                Join a Friend&apos;s P2P Room
              </h3>
            </div>
            {p2pStatus.active && p2pStatus.mode === 'client' && (
              <span className="px-2 py-0.5 rounded-full bg-emerald-500/15 text-emerald-300 text-[11px] font-medium">
                Connected ({p2pStatus.localAddress})
              </span>
            )}
          </div>

          <p className="text-zinc-400 leading-relaxed">
            Paste your friend&apos;s <span className="font-mono text-zinc-200">vmm-p2p:...</span>{' '}
            Room Code below. VMM opens a local encrypted tunnel and launches Vintage Story straight into their world.
          </p>

          {p2pStatus.active && p2pStatus.mode === 'client' ? (
            <div className="flex items-center gap-2 pt-1">
              <button
                onClick={() =>
                  p2pStatus.localAddress && api.joinServer(p2pStatus.localAddress)
                }
                className="flex items-center gap-1.5 px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 text-zinc-950 font-semibold cursor-pointer"
              >
                <Play className="w-3.5 h-3.5 fill-current" />
                <span>Launch Vintage Story</span>
              </button>
              <button
                onClick={handleStopP2p}
                disabled={busyP2p}
                className="px-3 py-2 rounded-lg bg-rose-500/15 hover:bg-rose-500/25 border border-rose-500/30 text-rose-300 font-medium cursor-pointer"
              >
                Disconnect
              </button>
            </div>
          ) : (
            <form onSubmit={handleJoinP2pRoom} className="flex gap-2">
              <input
                type="text"
                value={joinRoomCode}
                onChange={(e) => setJoinRoomCode(e.target.value)}
                placeholder="Paste vmm-p2p:... Room Code"
                className="flex-1 min-w-0 bg-[#0e1015] border border-white/[0.08] rounded-lg px-3 py-2 font-mono text-zinc-200 placeholder:text-zinc-500 focus:outline-none focus:border-amber-500/40"
              />
              <button
                type="submit"
                disabled={!joinRoomCode.trim() || busyP2p}
                className="flex items-center gap-1.5 px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-zinc-950 font-semibold shrink-0 cursor-pointer"
              >
                {busyP2p ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <Play className="w-3.5 h-3.5 fill-current" />
                )}
                <span>{busyP2p ? 'Connecting...' : 'Connect & Play'}</span>
              </button>
            </form>
          )}
        </div>
      </div>

      {/* Mode Switcher */}
      <div className="p-4 rounded-2xl bg-[#141720] border border-white/[0.06] flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2 bg-[#0e1015] p-1 rounded-xl border border-white/[0.06]">
          <button
            onClick={() => setHostMode('dedicated')}
            className={`flex items-center gap-2 px-3.5 py-2 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
              hostMode === 'dedicated'
                ? 'bg-amber-500/15 text-amber-300 border border-amber-500/30'
                : 'text-zinc-400 hover:text-zinc-200 border border-transparent'
            }`}
          >
            <Server className="w-3.5 h-3.5" />
            <span>Dedicated Server</span>
          </button>
          <button
            onClick={() => setHostMode('lan')}
            className={`flex items-center gap-2 px-3.5 py-2 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
              hostMode === 'lan'
                ? 'bg-amber-500/15 text-amber-300 border border-amber-500/30'
                : 'text-zinc-400 hover:text-zinc-200 border border-transparent'
            }`}
          >
            <Wifi className="w-3.5 h-3.5" />
            <span>In-Game Open to LAN</span>
          </button>
        </div>

        <div className="text-xs text-zinc-400">
          Active Mods:{' '}
          <span className="text-amber-300 font-medium">
            {activeProfile ? `${activeProfile.name} (${activeProfile.enabledModids.length})` : 'Default'}
          </span>
        </div>
      </div>

      {/* Host Section */}
      {hostMode === 'dedicated' ? (
        <div className="grid grid-cols-1 lg:grid-cols-5 gap-4">
          <div className="lg:col-span-2 p-5 rounded-2xl bg-[#141720] border border-white/[0.06] space-y-4 text-xs">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Server className="w-4 h-4 text-amber-400" />
                <h3 className="text-sm font-semibold text-zinc-100">Server Settings</h3>
              </div>
              <span
                className={`px-2 py-0.5 rounded-full text-[11px] font-medium ${
                  serverStatus.running
                    ? 'bg-emerald-500/15 text-emerald-300'
                    : 'bg-zinc-800 text-zinc-400'
                }`}
              >
                {serverStatus.running ? `Running (PID ${serverStatus.pid})` : 'Stopped'}
              </span>
            </div>

            <div className="space-y-3">
              <div>
                <label className="block text-zinc-400 mb-1">Server Name</label>
                <input
                  type="text"
                  value={serverConfig.serverName}
                  onChange={(e) =>
                    setServerConfig((c) => ({ ...c, serverName: e.target.value }))
                  }
                  className="w-full bg-[#0e1015] border border-white/[0.08] rounded-lg px-3 py-2 text-zinc-200 focus:outline-none focus:border-amber-500/40"
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-zinc-400 mb-1">Local Port</label>
                  <input
                    type="number"
                    value={serverConfig.port}
                    onChange={(e) =>
                      setServerConfig((c) => ({
                        ...c,
                        port: Number(e.target.value) || 42420,
                      }))
                    }
                    className="w-full bg-[#0e1015] border border-white/[0.08] rounded-lg px-3 py-2 text-zinc-200 focus:outline-none focus:border-amber-500/40"
                  />
                </div>
                <div>
                  <label className="block text-zinc-400 mb-1">Max Players</label>
                  <input
                    type="number"
                    value={serverConfig.maxClients}
                    onChange={(e) =>
                      setServerConfig((c) => ({
                        ...c,
                        maxClients: Number(e.target.value) || 16,
                      }))
                    }
                    className="w-full bg-[#0e1015] border border-white/[0.08] rounded-lg px-3 py-2 text-zinc-200 focus:outline-none focus:border-amber-500/40"
                  />
                </div>
              </div>

              <div>
                <label className="block text-zinc-400 mb-1">Password (optional)</label>
                <input
                  type="text"
                  value={serverConfig.password}
                  onChange={(e) =>
                    setServerConfig((c) => ({ ...c, password: e.target.value }))
                  }
                  placeholder="Leave blank for open"
                  className="w-full bg-[#0e1015] border border-white/[0.08] rounded-lg px-3 py-2 text-zinc-200 focus:outline-none focus:border-amber-500/40"
                />
              </div>

              <label className="flex items-center gap-2 text-zinc-300 cursor-pointer">
                <input
                  type="checkbox"
                  checked={serverConfig.allowPvp}
                  onChange={(e) =>
                    setServerConfig((c) => ({ ...c, allowPvp: e.target.checked }))
                  }
                  className="accent-amber-500"
                />
                <span>Allow PvP</span>
              </label>
            </div>

            <div className="flex items-center gap-2 pt-2">
              <button
                onClick={handleSaveConfig}
                disabled={savingConfig}
                className="px-3 py-2 rounded-lg bg-white/[0.06] hover:bg-white/[0.1] text-zinc-200 font-medium cursor-pointer"
              >
                {savingConfig ? 'Saving...' : 'Save Config'}
              </button>
              {serverStatus.running ? (
                <button
                  onClick={handleStopServer}
                  disabled={busyServer}
                  className="flex-1 flex items-center justify-center gap-1.5 px-4 py-2 rounded-lg bg-rose-500/20 hover:bg-rose-500/30 border border-rose-500/30 text-rose-300 font-semibold cursor-pointer"
                >
                  <Square className="w-3.5 h-3.5 fill-current" />
                  <span>Stop Server</span>
                </button>
              ) : (
                <button
                  onClick={handleStartServer}
                  disabled={busyServer}
                  className="flex-1 flex items-center justify-center gap-1.5 px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 text-zinc-950 font-semibold cursor-pointer"
                >
                  {busyServer ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <Play className="w-3.5 h-3.5 fill-current" />
                  )}
                  <span>Start Dedicated Server</span>
                </button>
              )}
            </div>
          </div>

          <div className="lg:col-span-3 p-5 rounded-2xl bg-[#141720] border border-white/[0.06] flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Terminal className="w-4 h-4 text-amber-400" />
                <h3 className="text-sm font-semibold text-zinc-100">Server Console</h3>
              </div>
              {serverStatus.dataPath && (
                <span className="text-[11px] text-zinc-500 font-mono truncate max-w-[260px]">
                  {serverStatus.dataPath}
                </span>
              )}
            </div>

            <div className="flex-1 min-h-[240px] max-h-[300px] overflow-y-auto rounded-xl bg-[#0b0d12] border border-white/[0.05] p-3 font-mono text-[11px] space-y-1">
              {logs.length === 0 ? (
                <div className="text-zinc-600">
                  Server output will appear here once started...
                </div>
              ) : (
                logs.map((entry, idx) => (
                  <div
                    key={idx}
                    className={entry.isError ? 'text-rose-300' : 'text-zinc-300'}
                  >
                    {entry.line}
                  </div>
                ))
              )}
              <div ref={logEndRef} />
            </div>

            <form onSubmit={handleSendCommand} className="flex gap-2">
              <input
                type="text"
                value={commandInput}
                onChange={(e) => setCommandInput(e.target.value)}
                disabled={!serverStatus.running}
                placeholder={
                  serverStatus.running
                    ? 'Send command (e.g. /op PlayerName, /time set day)...'
                    : 'Start server to send commands'
                }
                className="flex-1 bg-[#0e1015] border border-white/[0.08] rounded-lg px-3 py-2 text-xs font-mono text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-amber-500/40 disabled:opacity-50"
              />
              <button
                type="submit"
                disabled={!serverStatus.running || !commandInput.trim()}
                className="px-3.5 py-2 rounded-lg bg-white/[0.06] hover:bg-white/[0.1] disabled:opacity-40 text-zinc-200 text-xs cursor-pointer"
              >
                <Send className="w-3.5 h-3.5" />
              </button>
            </form>
          </div>
        </div>
      ) : (
        <div className="p-5 rounded-2xl bg-[#141720] border border-white/[0.06] space-y-3 text-xs">
          <div className="flex items-center gap-2">
            <Wifi className="w-4 h-4 text-amber-400" />
            <h3 className="text-sm font-semibold text-zinc-100">
              In-Game &ldquo;Open to LAN&rdquo; Mode
            </h3>
          </div>
          <p className="text-zinc-400 leading-relaxed">
            1. Launch Vintage Story normally and load your singleplayer world.
            <br />
            2. Press <span className="text-zinc-200 font-medium">Esc → Open to LAN</span>.
            <br />
            3. Click <span className="text-emerald-300 font-medium">Start P2P Room</span> above and send the Room Code to your friends.
          </p>
        </div>
      )}
    </div>
  );
}
