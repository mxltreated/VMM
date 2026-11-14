import { useCallback, useEffect, useRef, useState } from 'react';
import {
  Check,
  Copy,
  Globe,
  Loader2,
  Network,
  Play,
  RefreshCw,
  Send,
  Server,
  ShieldAlert,
  ShieldCheck,
  Square,
  Terminal,
  Wifi,
} from 'lucide-react';
import { api } from '../api/tauri';
import type {
  FirewallStatus,
  ModProfile,
  NetworkEndpoint,
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
  const [endpoints, setEndpoints] = useState<NetworkEndpoint[]>([]);
  const [loadingNet, setLoadingNet] = useState(false);
  const [firewall, setFirewall] = useState<FirewallStatus | null>(null);
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
  const [joinAddress, setJoinAddress] = useState('');
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [busyServer, setBusyServer] = useState(false);
  const [savingConfig, setSavingConfig] = useState(false);
  const logEndRef = useRef<HTMLDivElement | null>(null);

  const refreshAll = useCallback(async () => {
    setLoadingNet(true);
    try {
      const [eps, srv, cfg, recentLogs] = await Promise.all([
        api.detectNetworkEndpoints(),
        api.getServerStatus(),
        api.getServerConfig().catch(() => null),
        api.getServerLogs(),
      ]);
      setEndpoints(eps);
      setServerStatus(srv);
      if (cfg) {
        setServerConfig(cfg);
      }
      setLogs(recentLogs);
      const portToCheck = cfg?.port ?? srv.port ?? 42420;
      const fw = await api.checkFirewallRule(portToCheck).catch(() => null);
      setFirewall(fw);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingNet(false);
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
    if (!serverStatus.running) return;
    const timer = setInterval(() => {
      api.getServerStatus().then(setServerStatus).catch(() => {});
    }, 3000);
    return () => clearInterval(timer);
  }, [serverStatus.running]);

  const copyText = async (key: string, text: string) => {
    await navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => setCopiedKey(null), 1800);
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
      const fw = await api.checkFirewallRule(serverConfig.port).catch(() => null);
      setFirewall(fw);
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

  const handleAddFirewall = async () => {
    try {
      const fw = await api.addFirewallRule(serverConfig.port);
      setFirewall(fw);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCopyFullInvite = async (ep: NetworkEndpoint) => {
    try {
      let packCode = '';
      if (activeProfile) {
        packCode = await api.generateModpackCode({
          profileId: activeProfile.id,
          packName: activeProfile.name,
        });
      }
      const lines = [
        `Join my Vintage Story server!`,
        `Address: ${ep.ip}:${serverConfig.port} (${ep.label})`,
        serverConfig.password ? `Password: ${serverConfig.password}` : null,
        packCode ? `Modpack Code (VMM -> Profiles -> Import): ${packCode}` : null,
      ].filter(Boolean);
      await copyText(`invite-${ep.ip}`, lines.join('\n'));
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleJoinServer = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!joinAddress.trim()) return;
    try {
      await api.joinServer(joinAddress.trim());
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-6 max-w-5xl">
      {/* Mode Switcher + Join Friend */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <div className="lg:col-span-2 p-4 rounded-2xl bg-[#141720] border border-white/[0.06] flex flex-wrap items-center justify-between gap-3">
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

        <form
          onSubmit={handleJoinServer}
          className="p-4 rounded-2xl bg-[#141720] border border-white/[0.06] flex items-center gap-2"
        >
          <input
            type="text"
            value={joinAddress}
            onChange={(e) => setJoinAddress(e.target.value)}
            placeholder="Join IP:Port (e.g. 100.x.y.z:42420)"
            className="flex-1 min-w-0 bg-[#0e1015] border border-white/[0.08] rounded-lg px-3 py-2 text-xs text-zinc-200 placeholder:text-zinc-500 focus:outline-none focus:border-amber-500/40"
          />
          <button
            type="submit"
            disabled={!joinAddress.trim()}
            className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-zinc-950 text-xs font-semibold shrink-0 cursor-pointer"
          >
            <Play className="w-3.5 h-3.5 fill-current" />
            <span>Join</span>
          </button>
        </form>
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
                  <label className="block text-zinc-400 mb-1">Port (TCP/UDP)</label>
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
            No separate dedicated server needed. Launch Vintage Story normally, load your
            singleplayer world, press <span className="text-zinc-200 font-medium">Esc → Open to LAN</span>,
            and share one of your Mesh VPN addresses below with friends on the same Tailscale,
            ZeroTier, or Radmin VPN network.
          </p>
        </div>
      )}

      {/* Mesh VPN & Firewall Helper */}
      <div className="p-5 rounded-2xl bg-[#141720] border border-white/[0.06] space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2.5">
            <Globe className="w-4 h-4 text-amber-400" />
            <div>
              <h3 className="text-sm font-semibold text-zinc-100">
                Mesh VPN &amp; Reachable Addresses
              </h3>
              <p className="text-[11px] text-zinc-500">
                Friends on your Tailscale, ZeroTier, or Radmin VPN can join directly without router port-forwarding.
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            {firewall && (
              <div className="flex items-center gap-2">
                <span
                  className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-[11px] font-medium border ${
                    firewall.rulePresent
                      ? 'bg-emerald-500/10 text-emerald-300 border-emerald-500/25'
                      : 'bg-amber-500/10 text-amber-300 border-amber-500/25'
                  }`}
                >
                  {firewall.rulePresent ? (
                    <ShieldCheck className="w-3.5 h-3.5" />
                  ) : (
                    <ShieldAlert className="w-3.5 h-3.5" />
                  )}
                  <span>
                    {firewall.rulePresent
                      ? `Firewall Port ${firewall.port} Open`
                      : `Port ${firewall.port} Not Allowed`}
                  </span>
                </span>
                {!firewall.rulePresent && (
                  <button
                    onClick={handleAddFirewall}
                    className="px-2.5 py-1.5 rounded-lg bg-amber-500/15 hover:bg-amber-500/25 border border-amber-500/30 text-amber-300 text-[11px] font-medium cursor-pointer"
                  >
                    Allow Port
                  </button>
                )}
              </div>
            )}
            <button
              onClick={refreshAll}
              disabled={loadingNet}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-white/[0.05] hover:bg-white/[0.1] text-zinc-300 text-xs cursor-pointer"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${loadingNet ? 'animate-spin' : ''}`} />
              <span>Refresh</span>
            </button>
          </div>
        </div>

        {endpoints.length === 0 ? (
          <div className="p-4 rounded-xl bg-[#0e1015] border border-white/[0.05] text-xs text-zinc-400">
            No Mesh VPN or LAN IPv4 addresses detected. Connect to Tailscale, ZeroTier, or Radmin VPN and click Refresh.
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {endpoints.map((ep) => {
              const addr = `${ep.ip}:${serverConfig.port}`;
              return (
                <div
                  key={`${ep.kind}-${ep.ip}`}
                  className="p-3.5 rounded-xl bg-[#0e1015] border border-white/[0.06] flex items-center justify-between gap-3"
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <div
                      className={`w-8 h-8 rounded-lg flex items-center justify-center shrink-0 ${
                        ep.kind === 'lan'
                          ? 'bg-zinc-800 text-zinc-400'
                          : 'bg-emerald-500/15 text-emerald-300'
                      }`}
                    >
                      <Network className="w-4 h-4" />
                    </div>
                    <div className="min-w-0">
                      <div className="text-xs font-medium text-zinc-200 truncate">
                        {ep.label}
                      </div>
                      <div className="text-[11px] font-mono text-zinc-400">{addr}</div>
                    </div>
                  </div>

                  <div className="flex items-center gap-1.5 shrink-0">
                    <button
                      onClick={() => copyText(addr, addr)}
                      className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-white/[0.05] hover:bg-white/[0.1] text-zinc-200 text-[11px] cursor-pointer"
                      title="Copy IP:Port"
                    >
                      {copiedKey === addr ? (
                        <Check className="w-3 h-3 text-emerald-400" />
                      ) : (
                        <Copy className="w-3 h-3" />
                      )}
                      <span>Copy</span>
                    </button>
                    <button
                      onClick={() => handleCopyFullInvite(ep)}
                      className="px-2.5 py-1.5 rounded-lg bg-amber-500/15 hover:bg-amber-500/25 border border-amber-500/30 text-amber-300 text-[11px] font-medium cursor-pointer"
                      title="Copy IP:Port + Modpack Share Code"
                    >
                      {copiedKey === `invite-${ep.ip}` ? 'Copied!' : 'Invite'}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
