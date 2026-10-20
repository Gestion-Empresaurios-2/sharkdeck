// SharkDeck screen-template engine: deterministic pixel renderers for the
// K86 panel. Every draw is a pure function of (template, components, size,
// frame index, timestamp) so previews and pushed frames match exactly.

import { live } from './live';

export type CompKind =
  | 'clock'
  | 'date'
  | 'ticker'
  | 'wave'
  | 'rain'
  | 'border'
  | 'eq'
  | 'scanlines'
  | 'fin'
  | 'meter'
  | 'watch'
  | 'datewin'
  | 'battery'
  | 'ring'
  | 'steps'
  | 'hostbatt'
  | 'cpu'
  | 'ram'
  | 'devinfo'
  | 'uptime';

/**
 * Layout slots — a HyperOS-lock-screen style grid of anchor positions.
 * Widgets are assigned to a slot; the slot decides where they land.
 */
export type SlotId = 'tl' | 'tr' | 'c' | 'bl' | 'br' | 'bar' | 'l' | 'r';

export const SLOT_IDS: SlotId[] = ['tl', 'tr', 'c', 'bl', 'br', 'bar', 'l', 'r'];

export const SLOT_LABEL: Record<SlotId, string> = {
  tl: 'Top left',
  tr: 'Top right',
  c: 'Centre',
  bl: 'Bottom left',
  br: 'Bottom right',
  bar: 'Bottom bar',
  l: 'Left',
  r: 'Right',
};

export interface Comp {
  kind: CompKind;
  text?: string;   // ticker / meter label
  fg?: string;     // hex color
  seconds?: boolean;
  size?: number;   // clock: fraction of panel height (default auto)
  amp?: number;    // wave amplitude px
  wlen?: number;   // wave wavelength px
  speed?: number;  // anim px/frame or shift
  value?: number;  // meter 0..1
  bars?: number;   // eq bars
  batteryStyle?: 'segments' | 'ios'; // battery glyph style
  slot?: SlotId;   // layout slot this widget is pinned to
}

/**
 * A layout = a set of widget positions + a clock treatment, in the spirit of
 * a phone lock screen: pick a layout, then drop widgets into its slots.
 */
export interface LayoutDef {
  id: string;
  name: string;
  desc: string;
  /** slots this layout offers */
  slots: SlotId[];
  /** default clock placement + size (fraction of panel height) */
  clock: { slot: SlotId; size: number };
  /** widgets the layout starts with (kind + default slot) */
  widgets: { kind: CompKind; slot: SlotId; text?: string; fg?: string }[];
}

export interface Template {
  id: string;
  name: string;
  desc: string;
  bg: string;
  comps: Comp[];
  /** seconds per pushed frame (drives device delay byte) */
  dt: number;
  /** available layouts (templates without layouts keep their fixed composition) */
  layouts?: LayoutDef[];
  /** active layout id */
  layout?: string;
}

/* ------------------------------------------------------------------ */
/* Built-in gallery                                                    */
/* ------------------------------------------------------------------ */


/* ------------------------------------------------------------------ */
/* Layout-driven templates (HyperOS lock-screen style)                 */
/* ------------------------------------------------------------------ */

/** Build a template whose widgets come from its first layout. */
function mk(
  id: string,
  name: string,
  desc: string,
  bg: string,
  dt: number,
  layouts: LayoutDef[],
): Template {
  return {
    id,
    name,
    desc,
    bg,
    dt,
    layouts,
    layout: layouts[0].id,
    comps: compsFromLayout(layouts[0]),
  };
}

const LAYOUT_LOCK_CLASSIC: LayoutDef[] = [
  {
    id: 'classic',
    name: 'Classic',
    desc: 'Big clock centred, widgets around the edges',
    slots: ['tl', 'tr', 'c', 'bl', 'br', 'bar'],
    clock: { slot: 'c', size: 0.42 },
    widgets: [
      { kind: 'date', slot: 'tl' },
      { kind: 'hostbatt', slot: 'tr' },
      { kind: 'ticker', slot: 'bar', text: 'SharkDeck' },
      { kind: 'devinfo', slot: 'bl' },
      { kind: 'uptime', slot: 'br' },
    ],
  },
  {
    id: 'split',
    name: 'Split',
    desc: 'Clock hugging the left edge, data column on the right',
    slots: ['tl', 'tr', 'l', 'r', 'bl', 'br'],
    clock: { slot: 'l', size: 0.46 },
    widgets: [
      { kind: 'date', slot: 'tr' },
      { kind: 'hostbatt', slot: 'r' },
      { kind: 'cpu', slot: 'bl' },
      { kind: 'ram', slot: 'br' },
    ],
  },
  {
    id: 'corner',
    name: 'Corner',
    desc: 'Clock pinned to the top-left, everything else minimal',
    slots: ['tl', 'tr', 'bl', 'br'],
    clock: { slot: 'tl', size: 0.34 },
    widgets: [
      { kind: 'hostbatt', slot: 'tr' },
      { kind: 'ticker', slot: 'br', text: 'SharkDeck' },
    ],
  },
];

const LAYOUT_LOCK_FOCUS: LayoutDef[] = [
  {
    id: 'focus',
    name: 'Focus',
    desc: 'One big clock, one ring, nothing else',
    slots: ['c', 'tr', 'bl'],
    clock: { slot: 'c', size: 0.52 },
    widgets: [
      { kind: 'ring', slot: 'tr' },
      { kind: 'date', slot: 'bl' },
    ],
  },
  {
    id: 'cards',
    name: 'Cards',
    desc: 'Clock plus two stacked info cards',
    slots: ['tl', 'tr', 'bl', 'br'],
    clock: { slot: 'tl', size: 0.36 },
    widgets: [
      { kind: 'hostbatt', slot: 'tr' },
      { kind: 'cpu', slot: 'bl' },
      { kind: 'ram', slot: 'br' },
    ],
  },
];

