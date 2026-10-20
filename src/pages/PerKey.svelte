<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import * as api from '../lib/perkey';
  import { toast, errText, connected } from '../lib/store.svelte';
  import type { PerKeyCell, PerKeyGrid, PerKeyPushResult } from '../lib/perkey';

  type Rgb = [number, number, number];

  // Preset brush colours. The firmware keeps a small palette; "Snap to
  // palette" pulls every painted key onto one of these.
  const PALETTE: Rgb[] = [
    [255, 0, 0],
    [255, 96, 0],
    [255, 200, 0],
    [140, 255, 0],
    [0, 200, 120],
    [0, 160, 255],
    [60, 90, 255],
    [140, 80, 255],
    [230, 80, 200],
    [255, 255, 255],
    [150, 160, 180],
    [0, 0, 0],
  ];

  let grid = $state<PerKeyGrid | null>(null);
  let cells = $state<Rgb[]>([]);
  let brush = $state<Rgb>([255, 0, 0]);
  let custom = $state('#ff0000');
  let dryRun = $state(true);
  let busy = $state(false);
  let confirmOpen = $state(false);
  let last = $state<PerKeyPushResult | null>(null);
  let countdown = $state(0);
  let painting = $state(false);
  let erasing = $state(false);

  const boardCells: PerKeyCell[] = $derived(grid?.cells ?? []);
  const canvasW = $derived(grid?.canvas.width ?? 840);
  const canvasH = $derived(grid?.canvas.height ?? 280);
  const uploads = $derived(grid?.uploads ?? null);
  const remaining = $derived(last?.remaining ?? uploads?.remaining ?? 0);
  const maxUploads = $derived(uploads?.max ?? 2);

  let timer: ReturnType<typeof setInterval> | undefined;

  onMount(async () => {
    window.addEventListener('mouseup', endPaint);
    timer = setInterval(() => {
      if (countdown > 0) countdown -= 1;
    }, 1000);
    try {
      grid = await api.perkeyGrid();
      cells = Array.from({ length: grid.width * grid.height }, () => [0, 0, 0] as Rgb);
    } catch (e) {
      toast('err', errText(e));
    }
  });

  onDestroy(() => {
    window.removeEventListener('mouseup', endPaint);
    if (timer) clearInterval(timer);
  });

  function hexToRgb(hex: string): Rgb {
    const h = hex.replace('#', '');
    return [
      parseInt(h.slice(0, 2), 16) || 0,
      parseInt(h.slice(2, 4), 16) || 0,
      parseInt(h.slice(4, 6), 16) || 0,
    ];
  }

  function rgbCss(c: Rgb): string {
    return `rgb(${c[0]},${c[1]},${c[2]})`;
  }

  function hexCss(c: Rgb): string {
    return '#' + c.map((x) => x.toString(16).padStart(2, '0')).join('');
  }

  /** Ink colour that stays readable on the painted key. */
  function ink(c: Rgb): string {
    const l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    return l > 140 ? 'rgba(0,0,0,0.75)' : 'rgba(255,255,255,0.9)';
  }

  function cellRgb(idx: number): Rgb {
    return cells[idx] ?? [0, 0, 0];
  }

  function paint(idx: number, colour: Rgb) {
    if (idx < 0 || idx >= cells.length) return;
    cells[idx] = [colour[0], colour[1], colour[2]];
  }

  function startPaint(e: MouseEvent, idx: number) {
    painting = true;
    erasing = e.button === 2 || e.shiftKey;
    paint(idx, erasing ? [0, 0, 0] : brush);
  }

  function movePaint(idx: number) {
    if (!painting) return;
    paint(idx, erasing ? [0, 0, 0] : brush);
  }

  function endPaint() {
    painting = false;
  }

  function clearAll() {
    cells = cells.map(() => [0, 0, 0] as Rgb);
  }

  function fillAll() {
    cells = cells.map(() => [brush[0], brush[1], brush[2]] as Rgb);
  }

  /** Mirrors `perkey::apply_palette` in the core crate. */
  function snapToPalette() {
    cells = cells.map((c) => {
      let best = PALETTE[0];
      let bestD = Number.MAX_SAFE_INTEGER;
      for (const p of PALETTE) {
        const dr = p[0] - c[0];
        const dg = p[1] - c[1];
        const db = p[2] - c[2];
        const d = 2 * dr * dr + 4 * dg * dg + 3 * db * db;
        if (d < bestD) {
          bestD = d;
          best = p;
        }
      }
      return [best[0], best[1], best[2]] as Rgb;
    });
  }

  async function push() {
    if (!grid) return;
    if (!dryRun) {
      confirmOpen = true;
      return;
    }
    await runPush(true);
  }

  async function runPush(dry: boolean) {
    if (!grid) return;
    busy = true;
    try {
      last = await api.perkeyPush(cells, dry);
      countdown = Math.ceil(last.next_allowed_in_s);
      if (dry) {
        toast('ok', `Preview only: ${last.bytes} bytes in ${last.chunks} chunks — nothing written`);
      } else {
        toast('ok', `Uploaded ${last.bytes} bytes (${last.chunks} chunks) to flash`);
      }
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
      confirmOpen = false;
    }
  }

  function pctX(v: number): string {
    return `${(v / canvasW) * 100}%`;
  }

  function pctTop(v: number): string {
    return `${(Math.max(0, v) / canvasH) * 100}%`;
  }

  function pctH(y: number, h: number): string {
    const visible = y < 0 ? h + y : h;
    return `${(Math.max(1, visible) / canvasH) * 100}%`;
  }
