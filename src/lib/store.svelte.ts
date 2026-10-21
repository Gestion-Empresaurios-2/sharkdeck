// Global reactive app store (Svelte 5 runes module).
import * as api from './tauri';
import type { DeviceInfo } from './types';

type Toast = { id: number; kind: 'ok' | 'err' | 'info'; text: string };
export type Theme = 'light' | 'dark';
export type AccentKey = 'mint' | 'solar' | 'violet' | 'ice';

export const ACCENTS: { key: AccentKey; name: string; color: string; ink: string }[] = [
  { key: 'mint', name: 'Mint', color: '#cefa83', ink: '#16210f' },
  { key: 'solar', name: 'Solar', color: '#ffdf8f', ink: '#362619' },
  { key: 'violet', name: 'Violet', color: '#cfbaff', ink: '#281b41' },
  { key: 'ice', name: 'Ice', color: '#a6dcff', ink: '#0e2e45' },
];

function initAccent(): AccentKey {
  try {
    const a = localStorage.getItem('sd-accent');
    if (a && ACCENTS.some((x) => x.key === a)) return a as AccentKey;
  } catch {
    /* storage unavailable */
  }
  return 'mint';
}

export function applyAccent(key: AccentKey) {
  const a = ACCENTS.find((x) => x.key === key) ?? ACCENTS[0];
  store.accent = a.key;
  const root = document.documentElement;
  root.style.setProperty('--accent', a.color);
  root.style.setProperty('--accent-ink', a.ink);
  root.style.setProperty('--accent-soft', `color-mix(in srgb, ${a.color} 16%, transparent)`);
  root.style.setProperty('--accent-line', `color-mix(in srgb, ${a.color} 42%, transparent)`);
  try {
    localStorage.setItem('sd-accent', a.key);
  } catch {
    /* storage unavailable */
  }
}

function initTheme(): Theme {
  try {
    const t = localStorage.getItem('sd-theme');
    if (t === 'light' || t === 'dark') return t;
  } catch {
    /* storage unavailable */
  }
  // Notion light is the default look of the app.
  return 'light';
}

function initAutopush(): boolean {
  try {
    return localStorage.getItem('sd-autopush') === '1';
  } catch {
    /* storage unavailable */
  }
  return false;
}

export const store = $state({
  page: 'screen' as string,
  device: null as DeviceInfo | null,
  options: null,
  busy: false as boolean,
  toasts: [] as Toast[],
  theme: initTheme() as Theme,
  accent: initAccent() as AccentKey,
  // Master switch for *automatic* pushes to the keyboard screen. Off by
  // default: the app then only writes to the panel when the user asks for
  // it, so the keyboard keeps its own content instead of being overwritten
  // every few seconds.
  autopush: initAutopush() as boolean,
  // Exposed on the store as well as as named exports: pages call both
  // `store.toast(...)` and `toast(...)`.
  toast,
  errText,
  connected,
  probeDevice,
  applyTheme,
  toggleTheme,
  applyAccent,
  setAutopush,
});

/** Enable or disable every *automatic* push to the keyboard screen. */
export function setAutopush(on: boolean) {
  store.autopush = on;
  try {
    localStorage.setItem('sd-autopush', on ? '1' : '0');
  } catch {
    /* storage unavailable */
  }
}

export function applyTheme(t: Theme) {
  store.theme = t;
  document.documentElement.dataset.theme = t;
  try {
    localStorage.setItem('sd-theme', t);
  } catch {
    /* storage unavailable */
  }
}

export function toggleTheme() {
  applyTheme(store.theme === 'dark' ? 'light' : 'dark');
}

let nextId = 1;

export function toast(kind: Toast['kind'], text: string) {
  const id = nextId++;
  store.toasts.push({ id, kind, text });
  setTimeout(() => {
    const i = store.toasts.findIndex((t) => t.id === id);
    if (i >= 0) store.toasts.splice(i, 1);
  }, 4500);
}

export function errText(e: unknown): string {
  if (typeof e === 'string') return e;
  return String(e);
}

export function connected(): boolean {
  return !!store.device && !store.device.error;
}

/** Probe the keyboard; updates store.device + returns it. */
export async function probeDevice(): Promise<DeviceInfo | null> {
  store.busy = true;
  try {
    const d = await api.probe();
    store.device = d;
    if (d && !d.error) {
      toast('ok', `Connected: ${d.path} · fw ${d.firmware} · profile ${Number(d.profile) + 1}`);
    } else if (d) {
      toast('err', `Device found but unreadable: ${d.error}`);
    }
    return d;
  } catch (e) {
    store.device = null;
    toast('err', errText(e));
    return null;
  } finally {
    store.busy = false;
  }
}
