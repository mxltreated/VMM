import type { LocalMod } from '../types';

export const BUILTIN_GAME_MODIDS = new Set([
  'game',
  'survival',
  'creative',
  'essentials',
]);

export interface DependencyStatusEntry {
  modid: string;
  name: string;
  requiredVersion: string;
  status: 'installed' | 'disabled' | 'missing';
  installedVersion: string | null;
}

export interface EvaluatedDependencies {
  modDeps: DependencyStatusEntry[];
  gameRequirements: Record<string, string>;
  missingCount: number;
  disabledCount: number;
}

export function isBuiltinGameModId(modid: string): boolean {
  return BUILTIN_GAME_MODIDS.has(modid.trim().toLowerCase());
}

export function splitDependenciesMap(deps: Record<string, string> | undefined | null): {
  modDependencies: Record<string, string>;
  gameRequirements: Record<string, string>;
} {
  const modDependencies: Record<string, string> = {};
  const gameRequirements: Record<string, string> = {};
  if (!deps) {
    return { modDependencies, gameRequirements };
  }
  for (const [rawKey, val] of Object.entries(deps)) {
    const key = rawKey.trim();
    if (!key) continue;
    if (isBuiltinGameModId(key)) {
      gameRequirements[key] = val;
    } else {
      modDependencies[key] = val;
    }
  }
  return { modDependencies, gameRequirements };
}

export function evaluateDependenciesAgainstLocal(
  modDependencies: Record<string, string>,
  gameRequirements: Record<string, string>,
  localModsById: Map<string, LocalMod>
): EvaluatedDependencies {
  const modDeps: DependencyStatusEntry[] = [];
  let missingCount = 0;
  let disabledCount = 0;

  for (const [modid, reqVer] of Object.entries(modDependencies)) {
    const cleanId = modid.trim().toLowerCase();
    if (!cleanId || isBuiltinGameModId(cleanId)) continue;
    const local = localModsById.get(cleanId);
    if (!local) {
      missingCount += 1;
      modDeps.push({
        modid,
        name: modid,
        requiredVersion: reqVer,
        status: 'missing',
        installedVersion: null,
      });
    } else if (!local.enabled) {
      disabledCount += 1;
      modDeps.push({
        modid: local.modid || modid,
        name: local.name || modid,
        requiredVersion: reqVer,
        status: 'disabled',
        installedVersion: local.version,
      });
    } else {
      modDeps.push({
        modid: local.modid || modid,
        name: local.name || modid,
        requiredVersion: reqVer,
        status: 'installed',
        installedVersion: local.version,
      });
    }
  }

  // Sort missing first, then disabled, then installed
  const rank: Record<DependencyStatusEntry['status'], number> = {
    missing: 0,
    disabled: 1,
    installed: 2,
  };
  modDeps.sort((a, b) => rank[a.status] - rank[b.status] || a.name.localeCompare(b.name));

  return { modDeps, gameRequirements, missingCount, disabledCount };
}

export function formatVersionRequirement(req: string): string {
  const trimmed = req.trim();
  if (!trimmed || trimmed === '*' || trimmed.toLowerCase() === 'any') {
    return 'any version';
  }
  return `v${trimmed.replace(/^v/i, '')}`;
}