const LAYOUT_LOCK_WEATHER: LayoutDef[] = [
  {
    id: 'big',
    name: 'Big',
    desc: 'Large clock with a wide ticker band',
    slots: ['c', 'tl', 'tr', 'bar'],
    clock: { slot: 'c', size: 0.44 },
    widgets: [
      { kind: 'ticker', slot: 'bar', text: 'SharkDeck  ·  21C  ·  clear' },
      { kind: 'hostbatt', slot: 'tl' },
      { kind: 'devinfo', slot: 'tr' },
    ],
  },
  {
    id: 'compact',
    name: 'Compact',
    desc: 'Small clock tucked into the corner',
    slots: ['tl', 'tr', 'bl', 'br'],
    clock: { slot: 'tl', size: 0.26 },
    widgets: [
      { kind: 'ticker', slot: 'br', text: 'SharkDeck' },
      { kind: 'hostbatt', slot: 'tr' },
    ],
  },
];

const LAYOUT_LOCK_AGENDA: LayoutDef[] = [
  {
    id: 'agenda',
    name: 'Agenda',
    desc: 'Clock on the left, agenda list on the right',
    slots: ['l', 'r', 'tr', 'br'],
    clock: { slot: 'l', size: 0.4 },
    widgets: [
      { kind: 'ticker', slot: 'r', text: '09:30 standup' },
      { kind: 'date', slot: 'tr' },
      { kind: 'devinfo', slot: 'br' },
    ],
  },
  {
    id: 'split',
    name: 'Split',
    desc: 'Clock up top, agenda underneath',
    slots: ['c', 'bar', 'bl', 'br'],
    clock: { slot: 'c', size: 0.34 },
    widgets: [
      { kind: 'ticker', slot: 'bar', text: '09:30 standup' },
      { kind: 'hostbatt', slot: 'bl' },
      { kind: 'uptime', slot: 'br' },
    ],
  },
];

const LAYOUT_LOCK_MUSIC: LayoutDef[] = [
  {
    id: 'art',
    name: 'Art',
    desc: 'Clock left, equaliser art right',
    slots: ['l', 'r', 'tl', 'br'],
    clock: { slot: 'l', size: 0.38 },
    widgets: [
      { kind: 'eq', slot: 'r' },
      { kind: 'ticker', slot: 'br', text: 'now playing' },
    ],
  },
  {
    id: 'minimal',
    name: 'Minimal',
    desc: 'Clock centred, tiny hints',
    slots: ['c', 'tl', 'br'],
    clock: { slot: 'c', size: 0.48 },
    widgets: [
      { kind: 'ticker', slot: 'br', text: 'now playing' },
      { kind: 'hostbatt', slot: 'tl' },
    ],
  },
];

const LAYOUT_LOCK_BATTERY: LayoutDef[] = [
  {
    id: 'cell',
    name: 'Cell',
    desc: 'Big battery cell with a corner clock',
    slots: ['tl', 'c', 'bl', 'br'],
    clock: { slot: 'tl', size: 0.3 },
    widgets: [
      { kind: 'battery', slot: 'c' },
      { kind: 'hostbatt', slot: 'br' },
      { kind: 'devinfo', slot: 'bl' },
    ],
  },
  {
    id: 'row',
    name: 'Row',
    desc: 'Clock centred, battery row underneath',
    slots: ['c', 'bar', 'tl', 'tr'],
    clock: { slot: 'c', size: 0.34 },
    widgets: [
      { kind: 'battery', slot: 'bar' },
      { kind: 'hostbatt', slot: 'tl' },
      { kind: 'steps', slot: 'tr', text: '8.4k' },
    ],
  },
];

const LAYOUT_NEON_CITY: LayoutDef[] = [
  {
    id: 'horizon',
    name: 'Horizon',
    desc: 'City skyline silhouette under a wide clock',
    slots: ['tl', 'tr', 'bar', 'bl'],
    clock: { slot: 'tl', size: 0.32 },
    widgets: [
      { kind: 'wave', slot: 'bar' },
      { kind: 'hostbatt', slot: 'tr' },
      { kind: 'ticker', slot: 'bl', text: 'SharkDeck' },
    ],
  },
];

const LAYOUT_TERMINAL: LayoutDef[] = [
  {
    id: 'terminal',
    name: 'Terminal',
    desc: 'Amber terminal readout',
    slots: ['tl', 'tr', 'bl', 'br'],
    clock: { slot: 'tl', size: 0.32 },
    widgets: [
      { kind: 'ticker', slot: 'br', text: '$ sharkdeck --status' },
      { kind: 'devinfo', slot: 'bl' },
      { kind: 'uptime', slot: 'tr' },
    ],
  },
];

const LAYOUT_DEVOPS: LayoutDef[] = [
  {
    id: 'stack',
    name: 'Stack',
    desc: 'Clock on top, CPU/RAM stacked below',
    slots: ['tl', 'tr', 'bl', 'br'],
    clock: { slot: 'tl', size: 0.3 },
    widgets: [
      { kind: 'cpu', slot: 'tr' },
      { kind: 'ram', slot: 'bl' },
      { kind: 'devinfo', slot: 'br' },
    ],
  },
  {
    id: 'split',
    name: 'Split',
    desc: 'Clock left, metrics right',
    slots: ['l', 'r', 'tr', 'br'],
    clock: { slot: 'l', size: 0.34 },
    widgets: [
      { kind: 'cpu', slot: 'r' },
      { kind: 'ram', slot: 'br' },
      { kind: 'hostbatt', slot: 'tr' },
    ],
  },
];

