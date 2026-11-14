import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  ArrowLeft,
  ArrowRight,
  Check,
  Flame,
  FolderOpen,
  Layers,
  Package,
  SlidersHorizontal,
  Sparkles,
} from 'lucide-react';
import { api } from '../api/tauri';
import type { AppSettings } from '../types';

interface OnboardingModalProps {
  settings: AppSettings;
  onComplete: (nextSettings: AppSettings, selectedProfileId: string) => Promise<void>;
  onSkip: () => Promise<void>;
}

export function OnboardingModal({
  settings,
  onComplete,
  onSkip,
}: OnboardingModalProps) {
  const [step, setStep] = useState<1 | 2 | 3>(1);
  const [gameExePath, setGameExePath] = useState(settings.gameExePath ?? '');
  const [dataPath, setDataPath] = useState(settings.dataPath ?? '');
  const [selectedProfile, setSelectedProfile] = useState<string>(
    settings.activeProfileId ?? 'default'
  );
  const [submitting, setSubmitting] = useState(false);

  const pickExe = async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      title: 'Select Vintagestory.exe',
      filters: [{ name: 'Executable', extensions: ['exe'] }],
    });
    if (typeof selected === 'string') setGameExePath(selected);
  };

  const pickDataDir = async () => {
    const selected = await open({
      multiple: false,
      directory: true,
      title: 'Select VintagestoryData folder',
    });
    if (typeof selected === 'string') setDataPath(selected);
  };

  const handleAutoDetect = async () => {
    const detected = await api.detectPaths();
    if (detected.gameExePath) setGameExePath(detected.gameExePath);
    if (detected.dataPath) setDataPath(detected.dataPath);
  };

  const handleFinish = async () => {
    if (submitting) return;
    setSubmitting(true);
    try {
      await onComplete(
        {
          ...settings,
          gameExePath: gameExePath.trim() || null,
          dataPath: dataPath.trim() || null,
          onboardingCompleted: true,
        },
        selectedProfile
      );
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="animate-fade-in fixed inset-0 z-50 flex items-center justify-center bg-black/65 backdrop-blur-sm p-4">
      <div className="animate-scale-in w-full max-w-lg rounded-2xl bg-[#12151c] border border-white/[0.1] shadow-2xl overflow-hidden">
        {/* Header */}
        <div className="px-6 pt-6 pb-4 border-b border-white/[0.06] flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-amber-500/15 border border-amber-500/30 flex items-center justify-center text-amber-400">
              <Flame className="w-4 h-4" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h2 className="text-sm font-semibold text-zinc-100">
                  Welcome to Vintage Mod Manager
                </h2>
                <span className="text-[10px] font-semibold uppercase tracking-wider px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300 border border-amber-500/25">
                  Alpha 1.0
                </span>
              </div>
              <p className="text-xs text-zinc-500">
                Early alpha build • Step {step} of 3
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={onSkip}
            className="text-xs text-zinc-500 hover:text-zinc-300 transition-colors cursor-pointer"
          >
            Skip for now
          </button>
        </div>

        {/* Body */}
        <div key={step} className="animate-slide-up p-6 min-h-[260px]">
          {step === 1 && (
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-semibold text-zinc-100">
                    Locate Vintage Story
                  </h3>
                  <p className="text-xs text-zinc-400 mt-0.5">
                    Confirm your game executable and data folder.
                  </p>
                </div>
                <button
                  type="button"
                  onClick={handleAutoDetect}
                  className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-white/[0.05] hover:bg-white/[0.09] text-xs text-zinc-300 cursor-pointer"
                >
                  <Sparkles className="w-3.5 h-3.5 text-amber-400" />
                  <span>Auto-Detect</span>
                </button>
              </div>

              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-medium text-zinc-300">
                    Vintagestory.exe
                  </label>
                  <span
                    className={`text-[10px] px-1.5 py-0.5 rounded ${
                      gameExePath
                        ? 'bg-emerald-500/15 text-emerald-300'
                        : 'bg-amber-500/15 text-amber-300'
                    }`}
                  >
                    {gameExePath ? 'Detected' : 'Optional — set to launch'}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    value={gameExePath}
                    onChange={(e) => setGameExePath(e.target.value)}
                    placeholder="C:\...\Vintagestory\Vintagestory.exe"
                    className="flex-1 bg-[#0e1015] border border-white/[0.07] rounded-lg px-3 py-2 text-xs text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-amber-500/40"
                  />
                  <button
                    type="button"
                    onClick={pickExe}
                    className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-[#1c202c] hover:bg-white/[0.08] border border-white/[0.07] text-xs text-zinc-200 cursor-pointer"
                  >
                    <FolderOpen className="w-3.5 h-3.5" />
                    <span>Browse</span>
                  </button>
                </div>
              </div>

              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-medium text-zinc-300">
                    VintagestoryData Folder
                  </label>
                  <span
                    className={`text-[10px] px-1.5 py-0.5 rounded ${
                      dataPath
                        ? 'bg-emerald-500/15 text-emerald-300'
                        : 'bg-rose-500/15 text-rose-300'
                    }`}
                  >
                    {dataPath ? 'Detected' : 'Required for mods'}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    value={dataPath}
                    onChange={(e) => setDataPath(e.target.value)}
                    placeholder="%APPDATA%\VintagestoryData"
                    className="flex-1 bg-[#0e1015] border border-white/[0.07] rounded-lg px-3 py-2 text-xs text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-amber-500/40"
                  />
                  <button
                    type="button"
                    onClick={pickDataDir}
                    className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-[#1c202c] hover:bg-white/[0.08] border border-white/[0.07] text-xs text-zinc-200 cursor-pointer"
                  >
                    <FolderOpen className="w-3.5 h-3.5" />
                    <span>Browse</span>
                  </button>
                </div>
              </div>
            </div>
          )}

          {step === 2 && (
            <div className="space-y-4">
              <div>
                <h3 className="text-sm font-semibold text-zinc-100">
                  Choose Your Starting Profile
                </h3>
                <p className="text-xs text-zinc-400 mt-0.5">
                  Profiles switch mod sets instantly by moving files between Mods/ and ModsDisabled/.
                </p>
              </div>

              <div className="space-y-2.5">
                <button
                  type="button"
                  onClick={() => setSelectedProfile('default')}
                  className={`w-full p-4 rounded-xl border text-left transition-all cursor-pointer flex items-start justify-between gap-3 ${
                    selectedProfile === 'default'
                      ? 'bg-amber-500/[0.08] border-amber-500/40'
                      : 'bg-[#161922] border-white/[0.06] hover:border-white/[0.12]'
                  }`}
                >
                  <div>
                    <div className="text-sm font-semibold text-zinc-100">
                      Default (Keep Existing Mods)
                    </div>
                    <p className="text-xs text-zinc-400 mt-1">
                      Preserves whatever mods are currently in your Mods/ folder.
                    </p>
                  </div>
                  {selectedProfile === 'default' && (
                    <Check className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
                  )}
                </button>

                <button
                  type="button"
                  onClick={() => setSelectedProfile('vanilla')}
                  className={`w-full p-4 rounded-xl border text-left transition-all cursor-pointer flex items-start justify-between gap-3 ${
                    selectedProfile === 'vanilla'
                      ? 'bg-amber-500/[0.08] border-amber-500/40'
                      : 'bg-[#161922] border-white/[0.06] hover:border-white/[0.12]'
                  }`}
                >
                  <div>
                    <div className="text-sm font-semibold text-zinc-100">
                      Vanilla (Clean Slate)
                    </div>
                    <p className="text-xs text-zinc-400 mt-1">
                      Safely moves all current mods into ModsDisabled/ so you can start fresh.
                    </p>
                  </div>
                  {selectedProfile === 'vanilla' && (
                    <Check className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
                  )}
                </button>
              </div>
            </div>
          )}

          {step === 3 && (
            <div className="space-y-4">
              <div>
                <h3 className="text-sm font-semibold text-zinc-100">
                  You&apos;re Ready to Mod
                </h3>
                <p className="text-xs text-zinc-400 mt-0.5">
                  A quick look at how everything fits together:
                </p>
              </div>

              <div className="space-y-2.5">
                <div className="p-3.5 rounded-xl bg-[#161922] border border-white/[0.06] flex items-start gap-3">
                  <Package className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
                  <div>
                    <div className="text-xs font-semibold text-zinc-100">
                      Safe Enable / Disable
                    </div>
                    <p className="text-[11px] text-zinc-400 mt-0.5">
                      Toggling a mod off moves it to <span className="font-mono text-zinc-300">ModsDisabled/</span> instead of deleting it.
                    </p>
                  </div>
                </div>

                <div className="p-3.5 rounded-xl bg-[#161922] border border-white/[0.06] flex items-start gap-3">
                  <SlidersHorizontal className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
                  <div>
                    <div className="text-xs font-semibold text-zinc-100">
                      Pre-Boot Mod Configs
                    </div>
                    <p className="text-[11px] text-zinc-400 mt-0.5">
                      Tweak <span className="font-mono text-zinc-300">ModConfig/*.json</span> values in Form or JSON mode before launching.
                    </p>
                  </div>
                </div>

                <div className="p-3.5 rounded-xl bg-[#161922] border border-white/[0.06] flex items-start gap-3">
                  <Layers className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
                  <div>
                    <div className="text-xs font-semibold text-zinc-100">
                      Instant Profiles
                    </div>
                    <p className="text-[11px] text-zinc-400 mt-0.5">
                      Create and switch mod sets anytime from the top bar without duplicating files.
                    </p>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-6 py-4 border-t border-white/[0.06] bg-[#0e1015]/60 flex items-center justify-between">
          <div className="flex items-center gap-1.5">
            {[1, 2, 3].map((n) => (
              <span
                key={n}
                className={`h-1.5 rounded-full transition-all ${
                  step === n ? 'w-5 bg-amber-400' : 'w-1.5 bg-zinc-700'
                }`}
              />
            ))}
          </div>

          <div className="flex items-center gap-2">
            {step > 1 && (
              <button
                type="button"
                onClick={() => setStep((s) => (s === 3 ? 2 : 1))}
                className="flex items-center gap-1 px-3 py-1.5 rounded-lg bg-white/[0.05] hover:bg-white/[0.09] text-xs font-medium text-zinc-300 cursor-pointer"
              >
                <ArrowLeft className="w-3.5 h-3.5" />
                <span>Back</span>
              </button>
            )}

            {step < 3 ? (
              <button
                type="button"
                onClick={() => setStep((s) => (s === 1 ? 2 : 3))}
                className="flex items-center gap-1 px-4 py-1.5 rounded-lg bg-amber-500 hover:bg-amber-400 text-zinc-950 text-xs font-semibold cursor-pointer"
              >
                <span>Continue</span>
                <ArrowRight className="w-3.5 h-3.5" />
              </button>
            ) : (
              <button
                type="button"
                onClick={handleFinish}
                disabled={submitting}
                className="flex items-center gap-1.5 px-4 py-1.5 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-zinc-950 text-xs font-semibold cursor-pointer"
              >
                <Check className="w-3.5 h-3.5" />
                <span>{submitting ? 'Finishing...' : 'Finish Setup'}</span>
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
