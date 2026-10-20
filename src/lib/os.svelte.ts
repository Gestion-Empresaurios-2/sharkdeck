// FRAME OS — the keyboard screen as a small operating system.
//
// Everything here is authored at the panel's NATIVE resolution (240 x 135)
// and rendered to RGBA, so what you see in the studio is literally the
// pixel stream that gets pushed to the keyboard.

export type OsPage =
  | 'lock'
  | 'dash'
  | 'music'
  | 'perf'
  | 'focus'
  | 'shortcuts'
  | 'weather'
  | 'calendar'
  | 'settings';

export type AccentKey = 'mint' | 'sun' | 'lilac' | 'ice';
export type WallKey = 'aurora' | 'dune' | 'nebula' | 'glacier' | 'graphite' | 'citrus';
export type LockLayout = 'hero' | 'balanced' | 'minimal';
export type WidgetKey = 'cpu' | 'battery' | 'weather' | 'focus' | 'memory' | 'date';

export const OS_PAGES: { id: OsPage; label: string; hint: string }[] = [
  { id: 'lock', label: 'Home', hint: 'Lock screen' },
  { id: 'dash', label: 'Widgets', hint: 'At a glance' },
  { id: 'music', label: 'Media', hint: 'Now playing' },
  { id: 'perf', label: 'Monitor', hint: 'System telemetry' },
  { id: 'focus', label: 'Focus', hint: 'Deep work timer' },
  { id: 'shortcuts', label: 'Shortcuts', hint: 'One-touch macros' },
  { id: 'weather', label: 'Weather', hint: 'Forecast' },
  { id: 'calendar', label: 'Calendar', hint: 'Agenda' },
  { id: 'settings', label: 'Control', hint: 'Quick settings' },
];

export const ACCENTS: { id: AccentKey; label: string; color: string; ink: string }[] = [
  { id: 'mint', label: 'Mint', color: '#aff6c6', ink: '#0c2b20' },
  { id: 'sun', label: 'Solar', color: '#ffdf8f', ink: '#362619' },
  { id: 'lilac', label: 'Violet', color: '#cfbaff', ink: '#281b41' },
  { id: 'ice', label: 'Ice', color: '#a6dcff', ink: '#0e2e45' },
];

export const WALLS: { id: WallKey; label: string; stops: string[]; glow: string }[] = [
  { id: 'aurora', label: 'Aurora', stops: ['#0e263c', '#394e71', '#15192e'], glow: '#9adbd1' },
  { id: 'dune', label: 'Dune', stops: ['#392a43', '#ba6f71', '#472c44'], glow: '#efbf9e' },
  { id: 'nebula', label: 'Nebula', stops: ['#11142d', '#393167', '#21132b'], glow: '#a18af5' },
  { id: 'glacier', label: 'Glacier', stops: ['#0d3045', '#226b83', '#102538'], glow: '#9cdbed' },
  { id: 'graphite', label: 'Graphite', stops: ['#0d1017', '#293238', '#101218'], glow: '#6c8585' },
  { id: 'citrus', label: 'Citrus', stops: ['#28483f', '#879f7a', '#294a3c'], glow: '#f4df9b' },
];

export const WIDGETS: { id: WidgetKey; label: string }[] = [
  { id: 'cpu', label: 'CPU activity' },
  { id: 'battery', label: 'Battery' },
  { id: 'weather', label: 'Weather' },
  { id: 'focus', label: 'Focus timer' },
  { id: 'memory', label: 'Memory' },
  { id: 'date', label: 'Date' },
];

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

export interface OsSettings {
  accent: AccentKey;
  wall: WallKey;
  layout: LockLayout;
  widget1: WidgetKey;
  widget2: WidgetKey;
  seconds: boolean;
  brightness: number;
  message: string;
  city: string;
  format24: boolean;
  macros: string[];
}

export const OS_DEFAULTS: OsSettings = {
  accent: 'mint',
  wall: 'aurora',
  layout: 'hero',
  widget1: 'cpu',
  widget2: 'battery',
  seconds: false,
  brightness: 85,
  message: 'Make something extraordinary.',
  city: 'Granada',
  format24: true,
  macros: ['Launch app', 'Mute audio', 'Take note', 'Focus mode', 'Open mail', 'Screenshot'],
};

export const osState = $state({
  page: 'lock' as OsPage,
  power: true,
  settings: { ...OS_DEFAULTS, macros: [...OS_DEFAULTS.macros] } as OsSettings,
  musicPlaying: true,
  track: 0,
  progress: 34,
  focusSeconds: 25 * 60,
  focusRunning: false,
  metrics: { cpu: 24, gpu: 38, ram: 63, temp: 49 },
  history: [24, 29, 22, 27, 33, 25, 20, 32, 26, 35, 25, 29, 24],
  toast: '',
});

