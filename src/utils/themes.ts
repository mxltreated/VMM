import type { ThemeId } from '../types';

export interface ThemePreset {
  id: ThemeId;
  name: string;
  description: string;
  isLight: boolean;
  swatches: {
    bg: string;
    surface: string;
    accent: string;
    text: string;
  };
}

export const DEFAULT_THEME_ID: ThemeId = 'forge-ember';
const THEME_STORAGE_KEY = 'vmm_theme';

export const THEME_PRESETS: ThemePreset[] = [
  {
    id: 'forge-ember',
    name: 'Forge Ember',
    description: 'Classic dark charcoal with warm forge amber.',
    isLight: false,
    swatches: {
      bg: '#0e1015',
      surface: '#141720',
      accent: '#f59e0b',
      text: '#f4f4f5',
    },
  },
  {
    id: 'temporal-rift',
    name: 'Temporal Rift',
    description: 'Deep slate blues with temporal gear teal.',
    isLight: false,
    swatches: {
      bg: '#091017',
      surface: '#101a24',
      accent: '#14b8a6',
      text: '#e6f4f6',
    },
  },
  {
    id: 'seraph-light',
    name: 'Seraph Light',
    description: 'Warm parchment light mode with bronze ink.',
    isLight: true,
    swatches: {
      bg: '#f3ede2',
      surface: '#faf6ee',
      accent: '#b45309',
      text: '#1f1b16',
    },
  },
  {
    id: 'rustworks',
    name: 'Rustworks',
    description: 'Industrial iron dark with oxidized copper.',
    isLight: false,
    swatches: {
      bg: '#130f0e',
      surface: '#1c1614',
      accent: '#ea580c',
      text: '#f5efec',
    },
  },
  {
    id: 'redwood-forest',
    name: 'Redwood Forest',
    description: 'Deep pine & moss dark with emerald highlights.',
    isLight: false,
    swatches: {
      bg: '#0a120f',
      surface: '#111c17',
      accent: '#10b981',
      text: '#ebf5ef',
    },
  },
];

export function resolveThemeId(raw: string | null | undefined): ThemeId {
  const found = THEME_PRESETS.find((t) => t.id === raw);
  return found ? found.id : DEFAULT_THEME_ID;
}

export function getStoredThemeId(): ThemeId {
  try {
    return resolveThemeId(window.localStorage.getItem(THEME_STORAGE_KEY));
  } catch {
    return DEFAULT_THEME_ID;
  }
}

export function applyTheme(raw: string | null | undefined): ThemeId {
  const id = resolveThemeId(raw);
  document.documentElement.setAttribute('data-theme', id);
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, id);
  } catch {
    // ignore storage errors
  }
  return id;
}
