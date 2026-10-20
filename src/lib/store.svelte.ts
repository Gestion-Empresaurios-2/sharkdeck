// Global reactive app store (Svelte 5 runes module).
import * as api from './tauri';
import type { DeviceInfo } from './types';

type Toast = { id: number; kind: 'ok' | 'err' | 'info'; text: string };
export type Theme = 'light' | 'dark';

function initTheme(): Theme {
  try {
    const t = localStorage.getItem('sd-theme');
    if (t === 'light' || t === 'dark') return t;
  } catch {
    /* storage unavailable */
  }
  return 'dark';
}

export const store = $state({
  page: 'screen' as string,
  device: null as DeviceInfo | null,
  options: null,
  busy: false as boolean,
  toasts: [] as Toast[],
  theme: initTheme() as Theme,
  // Exposed on the store as well as as named exports: pages call both
  // `store.toast(...)` and `toast(...)`.
  toast,
  errText,
  connected,
  probeDevice,
  applyTheme,
  toggleTheme,
});

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