export function resetOs() {
  osState.settings = { ...OS_DEFAULTS, macros: [...OS_DEFAULTS.macros] };
  osState.page = 'lock';
  osState.power = true;
  osState.focusRunning = false;
  osState.focusSeconds = 25 * 60;
}

export function osAccent(): { color: string; ink: string } {
  const a = ACCENTS.find((x) => x.id === osState.settings.accent) ?? ACCENTS[0];
  return { color: a.color, ink: a.ink };
}

export function osWall(): (typeof WALLS)[number] {
  return WALLS.find((x) => x.id === osState.settings.wall) ?? WALLS[0];
}

// ---------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------

const UI = '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Inter", "Ubuntu", sans-serif';
const MONO = '"SF Mono", "Ubuntu Mono", "DejaVu Sans Mono", monospace';

function alpha(hex: string, a: number): string {
  const h = hex.replace('#', '');
  const r = parseInt(h.slice(0, 2), 16);
  const g = parseInt(h.slice(2, 4), 16);
  const b = parseInt(h.slice(4, 6), 16);
  return `rgba(${r},${g},${b},${a})`;
}

function rr(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  const k = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + k, y);
  ctx.lineTo(x + w - k, y);
  ctx.quadraticCurveTo(x + w, y, x + w, y + k);
  ctx.lineTo(x + w, y + h - k);
  ctx.quadraticCurveTo(x + w, y + h, x + w - k, y + h);
  ctx.lineTo(x + k, y + h);
  ctx.quadraticCurveTo(x, y + h, x, y + h - k);
  ctx.lineTo(x, y + k);
  ctx.quadraticCurveTo(x, y, x + k, y);
  ctx.closePath();
}

function glass(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number) {
  ctx.fillStyle = 'rgba(255,255,255,0.13)';
  rr(ctx, x, y, w, h, 7);
  ctx.fill();
  ctx.strokeStyle = 'rgba(255,255,255,0.20)';
  ctx.lineWidth = 0.6;
  ctx.stroke();
}

function text(
  ctx: CanvasRenderingContext2D,
  s: string,
  x: number,
  y: number,
  size: number,
  color: string,
  opts: { weight?: number; font?: string; align?: CanvasTextAlign; baseline?: CanvasTextBaseline } = {},
) {
  ctx.font = `${opts.weight ?? 600} ${size}px ${opts.font ?? UI}`;
  ctx.fillStyle = color;
  ctx.textAlign = opts.align ?? 'left';
  ctx.textBaseline = opts.baseline ?? 'alphabetic';
  ctx.fillText(s, x, y);
  ctx.textAlign = 'left';
  ctx.textBaseline = 'alphabetic';
}

function wallpaper(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const wall = osWall();
  const g = ctx.createLinearGradient(0, 0, w * 0.35, h);
  g.addColorStop(0, wall.stops[0]);
  g.addColorStop(0.55, wall.stops[1]);
  g.addColorStop(1, wall.stops[2]);
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, w, h);

  const rg = ctx.createRadialGradient(w * 0.82, h * 0.15, 2, w * 0.82, h * 0.15, w * 0.6);
  rg.addColorStop(0, alpha(wall.glow, 0.55));
  rg.addColorStop(1, alpha(wall.glow, 0));
  ctx.fillStyle = rg;
  ctx.fillRect(0, 0, w, h);

  // Ridge line — a quiet horizon so the clock always has a ground.
  ctx.fillStyle = 'rgba(9,13,22,0.55)';
  ctx.beginPath();
  ctx.moveTo(0, h);
  ctx.lineTo(0, h * 0.74);
  ctx.lineTo(w * 0.18, h * 0.58);
  ctx.lineTo(w * 0.32, h * 0.68);
  ctx.lineTo(w * 0.52, h * 0.44);
  ctx.lineTo(w * 0.68, h * 0.62);
  ctx.lineTo(w * 0.84, h * 0.52);
  ctx.lineTo(w, h * 0.66);
  ctx.lineTo(w, h);
  ctx.closePath();
  ctx.fill();

  const sh = ctx.createLinearGradient(0, 0, 0, h);
  sh.addColorStop(0, 'rgba(0,0,0,0.12)');
  sh.addColorStop(0.4, 'rgba(0,0,0,0)');
  sh.addColorStop(1, 'rgba(0,0,0,0.35)');
  ctx.fillStyle = sh;
  ctx.fillRect(0, 0, w, h);
}

