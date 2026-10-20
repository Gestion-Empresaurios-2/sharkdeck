<script lang="ts">
  import { onMount } from 'svelte';
  import { store } from '../lib/store.svelte';
  import {
    TEMPLATES,
    renderTemplate,
    renderFrames,
    renderStill,
    templateIsAnimated,
    activeLayout,
    compsFromLayout,
    SLOT_IDS,
    SLOT_LABEL,
    type LayoutDef,
    type SlotId,
    type Template,
    type Comp,
    type CompKind,
  } from '../lib/templates';
  import ScreenPreview from '../components/ScreenPreview.svelte';
  import TemplateThumb from '../components/TemplateThumb.svelte';
  import LiveMirror from '../components/LiveMirror.svelte';
  import { configGet, configSet, screenPattern, screenPush, screenAnimPush } from '../lib/tauri';
  import { startLive } from '../lib/live';
  import type { AppConfig } from '../lib/types';

  let cfg: AppConfig | null = $state(null);
  let tpl: Template = $state(TEMPLATES[0]);
  let frames = $state(30);
  let slot = $state(0);
  let busy = $state(false);

  // wizard
  let wizBusy = $state(false);
  let wizPreview = $state<string | null>(null);
  let wizW = $state(0);
  let wizH = $state(0);

  const anim = $derived(templateIsAnimated(tpl));
  const layouts = $derived(tpl.layouts ?? []);
  const lay = $derived(activeLayout(tpl));

  /** Switch to another layout of this template: rebuild the component set. */
  function useLayout(l: LayoutDef) {
    tpl = { ...tpl, layout: l.id, comps: compsFromLayout(l) };
    refresh();
  }

  function setSlot(i: number, slot: SlotId) {
    upd(i, { slot });
  }

  const SLOT_SHORT: Record<SlotId, string> = {
    tl: 'TL', tr: 'TR', c: 'C', bl: 'BL', br: 'BR', bar: 'BAR', l: 'L', r: 'R',
  };

  onMount(async () => {
    cfg = await configGet().catch(() => null);
    startLive();
  });

  function wth(): [number, number] {
    return [cfg?.screen_width ?? 240, cfg?.screen_height ?? 135];
  }

  /* preview of the composed template */
  let preview = $state<string | null>(null);
  let previewW = $state(240);
  let previewH = $state(135);
  $effect(() => {
    const [w, h] = wth();
    previewW = w;
    previewH = h;
    nonce;
    preview = renderStill(tpl, w, h);
  });

  const KIND_LABEL: Record<CompKind, string> = {
    clock: 'Clock', date: 'Date', ticker: 'Ticker', wave: 'Wave', rain: 'Rain',
    border: 'Frame', eq: 'Equalizer', scanlines: 'Scanlines', fin: 'Brand fin', meter: 'Meter',
    watch: 'Watch face', datewin: 'Date window', battery: 'Battery', ring: 'Ring', steps: 'Steps',
};

  /** Clock style presets — size = fraction of panel height (null = auto). */
  const CLOCK_STYLES: { label: string; size: number | null }[] = [
    { label: 'Strip', size: 0.2 },
    { label: 'Small', size: 0.3 },
    { label: 'Big', size: 0.5 },
    { label: 'Huge', size: 0.66 },
    { label: 'Auto', size: null },
  ];

  const EXTRA_LABELS: Record<string, string> = {
    hostbatt: 'Host battery',
    cpu: 'CPU load',
    ram: 'RAM',
    devinfo: 'Keyboard',
    uptime: 'Uptime',
  };
  const ALL_KINDS = [...Object.keys(KIND_LABEL), ...Object.keys(EXTRA_LABELS)];
  function labelOf(k: string): string {
    return KIND_LABEL[k as CompKind] ?? EXTRA_LABELS[k] ?? k;
  }

  function addComp(kind: CompKind) {
    const base: Comp = { kind, fg: '#f2f1f0' };
    if (kind === 'ticker') base.text = 'SharkDeck';
    if (kind === 'wave') { base.amp = 8; base.wlen = 40; base.speed = 1.2; }
    if (kind === 'rain') base.speed = 6;
    if (kind === 'eq') base.bars = 16;
    if (kind === 'meter') { base.text = 'Label'; base.value = 0.6; }
    if (kind === 'battery') { base.value = 0.8; base.batteryStyle = 'segments'; }
    if (kind === 'hostbatt') base.batteryStyle = 'segments';
    if (kind === 'ring') base.value = 0.6;
    if (kind === 'steps') { base.text = '8420'; }
    if (kind === 'watch') base.fg = '#e8eaf0';
    tpl = { ...tpl, comps: [...tpl.comps, base] };
    refresh();
  }

  function upd(i: number, patch: Partial<Comp>) {
    const comps = [...tpl.comps];
    comps[i] = { ...comps[i], ...patch };
    tpl = { ...tpl, comps };
    refresh();
  }

  function drop(i: number) {
    tpl = { ...tpl, comps: tpl.comps.filter((_, j) => j !== i) };
    refresh();
  }

  function shift(i: number, dir: number) {
    const j = i + dir;
    if (j < 0 || j >= tpl.comps.length) return;
    const comps = [...tpl.comps];
    [comps[i], comps[j]] = [comps[j], comps[i]];
    tpl = { ...tpl, comps };
    refresh();
  }

  // force a preview redraw after edits (input events also do)
  let nonce = $state(0);
  function refresh() { nonce++; }

  /* push to device */
  async function push() {
    if (!store.device) return store.toast('err', 'No device connected');
    busy = true;
    try {
      const [w, h] = wth();
      if (anim) {
        await screenAnimPush(renderFrames(tpl, w, h, Math.max(2, Math.min(90, frames))), w, h, cfg?.order);
        store.toast('ok', `Template animation pushed (${frames} frames)`);
      } else {
        await screenPush(renderStill(tpl, w, h), w, h, 'Stretch', cfg?.order, 10, slot);
        store.toast('ok', `Template pushed to slot ${slot + 1}`);
      }
    } catch (e) {
      store.toast('err', store.errText(e));
    } finally {
      busy = false;
    }
  }

  /* panel-size wizard: push a ruler/orientation pattern at a candidate size */
  async function wizard(kind: string, w: number, h: number) {
    if (!store.device) return store.toast('err', 'No device connected');
    wizBusy = true;
    try {
      const res = await screenPattern(kind, 0, w, h);
      wizPreview = res.preview;
      wizW = w;
      wizH = h;
      store.toast('ok', `Uploaded ${kind} ${w}×${h} (slot 1) — compare on the keyboard`);
    } catch (e) {
      store.toast('err', store.errText(e));
    } finally {
      wizBusy = false;
    }
  }

  async function saveGeom() {
    if (!cfg) return;
    try {
      await configSet(cfg);
      store.toast('ok', 'Panel geometry saved');
    } catch (e) {
      store.toast('err', store.errText(e));
    }
  }