</script>

<div class="page">
  <div class="page-head">
    <h2>Per-key RGB</h2>
    <p>
      Paint a colour for every key of the K86 and push the map to the keyboard's flash with
      <span class="mono">SET_USERPIC</span> (<span class="mono">0x0C</span>).
    </p>
  </div>

  <div class="panel warn">
    <div class="warn-head">
      <span class="pill warn">flash write</span>
      <strong>Uploads are rate limited and can leave the keyboard unresponsive.</strong>
    </div>
    <ul class="warn-list">
      <li>
        The firmware accepts at most <strong>2 uploads per 10 seconds</strong>. Further uploads are
        refused until the window frees up.
      </li>
      <li>
        A rejected or interrupted upload can stall the endpoint: the keyboard then stops answering
        until it is <strong>unplugged and replugged</strong>.
      </li>
      <li>
        The exact 0x0C payload layout is <strong>best effort and unverified</strong> against the
        vendor app. Keep <em>Dry run</em> on while designing and only write when you can replug the
        keyboard if it goes quiet.
      </li>
    </ul>
  </div>

  <div class="cols">
    <div class="panel">
      <h3>
        Keyboard
        <span class="muted count">
          {grid ? `${grid.keyCount} keys · ${grid.width}×${grid.height} grid` : 'loading…'}
        </span>
      </h3>

      <div
        class="board"
        role="presentation"
        style="aspect-ratio: {canvasW} / {canvasH}"
        oncontextmenu={(e) => e.preventDefault()}
      >
        {#each boardCells as c (c.slot)}
          <button
            class="key"
            style="left:{pctX(c.x)};top:{pctTop(c.y)};width:{pctX(c.w)};height:{pctH(c.y, c.h)};background:{rgbCss(cellRgb(c.cell))};color:{ink(cellRgb(c.cell))}"
            title="{c.code} · slot {c.slot}"
            onmousedown={(e) => startPaint(e, c.cell)}
            onmouseenter={() => movePaint(c.cell)}
          >
            <span>{c.text}</span>
          </button>
        {/each}
      </div>

      <p class="hint" style="margin-top:10px">
        Click or drag to paint with the brush colour. Right-click (or shift-click) erases the key.
      </p>
    </div>

    <div class="grid" style="gap:16px">
      <div class="panel">
        <h3>Brush</h3>
        <div class="swatches">
          {#each PALETTE as p (hexCss(p))}
            <button
              class="swatch"
              class:sel={hexCss(p) === hexCss(brush)}
              style="background:{rgbCss(p)}"
              title={hexCss(p)}
              onclick={() => (brush = [p[0], p[1], p[2]])}
            ></button>
          {/each}
        </div>
        <div class="row" style="margin-top:12px">
          <input
            type="color"
            bind:value={custom}
            oninput={() => (brush = hexToRgb(custom))}
            aria-label="Custom brush colour"
          />
          <span class="mono">{hexCss(brush)}</span>
        </div>
        <div class="row" style="margin-top:12px; flex-wrap:wrap">
          <button class="btn sm" onclick={clearAll}>Clear all</button>
          <button class="btn sm" onclick={fillAll}>Fill all</button>
          <button class="btn sm" onclick={snapToPalette}>Snap to palette</button>
        </div>
      </div>

      <div class="panel">
        <h3>Upload</h3>

        <label class="toggle">
          <input type="checkbox" bind:checked={dryRun} />
          <span>Dry run (preview only)</span>
        </label>
        <p class="hint" style="margin-top:8px">
          {dryRun
            ? 'Dry run encodes the payload and reports its size — nothing is written to the keyboard.'
            : 'Live mode writes the image to the keyboard flash (0x0C).'}
        </p>

        <div class="kv" style="margin-top:12px">
          <span class="k">Uploads remaining</span>
          <span class="v">{remaining} / {maxUploads}</span>
        </div>
        <div class="kv">
          <span class="k">Next upload allowed in</span>
          <span class="v">{countdown > 0 ? `${countdown} s` : 'now'}</span>
        </div>
        {#if last}
          <div class="kv">
            <span class="k">Last payload</span>
            <span class="v">
              {last.bytes} B · {last.chunks} chunks · {last.sent ? 'written' : 'preview only'}
            </span>
          </div>
        {/if}

        {#if !connected()}
          <p class="hint" style="margin-top:10px">
            No device connected — plug the keyboard in (USB cable) before uploading.
          </p>
        {/if}

        <div class="row" style="margin-top:14px; flex-wrap:wrap">
          <button class="btn primary" onclick={push} disabled={busy || !grid}>
            {#if busy}<span class="spinner"></span>{/if}
            {dryRun ? 'Preview payload' : 'Upload to keyboard flash'}
          </button>
        </div>

        {#if confirmOpen}
          <div class="confirm">
            <p>
              This writes the current per-key map to the keyboard's flash with
              <span class="mono">SET_USERPIC</span> (<span class="mono">0x0C</span>).
            </p>
            <p class="hint">
              At most 2 uploads per 10 s. If the keyboard stops answering, unplug and replug it.
            </p>
            <div class="row" style="margin-top:10px; flex-wrap:wrap">
              <button class="btn sm" onclick={() => (confirmOpen = false)} disabled={busy}>Cancel</button>
              <button class="btn danger" onclick={() => runPush(false)} disabled={busy}>
                {#if busy}<span class="spinner"></span>{/if}
                Yes, write to flash
              </button>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .cols {
    display: grid;
    grid-template-columns: minmax(420px, 1.4fr) minmax(300px, 1fr);
    gap: 16px;
    align-items: start;
    margin-top: 16px;
  }
  @media (max-width: 1000px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
  .warn {
    border-color: rgba(195, 85, 3, 0.35);
    background: rgba(195, 85, 3, 0.06);
  }
  .warn-head {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }
  .warn-list {
    margin: 10px 0 0;
    padding-left: 18px;
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--text-dim);
  }
  .warn-list li {
    margin-bottom: 3px;
  }
  .board {
    position: relative;
    width: 100%;
    user-select: none;
    touch-action: none;
  }
  .key {
    position: absolute;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid rgba(0, 0, 0, 0.28);
    border-radius: 4px;
    padding: 0;
    font-size: 9px;
    font-weight: 600;
    line-height: 1;
    cursor: crosshair;
    overflow: hidden;
  }
  .key span {
    pointer-events: none;
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .swatch {
    width: 26px;
    height: 26px;
    border-radius: 7px;
    border: 1px solid var(--border-strong);
    cursor: pointer;
    padding: 0;
  }
  .swatch.sel {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 13px;
    cursor: pointer;
  }
  .confirm {
    margin-top: 14px;
    padding: 12px;
    border: 1px solid rgba(215, 28, 45, 0.35);
    background: rgba(215, 28, 45, 0.06);
    border-radius: var(--radius-sm);
    font-size: 13px;
    line-height: 1.5;
  }
  .count {
    font-size: 11px;
    font-weight: 500;
    text-transform: none;
    letter-spacing: 0;
  }
</style>