const LAYOUT_TEMPLATES: Template[] = [
  mk('lock-classic', 'Lock · Classic', 'HyperOS-style lock screen, three layouts', '#0b0e14', 1, LAYOUT_LOCK_CLASSIC),
  mk('lock-focus', 'Lock · Focus', 'Distraction-free clock with cards', '#0d0f14', 1, LAYOUT_LOCK_FOCUS),
  mk('lock-weather', 'Lock · Weather', 'Clock plus a status ticker', '#0a1014', 1, LAYOUT_LOCK_WEATHER),
  mk('lock-agenda', 'Lock · Agenda', 'Day planner on the panel', '#101218', 1, LAYOUT_LOCK_AGENDA),
  mk('lock-music', 'Lock · Music', 'Now playing with equaliser art', '#120d12', 1, LAYOUT_LOCK_MUSIC),
  mk('lock-battery', 'Lock · Battery', 'Battery monitor for the panel', '#0d120e', 1, LAYOUT_LOCK_BATTERY),
  mk('neon-city', 'Neon City', 'Skyline silhouette with a wide clock', '#150d1c', 1, LAYOUT_NEON_CITY),
  mk('terminal-amber', 'Terminal Amber', 'Amber console readout', '#140f06', 1, LAYOUT_TERMINAL),
  mk('devops', 'DevOps', 'CPU, RAM and keyboard state', '#0b1216', 2, LAYOUT_DEVOPS),
];

export const TEMPLATES: Template[] = [
  {
    id: 'dashboard',
    name: 'Dashboard',
    desc: 'Big clock, date, status ticker',
    bg: '#04070d',
    dt: 1,
    comps: [
      { kind: 'clock', fg: '#f2f1f0', seconds: false },
      { kind: 'date', fg: '#8e97a8' },
      { kind: 'ticker', text: 'SharkDeck ready', fg: '#e95420', speed: 1 },
    ],
  },
  {
    id: 'minimal',
    name: 'Minimal',
    desc: 'Just the clock on black',
    bg: '#000000',
    dt: 1,
    comps: [{ kind: 'clock', fg: '#f2f1f0', seconds: false }],
  },
  {
    id: 'retro',
    name: 'Retro terminal',
    desc: 'Green phosphor with scanlines',
    bg: '#001400',
    dt: 1,
    comps: [
      { kind: 'clock', fg: '#39ff7a', seconds: true },
      { kind: 'scanlines', fg: '#000000' },
    ],
  },
  {
    id: 'matrix',
    name: 'Matrix rain',
    desc: 'Falling glyphs behind a clock',
    bg: '#010a03',
    dt: 0.08,
    comps: [
      { kind: 'rain', fg: '#19d449', speed: 7 },
      { kind: 'clock', fg: '#d9ffe2', seconds: true },
    ],
  },
  {
    id: 'waves',
    name: 'Deep waves',
    desc: 'Layered sine surf with a clock',
    bg: '#001120',
    dt: 0.08,
    comps: [
      { kind: 'wave', fg: '#0b4f77', amp: 10, wlen: 54, speed: 1.1 },
      { kind: 'wave', fg: '#1279b7', amp: 7, wlen: 34, speed: 1.8 },
      { kind: 'clock', fg: '#eaf4ff', seconds: false },
    ],
  },
  {
    id: 'equalizer',
    name: 'Equalizer',
    desc: 'Bouncing bars deck',
    bg: '#02040a',
    dt: 0.08,
    comps: [
      { kind: 'eq', fg: '#e95420', bars: 20 },
      { kind: 'clock', fg: '#c7cdd8', seconds: false },
    ],
  },
  {
    id: 'shark',
    name: 'SharkDeck',
    desc: 'Brand idle screen',
    bg: '#050b14',
    dt: 0.5,
    comps: [
      { kind: 'fin', fg: '#7a7ff0' },
      { kind: 'ticker', text: 'SharkDeck', fg: '#aab4ff', speed: 1 },
    ],
  },
  {
    id: 'classic-watch',
    name: 'Classic watch',
    desc: 'Analog dial, date window, battery',
    bg: '#0b0d12',
    dt: 0.5,
    comps: [
      { kind: 'watch', fg: '#e8eaf0' },
      { kind: 'datewin', fg: '#f2f1f0' },
      { kind: 'battery', fg: '#9fe3b0' },
      { kind: 'ticker', text: 'SharkDeck', fg: '#e95420', speed: 1 },
    ],
  },
  {
    id: 'pixel-watch',
    name: 'Pixel watch',
    desc: 'Green 8-bit dial with widgets around',
    bg: '#0d1411',
    dt: 0.5,
    comps: [
      { kind: 'watch', fg: '#7ef0c0' },
      { kind: 'battery', fg: '#7ef0c0' },
      { kind: 'ring', fg: '#e95420', value: 0.64 },
      { kind: 'steps', fg: '#7ef0c0', text: '8.4k' },
    ],
  },
  {
    id: 'digital-watch',
    name: 'Digital watch',
    desc: 'Big digits ringed by date + battery',
    bg: '#05070a',
    dt: 1,
    comps: [
      { kind: 'border', fg: '#2a3240' },
      { kind: 'clock', fg: '#f2f1f0', seconds: false, size: 0.40 },
      { kind: 'datewin', fg: '#e95420' },
      { kind: 'battery', fg: '#9fe3b0' },
      { kind: 'ring', fg: '#5b8cff', value: 0.42 },
    ],
  },
  {
    id: 'chrono-watch',
    name: 'Chronograph',
    desc: 'Dial, progress ring, date line',
    bg: '#0a0a0c',
    dt: 0.5,
    comps: [
      { kind: 'watch', fg: '#f0efe9' },
      { kind: 'ring', fg: '#e95420', value: 0.78 },
      { kind: 'date', fg: '#8e97a8' },
    ],
  },
  ...LAYOUT_TEMPLATES,
];