</script>

<div class="page">
  <div class="page-head">
    <h2>Screen templates</h2>
    <p>Compose the keyboard screen from designed components — preview matches what lands on the panel.</p>
  </div>

  <div class="tpl-grid">
    <!-- gallery + preview -->
    <div class="col">
      <div class="panel">
        <h3>Templates</h3>
        <div class="gallery">
          {#each TEMPLATES as t (t.id)}
            <button class="tpl-card" class:sel={t.id === tpl.id} onclick={() => { tpl = { ...t, comps: t.comps.map((c) => ({ ...c })) }; refresh(); }}>
              <TemplateThumb tpl={t} w={240} h={135} />
              <span class="tpl-name">{t.name}</span>
              <span class="tpl-desc">{t.desc}</span>
            </button>
          {/each}
        </div>
      </div>

      <div class="panel">
        <h3>Preview <span class="muted">{previewW}×{previewH}</span></h3>
        <LiveMirror render={(f) => renderTemplate(tpl, previewW, previewH, f)} w={previewW} h={previewH} fps={tpl.dt >= 1 ? 1 : 2} label={anim ? `Live · ${frames} frames` : 'Live panel'} />
        <div class="row" style="margin-top:10px">
          <button class="btn primary" disabled={busy || !store.device} onclick={() => void push()}>
            {#if busy}<span class="spinner"></span>{/if}
            {anim ? `Push animation (${frames} frames)` : 'Push to slot'}
          </button>
          {#if !anim}
            <select bind:value={slot} class="slot-sel">
              <option value={0}>Slot 1</option>
              <option value={1}>Slot 2</option>
              <option value={2}>Slot 3</option>
              <option value={3}>Slot 4</option>
              <option value={4}>Slot 5</option>
            </select>
          {:else}
            <input class="num" type="number" min="2" max="90" bind:value={frames} />
            <span class="hint">frames</span>
          {/if}
        </div>
        <span class="hint"></span>
        {#if anim}
          <div class="hint">Animations replace the frame image the keyboard menu shows.</div>
        {:else}
          <div class="hint">Stills land in one of the five on-device slots.</div>
        {/if}
      </div>
    </div>

    <!-- composer -->
    <div class="col grow">
      {#if layouts.length}
        <div class="panel">
          <h3>Layout</h3>
          <div class="row" style="gap:6px; flex-wrap:wrap">
            {#each layouts as l (l.id)}
              <button class="chip" class:sel={l.id === tpl.layout} onclick={() => useLayout(l)}>{l.name}</button>
            {/each}
          </div>
          {#if lay}
            <div class="hint" style="margin-top:7px">
              {lay.desc} · clock at {Math.round(lay.clock.size * 100)}% in {SLOT_LABEL[lay.clock.slot]}
            </div>
          {/if}
          <div class="hint" style="margin-top:4px">
            Pick a layout, then move any widget onto one of the eight anchor positions below.
          </div>
        </div>
      {/if}
      <div class="panel">
        <h3>Components</h3>
        {#each tpl.comps as c, i (i)}
          <div class="comp-row">
            <span class="comp-kind">{labelOf(c.kind)}</span>
            <span class="chiprow" title="anchor position">
              {#each SLOT_IDS as s (s)}
                <button class="chip tiny" class:sel={(c.slot ?? 'tl') === s} onclick={() => setSlot(i, s)}>{SLOT_SHORT[s]}</button>
              {/each}
            </span>
            {#if c.kind === 'ticker' || c.kind === 'meter'}
              <input class="t" type="text" value={c.text ?? ''} oninput={(e) => upd(i, { text: e.currentTarget.value })} placeholder="text" />
            {/if}
            {#if c.kind === 'clock'}
              <label class="chk"><input type="checkbox" checked={c.seconds ?? false} onchange={(e) => upd(i, { seconds: e.currentTarget.checked })} /> seconds</label>
              <div class="row" style="gap:4px">
                {#each CLOCK_STYLES as s (s.label)}
                  <button class="chip" class:sel={(c.size ?? null) === s.size} onclick={() => upd(i, { size: s.size })}>{s.label}</button>
                {/each}
              </div>
            {/if}
            {#if c.kind === 'wave'}
              <label class="numlab">amp <input class="num" type="number" bind:value={c.amp} oninput={() => upd(i, { amp: c.amp ?? 8 })} /></label>
              <label class="numlab">len <input class="num" type="number" bind:value={c.wlen} oninput={() => upd(i, { wlen: c.wlen ?? 40 })} /></label>
            {/if}
            {#if c.kind === 'rain'}
              <label class="numlab">speed <input class="num" type="number" bind:value={c.speed} oninput={() => upd(i, { speed: c.speed ?? 6 })} /></label>
            {/if}
            {#if c.kind === 'eq'}
              <label class="numlab">bars <input class="num" type="number" bind:value={c.bars} oninput={() => upd(i, { bars: c.bars ?? 16 })} /></label>
            {/if}
            {#if c.kind === 'battery' || c.kind === 'hostbatt'}
              <span class="chiprow">
                <button class="chip" class:sel={(c.batteryStyle ?? 'segments') === 'segments'} onclick={() => upd(i, { batteryStyle: 'segments' })}>Segments</button>
                <button class="chip" class:sel={(c.batteryStyle ?? 'segments') === 'ios'} onclick={() => upd(i, { batteryStyle: 'ios' })}>iOS</button>
              </span>
            {/if}
            {#if c.kind === 'battery'}
              <label class="numlab">level
                <input class="num" type="number" min="0" max="100" value={Math.round((c.value ?? 0.82) * 100)} oninput={(e) => upd(i, { value: (e.currentTarget.valueAsNumber || 0) / 100 })} />
                %
              </label>
            {/if}
            {#if c.kind === 'meter'}
              <input class="num" type="number" min="0" max="1" step="0.05" bind:value={c.value} oninput={() => upd(i, { value: c.value ?? 0.6 })} />
            {/if}
            <input class="color" type="color" value={c.fg ?? '#ffffff'} oninput={(e) => upd(i, { fg: e.currentTarget.value })} />
            <button class="icon-btn" title="up" onclick={() => shift(i, -1)}>↑</button>
            <button class="icon-btn" title="down" onclick={() => shift(i, 1)}>↓</button>
            <button class="icon-btn danger" title="remove" onclick={() => drop(i)}>×</button>
          </div>
        {:else}
          <div class="hint">No components — add one below for a solid panel.</div>
        {/each}

        <div class="row add-row">
          <span class="hint">Add:</span>
          {#each ALL_KINDS as k (k)}
            <button class="chip" onclick={() => addComp(k as CompKind)}>{labelOf(k)}</button>
          {/each}
        </div>
      </div>

      <!-- geometry + wizard -->
      <div class="panel">
        <h3>Panel size campaign</h3>
        <p class="hint" style="margin:4px 0 8px">
          The K86 panel may be wider than a square. Push rulers at candidate sizes and compare where the image clips on the device.
        </p>
        <div class="row" style="gap:6px">
          {#each [['ruler', 240, 135], ['ruler', 128, 128], ['orientation', 240, 135], ['band', 240, 135]] as [kind, w, h] (kind + '' + w + h)}
            <button class="btn sm" disabled={wizBusy || !store.device} onclick={() => void wizard(kind as string, w as number, h as number)}>
              {kind === 'ruler' ? 'Ruler' : kind === 'orientation' ? 'Orientation' : 'Bands'} {w}×{h}
            </button>
          {/each}
        </div>
        <div class="row geom">
          {#if cfg}
            <label class="numlab">W <input class="num" type="number" bind:value={cfg.screen_width} oninput={() => void saveGeom()} /></label>
            <label class="numlab">H <input class="num" type="number" bind:value={cfg.screen_height} oninput={() => void saveGeom()} /></label>
            <label class="numlab">order
              <select bind:value={cfg.order} onchange={() => void saveGeom()}>
                <option value="Column">Column</option>
                <option value="Row">Row</option>
              </select>
            </label>
          {/if}
        </div>
        {#if wizPreview}
          <div style="margin-top:10px">
            <ScreenPreview preview={wizPreview} width={wizW} height={wizH} zoom={2} label={`last probe ${wizW}×${wizH}`} />
          </div>
        {/if}
        <div class="hint" style="margin-top:8px">
          If the ruler lands perfectly edge-to-edge with no dead border, that size + order is your panel.
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .tpl-grid {
    display: grid;
    grid-template-columns: 340px 1fr;
    gap: 12px;
    align-items: start;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .gallery {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }
  .tpl-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    background: var(--panel-2);
    border-radius: var(--radius);
    padding: 8px;
    cursor: pointer;
    text-align: center;
  box-shadow: var(--elev-1);
}
  .tpl-card:hover { background: var(--hover); }
  .tpl-card.sel {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-soft);
  }
  .tpl-card img {
    width: 120px;
    height: 67px;
    border-radius: 5px;
    display: block;
    image-rendering: pixelated;
}
  .tpl-name { font-size: 12.5px; font-weight: 600; color: var(--text); }
  .tpl-desc { font-size: 10.5px; color: var(--dim); }
  .comp-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 8px;
    background: var(--panel-2);
    margin-bottom: 6px;
    flex-wrap: wrap;
  box-shadow: var(--elev-1);
}
  .comp-kind {
    font-size: 12px;
    font-weight: 600;
    min-width: 64px;
  }
  .comp-row .t {
    flex: 1;
    min-width: 90px;
  }
  .add-row { margin-top: 8px; flex-wrap: wrap; }
  .color {
    width: 34px;
    height: 26px;
    padding: 1px;
    border-radius: 6px;
    background: var(--panel);
  box-shadow: var(--elev-1);
}
  .numlab {
    font-size: 11.5px;
    color: var(--dim);
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .icon-btn {
    background: var(--panel);
    border-radius: 6px;
    width: 24px;
    height: 24px;
    line-height: 1;
    color: var(--dim);
    cursor: pointer;
  box-shadow: var(--elev-1);
}
  .icon-btn:hover { background: var(--hover); color: var(--text); }
  .icon-btn.danger:hover { color: var(--err); border-color: var(--err);
}
  .chiprow {
    display: inline-flex;
    gap: 4px;
  }
  .chip.tiny {
    padding: 2px 5px;
    font-size: 9.5px;
    letter-spacing: 0.04em;
    min-width: 20px;
  }
  .geom { margin-top: 8px; }
  .slot-sel {
    padding: 7px 10px;
    border-radius: 8px;
    background: var(--panel);
  box-shadow: var(--elev-1);
}
</style>
