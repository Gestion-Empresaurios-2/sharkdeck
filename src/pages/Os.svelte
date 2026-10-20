<script lang="ts">
  import { onMount } from 'svelte';
  import {
    osState,
    OS_PAGES,
    ACCENTS,
    WALLS,
    WIDGETS,
    renderOsPage,
    osFrames,
    b64,
    tickOs,
    osHost,
    resetOs,
    type OsPage,
    type LockLayout,
  } from '../lib/os.svelte';
  import { store, toast, errText } from '../lib/store.svelte';
  import { startLive, stopLive, live } from '../lib/live';
  import * as api from '../lib/tauri';
  import type { AppConfig } from '../lib/types';

  let cfg: AppConfig | null = $state(null);
  let slot = $state(0);
  let busy = $state(false);
  let canvas: HTMLCanvasElement | undefined = $state();
  let dpr = 1;

  const W = $derived(cfg?.screen_width ?? 240);
  const H = $derived(cfg?.screen_height ?? 135);
  const page = $derived(osState.page);
  const accent = $derived(ACCENTS.find((a) => a.id === osState.settings.accent) ?? ACCENTS[0]);
  const wall = $derived(WALLS.find((w) => w.id === osState.settings.wall) ?? WALLS[0]);
  const lay = $derived(osState.settings.layout);

  onMount(async () => {
    cfg = await api.configGet().catch(() => null);
    startLive();
    const t = setInterval(() => {
      tickOs();
      paint();
    }, 1000);
    return () => {
      clearInterval(t);
      stopLive();
    };
  });

  function paint() {
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    const data = renderOsPage(osState.page, W, H);
    const img = ctx.createImageData(W, H);
    img.data.set(data);
    ctx.putImageData(img, 0, 0);
  }

  $effect(() => {
    // re-render whenever the OS state or panel size changes
    osState.page;
    osState.settings.accent;
    osState.settings.wall;
    osState.settings.layout;
    W;
    H;
    paint();
  });

  $effect(() => {
    const info = liveInfo();
    if (info) {
      osHost.battery = info.battery;
      osHost.charging = info.charging;
      if (typeof info.cpu === 'number') osState.metrics.cpu = Math.round(info.cpu);
      if (typeof info.ram === 'number') osState.metrics.ram = Math.round(info.ram);
    }
  });

  function liveInfo() {
    return live.info;
  }

  function cycle(dir: number) {
    const i = OS_PAGES.findIndex((p) => p.id === osState.page);
    const n = (i + dir + OS_PAGES.length) % OS_PAGES.length;
    osState.page = OS_PAGES[n].id;
  }

  async function push() {
    if (!store.device) return toast('err', 'No device connected');
    busy = true;
    try {
      const frames = osFrames(osState.page, W, H, 1);
      await api.screenAnimPush(frames, W, H, cfg?.order);
      toast('ok', `${OS_PAGES.find((p) => p.id === osState.page)?.label} pushed to the keyboard`);
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  async function pushAnimation() {
    if (!store.device) return toast('err', 'No device connected');
    busy = true;
    try {
      const frames = osFrames(osState.page, W, H, 12);
      await api.screenAnimPush(frames, W, H, cfg?.order);
      toast('ok', 'Animated OS page pushed (12 frames)');
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }
</script>

<div class="os-studio">
  <!-- left rail: the OS apps -->
  <nav class="os-rail">
    <div class="rail-head">Screen OS</div>
    {#each OS_PAGES as p (p.id)}
      <button class="rail-item" class:sel={p.id === osState.page} onclick={() => (osState.page = p.id)}>
        <span class="rail-dot" style="background:{p.id === osState.page ? accent.color : 'transparent'}"></span>
        <span class="rail-label">{p.label}</span>
        <span class="rail-hint">{p.hint}</span>
      </button>
    {/each}
  </nav>

  <!-- centre: the device -->
  <section class="os-stage">
    <header class="stage-head">
      <div>
        <div class="stage-eyebrow">LIVE PREVIEW · NATIVE {W}×{H}</div>
        <h1>{OS_PAGES.find((p) => p.id === osState.page)?.label}</h1>
      </div>
      <div class="stage-actions">
        <button class="btn sm" onclick={() => cycle(-1)} aria-label="Previous screen">‹</button>
        <button class="btn sm" onclick={() => cycle(1)} aria-label="Next screen">›</button>
        <button class="btn sm" onclick={resetOs}>Reset</button>
        <button class="btn primary" disabled={busy || !store.device} onclick={() => void push()}>
          {#if busy}<span class="spinner"></span>{/if}
          Push to keyboard
        </button>
      </div>
    </header>

    <div class="device">
      <div class="device-shell" style="--accent:{accent.color}">
        <div class="device-screen">
          <canvas bind:this={canvas} width={W} height={H} class="device-canvas"></canvas>
          {#if !osState.power}
            <div class="device-off">DISPLAY SLEEPING</div>
          {/if}
        </div>
      </div>
      <div class="device-meta">
        <span class="mono">{W}×{H}</span>
        <span class="sep"></span>
        <span>{wall.label} wallpaper</span>
        <span class="sep"></span>
        <span>{accent.label} accent</span>
      </div>
    </div>

    <div class="stage-foot">
      <button class="btn sm" disabled={busy || !store.device} onclick={() => void pushAnimation()}>Push 12-frame animation</button>
      <span class="hint">The keyboard loops animated frames; the live clock on the home screen re-pushes itself every minute.</span>
    </div>
  </section>

  <!-- right: inspector -->
  <aside class="os-inspector">
    <div class="insp-head">
      <h3>Appearance</h3>
      <p>Every detail, down to the pixel.</p>
    </div>

    <section class="insp-sec">
      <div class="insp-title">Accent colour</div>
      <div class="accents">
        {#each ACCENTS as a (a.id)}
          <button
            class="accent"
            class:sel={a.id === osState.settings.accent}
            onclick={() => (osState.settings.accent = a.id)}
            title={a.label}
          >
            <span class="accent-swatch" style="background:{a.color}"></span>
            <span>{a.label}</span>
          </button>
        {/each}
      </div>
    </section>

    <section class="insp-sec">
      <div class="insp-title">Wallpaper</div>
      <div class="walls">
        {#each WALLS as wl (wl.id)}
          <button
            class="wall"
            class:sel={wl.id === osState.settings.wall}
            onclick={() => (osState.settings.wall = wl.id)}
            title={wl.label}
            style="background:linear-gradient(145deg,{wl.stops[0]},{wl.stops[1]} 55%,{wl.stops[2]})"
          >
            <span>{wl.label}</span>
          </button>
        {/each}
      </div>
    </section>

    <section class="insp-sec">
      <div class="insp-title">Home layout</div>
      <div class="layouts">
        {#each [['hero', 'Signature'], ['balanced', 'Split'], ['minimal', 'Minimal']] as [id, label] (id)}
          <button
            class="layout"
            class:sel={lay === id}
            onclick={() => (osState.settings.layout = id as LockLayout)}
          >
            <span class="layout-sketch {id}"><i></i><i></i></span>
            <span>{label}</span>
          </button>
        {/each}
      </div>
    </section>

    <section class="insp-sec">
      <div class="insp-title">Complications</div>
      <label class="field">
        <span>Top widget</span>
        <select bind:value={osState.settings.widget1}>
          {#each WIDGETS as wd (wd.id)}
            <option value={wd.id}>{wd.label}</option>
          {/each}
        </select>
      </label>
      <label class="field">
        <span>Bottom widget</span>
        <select bind:value={osState.settings.widget2}>
          {#each WIDGETS as wd (wd.id)}
            <option value={wd.id}>{wd.label}</option>
          {/each}
        </select>
      </label>
    </section>

    <section class="insp-sec">
      <div class="insp-title">Typography &amp; light</div>
      <div class="row-between">
        <span class="field-label">Clock format</span>
        <div class="seg">
          <button class:sel={osState.settings.format24} onclick={() => (osState.settings.format24 = true)}>24h</button>
          <button class:sel={!osState.settings.format24} onclick={() => (osState.settings.format24 = false)}>12h</button>
        </div>
      </div>
      <label class="field">
        <span>Home message</span>
        <input type="text" maxlength="34" bind:value={osState.settings.message} />
      </label>
      <label class="field">
        <span>City</span>
        <input type="text" maxlength="20" bind:value={osState.settings.city} />
      </label>
    </section>

    <section class="insp-sec">
      <div class="insp-title">Shortcut deck</div>
      {#each osState.settings.macros as m, i (i)}
        <label class="field">
          <span>Macro {i + 1}</span>
          <input type="text" maxlength="18" bind:value={osState.settings.macros[i]} />
        </label>
      {/each}
    </section>
  </aside>
</div>

<style>
  .os-studio {
    display: grid;
    grid-template-columns: 208px minmax(0, 1fr) 320px;
    height: 100%;
    min-height: 0;
  }

  /* ---- left rail ---- */
  .os-rail {
    border-right: 1px solid var(--border);
    padding: 22px 12px 18px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    overflow-y: auto;
  }
  .rail-head {
    font-size: 10px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--faint);
    font-weight: 700;
    padding: 0 12px 12px;
  }
  .rail-item {
    display: grid;
    grid-template-columns: 8px 1fr;
    grid-template-rows: auto auto;
    gap: 0 9px;
    align-items: center;
    text-align: left;
    padding: 9px 12px;
    border-radius: 9px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }
  .rail-item:hover {
    background: var(--hover);
  }
  .rail-item.sel {
    background: var(--panel-2);
    border-color: var(--border);
  }
  .rail-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    grid-row: 1 / span 2;
    justify-self: center;
  }
  .rail-label {
    font-size: 13px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .rail-hint {
    font-size: 10.5px;
    color: var(--faint);
  }

  /* ---- stage ---- */
  .os-stage {
    padding: 26px 30px 24px;
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow-y: auto;
  }
  .stage-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }
  .stage-eyebrow {
    font-size: 9.5px;
    letter-spacing: 0.18em;
    color: var(--faint);
    font-weight: 700;
  }
  .stage-head h1 {
    margin: 7px 0 0;
    font-size: 30px;
    letter-spacing: -0.03em;
    font-weight: 600;
  }
  .stage-actions {
    display: flex;
    gap: 7px;
    align-items: center;
  }
  .device {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 22px 0 8px;
    min-height: 0;
  }
  .device-shell {
    padding: 13px;
    border-radius: 20px;
    background: linear-gradient(150deg, #3d454c 0%, #171d23 18%, #10151a 80%, #2b333a 100%);
    border: 1px solid #4b545d;
    box-shadow: 0 26px 60px rgba(0, 0, 0, 0.32), 0 2px 6px rgba(0, 0, 0, 0.3);
    position: relative;
  }
  .device-shell::after {
    content: '';
    position: absolute;
    inset: 6px;
    border-radius: 15px;
    border: 1px solid rgba(0, 0, 0, 0.5);
    pointer-events: none;
  }
  .device-screen {
    position: relative;
    border-radius: 8px;
    overflow: hidden;
    background: #05070a;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
  }
  .device-canvas {
    display: block;
    width: min(720px, 46vw);
    height: auto;
    image-rendering: pixelated;
  }
  .device-off {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: #04060a;
    color: #5a6673;
    font-size: 9px;
    letter-spacing: 0.14em;
  }
  .device-meta {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-top: 13px;
    font-size: 10.5px;
    color: var(--faint);
  }
  .sep {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--faint);
  }
  .stage-foot {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-top: 6px;
  }

  /* ---- inspector ---- */
  .os-inspector {
    border-left: 1px solid var(--border);
    background: var(--panel);
    padding: 20px 20px 28px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  .insp-head h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .insp-head p {
    margin: 4px 0 0;
    font-size: 11px;
    color: var(--faint);
  }
  .insp-title {
    font-size: 9.5px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
    font-weight: 700;
    margin-bottom: 10px;
  }
  .accents {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 7px;
  }
  .accent {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 8px 4px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--panel-2);
    color: var(--dim);
    font-size: 10px;
    cursor: pointer;
  }
  .accent.sel {
    border-color: var(--accent);
    color: var(--text);
  }
  .accent-swatch {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.12);
  }
  .walls {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 7px;
  }
  .wall {
    height: 52px;
    border-radius: 9px;
    border: 2px solid transparent;
    color: #fff;
    font-size: 9.5px;
    font-weight: 600;
    display: flex;
    align-items: flex-end;
    padding: 5px 6px;
    cursor: pointer;
    text-shadow: 0 1px 4px rgba(0, 0, 0, 0.5);
  }
  .wall.sel {
    border-color: var(--accent);
  }
  .layouts {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 7px;
  }
  .layout {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 9px 5px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--panel-2);
    color: var(--dim);
    font-size: 10px;
    cursor: pointer;
  }
  .layout.sel {
    border-color: var(--accent);
    color: var(--text);
  }
  .layout-sketch {
    width: 54px;
    height: 30px;
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    display: flex;
    gap: 3px;
    padding: 3px;
  }
  .layout-sketch i {
    background: var(--faint);
    border-radius: 2px;
    opacity: 0.6;
  }
  .layout-sketch.hero i:first-child {
    width: 62%;
    height: 100%;
  }
  .layout-sketch.hero i:last-child {
    width: 32%;
    height: 100%;
  }
  .layout-sketch.balanced i {
    width: 50%;
    height: 100%;
  }
  .layout-sketch.minimal {
    align-items: center;
    justify-content: center;
  }
  .layout-sketch.minimal i {
    width: 58%;
    height: 40%;
  }
  .field {
    display: block;
    margin-bottom: 10px;
  }
  .field > span {
    display: block;
    font-size: 10.5px;
    color: var(--dim);
    margin-bottom: 5px;
  }
  .field input,
  .field select {
    width: 100%;
    padding: 7px 9px;
    border-radius: 8px;
    border: 1px solid var(--border-strong);
    background: var(--field);
    color: var(--text);
    font-size: 12px;
  }
  .row-between {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }
  .field-label {
    font-size: 11.5px;
    color: var(--text);
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
  }
  .seg button {
    border: none;
    background: transparent;
    color: var(--dim);
    font-size: 11px;
    font-weight: 600;
    padding: 5px 10px;
    cursor: pointer;
  }
  .seg button.sel {
    background: var(--panel-2);
    color: var(--text);
  }
</style>