/** Animation-per-frame components (only these vary with frame index). */
export function templateIsAnimated(t: Template): boolean {
  const t0 = renderTemplate(t, 8, 4, 0);
  const t1 = renderTemplate(t, 8, 4, 1);
  return t1.some((v, i) => v !== t0[i]);
}

/* ------------------------------------------------------------------ */
/* Renderer                                                            */
/* ------------------------------------------------------------------ */

const MONO = '"Ubuntu Mono", "DejaVu Sans Mono", monospace';
const UI = '"Ubuntu", "Cantarell", sans-serif';

function hex(c: string, fallback = '#ffffff'): [number, number, number] {
  const m = /^#?([0-9a-f]{6})$/i.exec(c.trim() ?? '');
  if (!m) return hex(fallback);
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

function withAlpha(c: [number, number, number], a: number) {
  return `rgba(${c[0]},${c[1]},${c[2]},${a})`;
}

/** Rounded-rect path (webview-safe, no ctx.roundRect dependency). */
function rrPath(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  const rad = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + rad, y);
  ctx.lineTo(x + w - rad, y);
  ctx.quadraticCurveTo(x + w, y, x + w, y + rad);
  ctx.lineTo(x + w, y + h - rad);
  ctx.quadraticCurveTo(x + w, y + h, x + w - rad, y + h);
  ctx.lineTo(x + rad, y + h);
  ctx.quadraticCurveTo(x, y + h, x, y + h - rad);
  ctx.lineTo(x, y + rad);
  ctx.quadraticCurveTo(x, y, x + rad, y);
  ctx.closePath();
}

/**
 * Battery glyphs.
 *  'segments' — classic cell: outlined body with vertical segment bars.
 *  'ios'      — iOS / HyperOS style: rounded body with a smooth fill + nub.
 * `pct` is 0..1.
 */
function drawBattery(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  bw: number,
  bh: number,
  pct: number,
  fg: [number, number, number],
  style: 'segments' | 'ios',
) {
  const p = Math.max(0, Math.min(1, pct));
  // terminal nub
  ctx.fillStyle = withAlpha(fg, 0.85);
  ctx.fillRect(x + bw + 1, y + Math.floor(bh / 3), 2, Math.max(2, Math.floor(bh / 3)));

  if (style === 'ios') {
    ctx.strokeStyle = withAlpha(fg, 0.9);
    ctx.lineWidth = 1;
    rrPath(ctx, x + 0.5, y + 0.5, bw - 1, bh - 1, Math.max(2, Math.floor(bh / 4)));
    ctx.stroke();
    if (p > 0.004) {
      const iw = Math.max(2, Math.round((bw - 5) * p));
      ctx.fillStyle = withAlpha(fg, 0.95);
      rrPath(ctx, x + 2, y + 2, iw, bh - 4, 1);
      ctx.fill();
    }
    return;
  }

  // classic: body outline + vertical segment bars
  ctx.strokeStyle = withAlpha(fg, 0.85);
  ctx.lineWidth = 1;
  ctx.strokeRect(x + 0.5, y + 0.5, bw - 1, bh - 1);
  const segs = 5;
  const pad = 3;
  const gap = 1;
  const inner = bw - pad * 2;
  const sw = Math.max(1, Math.floor((inner - gap * (segs - 1)) / segs));
  const filled = Math.round(p * segs);
  for (let i = 0; i < segs; i++) {
    const sx = x + pad + i * (sw + gap);
    ctx.fillStyle = withAlpha(fg, i < filled ? 0.95 : 0.18);
    ctx.fillRect(sx, y + pad, sw, bh - pad * 2);
  }
}

/** Deterministic pseudo random per seed (stable across frames). */
function rand(seed: number): number {
  let x = (seed * 1103515245 + 12345) & 0x7fffffff;
  x ^= x << 13;
  x ^= x >>> 17;
  x ^= x << 5;
  return ((x >>> 0) % 10000) / 10000;
}

/**
 * Resolve a widget slot into a pixel anchor + text alignment.
 * Layouts are the lock-screen style grid: corners, edges, centre.
 */
function anchor(
  slot: SlotId | undefined,
  w: number,
  h: number,
  fallback: SlotId,
): { x: number; y: number; align: CanvasTextAlign; base: CanvasTextBaseline } {
  const m = 8;
  switch (slot ?? fallback) {
    case 'tl': return { x: m, y: m, align: 'left', base: 'top' };
    case 'tr': return { x: w - m, y: m, align: 'right', base: 'top' };
    case 'bl': return { x: m, y: h - m, align: 'left', base: 'bottom' };
    case 'br': return { x: w - m, y: h - m, align: 'right', base: 'bottom' };
    case 'bar': return { x: w / 2, y: h - m, align: 'center', base: 'bottom' };
    case 'l': return { x: m, y: h / 2, align: 'left', base: 'middle' };
    case 'r': return { x: w - m, y: h / 2, align: 'right', base: 'middle' };
    case 'c':
    default: return { x: w / 2, y: h / 2, align: 'center', base: 'middle' };
  }
}

