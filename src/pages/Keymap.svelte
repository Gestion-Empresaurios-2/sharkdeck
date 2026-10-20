<script lang="ts">
  import * as api from '../lib/tauri';
  import { toast, errText, connected } from '../lib/store.svelte';
  import type { Entry4, Layout } from '../lib/types';
  import { usageLabel, modsLabel, CONSUMER_CODES, FIRMWARE_ACTIONS as FWARE } from '../lib/types';
  import BindingEditor from '../components/BindingEditor.svelte';
  import { onMount } from 'svelte';

  const CANVAS_W = 840;
  const CANVAS_H = 280;

  let layout = $state<Layout | null>(null);
  let profile = $state(0);
  let fnLayer = $state(false);
  let bytes = $state<number[]>([]);
  let loading = $state(false);
  let selSlot = $state<number | null>(null);
  let wrapW = $state(700);

  function entry_at(slot: number): Entry4 {
    const i = slot * 4;
    if (i + 3 >= bytes.length) return [0, 0, 0, 0];
    return [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]];
  }

  function summary(e: Entry4): string {
    if (!e[0] && !e[1] && !e[2] && !e[3]) return '';
    if (e[0] === 0) {
      const m = modsLabel(e[1]);
      return (m ? m + '+' : '') + usageLabel(e[2]);
    }
    if (e[0] === 3) return mediaLabel(e[2] | (e[3] << 8));
    if (e[0] === 9) return `Macro ${e[2] + 1}`;
    if (e[0] === 10) return FWARE.find(([c]) => parseInt(c, 16) === e[1])?.[1] ?? `Act ${e[1]}`;
    return `[${e.join(',')}]`;
  }

  function mediaLabel(code: number): string {
    const c = code.toString(16).padStart(4, '0');
    const hit = CONSUMER_CODES.find(([cc]) => cc === c);
    return hit ? hit[1] : `Media 0x${c}`;
  }

  onMount(async () => {
    try {
      layout = await api.layoutGet();
    } catch (e) {
      toast('err', errText(e));
    }
    if (connected()) await reload();
    const el = document.querySelector('.kbd-wrap') as HTMLElement | null;
    if (el) {
      const ro = new ResizeObserver(() => (wrapW = el.clientWidth));
      ro.observe(el);
      wrapW = el.clientWidth;
    }
  });

  async function reload(full = false) {
    if (!connected()) return;
    loading = true;
    try {
      const blob = await api.keymapRead(profile, fnLayer);
      bytes = Array.from(blob);
      if (full) toast('ok', `Keymap read (${fnLayer ? 'Fn' : 'base'} layer, 128 slots)`);
    } catch (e) {
      toast('err', errText(e));
    } finally {
      loading = false;
    }
  }

  async function save(slot: number, entry: Entry4) {
    try {
      const res = await api.keymapWrite(profile, fnLayer, slot, entry);
      if (res.ok) toast('ok', `Slot ${slot} updated${fnLayer ? ' (Fn layer)' : ''}`);
      else toast('err', `Firmware refused (read back [${res.got?.join(',') ?? ''}]) — rolled back`);
      await reload();
    } catch (e) {
      toast('err', errText(e));
    }
  }

  async function backup() {
    if (!connected()) return;
    try {
      const path = await api.pickSaveFile(`sharkdeck-keymap-p${profile + 1}.json`);
      if (!path) return;
      await api.keymapBackup(path, profile);
      toast('ok', 'Backup written');
    } catch (e) {
      toast('err', errText(e));
    }
  }

  async function restore() {
    if (!connected()) return;
    try {
      const path = await api.pickOpenJson();
      if (!path) return;
      const n = await api.keymapRestore(path);
      toast('ok', `${n} slots restored`);
      await reload();
    } catch (e) {
      toast('err', errText(e));
    }
  }

  const scale = $derived(wrapW / CANVAS_W);
  const layoutKey = $derived(layout?.keys ?? []);
  const selected = $derived(selSlot === null ? null : entry_at(selSlot));
</script>

