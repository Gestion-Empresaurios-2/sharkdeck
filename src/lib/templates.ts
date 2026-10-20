// SharkDeck screen-template engine: deterministic pixel renderers for the
// K86 panel. Every draw is a pure function of (template, components, size,
// frame index, timestamp) so previews and pushed frames match exactly.

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
  | 'meter';

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
}

export interface Template {
  id: string;
  name: string;
  desc: string;
  bg: string;
  comps: Comp[];
  /** seconds per pushed frame (drives device delay byte) */
  dt: number;
}

/* ------------------------------------------------------------------ */
/* Built-in gallery                                                    */
/* ------------------------------------------------------------------ */

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

/** Deterministic pseudo random per seed (stable across frames). */
function rand(seed: number): number {
  let x = (seed * 1103515245 + 12345) & 0x7fffffff;
  x ^= x << 13;
  x ^= x >>> 17;
  x ^= x << 5;
  return ((x >>> 0) % 10000) / 10000;
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
        const size = Math.floor(h * (c.size ?? (big ? 0.52 : 0.30)));
        ctx.font = `bold ${size}px ${MONO}`;
        ctx.fillStyle = withAlpha(fg, 1);
        ctx.fillText(text, w / 2, h * (big ? 0.62 : 0.34));
        break;
      }
      // ------------------------------------------------- date
      case 'date': {
        const d = new Date(Date.now() + frame * t.dt * 1000);
        const text = d.toLocaleDateString('en-GB', { weekday: 'short', day: '2-digit', month: 'short' });
        ctx.font = `${Math.floor(h * 0.115)}px ${UI}`;
        ctx.fillStyle = withAlpha(fg, 0.95);
        ctx.fillText(text, w / 2, h * 0.88);
        break;
      }
      // ------------------------------------------------- ticker
      case 'ticker': {
        const txt = c.text ?? 'SharkDeck';
        const spd = c.speed ?? 1;
        const size = Math.floor(h * 0.15);
        ctx.font = `500 ${size}px ${UI}`;
        ctx.textAlign = 'left';
        ctx.fillStyle = withAlpha(fg, 0.95);
        const tw = ctx.measureText(txt).width;
        const cycle = w + tw + 16;
        let x = w - ((frame * spd * 2) % cycle);
        for (let k = 0; x < w; k++) {
          ctx.fillText(txt, x, h * 0.87);
          x = x + cycle;
          if (k > 4) break;
        }
        ctx.textAlign = 'center';
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