/** Active layout of a template (undefined for fixed-composition templates). */
export function activeLayout(t: Template): LayoutDef | undefined {
  if (!t.layouts || t.layouts.length === 0) return undefined;
  return t.layouts.find((l) => l.id === t.layout) ?? t.layouts[0];
}

/** Build a widget set from a layout definition (clock first, then widgets). */
export function compsFromLayout(l: LayoutDef): Comp[] {
  const comps: Comp[] = [
    { kind: 'clock', fg: '#f2f1f0', seconds: false, size: l.clock.size, slot: l.clock.slot },
  ];
  for (const wd of l.widgets) {
    const c: Comp = { kind: wd.kind, slot: wd.slot, fg: wd.fg ?? '#c7cdd8' };
    if (wd.text) c.text = wd.text;
    if (wd.kind === 'battery') c.value = 0.82;
    if (wd.kind === 'ring') c.value = 0.6;
    if (wd.kind === 'meter') c.value = 0.6;
    comps.push(c);
  }
  return comps;
}

export function renderTemplate(t: Template, w: number, h: number, frame: number): Uint8ClampedArray {
  // OffscreenCanvas is unavailable on ancient webkit; fall back to a DOM canvas.
  let ctx: CanvasRenderingContext2D
  if (typeof OffscreenCanvas === 'function') {
    ctx = new OffscreenCanvas(Math.max(1, w), Math.max(1, h)).getContext('2d') as CanvasRenderingContext2D;
  } else {
    const cv = document.createElement('canvas');
    cv.width = Math.max(1, w);
    cv.height = Math.max(1, h);
    ctx = cv.getContext('2d') as CanvasRenderingContext2D;
  }
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';

  ctx.fillStyle = t.bg;
  ctx.fillRect(0, 0, w, h);

  const clockIdx = t.comps.findIndex((c) => c.kind === 'clock');
  for (let i = 0; i < t.comps.length; i++) {
    const c = t.comps[i];
    const fg = hex(c.fg ?? '#ffffff');
    switch (c.kind) {
      // ------------------------------------------------- clock
      case 'clock': {
        const secs = c.seconds ?? false;
        const d = new Date(Date.now() + frame * t.dt * 1000);
        const blink = secs && frame % 2 === 1;
        const text = `${String(d.getHours()).padStart(2, '0')}${blink ? ' ' : ':'}${String(d.getMinutes()).padStart(2, '0')}${secs ? (blink ? ' ' : ':') + String(d.getSeconds()).padStart(2, '0') : ''}`;
        const big = !secs && (clockIdx === i);
        const lay = activeLayout(t);
        const defSize = lay ? lay.clock.size : (big ? 0.52 : 0.30);
        const defSlot: SlotId = lay ? lay.clock.slot : 'c';
        const size = Math.floor(h * (c.size ?? defSize));
        const a = anchor(c.slot ?? defSlot, w, h, 'c');
        if (!lay && big) a.y = h * 0.62;
        ctx.font = `bold ${size}px ${MONO}`;
        ctx.textAlign = a.align;
        ctx.textBaseline = a.base;
        ctx.fillStyle = withAlpha(fg, 1);
        ctx.fillText(text, a.x, a.y);
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        break;
      }
      // ------------------------------------------------- date
      case 'date': {
        const d = new Date(Date.now() + frame * t.dt * 1000);
        const text = d.toLocaleDateString('en-GB', { weekday: 'short', day: '2-digit', month: 'short' });
        const a = anchor(c.slot, w, h, 'bar');
        ctx.font = `${Math.floor(h * 0.115)}px ${UI}`;
        ctx.textAlign = a.align;
        ctx.textBaseline = a.base;
        ctx.fillStyle = withAlpha(fg, 0.95);
        ctx.fillText(text, a.x, a.y);
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        break;
      }
      // ------------------------------------------------- ticker
      case 'ticker': {
        const txt = c.text ?? 'SharkDeck';
        const spd = c.speed ?? 1;
        const size = Math.floor(h * 0.15);
        const a = anchor(c.slot, w, h, 'bar');
        ctx.font = `500 ${size}px ${UI}`;
        ctx.textAlign = 'left';
        ctx.textBaseline = a.base;
        ctx.fillStyle = withAlpha(fg, 0.95);
        const tw = ctx.measureText(txt).width;
        const cycle = w + tw + 16;
        const startX = a.align === 'right' ? w : a.align === 'center' ? w / 2 : 0;
        let x = startX - ((frame * spd * 2) % cycle);
        for (let k = 0; x < w; k++) {
          ctx.fillText(txt, x, a.y);
          x = x + cycle;
          if (k > 4) break;
        }
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        break;
      }
      // ------------------------------------------------- wave
      case 'wave': {
        const amp = c.amp ?? 8;
        const wlen = Math.max(8, c.wlen ?? 40);
        const spd = c.speed ?? 1;
        ctx.beginPath();
        ctx.moveTo(0, h);
        for (let x = 0; x <= w; x++) {
          const y = h * 0.80 - amp * Math.sin((x / wlen) + frame * 0.35 * spd);
          ctx.lineTo(x, y);
        }
        ctx.lineTo(w, h);
        ctx.closePath();
        ctx.fillStyle = withAlpha(fg, 0.85);
        ctx.fill();
        break;
      }
      // ------------------------------------------------- rain
      case 'rain': {
        const spd = c.speed ?? 6;
        const drops = Math.max(10, Math.floor(w / 9));
        ctx.fillStyle = withAlpha(fg, 0.8);
        for (let i = 0; i < drops; i++) {
          const x = Math.floor(rand(i + 1) * w);
          const len = 4 + Math.floor(rand(i + 41) * 10);
          const y = ((rand(i + 97) * (h + 40)) + frame * spd) % (h + 40) - 20;
          ctx.fillRect(x, y, 2, len);
        }
        break;
      }
      // ------------------------------------------------- border
      case 'border': {
        ctx.strokeStyle = withAlpha(fg, 0.9);
        ctx.lineWidth = 2;
        ctx.strokeRect(2, 2, w - 4, h - 4);
        break;
      }
      // ------------------------------------------------- eq
      case 'eq': {
        const bars = Math.max(4, c.bars ?? 16);
        const gap = 3;
        const bw = Math.floor((w - gap * (bars + 1)) / bars);
        const base = h * 0.94;
        const maxH = h * 0.45;
        ctx.fillStyle = withAlpha(fg, 0.95);
        for (let b = 0; b < bars; b++) {
          const v = Math.abs(Math.sin(b * 1.31 + frame * 0.42));
          const bh = Math.max(4, maxH * (0.15 + 0.85 * v));
          let x = gap + b * (bw + gap);
          if (b === bars - 1) x = Math.min(x, w - bw - gap);
          ctx.fillRect(x, base - bh, bw, bh);
        }
        break;
      }
      // ------------------------------------------------- meter
      case 'meter': {
        const v = Math.max(0, Math.min(1, c.value ?? 0.6));
        const label = c.text ?? 'Meter';
        const barY = h * 0.80;
        const barH = h * 0.10;
        const inset = Math.floor(w * 0.08);
        ctx.font = `${Math.floor(h * 0.10)}px ${UI}`;
        ctx.textAlign = 'left';
        ctx.fillStyle = withAlpha(fg, 0.9);
        ctx.fillText(label, inset, barY - barH - 6);
        ctx.strokeStyle = withAlpha(fg, 0.35);
        ctx.lineWidth = 1;
        ctx.strokeRect(inset, barY, w - 2 * inset, barH);
        ctx.fillStyle = withAlpha(fg, 0.95);
        ctx.fillRect(inset + 2, barY + 2, (w - 2 * inset - 4) * v, barH - 4);
        ctx.textAlign = 'center';
        break;
      }
      // ------------------------------------------------- scanlines
      case 'scanlines': {
        ctx.fillStyle = withAlpha([0, 0, 0], 0.32);
        for (let y = 0; y < h; y += 3) ctx.fillRect(0, y, w, 1);
        break;
      }
      // ------------------------------------------------- brand fin
      case 'fin': {
        const s = Math.min(w, h) * 0.52;
        const cx = w / 2 - s * 0.05;
        const cy = h * 0.46;
        const p = (x: number, y: number) => [cx + (x - 11.9) * (s / 12), cy + (y - 12) * (s / 12)];
        ctx.beginPath();
        for (let k = 0; k <= 48; k++) {
          const tt = k / 48;
          const x = (1 - tt) ** 2 * 12.6 + 2 * (1 - tt) * tt * (6.0) + tt ** 2 * 7.6;
          const y = (1 - tt) ** 2 * 2.4 + 2 * (1 - tt) * tt * (11.0) + tt ** 2 * 21.6;
          const [px, py] = p(x, y);
          k === 0 ? ctx.moveTo(px, py) : ctx.lineTo(px, py);
        }
        for (let k = 48; k >= 0; k--) {
          const tt = k / 48;
          const x = (1 - tt) ** 2 * 12.6 + 2 * (1 - tt) * tt * (14.2) + tt ** 2 * 15.8;
          const y = (1 - tt) ** 2 * 2.4 + 2 * (1 - tt) * tt * (10.2) + tt ** 2 * 21.6;
          const [px, py] = p(x, y);
          ctx.lineTo(px, py);
        }
        ctx.closePath();
        ctx.fillStyle = withAlpha(fg, 0.92);
        ctx.fill();
        break;
      }
      // ------------------------------------------------- analog watch face
      case 'watch': {
        const d = new Date(Date.now() + frame * t.dt * 1000);
        const cx = w / 2;
        const cy = h / 2;
        const R = Math.min(w, h) * 0.42;
        const accent = hex(c.fg ?? '#f2f1f0');
        // bezel
        ctx.strokeStyle = withAlpha(accent, 0.30);
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.arc(cx, cy, R, 0, Math.PI * 2);
        ctx.stroke();
        // minute ticks + hour markers
        for (let i = 0; i < 60; i++) {
          const a = (i / 60) * Math.PI * 2;
          const major = i % 5 === 0;
          const len = major ? R * 0.13 : R * 0.06;
          const wid = major ? 2 : 1;
          ctx.save();
          ctx.translate(cx, cy);
          ctx.rotate(a - Math.PI / 2);
          ctx.fillStyle = withAlpha(accent, major ? 0.95 : 0.42);
          ctx.fillRect(-wid / 2, -R, wid, len);
          ctx.restore();
        }
        // hands
        const ms = d.getMilliseconds() / 1000;
        const sec = d.getSeconds() + ms;
        const min = d.getMinutes() + sec / 60;
        const hr = (d.getHours() % 12) + min / 60;
        const hand = (turns: number, len: number, wid: number, col: [number, number, number]) => {
          ctx.save();
          ctx.translate(cx, cy);
          ctx.rotate(turns * Math.PI * 2 - Math.PI / 2);
          ctx.fillStyle = withAlpha(col, 1);
          ctx.fillRect(-wid / 2, -len, wid, len);
          ctx.restore();
        };
        hand(hr / 12, R * 0.48, 4, accent);
        hand(min / 60, R * 0.72, 3, accent);
        hand(sec / 60, R * 0.80, 1, hex('#e95420'));
        // cap
        ctx.fillStyle = withAlpha(accent, 1);
        ctx.beginPath();
        ctx.arc(cx, cy, 2.5, 0, Math.PI * 2);
        ctx.fill();
        break;
      }
      // ------------------------------------------------- date window
      case 'datewin': {
        const d = new Date(Date.now() + frame * t.dt * 1000);
        const R = Math.min(w, h) * 0.42;
        const bw = Math.max(15, Math.round(R * 0.44));
        const bh = Math.max(11, Math.round(R * 0.30));
        const a = anchor(c.slot, w, h, 'tr');
        const cx = c.slot
          ? (a.align === 'right' ? a.x - bw / 2 : a.align === 'center' ? a.x : a.x + bw / 2)
          : w / 2 + R * 0.56;
        const cy = c.slot
          ? (a.base === 'top' ? a.y + bh / 2 : a.base === 'bottom' ? a.y - bh / 2 : a.y)
          : h / 2;
        ctx.fillStyle = withAlpha([0, 0, 0], 0.55);
        ctx.fillRect(cx - bw / 2, cy - bh / 2, bw, bh);
        ctx.strokeStyle = withAlpha(fg, 0.35);
        ctx.lineWidth = 1;
        ctx.strokeRect(cx - bw / 2 + 0.5, cy - bh / 2 + 0.5, bw - 1, bh - 1);
        ctx.font = `bold ${Math.floor(bh * 0.78)}px ${UI}`;
        ctx.fillStyle = withAlpha(fg, 1);
        ctx.fillText(String(d.getDate()), cx, cy);
        break;
      }
      // ------------------------------------------------- battery gauge
      case 'battery': {
        const bw = Math.max(20, Math.floor(w * 0.14));
        const bh = Math.max(11, Math.floor(h * 0.12));
        const a = anchor(c.slot, w, h, 'tl');
        const x = a.align === 'right' ? a.x - bw : a.align === 'center' ? a.x - bw / 2 : a.x;
        const y = a.base === 'bottom' ? a.y - bh : a.base === 'middle' ? a.y - bh / 2 : a.y;
        const pct = Math.max(0, Math.min(1, c.value ?? 0.82));
        drawBattery(ctx, x, y, bw, bh, pct, fg, c.batteryStyle ?? 'segments');
        ctx.font = `${Math.floor(h * 0.1)}px ${MONO}`;
        ctx.textAlign = 'left';
        ctx.fillStyle = withAlpha(fg, 0.9);
        ctx.fillText(`${Math.round(pct * 100)}%`, x + bw + 8, y + bh - 1);
        ctx.textAlign = 'center';
        break;
      }
      // ------------------------------------------------- progress ring
      case 'ring': {
        const v = Math.max(0, Math.min(1, c.value ?? 0.6));
        const R = Math.min(w, h) * 0.20;
        const a = anchor(c.slot, w, h, 'tr');
        const cx = a.align === 'right' ? a.x - R : a.align === 'center' ? a.x : a.x + R;
        const cy = a.base === 'bottom' ? a.y - R : a.base === 'middle' ? a.y : a.y + R;
        ctx.strokeStyle = withAlpha(fg, 0.22);
        ctx.lineWidth = 4;
        ctx.beginPath();
        ctx.arc(cx, cy, R, 0, Math.PI * 2);
        ctx.stroke();
        ctx.strokeStyle = withAlpha(fg, 0.95);
        ctx.beginPath();
        ctx.arc(cx, cy, R, -Math.PI / 2, -Math.PI / 2 + v * Math.PI * 2);
        ctx.stroke();
        break;
      }
      // ------------------------------------------------- step counter
      case 'steps': {
        const a = anchor(c.slot, w, h, 'bl');
        ctx.font = `${Math.floor(h * 0.13)}px ${MONO}`;
        ctx.textAlign = a.align;
        ctx.textBaseline = a.base;
        ctx.fillStyle = withAlpha(fg, 0.9);
        ctx.fillText(c.text ?? '8.4k', a.x, a.y);
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        break;
      }
      // ------------------------------------------------- host battery
      case 'hostbatt': {
        const info = live.info;
        const pct = info?.battery ?? null;
        const bw = Math.max(20, Math.floor(w * 0.15));
        const bh = Math.max(11, Math.floor(h * 0.12));
        const a = anchor(c.slot, w, h, 'tl');
        const x = a.align === 'right' ? a.x - bw : a.align === 'center' ? a.x - bw / 2 : a.x;
        const y = a.base === 'bottom' ? a.y - bh : a.base === 'middle' ? a.y - bh / 2 : a.y;
        if (pct !== null) {
          drawBattery(ctx, x, y, bw, bh, pct / 100, fg, c.batteryStyle ?? 'segments');
          ctx.font = `${Math.floor(h * 0.1)}px ${MONO}`;
          ctx.textAlign = 'left';
          ctx.fillStyle = withAlpha(fg, 0.95);
          ctx.fillText(`${pct}%${info?.charging ? '+' : ''}`, x + bw + 8, y + bh - 1);
          ctx.textAlign = 'center';
        } else {
          // no host battery: outline only, honest
          ctx.strokeStyle = withAlpha(fg, 0.4);
          ctx.lineWidth = 1;
          ctx.strokeRect(x + 0.5, y + 0.5, bw - 1, bh - 1);
          ctx.fillStyle = withAlpha(fg, 0.4);
          ctx.fillRect(x + bw + 1, y + Math.floor(bh / 3), 2, Math.max(2, Math.floor(bh / 3)));
          ctx.font = `${Math.floor(h * 0.09)}px ${MONO}`;
          ctx.textAlign = 'left';
          ctx.fillText('n/a', x + bw + 8, y + bh - 1);
          ctx.textAlign = 'center';
        }
        break;
      }
      // ------------------------------------------------- cpu load
      case 'cpu': {
        const pct = live.info?.cpu ?? 0;
        const bw = Math.max(26, Math.floor(w * 0.17));
        const bh = Math.max(7, Math.floor(h * 0.08));
        const a = anchor(c.slot, w, h, 'tl');
        const x = a.align === 'right' ? a.x - bw : a.align === 'center' ? a.x - bw / 2 : a.x;
        const y = a.base === 'bottom' ? a.y - bh : a.base === 'middle' ? a.y - bh / 2 : a.y;
        ctx.fillStyle = withAlpha(fg, 0.18);
        ctx.fillRect(x, y, bw, bh);
        ctx.fillStyle = withAlpha(fg, 0.95);
        ctx.fillRect(x, y, Math.max(1, (bw * pct) / 100), bh);
        ctx.font = `${Math.floor(h * 0.1)}px ${MONO}`;
        ctx.textAlign = 'left';
        ctx.fillStyle = withAlpha(fg, 0.9);
        ctx.fillText(`CPU ${pct}%`, x, y + bh + Math.floor(h * 0.12));
        ctx.textAlign = 'center';
        break;
      }
      // ------------------------------------------------- ram load
      case 'ram': {
        const pct = live.info?.ram ?? 0;
        const bw = Math.max(26, Math.floor(w * 0.17));
        const bh = Math.max(7, Math.floor(h * 0.08));
        const a = anchor(c.slot, w, h, 'tl');
        const x = a.align === 'right' ? a.x - bw : a.align === 'center' ? a.x - bw / 2 : a.x;
        const y = a.base === 'bottom' ? a.y - bh : a.base === 'middle' ? a.y - bh / 2 : a.y;
        ctx.fillStyle = withAlpha(fg, 0.18);
        ctx.fillRect(x, y, bw, bh);
        ctx.fillStyle = withAlpha(fg, 0.95);
        ctx.fillRect(x, y, Math.max(1, (bw * pct) / 100), bh);
        ctx.font = `${Math.floor(h * 0.1)}px ${MONO}`;
        ctx.textAlign = 'left';
        ctx.fillStyle = withAlpha(fg, 0.9);
        ctx.fillText(`RAM ${pct}%`, x, y + bh + Math.floor(h * 0.12));
        ctx.textAlign = 'center';
        break;
      }
      // ------------------------------------------------- keyboard state
      case 'devinfo': {
        const info = live.info;
        const label = info && info.firmware ? `K86 fw ${info.firmware} · P${info.profile + 1}` : 'K86';
        const a = anchor(c.slot, w, h, 'bl');
        ctx.font = `${Math.floor(h * 0.1)}px ${MONO}`;
        ctx.textAlign = a.align;
        ctx.textBaseline = a.base;
        ctx.fillStyle = withAlpha(fg, 0.9);
        ctx.fillText(label, a.x, a.y);
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        break;
      }
      // ------------------------------------------------- host uptime
      case 'uptime': {
        const s = live.info?.uptime ?? 0;
        const hh = Math.floor(s / 3600);
        const mm = Math.floor((s % 3600) / 60);
        const a = anchor(c.slot, w, h, 'bl');
        ctx.font = `${Math.floor(h * 0.1)}px ${MONO}`;
        ctx.textAlign = a.align;
        ctx.textBaseline = a.base;
        ctx.fillStyle = withAlpha(fg, 0.9);
        ctx.fillText(`UP ${hh}h ${String(mm).padStart(2, '0')}m`, a.x, a.y);
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        break;
      }
    }
  }
  return ctx.getImageData(0, 0, w, h).data;
}

/* ------------------------------------------------------------------ */
/* Encode helpers                                                      */
/* ------------------------------------------------------------------ */

/** RGBA bytes (ImageData.data) → base64 (matches backend screen_push input). */
export function rgbaToB64(data: Uint8ClampedArray): string {
  let s = '';
  const CH = 8192;
  for (let i = 0; i < data.length; i += CH) {
    s += String.fromCharCode(...Array.from(data.subarray(i, i + CH)));
  }
  return btoa(s);
}

/** Render an animation: [{rgbaB64, delayByte}] with device delay units. */
export function renderFrames(
  t: Template,
  w: number,
  h: number,
  count: number,
): { rgba_b64: string; delay: number }[] {
  const out = [];
  for (let i = 0; i < count; i++) {
    const data = renderTemplate(t, w, h, i);
    out.push({
      rgba_b64: rgbaToB64(data),
      delay: Math.max(1, Math.min(255, Math.round(t.dt * 100))),
    });
  }
  return out;
}

/** Render a single still (frame 0) for screenPush. */
export function renderStill(t: Template, w: number, h: number): string {
  return rgbaToB64(renderTemplate(t, w, h, 0));
}