<div class="page">
  <div class="page-head">
    <h2>Keymap</h2>
    <p>
      Full 75% layout (factory matrix from the vendor tool). Click a key to re-bind its slot.
      Writes are read-verified — anything the firmware silently drops is rolled back from backup.
    </p>
  </div>

  <div class="row" style="margin-bottom:14px; flex-wrap:wrap">
    <span class="muted" style="font-size:12.5px">Profile</span>
    {#each [0, 1, 2] as p (p)}
      <button class="chip" class:sel={profile === p} onclick={() => ((profile = p), selSlot = null, reload(false))}>
        Profile {p + 1}
      </button>
    {/each}
    <span style="width:14px"></span>
    <span class="muted" style="font-size:12.5px">Layer</span>
    <button class="chip" class:sel={!fnLayer} onclick={() => ((fnLayer = false), selSlot = null, reload(false))}>
      Base
    </button>
    <button class="chip" class:sel={fnLayer} onclick={() => ((fnLayer = true), selSlot = null, reload(false))}>
      Fn
    </button>
    <span class="grow"></span>
    <button class="btn sm" onclick={() => reload(true)} disabled={!connected() || loading}>
      {#if loading}<span class="spinner"></span>{/if} Re-read
    </button>
    <button class="btn sm" onclick={backup} disabled={!connected()}>Backup JSON</button>
    <button class="btn sm" onclick={restore} disabled={!connected()}>Restore JSON</button>
  </div>

  {#if !connected()}
    <div class="panel" style="text-align:center; padding:40px">
      <p class="muted">No device connected — probe it in the sidebar first.</p>
    </div>
  {:else}
    <div class="panel">
      <h3>
        Layout
        <span class="grow"></span>
        {#if selected}
          <span class="mono muted" style="font-weight: 400; font-size: 11px">
            slot {selSlot} · {selected.join(',')}
          </span>
        {/if}
      </h3>
      <div class="kbd-wrap" style="height:{CANVAS_H * scale}px">
        <div class="kbd" style="width:{CANVAS_W}px; height:{CANVAS_H}px; transform: scale({scale}); transform-origin: top left">
          {#each layoutKey as k (k.slot)}
            <button
              class="key"
              style="left:{k.x}px; top:{k.y}px; width:{k.w}px; height:{k.h}px"
              class:sel={selSlot === k.slot}
              onclick={() => (selSlot = k.slot)}
            >
              <span class="kt">{k.text}</span>
              {#if fnLayer}
                <span class="kb">{summary(entry_at(k.slot)) || k.text}</span>
              {:else}
                <span class="kb">{summary(entry_at(k.slot))}</span>
              {/if}
            </button>
          {/each}
        </div>
      </div>
      <p class="hint" style="margin-top:12px">
        Bottom row on each keycap shows the current binding. Slot indicators cover keys and the
        knob columns; slots without a physical key live in Device → unused matrix.
      </p>
    </div>

    {#if selSlot !== null}
      <div class="panel editbox" style="margin-top:16px">
        <BindingEditor
          entry={selected}
          onSave={(e) => save(selSlot!, e)}
          onCancel={() => (selSlot = null)}
          title={`Slot ${selSlot}${layoutKey.find((k) => k.slot === selSlot)?.text ? ' · ' + layoutKey.find((k) => k.slot === selSlot)?.text : ''}`}
        />
      </div>
    {/if}
  {/if}
</div>

<style>
  .kbd-wrap {
    width: 100%;
    overflow: hidden;
  }
  .kbd {
    position: relative;
  }
  .key {
    position: absolute;
    background: rgba(255, 255, 255, 0.055);
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1px;
    padding: 1px;
    transition: all 0.12s;
    overflow: hidden;
  box-shadow: var(--elev-1);
}
  .key:hover {
    background: rgba(91, 140, 255, 0.14);
    border-color: rgba(91, 140, 255, 0.4);
  box-shadow: var(--elev-1);
}
  .key.sel {
    background: var(--grad);
    border-color: transparent;
  box-shadow: var(--elev-1);
}
  .kt {
    font-size: 12px;
    font-weight: 620;
    line-height: 1.1;
    white-space: nowrap;
  }
  .kb {
    font-size: 9.5px;
    color: var(--text-dim);
    line-height: 1.2;
    white-space: nowrap;
    overflow: hidden;
    max-width: 100%;
  }
  .key.sel .kb {
    color: rgba(255, 255, 255, 0.8);
  }
  .editbox {
    border-color: rgba(91, 140, 255, 0.35);
}
</style>
