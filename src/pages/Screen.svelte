<script lang="ts">
  import { onMount } from 'svelte';
  import * as api from '../lib/tauri';
  import { store, toast, errText, connected } from '../lib/store.svelte';
  import type { AppConfig } from '../lib/types';
  import ScreenPreview from '../components/ScreenPreview.svelte';
  import LiveMirror from '../components/LiveMirror.svelte';
  import { renderTemplate, type Template } from '../lib/templates';
  import { startLive } from '../lib/live';

  const PATTERNS: [string, string][] = [
    ['test', 'RGBW quadrants'],
    ['ruler', 'Border + crosshair'],
    ['orientation', 'Orientation tester'],
    ['band', 'Band grid'],
    ['solid-red', 'Red'],
    ['solid-green', 'Green'],
    ['solid-blue', 'Blue'],
    ['solid-white', 'White'],
    ['blank', 'Blank'],
  ];

  let cfg = $state<AppConfig | null>(null);
  let preview = $state('');
  let previewW = $state(128);
  let previewH = $state(128);
  let slot = $state(0);
  let mode = $state<'Fill' | 'Fit' | 'Stretch'>('Fill');
  let busy = $state('');
  let lastUpload = $state('');

  // clock controls
  let clockFrames = $state(15);
  let clockSeconds = $state(false);
  let clockFg = $state('#E6E9F2');
  let clockBg = $state('#0A0C12');
  let clockLive = $state(false);

  // Live mirror of what the keyboard is showing. The device has no screen
  // read-back, so we mirror exactly what SharkDeck pushes.
  const clockTpl: Template = {
    id: 'live-clock',
    name: 'Live clock',
    desc: 'Mirrors the clock pushed to the keyboard',
    bg: '#0A0C12',
    dt: 1,
    comps: [{ kind: 'clock', fg: '#E6E9F2', seconds: true, size: 0.5 }],
  };

  function b64ToRgba(b64: string): Uint8ClampedArray {
    const bin = atob(b64);
    const out = new Uint8ClampedArray(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }

  const clockRender = (f: number) => renderTemplate(clockTpl, previewW, previewH, f);
  const imageRender = () =>
    preview
      ? b64ToRgba(preview)
      : new Uint8ClampedArray(previewW * previewH * 4);
  const liveRender = $derived(clockLive ? clockRender : imageRender);

  onMount(async () => {
    try {
      cfg = await api.configGet();
      if (cfg) {
        previewW = cfg.screen_width;
        previewH = cfg.screen_height;
      }
    } catch (e) {
      toast('err', errText(e));
    }
    startLive();
    // The keyboard loops the frames we push, so the clock has to be re-anchored
    // to real time well before the window runs out. Every frame carries an
    // absolute timestamp, so a re-push corrects any drift instead of stacking
    // it up, and the schedule is aligned to whole wall-clock seconds.
    // Traffic matters: a 240x135 frame is ~1158 feature reports, so a
    // seconds-accurate clock costs ~1 frame/s of USB traffic and can starve
    // the keyboard's own key reports. Without seconds we re-anchor once a
    // minute and stay out of the way.
    const REFRESH_MS = clockSeconds ? 10000 : 60000;
    const tick = () => {
      const now = Date.now();
      const refresh = clockSeconds ? 10000 : 60000;
      const next = Math.ceil((now + 250) / refresh) * refresh;
      timer = window.setTimeout(async () => {
        if (store.autopush && clockLive && connected() && !busy) await clockPush(true);
        tick();
      }, next - now);
    };
    tick();
  });

  let timer = 0;
  $effect(() => () => {
    if (timer) window.clearTimeout(timer);
  });

  const zoom = $derived(Math.max(2, Math.min(5, Math.floor(previewW / 64) * 2 || 3)));

  function hexToRgb(hexString: string): number[] {
    const h = hexString.replace('#', '');
    return [
      parseInt(h.slice(0, 2), 16),
      parseInt(h.slice(2, 4), 16),
      parseInt(h.slice(4, 6), 16),
    ];
  }

  async function importFile() {
    if (!connected()) return toast('err', 'Probe the device first');
    busy = 'import';
    try {
      const path = await api.pickOpenImage();
      if (!path) return;
      const res = await api.screenImport(path, mode, cfg?.order, slot, 5);
      preview = res.previews[0] ?? '';
      previewW = res.width;
      previewH = res.height;
      lastUpload = path.split('/').pop() ?? path;
      clockLive = false;
      toast(
        'ok',
        res.animated
          ? `GIF pushed: ${res.count} frames to keyboard player`
          : `${res.count > 1 ? res.count + ' slots inverted' : 'Image'} pushed to slot ${slot}`,
      );
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = '';
    }
  }

  async function pattern(p: string) {
    if (!connected()) return toast('err', 'Probe the device first');
    busy = 'pattern';
    try {
      const res = await api.screenPattern(p, slot);
      preview = res.preview;
      previewW = res.width;
      previewH = res.height;
      lastUpload = p;
      clockLive = false;
      toast('ok', `Pattern "${p}" uploaded to slot ${slot}`);
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = '';
    }
  }

  async function clockPush(silent = false) {
    if (!connected()) return toast('err', 'Probe the device first');
    busy = 'clock';
    try {
      await api.screenClock(clockFrames, clockSeconds, hexToRgb(clockFg), hexToRgb(clockBg));
      preview = '';
      lastUpload = 'clock';
      clockLive = true;
      if (!silent) {
        toast('ok', `Clock pushed (${clockFrames} frames, ${clockSeconds ? 'HH:MM:SS' : 'HH:MM'})`);
      }
    } catch (e) {
      if (!silent) toast('err', errText(e));
    } finally {
      busy = '';
    }
  }

  async function saveConfig() {
    if (!cfg) return;
    try {
      await api.configSet(cfg);
      toast('ok', 'Saved screen preferences');
    } catch (e) {
      toast('err', errText(e));
    }
  }
</script>

<div class="page">
  <div class="page-head">
    <h2>Screen</h2>
    <p>
      Upload images, GIFs and generated content to the 5 on-device image slots. Slot switching is
      done from the keyboard's own menu (turn the knob in screen mode) — improves the panel without
      firmware hacks.
    </p>
  </div>

  <div class="layout">
    <div class="left">
      <div class="panel">
        <h3>Preview · {previewW}×{previewH}</h3>
        <ScreenPreview {preview} width={previewW} height={previewH} {zoom} />
        {#if preview}
          <div class="row" style="justify-content:center; margin-top:12px">
            <button
              class="btn tint"
              onclick={() => {
                if (!connected()) return toast('err', 'Probe the device first');
                api
                  .screenPush(preview, previewW, previewH, mode, cfg?.order, 10, slot)
                  .then(() => toast('ok', `Preview pushed to slot ${slot}`))
                  .catch((e) => toast('err', errText(e)));
              }}
            >
              Push preview → slot {slot}
            </button>
          </div>
        {/if}
      </div>

      <div class="panel">
        <h3>On the keyboard now</h3>
        <LiveMirror
          render={liveRender}
          w={previewW}
          h={previewH}
          fps={clockLive ? 1 : 2}
          label={clockLive ? 'Live · clock' : 'Live panel'}
        />
        <p class="hint" style="margin:8px 0 0">
          The keyboard exposes no screen read-back, so this mirrors exactly what SharkDeck sends.
          {#if clockLive}It refreshes the device clock about once a minute so the loop never counts backwards.{/if}
        </p>
      </div>
    </div>

    <div class="right grid">
      <div class="panel">
        <h3>Image slot & scaling</h3>
        <div class="row" style="margin-bottom:12px">
          {#each [0, 1, 2, 3, 4] as s (s)}
            <button class="chip" class:sel={slot === s} onclick={() => (slot = s)}>
              Slot {s + 1}
            </button>
          {/each}
        </div>
        <div class="row">
          <span class="muted" style="font-size:12.5px">Scale</span>
          {#each ['Fill', 'Fit', 'Stretch'] as m (m)}
            <button class="chip" class:sel={mode === m} onclick={() => (mode = m)}>{m}</button>
          {/each}
        </div>
      </div>

      <div class="panel">
        <h3>Image / GIF</h3>
        <div class="row">
          <button class="btn primary" onclick={importFile} disabled={busy === 'import'}>
            {#if busy === 'import'}<span class="spinner"></span>{/if}
            {busy === 'import' ? 'Uploading…' : 'Pick image / GIF…'}
          </button>
          {#if lastUpload}<span class="hint">{lastUpload}</span>{/if}
        </div>
        <p class="hint" style="margin-top:10px">
          Animated GIFs play on-device (up to {cfg?.max_frames ?? 30} frames, delays from the file).
        </p>
      </div>

      <div class="panel">
        <h3>Built-in patterns</h3>
        <div class="chips">
          {#each PATTERNS as [p, label] (p)}
            <button class="chip" onclick={() => pattern(p)} disabled={busy === 'pattern'}>{label}</button>
          {/each}
        </div>
        <p class="hint" style="margin-top:10px">
          Use <b>Orientation tester</b> / <b>Border + crosshair</b> to verify panel size in
          Device → Display calibration.
        </p>
      </div>

      <div class="panel">
        <h3>Clock</h3>
        <div class="row" style="flex-wrap:wrap">
          <label class="row" style="gap:6px; font-size:13px">
            <input type="checkbox" bind:checked={clockSeconds} /> Show seconds
          </label>
          <span class="muted" style="font-size:12.5px">frames</span>
          <input type="number" min={1} max={60} bind:value={clockFrames} style="width:64px" />
          <input type="color" bind:value={clockFg} />
          <input type="color" bind:value={clockBg} />
          <button class="btn tint" onclick={clockPush} disabled={busy === 'clock'}>
            {#if busy === 'clock'}<span class="spinner"></span>{/if}
            Upload clock
          </button>
          <button class="btn" onclick={() => void clockPush(true)} disabled={busy === 'clock'} title="Re-anchor the clock to the current second right now">
            Sync now
          </button>
        </div>
        <p class="hint" style="margin-top:10px">
          Renders a live clock animation into slot 0 — reboot the keyboard to switch slots back.
          Every frame carries an absolute timestamp, and the app re-anchors the window every 10 s, so the
          seconds count forwards one at a time and never rewinds or skips.
        </p>
      </div>

      {#if cfg}
        <div class="panel">
          <h3>Panel geometry</h3>
          <div class="row" style="flex-wrap:wrap">
            <span class="muted" style="font-size:12.5px">W</span>
            <input type="number" min={8} max={1024} bind:value={cfg.screen_width} style="width:72px" />
            <span class="muted" style="font-size:12.5px">H</span>
            <input type="number" min={8} max={1024} bind:value={cfg.screen_height} style="width:72px" />
            <span class="muted" style="font-size:12.5px">order</span>
            <select bind:value={cfg.order} style="width:96px">
              <option value="Column">Column</option>
              <option value="Row">Row</option>
            </select>
            <button class="btn sm" onclick={saveConfig}>Save</button>
          </div>
          <p class="hint" style="margin-top:10px">
            Default 128×128 column-order is hardware-verified. Wrong geometry looks sheared —
            fix with the calibrators above.
          </p>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .layout {
    display: grid;
    grid-template-columns: minmax(300px, 380px) 1fr;
    gap: 16px;
    align-items: start;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
</style>
