<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import {
    osState,
    OS_PAGES,
    ACCENTS,
    WALLS,
    WIDGETS,
    renderOsPage,
    osFrames,
    osCarouselFrames,
    osCartridgeFrames,
    b64,
    tickOs,
    osHost,
    resetOs,
    type OsPage,
    type LockLayout,
  } from '../lib/os.svelte';
  import { listen } from '@tauri-apps/api/event';
  import { store, toast, errText } from '../lib/store.svelte';
  import { startLive, stopLive, live } from '../lib/live';
  import * as api from '../lib/tauri';
  import type { AppConfig } from '../lib/types';

  let cfg: AppConfig | null = $state(null);
  let slot = $state(0);
  let busy = $state(false);
  // Minimum gap between two pushes. The keyboard is a slow, exclusive
  // device: pushing back-to-back floods it with handshakes and can make it
  // reboot, so every push path goes through this guard.
  const PUSH_COOLDOWN_MS = 2000;
  let lastPushAt = 0;

  let canvas: HTMLCanvasElement | undefined = $state();
  let dpr = 1;
  let cycling = $state(false);
  let cycleSecs = $state(6);
  let pushOnCycle = $state(false);

  const W = $derived(cfg?.screen_width ?? 240);
  const H = $derived(cfg?.screen_height ?? 135);
  const page = $derived(osState.page);
  const accent = $derived(ACCENTS.find((a) => a.id === osState.settings.accent) ?? ACCENTS[0]);
  const wall = $derived(WALLS.find((w) => w.id === osState.settings.wall) ?? WALLS[0]);
  const lay = $derived(osState.settings.layout);

  onMount(async () => {
    cfg = await api.configGet().catch(() => null);
    startLive();
    let last = performance.now();
    const t = setInterval(() => {
      const now = performance.now();
      const dt = Math.min(0.5, (now - last) / 1000);
      last = now;
      tickOs(dt);
      paint();
    }, 66);
    window.addEventListener('keydown', onKey);
    // Native knob: the backend watches the keyboard's evdev node and emits
    // 'sharkdeck://knob' for every detent (turn left / turn right / press).
    void api
      .knobSubscribe()
      .then((r) => (knobNodes = r.nodes))
      .catch(() => (knobNodes = []));
    const unlisten = listen<api.KnobAction>('sharkdeck://knob', (e) => {
      const dir = e.payload?.dir;
      if (!knobControl) return;
      if (dir === 'left') cycle(-1);
      else if (dir === 'right') cycle(1);
      else if (dir === 'press') void push(true);
    });
    return () => {
      clearInterval(t);
      stopLive();
      window.removeEventListener('keydown', onKey);
      void unlisten.then((fn) => fn());
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
    // re-render whenever the OS state or panel size changes.
    // untrack: the paint itself reads live metrics, which must not become
    // dependencies of this effect (it would loop on every simulation tick).
    osState.page;
    osState.settings.accent;
    osState.settings.wall;
    osState.settings.layout;
    W;
    H;
    untrack(paint);
  });

  $effect(() => {
    const info = liveInfo();
    if (info) {
      osHost.battery = info.battery;
      osHost.charging = info.charging;
      if (typeof info.cpu === 'number') osHost.cpu = info.cpu;
      if (typeof info.ram === 'number') osHost.ram = info.ram;
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

  /* Auto-cycling: step through all 9 screens, optionally pushing each one. */
  $effect(() => {
    if (!cycling) return;
    const every = Math.max(2, cycleSecs) * 1000;
    const id = setInterval(() => {
      cycle(1);
      if (pushOnCycle) void push(true);
    }, every);
    return () => clearInterval(id);
  });

  /* Keyboard: ← / → walk the screens. */
  function onKey(e: KeyboardEvent) {
    const el = e.target as HTMLElement | null;
    if (el && ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName)) return;
    if (e.key === 'ArrowLeft') {
      e.preventDefault();
      cycle(-1);
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      cycle(1);
    } else if (/^[1-9]$/.test(e.key)) {
      const pg = OS_PAGES[Number(e.key) - 1];
      if (pg) osState.page = pg.id as OsPage;
    }
  }


  /* ---- fast switching between the 9 screens ---- */

  /* Carousel: ONE push whose frames are the 9 screens; the keyboard then
     rotates them on its own — no further pushes needed. */
  let dwell = $state(100);
  let knobControl = $state(false);
  let knobNodes = $state<string[]>([]);
  async function pushCarousel() {
    if (!store.device) return toast('err', 'No device connected');
    const now = Date.now();
    if (now - lastPushAt < PUSH_COOLDOWN_MS) {
      if (!silent) toast('info', 'Please wait a moment before pushing again');
      return;
    }
    lastPushAt = now;
    busy = true;
    try {
      const frames = osCarouselFrames(
        OS_PAGES.map((pg) => pg.id as OsPage),
        W,
        H,
        Math.max(1, Math.min(255, Math.round(dwell))),
      );
      await api.screenAnimPush(frames, W, H, cfg?.order);
      toast('ok', `Carousel pushed — ${frames.length} screens now rotate on the keyboard`);
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  /* Favourites -> the 5 on-device image slots, so the keyboard's own knob
     menu switches between them instantly. */
  let slotPages = $state<string[]>(['lock', 'dash', 'music', 'perf', 'focus']);
  async function pushSlots() {
    if (!store.device) return toast('err', 'No device connected');
    const now = Date.now();
    if (now - lastPushAt < PUSH_COOLDOWN_MS) {
      if (!silent) toast('info', 'Please wait a moment before pushing again');
      return;
    }
    lastPushAt = now;
    busy = true;
    try {
      for (let i = 0; i < 5; i++) {
        const page = slotPages[i] as OsPage;
        await api.screenPush(b64(renderOsPage(page, W, H)), W, H, 'Stretch', cfg?.order, 10, i);
      }
      toast('ok', '5 screens pushed to the image slots — switch instantly from the keyboard');
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  /* ---- CARTRIDGE: a self-running timeline the keyboard plays on its own ----
   *
   * The panel is a dumb framebuffer, but its firmware plays a stored animation
   * on its own timer with a per-frame delay. That player is the only engine
   * the hardware has, so instead of pushing a screen again and again we
   * compile a scene into a CARTRIDGE (frames + delays). Once it is on the
   * keyboard it loops by itself — laptop closed, keyboard on battery. */

  let cartFrames = $state(60);
  let cartDwell = $state(100); // centiseconds per frame (100 = 1.00 s)
  let cartPage = $state<OsPage>('lock');
  let cartAnchor = $state(true);

  const cartSeconds = $derived((cartFrames * cartDwell) / 100);

  async function pushCartridge(silent = false) {
    if (!store.device) return toast('err', 'No device connected');
    const now = Date.now();
    if (now - lastPushAt < PUSH_COOLDOWN_MS) {
      if (!silent) toast('info', 'Please wait a moment before pushing again');
      return;
    }
    lastPushAt = now;
    busy = true;
    try {
      const count = Math.max(1, Math.min(255, Math.round(cartFrames)));
      const delay = Math.max(1, Math.min(255, Math.round(cartDwell)));
      // The device only starts playing once every frame has landed, so the
      // frames are stamped with the moment the keyboard will reach frame 0.
      const est = await api.screenCartridgeEstimate(count, W * H * 4);
      const frames = osCartridgeFrames(
        cartPage,
        W,
        H,
        count,
        delay,
        Math.ceil(est / 1000),
      );
      await api.screenAnimPush(frames, W, H, cfg?.order);
      if (!silent) {
        toast('ok', `Cartridge loaded — ${count} frames, ${Math.round((count * delay) / 100)}s running on the keyboard alone`);
      }
    } catch (e) {
      if (!silent) toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  /* Re-anchor the cartridge before it runs out so the clock stays exact while
     the host is connected. With the laptop off, nothing is pushed at all —
     the keyboard keeps playing the last cartridge on its own. */
  $effect(() => {
    if (!cartAnchor) return;
    const period = Math.max(10, Math.round((cartFrames * cartDwell) / 200)) * 1000;
    const id = setInterval(() => {
      if (store.device && !busy) void pushCartridge(true);
    }, period);
    return () => clearInterval(id);
  });

  /* Keep the keyboard live: re-anchor the pushed animation before the
     30 frames run out, so the clock and the counters never rewind. */
  let keepLive = $state(false);
  $effect(() => {
    if (!keepLive) return;
    const id = setInterval(() => {
      if (store.device && !busy) void pushAnimation(true);
    }, 25000);
    return () => clearInterval(id);
  });

  async function push(silent = false) {
    if (!store.device) return toast('err', 'No device connected');
    const now = Date.now();
    if (now - lastPushAt < PUSH_COOLDOWN_MS) {
      if (!silent) toast('info', 'Please wait a moment before pushing again');
      return;
    }
    lastPushAt = now;
    busy = true;
    try {
      const frames = osFrames(osState.page, W, H, 1);
      await api.screenAnimPush(frames, W, H, cfg?.order);
      if (!silent) {
        toast('ok', `${OS_PAGES.find((p) => p.id === osState.page)?.label} pushed to the keyboard`);
      }
    } catch (e) {
      if (!silent) toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  async function pushAnimation(silent = false) {
    if (!store.device) return toast('err', 'No device connected');
    const now = Date.now();
    if (now - lastPushAt < PUSH_COOLDOWN_MS) {
      if (!silent) toast('info', 'Please wait a moment before pushing again');
      return;
    }
    lastPushAt = now;
    busy = true;
    try {
      const frames = osFrames(osState.page, W, H, 30);
      await api.screenAnimPush(frames, W, H, cfg?.order);
      if (!silent) toast('ok', 'Animated OS page pushed (30 live frames)');
    } catch (e) {
      if (!silent) toast('err', errText(e));
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

    <div class="rail-head">Quick switch</div>
    <div class="quick-grid">
      {#each OS_PAGES as pg, i (pg.id)}
        <button
          class="quick-btn"
          class:sel={pg.id === osState.page}
          title={pg.label}
          onclick={() => (osState.page = pg.id as OsPage)}
        >{i + 1}</button>
      {/each}
    </div>

    <div class="rail-head">Image slots</div>
    <div class="slot-picks">
      {#each slotPages as sp, i}
        <label class="slot-pick">
          <span>Slot {i + 1}</span>
          <select bind:value={slotPages[i]}>
            {#each OS_PAGES as pg (pg.id)}
              <option value={pg.id}>{pg.label}</option>
            {/each}
          </select>
        </label>
      {/each}
      <button class="btn sm" disabled={busy || !store.device} onclick={() => void pushSlots()}>
        Push 5 to slots
      </button>
    </div>
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
        <button class="btn sm" class:active={cycling} onclick={() => (cycling = !cycling)} aria-pressed={cycling}>
          {cycling ? 'Stop cycle' : 'Cycle screens'}
        </button>
        <select class="btn sm cycle-secs" bind:value={cycleSecs} aria-label="Cycle interval">
          <option value={3}>3s</option>
          <option value={6}>6s</option>
          <option value={10}>10s</option>
          <option value={20}>20s</option>
        </select>
        <label class="chk push-cycle">
          <input type="checkbox" bind:checked={pushOnCycle} />
          push each
        </label>
        <label class="chk push-cycle" title="Re-push the animation every 25 s so the clock and counters never rewind">
          <input type="checkbox" bind:checked={keepLive} />
          keep live
        </label>
        <button class="btn sm" onclick={resetOs}>Reset</button>
        <label class="chk push-cycle" title="Dwell per screen in the carousel">
          dwell
          <input class="num" type="number" min="10" max="255" bind:value={dwell} />
          cs
        </label>
        <button class="btn sm" disabled={busy || !store.device} onclick={() => void pushCarousel()}>
          Push carousel
        </button>
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
      <button class="btn sm" disabled={busy || !store.device} onclick={() => void pushAnimation()}>Push 30-frame animation</button>
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
      <div class="insp-title">Cartridge</div>
      <p class="hint" style="margin: 0 0 10px">
        The panel is a framebuffer, but its firmware plays a stored animation on its own timer.
        A cartridge is that animation — compiled once, it loops on the keyboard forever, with the
        laptop closed.
      </p>
      <label class="chk push-cycle" style="margin-bottom: 8px">
        Screen
        <select class="btn sm" bind:value={cartPage}>
          {#each OS_PAGES as p (p.id)}
            <option value={p.id}>{p.label}</option>
          {/each}
        </select>
      </label>
      <div class="row" style="gap: 8px; align-items: center; margin-bottom: 8px">
        <label class="chk push-cycle">
          frames
          <input class="num" type="number" min="1" max="255" bind:value={cartFrames} />
        </label>
        <label class="chk push-cycle">
          dwell
          <input class="num" type="number" min="10" max="255" bind:value={cartDwell} />
          cs
        </label>
      </div>
      <label class="chk push-cycle" style="margin-bottom: 10px" title="Re-push the cartridge before it runs out so the clock stays exact while the app is open">
        <input type="checkbox" bind:checked={cartAnchor} />
        re-anchor while connected
      </label>
      <button class="btn tint" style="width: 100%" disabled={busy || !store.device} onclick={() => void pushCartridge()}>
        {#if busy}<span class="spinner"></span>{/if}
        Load cartridge
      </button>
      <p class="hint" style="margin-top: 8px">
        {Math.round(cartSeconds)}s of timeline · {cartDwell}cs per frame
        {#if cartDwell === 100}· one second per frame{/if}
        · the device verified up to 255 frames (~4m15s of self-running clock)
      </p>
      <p class="hint" style="margin-top: 6px">
        With 60 one-second frames the seconds stay exact for ever (the keyboard
        loops them) and only the hour/minute freeze; more frames keep the whole
        clock exact for longer.
      </p>
    </section>

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
      <div class="insp-title">Knob control</div>
      <label class="switch-row">
        <input type="checkbox" bind:checked={knobControl} />
        <span>Let the knob drive the screen</span>
      </label>
      <p class="hint">
        Turn left / right to change the keyboard screen, press to push the current one.
        {#if knobNodes.length}
          Reading {knobNodes[0]}.
        {:else}
          No keyboard input node found — check the udev rule.
        {/if}
      </p>
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
    background: transparent;
    color: var(--text);
    cursor: pointer;
  box-shadow: var(--elev-1);
}
  .rail-item:hover {
    background: var(--hover);
  }
  .rail-item.sel {
    background: var(--panel-2);
    border-color: var(--border);
  box-shadow: var(--elev-1);
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
    box-shadow: 0 26px 60px rgba(0, 0, 0, 0.32), 0 2px 6px rgba(0, 0, 0, 0.3);
    position: relative;
  }
  .device-shell::after {
    content: '';
    position: absolute;
    inset: 6px;
    border-radius: 15px;
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
  box-shadow: var(--elev-1);
}
  .stage-foot {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-top: 6px;
  }

  /* ---- inspector ---- */
  .os-inspector {
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
    background: var(--panel-2);
    color: var(--dim);
    font-size: 10px;
    cursor: pointer;
  box-shadow: var(--elev-1);
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
    background: var(--panel-2);
    color: var(--dim);
    font-size: 10px;
    cursor: pointer;
  box-shadow: var(--elev-1);
}
  .layout.sel {
    border-color: var(--accent);
    color: var(--text);
}
  .layout-sketch {
    width: 54px;
    height: 30px;
    border-radius: 4px;
    display: flex;
    gap: 3px;
    padding: 3px;
}
  .layout-sketch i {
    background: var(--faint);
    border-radius: 2px;
    opacity: 0.6;
  box-shadow: var(--elev-1);
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
    background: var(--field);
    color: var(--text);
    font-size: 12px;
  box-shadow: var(--elev-1);
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
  box-shadow: var(--elev-1);
}
  .seg button.sel {
    background: var(--panel-2);
    color: var(--text);
  }

  /* cycle toolbar */
  .stage-actions .btn.active {
    background: var(--accent);
    color: #fff;
    box-shadow: var(--elev-1);
  }
  .cycle-secs {
    padding: 0 8px;
    min-width: 62px;
  }
  .push-cycle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--text-dim);
  }
  .push-cycle input {
    accent-color: var(--accent);
  }

  .quick-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 5px; margin: 0 0 14px; }
  .quick-btn {
    height: 26px; border-radius: 7px; border: 1px solid var(--line); background: var(--panel-2);
    color: var(--muted); font: 600 11px/1 var(--mono); cursor: pointer; transition: all .16s ease;
  }
  .quick-btn:hover { color: var(--text); border-color: var(--accent-line); }
  .quick-btn.sel { color: var(--accent-ink); background: var(--accent); border-color: var(--accent); }
  .slot-picks { display: grid; gap: 6px; margin-bottom: 14px; }
  .slot-pick { display: grid; grid-template-columns: 44px 1fr; align-items: center; gap: 6px; font-size: 10px; color: var(--muted); }
  .slot-pick select { width: 100%; height: 24px; font-size: 11px; padding: 0 6px; }
</style>