function statusBar(ctx: CanvasRenderingContext2D, w: number, label: string) {
  const a = osAccent();
  text(ctx, label, 9, 11, 6, 'rgba(255,255,255,0.86)', { weight: 750 });
  // signal bars
  for (let i = 0; i < 3; i++) {
    ctx.fillStyle = 'rgba(255,255,255,0.75)';
    ctx.fillRect(w - 44 + i * 3, 8.5 - i * 1.4, 1.6, 2.4 + i * 1.4);
  }
  // battery shell + fill (classic segments)
  ctx.strokeStyle = 'rgba(255,255,255,0.55)';
  ctx.lineWidth = 0.6;
  rr(ctx, w - 30, 6.4, 12, 5.4, 1.4);
  ctx.stroke();
  ctx.fillStyle = 'rgba(255,255,255,0.55)';
  ctx.fillRect(w - 17.4, 8, 1.2, 2.2);
  ctx.fillStyle = a.color;
  ctx.fillRect(w - 29, 7.4, 8.4 * 0.86, 3.4);
}

function pad(n: number) {
  return String(n).padStart(2, '0');
}

export const osClock = $state({ offsetSec: 0 });

function timeParts() {
  const d = new Date();
  const d2 = new Date(d.getTime() + osClock.offsetSec * 1000);
  const h24 = d2.getHours();
  const h = osState.settings.format24 ? h24 : h24 % 12 || 12;
  return {
    hhmm: `${pad(h)}:${pad(d2.getMinutes())}`,
    ss: pad(d2.getSeconds()),
    day: d2.toLocaleDateString('en-US', { weekday: 'long', month: 'long', day: 'numeric' }),
    short: d2.toLocaleDateString('en-US', { month: 'short', day: 'numeric' }),
    num: d2.getDate(),
    ampm: h24 >= 12 ? 'PM' : 'AM',
  };
}

export const osHost = $state({ battery: null as number | null, charging: false });

export function widgetData(key: WidgetKey): { label: string; value: string } {
  const t = timeParts();
  switch (key) {
    case 'cpu':
      return { label: 'CPU LOAD', value: `${osState.metrics.cpu}%` };
    case 'battery':
      return {
        label: osHost.battery == null ? 'BATTERY' : osHost.charging ? 'BATTERY · CHG' : 'BATTERY',
        value: osHost.battery == null ? '—' : `${osHost.battery}%`,
      };
    case 'weather':
      return { label: 'WEATHER', value: '19°' };
    case 'focus':
      return { label: 'FOCUS', value: `${pad(Math.floor(osState.focusSeconds / 60))}:${pad(osState.focusSeconds % 60)}` };
    case 'memory':
      return { label: 'MEMORY', value: `${osState.metrics.ram}%` };
    case 'date':
      return { label: 'TODAY', value: t.short.toUpperCase() };
  }
}

function drawWidget(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, key: WidgetKey) {
  const d = widgetData(key);
  glass(ctx, x, y, w, h);
  text(ctx, d.label, x + 6, y + 11, 6, 'rgba(255,255,255,0.66)', { weight: 700 });
  text(ctx, d.value, x + 6, y + 24, 12, '#ffffff', { weight: 760 });
}

