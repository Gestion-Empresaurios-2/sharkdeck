<script lang="ts">
  import * as api from '../lib/tauri';
  import { store, toast, errText, connected } from '../lib/store.svelte';
  import type { LightingState } from '../lib/types';
  import { onMount } from 'svelte';

  const EFFECTS: [number, string][] = [
    [0, 'Off'],
    [1, 'Static'],
    [2, 'Breathing'],
    [3, 'Neon'],
    [4, 'Wave'],
    [5, 'Ripple'],
    [6, 'Raindrop'],
    [7, 'Snake'],
    [8, 'Keypress'],
    [9, 'Converge'],
    [10, 'Sine'],
    [11, 'Kaleidoscope'],
    [12, 'Line wave'],
    [14, 'Laser'],
    [15, 'Circle wave'],
    [16, 'Dazzle'],
    [17, 'Rain down'],
    [18, 'Meteor'],
    [23, 'Train'],
    [24, 'Fireworks'],
  ];

  const PRESETS: [number, string][] = [
    [0, 'Red'],
    [1, 'Green'],
    [2, 'Blue'],
    [3, 'Orange'],
    [4, 'Magenta'],
    [5, 'Yellow'],
    [6, 'White'],
  ];

  let main = $state<LightingState | null>(null);
  let side = $state<LightingState | null>(null);

  // editor state
  let effect = $state(2);
  let brightness = $state(3);
  let speed = $state(2);
  let direction = $state(0);
  let color = $state('#5B8CFF');
  let presetMode = $state<number | null>(null); // null = custom RGB, else 0..6
  let rainbow = $state(false);
  let busy = $state(false);

  onMount(async () => {
    if (!connected()) return;
    try {
      main = await api.lightGet(false);
      side = await api.lightGet(true);
    } catch (e) {
      toast('err', errText(e));
    }
  });

  function hexToRgb(hexString: string): [number, number, number] {
    const h = hexString.replace('#', '');
    return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
  }

  function rgbToHex(r: number, g: number, b: number): string {
    return '#' + [r, g, b].map((x) => x.toString(16).padStart(2, '0')).join('');
  }

  function st(): LightingState {
    const [r, g, b] = hexToRgb(color);
    // firmware's preset colors live on nibble 0..6; rainbow is nibble 8
    let m: number;
    let le: number;
    if (rainbow) {
      m = 8;
      le = effect <= 2 ? 1 : effect;
    } else if (presetMode !== null) {
      m = presetMode;
      le = 1;
    } else {
      m = 7;
      le = effect;
    }
    return { effect: le, speed, brightness, direction, mode: m, r, g, b };
  }

  function reflect(l: LightingState | null) {
    if (!l) return;
    effect = l.effect;
    brightness = l.brightness;
    speed = l.speed;
    direction = l.direction;
    if (l.mode === 8) {
      rainbow = true;
      presetMode = null;
    } else if (l.mode === 7) {
      rainbow = false;
      presetMode = null;
    } else {
      rainbow = false;
      presetMode = l.mode;
    }
    color = rgbToHex(l.r, l.g, l.b);
  }

  async function apply(side_: boolean) {
    if (!connected()) return toast('err', 'Probe the device first');
    if (!main) {
      try {
        main = await api.lightGet(false);
      } catch {
        /* ignore */
      }
    }
    busy = true;
    try {
      const s = st();
      if (side_) await api.lightSet(true, s);
      else await api.lightSet(false, s);
      toast('ok', side_ ? 'Side strips updated' : 'Main RGB updated');
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  async function applyAll() {
    if (!connected()) return toast('err', 'Probe the device first');
    busy = true;
    try {
      const s = st();
      await api.lightSet(false, s);
      await api.lightSet(true, s);
      toast('ok', 'Main + side updated');
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  async function fixLeds() {
    if (!connected()) return toast('err', 'Probe the device first');
    busy = true;
    try {
      store.options = await api.ledsOn();
      toast('ok', 'Power-save off, LEDs un-muted');
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }
</script>

<div class="page">
  <div class="page-head">
    <h2>Lighting</h2>
    <p>
      Main RGB and underline side strips. Effect codes mirror those shipped in the originals.
    </p>
  </div>

  <div class="cols">
    <div class="grid" style="gap:16px">
      <div class="panel">
        <h3>Effect</h3>
        <div class="chips">
          {#each EFFECTS as [id, label] (id)}
            <button
              class="chip"
              class:sel={!rainbow && presetMode === null && effect === id}
              onclick={() => ((rainbow = false), (presetMode = null), (effect = id))}
            >
              {label}
            </button>
          {/each}
          <button class="chip" class:sel={rainbow} onclick={() => ((rainbow = true), (presetMode = null))}>
            Rainbow
          </button>
        </div>
      </div>

      <div class="panel">
        <h3>Color source</h3>
        <div class="row" style="flex-wrap:wrap">
          <input type="color" bind:value={color} />
          {#each PRESETS as [m, label] (m)}
            <button
              class="chip"
              class:sel={!rainbow && presetMode === m}
              onclick={() => ((rainbow = false), (presetMode = m))}
            >
              {label}
            </button>
          {/each}
        </div>
        <p class="hint" style="margin-top:10px">
          Preset nibbles also light the RGB when the custom color is too dark to pass the
          firmware's (8,8,8) threshold.
        </p>
      </div>

      <div class="panel">
        <h3>Brightness & speed</h3>
        <label class="slider">
          <span>Brightness</span>
          <input type="range" min="0" max="4" step="1" bind:value={brightness} />
          <span class="mono v">{brightness}</span>
        </label>
        <label class="slider">
          <span>Speed</span>
          <input type="range" min="0" max="4" step="1" bind:value={speed} />
          <span class="mono v">{speed}</span>
        </label>
        <label class="slider">
          <span>Direction</span>
          <div class="row">
            {#each [[0, 'Front →'], [1, '← Back']] as [d, label] (d)}
              <button
                class="chip"
                class:sel={direction === d}
                onclick={() => (direction = Number(d))}
              >
                {label}
              </button>
            {/each}
          </div>
        </label>
      </div>
    </div>

    <div class="grid" style="gap:16px">
      <div class="panel">
        <h3>Readback</h3>
        <div class="kv">
          <span class="k">Main</span>
          <span class="v">{main ? `${EFFECTS.find(([i]) => i === main!.effect)?.[1] ?? main!.effect} · mode ${main!.mode} · rgb(${main!.r},${main!.g},${main!.b})` : '—'}</span>
        </div>
        <div class="kv">
          <span class="k">Side</span>
          <span class="v">{side ? `${EFFECTS.find(([i]) => i === side!.effect)?.[1] ?? side!.effect} · mode ${side!.mode} · rgb(${side!.r},${side!.g},${side!.b})` : '—'}</span>
        </div>
        <div class="row" style="margin-top:12px; flex-wrap: wrap">
          <button class="btn sm" onclick={() => { reflect(main); }}>Load main</button>
          <button class="btn sm" onclick={() => { reflect(side); }}>Load side</button>
        </div>
      </div>

      <div class="panel">
        <h3>Apply</h3>
        <div class="row" style="flex-wrap:wrap">
          <button class="btn primary" onclick={() => apply(false)} disabled={busy}>
            Main RGB
          </button>
          <button class="btn primary" onclick={() => apply(true)} disabled={busy}>
            Side strips
          </button>
          <button class="btn" onclick={applyAll} disabled={busy}>All</button>
        </div>
      </div>

      <div class="panel">
        <h3>Nothing lights up?</h3>
        <p class="hint">
          Power-save or the LED-off bits silence both strips. This fixes them (read-modify-write
          of the option bytes, nothing else touched).
        </p>
        <button class="btn" style="margin-top:12px" onclick={fixLeds} disabled={busy}>
          {#if busy}<span class="spinner"></span>{/if} Run LED fixer
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 16px;
    align-items: start;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .slider {
    display: grid;
    grid-template-columns: 80px 1fr 40px;
    align-items: center;
    gap: 14px;
    margin-bottom: 14px;
    font-size: 13px;
  }
  .slider:last-child {
    margin-bottom: 0;
  }
  .v {
    text-align: right;
    color: var(--text-dim);
    font-size: 12px;
  }
</style>