function lockPage(ctx: CanvasRenderingContext2D, w: number, h: number) {
  wallpaper(ctx, w, h);
  const t = timeParts();
  const a = osAccent();
  const layout = osState.settings.layout;

  if (layout !== 'minimal') statusBar(ctx, w, 'FRAME / OS');

  if (layout === 'hero') {
    text(ctx, t.day.toUpperCase(), 11, 30, 7, 'rgba(255,255,255,0.84)', { weight: 700 });
    text(ctx, t.hhmm, 8, 62, 38, '#ffffff', { weight: 670 });
    if (osState.settings.seconds) text(ctx, t.ss, 96, 62, 11, 'rgba(255,255,255,0.85)', { weight: 600 });
    text(ctx, osState.settings.message, 11, 76, 7, 'rgba(255,255,255,0.82)', { weight: 500 });
    drawWidget(ctx, w - 75, 29, 67, 32, osState.settings.widget1);
    drawWidget(ctx, w - 75, 66, 67, 32, osState.settings.widget2);
  } else if (layout === 'balanced') {
    text(ctx, t.day.toUpperCase(), 11, 36, 7, 'rgba(255,255,255,0.84)', { weight: 700 });
    text(ctx, t.hhmm, 10, 64, 32, '#ffffff', { weight: 680 });
    if (osState.settings.seconds) text(ctx, t.ss, 82, 64, 10, 'rgba(255,255,255,0.8)', { weight: 600 });
    drawWidget(ctx, w - 99, 33, 91, 35, osState.settings.widget1);
    drawWidget(ctx, w - 99, 72, 91, 35, osState.settings.widget2);
  } else {
    text(ctx, t.hhmm, 11, 66, 44, '#ffffff', { weight: 640 });
    if (osState.settings.seconds) text(ctx, t.ss, 116, 66, 12, 'rgba(255,255,255,0.8)', { weight: 600 });
    text(ctx, t.day.toUpperCase(), 12, 78, 6.5, 'rgba(255,255,255,0.7)', { weight: 650 });
  }

  // music dock
  if (layout !== 'minimal') {
    glass(ctx, 9, h - 34, w - 18, 26);
    ctx.fillStyle = 'rgba(255,255,255,0.22)';
    rr(ctx, 14, h - 29, 17, 17, 5);
    ctx.fill();
    ctx.fillStyle = a.color;
    ctx.beginPath();
    ctx.arc(22.5, h - 20.5, 3.4, 0, Math.PI * 2);
    ctx.fill();
    text(ctx, ['Afterglow', 'Night Drive', 'Soft Signals'][osState.track], 36, h - 22, 7.2, '#ffffff', { weight: 700 });
    text(ctx, 'Eunoia · Ambient Edit', 36, h - 14, 6, 'rgba(255,255,255,0.66)', { weight: 500 });
    ctx.fillStyle = 'rgba(255,255,255,0.24)';
    ctx.beginPath();
    ctx.arc(w - 20, h - 21, 8.5, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = '#ffffff';
    if (osState.musicPlaying) {
      ctx.fillRect(w - 23.5, h - 24, 2.2, 6);
      ctx.fillRect(w - 19.4, h - 24, 2.2, 6);
    } else {
      ctx.beginPath();
      ctx.moveTo(w - 22, h - 25);
      ctx.lineTo(w - 17, h - 21);
      ctx.lineTo(w - 22, h - 17);
      ctx.closePath();
      ctx.fill();
    }
  }
  // home indicator
  ctx.fillStyle = 'rgba(255,255,255,0.32)';
  rr(ctx, w / 2 - 11, h - 2.5, 22, 2, 1);
  ctx.fill();
}

function dashPage(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const a = osAccent();
  ctx.fillStyle = '#111922';
  ctx.fillRect(0, 0, w, h);
  const rg = ctx.createRadialGradient(w * 0.95, 0, 2, w * 0.95, 0, w * 0.8);
  rg.addColorStop(0, alpha(a.color, 0.22));
  rg.addColorStop(1, alpha(a.color, 0));
  ctx.fillStyle = rg;
  ctx.fillRect(0, 0, w, h);
  statusBar(ctx, w, 'FRAME / DASH');
  text(ctx, 'At a glance', 9, 26, 15, '#ffffff', { weight: 780 });
  text(ctx, '.', 74, 26, 15, a.color, { weight: 780 });
  text(ctx, 'LIVE', w - 9, 26, 6, 'rgba(255,255,255,0.5)', { weight: 700, align: 'right', font: MONO });

  // stat tile
  glass(ctx, 9, 33, 96, 52);
  text(ctx, 'CPU ACTIVITY', 16, 44, 6, 'rgba(255,255,255,0.6)', { weight: 700 });
  text(ctx, String(osState.metrics.cpu), 16, 66, 20, metricColor(osState.metrics.cpu, 'load'), { weight: 760 });
  text(ctx, '%', 38, 66, 9, 'rgba(255,255,255,0.6)', { weight: 600 });
  // sparkline
  ctx.strokeStyle = a.color;
  ctx.lineWidth = 1.4;
  ctx.beginPath();
  osState.history.forEach((v, i) => {
    const x = 50 + (i / (osState.history.length - 1)) * 50;
    const y = 79 - (v / 100) * 34;
    i ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
  });
  ctx.stroke();

  // song card
  glass(ctx, 111, 33, 120, 30);
  ctx.fillStyle = alpha(a.color, 0.35);
  rr(ctx, 116, 38, 20, 20, 5);
  ctx.fill();
  ctx.fillStyle = a.color;
  ctx.beginPath();
  ctx.arc(126, 48, 4, 0, Math.PI * 2);
  ctx.fill();
  text(ctx, ['Afterglow', 'Night Drive', 'Soft Signals'][osState.track], 141, 45, 8, '#ffffff', { weight: 700 });
  text(ctx, osState.musicPlaying ? 'Now playing · Music' : 'Paused · Music', 141, 55, 6.5, 'rgba(255,255,255,0.62)', { weight: 500 });

  // two mini tiles
  glass(ctx, 111, 67, 58, 18);
  text(ctx, 'WEATHER', 117, 78, 6, 'rgba(255,255,255,0.6)', { weight: 700 });
  text(ctx, '19°', 163, 79, 12, '#ffffff', { weight: 780, align: 'right' });
  glass(ctx, 173, 67, 58, 18);
  text(ctx, 'FOCUS', 179, 78, 6, 'rgba(255,255,255,0.6)', { weight: 700 });
  text(ctx, `${Math.floor(osState.focusSeconds / 60)}m`, 225, 79, 12, '#ffffff', { weight: 780, align: 'right' });
}

function musicPage(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const a = osAccent();
  const g = ctx.createRadialGradient(w * 0.2, h * 0.5, 4, w * 0.2, h * 0.5, w * 0.9);
  g.addColorStop(0, '#4a3a66');
  g.addColorStop(1, '#151526');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, w, h);
  statusBar(ctx, w, 'FRAME / AUDIO');

  // album art
  const ag = ctx.createLinearGradient(11, 24, 99, 112);
  ag.addColorStop(0, '#2f567c');
  ag.addColorStop(0.6, '#a16c9f');
  ag.addColorStop(1, '#dfac91');
  ctx.fillStyle = ag;
  rr(ctx, 11, 24, 88, 88, 9);
  ctx.fill();
  ctx.fillStyle = 'rgba(40,61,101,0.85)';
  ctx.beginPath();
  ctx.ellipse(45, 74, 46, 12, -0.33, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = 'rgba(196,133,175,0.8)';
  ctx.beginPath();
  ctx.ellipse(58, 86, 40, 10, 0.38, 0, Math.PI * 2);
  ctx.fill();

  text(ctx, osState.musicPlaying ? 'NOW PLAYING' : 'PAUSED', 110, 30, 6.5, 'rgba(255,255,255,0.6)', { weight: 700, font: MONO });
  text(ctx, ['Afterglow', 'Night Drive', 'Soft Signals'][osState.track], 110, 46, 15, '#ffffff', { weight: 800 });
  text(ctx, 'Eunoia · Ambient Edit', 110, 57, 7, 'rgba(255,255,255,0.72)', { weight: 500 });

  // transport
  const cy = 82;
  ctx.fillStyle = 'rgba(255,255,255,0.2)';
  ctx.beginPath();
  ctx.arc(120, cy, 9, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = a.color;
  ctx.beginPath();
  ctx.arc(142, cy, 12, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = a.ink;
  if (osState.musicPlaying) {
    ctx.fillRect(138.5, cy - 5, 2.6, 10);
    ctx.fillRect(143, cy - 5, 2.6, 10);
  } else {
    ctx.beginPath();
    ctx.moveTo(138, cy - 6);
    ctx.lineTo(147, cy);
    ctx.lineTo(138, cy + 6);
    ctx.closePath();
    ctx.fill();
  }
  ctx.fillStyle = 'rgba(255,255,255,0.85)';
  ctx.beginPath();
  ctx.moveTo(126, cy - 4);
  ctx.lineTo(119, cy);
  ctx.lineTo(126, cy + 4);
  ctx.closePath();
  ctx.fill();
  ctx.beginPath();
  ctx.moveTo(160, cy - 4);
  ctx.lineTo(167, cy);
  ctx.lineTo(160, cy + 4);
  ctx.closePath();
  ctx.fill();

  // progress
  ctx.fillStyle = 'rgba(255,255,255,0.3)';
  rr(ctx, 111, 113, 118, 3, 1.5);
  ctx.fill();
  ctx.fillStyle = a.color;
  rr(ctx, 111, 113, 118 * (osState.progress / 100), 3, 1.5);
  ctx.fill();
  text(ctx, '0:52', 111, 126, 6, 'rgba(255,255,255,0.6)', { weight: 600, font: MONO });
  text(ctx, '3:52', 229, 126, 6, 'rgba(255,255,255,0.6)', { weight: 600, font: MONO, align: 'right' });
}

/** Value-reactive metric colour: green → amber → red as the metric climbs. */
export function metricColor(v: number, kind: 'load' | 'mem' | 'temp' = 'load'): string {
  if (kind === 'temp') return v < 60 ? '#59d38d' : v < 76 ? '#f5c451' : '#ff6b6b';
  if (kind === 'mem') return v < 60 ? '#59d38d' : v < 85 ? '#f5c451' : '#ff6b6b';
  return v < 40 ? '#59d38d' : v < 70 ? '#f5c451' : '#ff6b6b';
}

function perfPage(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const a = osAccent();
  ctx.fillStyle = '#111922';
  ctx.fillRect(0, 0, w, h);
  statusBar(ctx, w, 'FRAME / MONITOR');
  text(ctx, 'Performance', 9, 26, 15, '#ffffff', { weight: 780 });
  text(ctx, '.', 74, 26, 15, a.color, { weight: 780 });
  text(ctx, 'LIVE', w - 9, 26, 6, 'rgba(255,255,255,0.5)', { weight: 700, align: 'right', font: MONO });

  const cards: [string, number, string][] = [
    ['CPU LOAD', osState.metrics.cpu, '%'],
    ['GPU LOAD', osState.metrics.gpu, '%'],
    ['MEMORY', osState.metrics.ram, '%'],
  ];
  cards.forEach(([label, value, unit], i) => {
    const x = 9 + i * 75;
    glass(ctx, x, 33, 70, 42);
    text(ctx, label, x + 7, 44, 6, 'rgba(255,255,255,0.6)', { weight: 700 });
    const mc = metricColor(value, label === 'MEMORY' ? 'mem' : 'load');
    text(ctx, String(value), x + 7, 64, 18, mc, { weight: 780 });
    text(ctx, unit, x + 7 + ctx.measureText(String(value)).width + 10, 64, 8, 'rgba(255,255,255,0.5)', { weight: 600 });
    ctx.fillStyle = alpha(mc, 0.9);
    ctx.fillRect(x + 7, 68, 56 * Math.min(1, value / 100), 1.5);
  });

  text(ctx, 'UTILIZATION · LAST 30 S', 10, 88, 6, 'rgba(255,255,255,0.5)', { weight: 700, font: MONO });
  text(ctx, `${osState.metrics.temp}°C`, w - 10, 88, 6, metricColor(osState.metrics.temp, 'temp'), { weight: 700, font: MONO, align: 'right' });
  ctx.strokeStyle = metricColor(osState.metrics.cpu, 'load');
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  osState.history.forEach((v, i) => {
    const x = 10 + (i / (osState.history.length - 1)) * (w - 20);
    const y = 126 - (v / 100) * 30;
    i ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
  });
  ctx.stroke();
  ctx.lineTo(w - 10, 130);
  ctx.lineTo(10, 130);
  ctx.closePath();
  ctx.fillStyle = alpha(a.color, 0.1);
  ctx.fill();
}

function focusPage(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const a = osAccent();
  ctx.fillStyle = '#17251f';
  ctx.fillRect(0, 0, w, h);
  const rg = ctx.createRadialGradient(w * 0.8, h * 0.5, 4, w * 0.8, h * 0.5, w * 0.7);
  rg.addColorStop(0, 'rgba(129,178,153,0.35)');
  rg.addColorStop(1, 'rgba(129,178,153,0)');
  ctx.fillStyle = rg;
  ctx.fillRect(0, 0, w, h);
  statusBar(ctx, w, 'FRAME / FOCUS');
  text(ctx, 'DEEP WORK SESSION', 12, 40, 7, 'rgba(190,220,200,0.85)', { weight: 730 });
  text(ctx, `${pad(Math.floor(osState.focusSeconds / 60))}:${pad(osState.focusSeconds % 60)}`, 11, 76, 35, '#ffffff', { weight: 770 });
  text(ctx, osState.focusRunning ? 'Stay in your flow.' : 'Ready when you are.', 13, 90, 7, 'rgba(170,200,180,0.8)', { weight: 500 });

  // start/pause pill
  ctx.fillStyle = a.color;
  rr(ctx, 12, 104, 62, 16, 5);
  ctx.fill();
  text(ctx, osState.focusRunning ? 'PAUSE' : 'START FOCUS', 43, 114, 7, a.ink, { weight: 850, align: 'center' });

  // progress ring
  const cx = 186;
  const cy = 74;
  const r = 34;
  ctx.strokeStyle = 'rgba(255,255,255,0.14)';
  ctx.lineWidth = 4;
  ctx.beginPath();
  ctx.arc(cx, cy, r, 0, Math.PI * 2);
  ctx.stroke();
  const frac = 1 - osState.focusSeconds / (25 * 60);
  ctx.strokeStyle = a.color;
  ctx.lineWidth = 4;
  ctx.lineCap = 'round';
  ctx.beginPath();
  ctx.arc(cx, cy, r, -Math.PI / 2, -Math.PI / 2 + frac * Math.PI * 2);
  ctx.stroke();
  text(ctx, `${Math.round(frac * 100)}%`, cx, cy + 3, 11, '#ffffff', { weight: 760, align: 'center' });
}

function shortcutsPage(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const a = osAccent();
  ctx.fillStyle = '#111922';
  ctx.fillRect(0, 0, w, h);
  statusBar(ctx, w, 'FRAME / COMMAND');
  text(ctx, 'Shortcuts', 9, 26, 15, '#ffffff', { weight: 780 });
  text(ctx, '.', 66, 26, 15, a.color, { weight: 780 });
  text(ctx, '6 MACROS', w - 9, 26, 6, 'rgba(255,255,255,0.5)', { weight: 700, align: 'right', font: MONO });

  const glyphs = ['terminal', 'volume', 'pencil', 'timer', 'mail', 'maximize'];
  osState.settings.macros.forEach((m, i) => {
    const col = i % 3;
    const row = Math.floor(i / 3);
    const x = 9 + col * 75;
    const y = 36 + row * 46;
    glass(ctx, x, y, 70, 40);
    ctx.strokeStyle = a.color;
    ctx.lineWidth = 1.4;
    ctx.beginPath();
    // simple glyph: circle + tick
    ctx.arc(x + 12, y + 13, 5, 0, Math.PI * 2);
    ctx.stroke();
    text(ctx, m.length > 13 ? m.slice(0, 12) + '…' : m, x + 22, y + 16, 7, '#e9f3ec', { weight: 650 });
    text(ctx, `M0${i + 1}`, x + 8, y + 32, 5.5, 'rgba(255,255,255,0.45)', { weight: 600, font: MONO });
  });
}

function weatherPage(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const g = ctx.createLinearGradient(0, 0, w, h);
  g.addColorStop(0, '#16445b');
  g.addColorStop(0.6, '#377e91');
  g.addColorStop(1, '#113d59');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, w, h);
  statusBar(ctx, w, 'FRAME / WEATHER');
  text(ctx, '19°', 12, 68, 40, '#ffffff', { weight: 760 });
  text(ctx, osState.settings.city || 'Your city', 13, 84, 8, 'rgba(255,255,255,0.9)', { weight: 650 });
  text(ctx, 'Mostly sunny · Feels like 21°', 13, 95, 7, 'rgba(225,246,255,0.75)', { weight: 500 });

  // sun + cloud
  ctx.fillStyle = 'rgba(255,224,165,0.95)';
  ctx.beginPath();
  ctx.arc(196, 44, 16, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = 'rgba(215,237,238,0.9)';
  ctx.beginPath();
  ctx.arc(178, 60, 12, 0, Math.PI * 2);
  ctx.arc(196, 56, 15, 0, Math.PI * 2);
  ctx.arc(212, 62, 10, 0, Math.PI * 2);
  ctx.fill();

  glass(ctx, 10, 104, w - 20, 23);
  const fc: [string, string][] = [
    ['NOW', '19°'],
    ['12 PM', '22°'],
    ['3 PM', '24°'],
    ['6 PM', '18°'],
  ];
  fc.forEach(([k, v], i) => {
    const x = 18 + i * 52;
    text(ctx, k, x, 114, 6, 'rgba(255,255,255,0.6)', { weight: 650 });
    text(ctx, v, x, 123, 8, '#ecfaff', { weight: 700 });
  });
}

function calendarPage(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const a = osAccent();
  const g = ctx.createLinearGradient(0, 0, w, h);
  g.addColorStop(0, '#15202d');
  g.addColorStop(1, '#212536');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, w, h);
  statusBar(ctx, w, 'FRAME / PLANNER');
  const now = new Date();
  text(ctx, now.toLocaleDateString('en-US', { month: 'long' }), 9, 26, 14, '#ffffff', { weight: 780 });
  text(ctx, String(now.getFullYear()), 88, 26, 14, a.color, { weight: 780 });

  const days = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate();
  const offset = (new Date(now.getFullYear(), now.getMonth(), 1).getDay() + 6) % 7;
  const cw = 17;
  const ch = 13;
  const gx = 10;
  const gy = 36;
  'MTWTFSS'.split('').forEach((d, i) => {
    text(ctx, d, gx + i * cw + cw / 2, gy + 6, 6, 'rgba(255,255,255,0.45)', { weight: 700, align: 'center' });
  });
  for (let i = 0; i < 35; i++) {
    const n = i - offset + 1;
    if (n < 1 || n > days) continue;
    const col = i % 7;
    const row = Math.floor(i / 7);
    const x = gx + col * cw;
    const y = gy + 9 + row * ch;
    if (n === now.getDate()) {
      ctx.fillStyle = a.color;
      rr(ctx, x + 1, y, cw - 3, ch - 2, 3);
      ctx.fill();
    }
    text(ctx, String(n), x + cw / 2, y + 8, 7, n === now.getDate() ? a.ink : 'rgba(255,255,255,0.8)', {
      weight: n === now.getDate() ? 850 : 600,
      align: 'center',
    });
  }

  // agenda
  const ax = 141;
  text(ctx, `TODAY · ${pad(now.getDate())}`, ax, 46, 7, a.color, { weight: 800 });
  const events: [string, string][] = [
    ['Design review', '10:30 · Studio'],
    ['Deep work', '14:00 · Focus'],
  ];
  events.forEach(([t, s], i) => {
    const y = 58 + i * 26;
    ctx.fillStyle = a.color;
    ctx.fillRect(ax, y - 7, 1.6, 16);
    text(ctx, t, ax + 6, y, 7, '#ffffff', { weight: 700 });
    text(ctx, s, ax + 6, y + 9, 6, 'rgba(255,255,255,0.6)', { weight: 500 });
  });
}

function settingsPage(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const a = osAccent();
  ctx.fillStyle = '#111922';
  ctx.fillRect(0, 0, w, h);
  statusBar(ctx, w, 'FRAME / CONTROL');
  text(ctx, 'Control', 9, 26, 15, '#ffffff', { weight: 780 });
  text(ctx, '.', 56, 26, 15, a.color, { weight: 780 });

  const tiles: [string, boolean][] = [
    ['Bluetooth', true],
    ['RGB lights', true],
    ['Media dock', true],
    ['Focus', false],
    ['Monitor', false],
    ['Sleep', false],
  ];
  tiles.forEach(([label, active], i) => {
    const col = i % 3;
    const row = Math.floor(i / 3);
    const x = 9 + col * 75;
    const y = 36 + row * 46;
    ctx.fillStyle = active ? alpha(a.color, 0.2) : 'rgba(255,255,255,0.07)';
    rr(ctx, x, y, 70, 40, 8);
    ctx.fill();
    ctx.strokeStyle = active ? alpha(a.color, 0.5) : 'rgba(255,255,255,0.12)';
    ctx.lineWidth = 0.8;
    ctx.stroke();
    ctx.strokeStyle = active ? a.color : 'rgba(255,255,255,0.6)';
    ctx.lineWidth = 1.3;
    ctx.beginPath();
    ctx.arc(x + 35, y + 15, 6, 0, Math.PI * 2);
    ctx.stroke();
    text(ctx, label, x + 35, y + 32, 7, active ? '#e5edeb' : 'rgba(255,255,255,0.6)', { weight: 620, align: 'center' });
  });
  text(ctx, `Brightness ${osState.settings.brightness}% · FRAME OS 1.0`, 13, 128, 7, 'rgba(255,255,255,0.5)', { weight: 550 });
}

// ---------------------------------------------------------------------------
// Public render
// ---------------------------------------------------------------------------

export function renderOsPage(page: OsPage, w: number, h: number): Uint8ClampedArray {
  const canvas = document.createElement('canvas');
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext('2d');
  if (!ctx) return new Uint8ClampedArray(w * h * 4);
  ctx.imageSmoothingEnabled = false;
  switch (page) {
    case 'lock':
      lockPage(ctx, w, h);
      break;
    case 'dash':
      dashPage(ctx, w, h);
      break;
    case 'music':
      musicPage(ctx, w, h);
      break;
    case 'perf':
      perfPage(ctx, w, h);
      break;
    case 'focus':
      focusPage(ctx, w, h);
      break;
    case 'shortcuts':
      shortcutsPage(ctx, w, h);
      break;
    case 'weather':
      weatherPage(ctx, w, h);
      break;
    case 'calendar':
      calendarPage(ctx, w, h);
      break;
    case 'settings':
      settingsPage(ctx, w, h);
      break;
  }
  if (osState.toast) {
    ctx.fillStyle = 'rgba(32,39,43,0.86)';
    rr(ctx, 35, h - 24, w - 70, 13, 4);
    ctx.fill();
    ctx.strokeStyle = 'rgba(255,255,255,0.25)';
    ctx.lineWidth = 0.6;
    ctx.stroke();
    text(ctx, osState.toast, w / 2, h - 15, 7, '#ffffff', { weight: 600, align: 'center' });
  }
  return ctx.getImageData(0, 0, w, h).data;
}

export function osFrames(page: OsPage, w: number, h: number, count: number, delayCentis = 100) {
  const out: { rgba_b64: string; delay: number }[] = [];
  const base = osClock.offsetSec;
  for (let i = 0; i < count; i++) {
    osClock.offsetSec = base + i;
    const data = renderOsPage(page, w, h);
    out.push({ rgba_b64: b64(data), delay: delayCentis });
  }
  osClock.offsetSec = base;
  return out;
}

export function b64(data: Uint8ClampedArray): string {
  let s = '';
  const chunk = 0x8000;
  for (let i = 0; i < data.length; i += chunk) {
    s += String.fromCharCode.apply(null, Array.from(data.subarray(i, i + chunk)) as unknown as number[]);
  }
  return btoa(s);
}

/** Tick the simulated OS state (metrics drift, focus timer, music progress). */
export function tickOs() {
  if (osState.focusRunning && osState.focusSeconds > 0) {
    osState.focusSeconds--;
    if (osState.focusSeconds === 0) osState.focusRunning = false;
  }
  if (osState.musicPlaying) {
    osState.progress = (osState.progress + 0.6) % 100;
  }
  const m = osState.metrics;
  m.cpu = Math.round(Math.max(8, Math.min(88, m.cpu + (Math.random() - 0.5) * 14)));
  m.gpu = Math.round(Math.max(10, Math.min(92, m.gpu + (Math.random() - 0.5) * 10)));
  m.ram = Math.round(Math.max(52, Math.min(78, m.ram + (Math.random() - 0.5) * 3)));
  m.temp = Math.round(Math.max(39, Math.min(77, m.temp + (Math.random() - 0.5) * 3)));
  osState.history.push(m.cpu);
  if (osState.history.length > 22) osState.history.shift();
}
