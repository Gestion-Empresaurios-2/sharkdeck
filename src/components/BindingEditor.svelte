<script lang="ts">
  import {
    FIRMWARE_ACTIONS,
    HID_CODES,
    MODS,
    CONSUMER_CODES,
    hexCode,
    usageLabel,
  } from '../lib/types';
  import type { Entry4 } from '../lib/types';

  let {
    entry,
    onSave,
    onCancel,
    title = 'Binding',
  }: {
    entry: Entry4 | null;
    onSave: (e: Entry4) => Promise<void> | void;
    onCancel?: () => void;
    title?: string;
  } = $props();

  type Kind = 'none' | 'key' | 'media' | 'firmware' | 'macro' | 'unknown';
  let kind = $state<Kind>('none');
  let usage = $state(0x29);
  let mods = $state(0);
  let media = $state('00e2');
  let fwAction = $state('09');
  let macroIdx = $state(0);
  let macroMode = $state(0);
  let saving = $state(false);

  $effect(() => {
    // re-seed editor state whenever the target entry changes
    const e = entry;
    if (!e) {
      kind = 'none';
      return;
    }
    const [a, b, c] = e;
    if (a === 0 && !b && !c) kind = 'none';
    else if (a === 0) {
      kind = 'key';
      mods = b;
      usage = c;
    } else if (a === 3) {
      kind = 'media';
      media = hexCode((c & 0xff) | (b << 8), false);
    } else if (a === 9) {
      kind = 'macro';
      macroMode = b;
      macroIdx = c;
    } else if (a === 10) {
      kind = 'firmware';
      fwAction = b.toString(16).padStart(2, '0');
    } else {
      kind = 'unknown';
    }
  });

  function buildEntry(): Entry4 {
    switch (kind) {
      case 'key':
        return [0, mods, usage, 0];
      case 'media': {
        const code = parseInt(media, 16);
        return [3, 0, code & 0xff, (code >> 8) & 0xff];
      }
      case 'firmware': {
        const act = parseInt(fwAction, 16);
        return [10, act, 0, 0];
      }
      case 'macro':
        return [9, macroMode, macroIdx, 0];
      default:
        return [0, 0, 0, 0];
    }
  }

  async function save() {
    saving = true;
    try {
      await onSave(buildEntry());
    } finally {
      saving = false;
    }
  }

  const kinds: [Kind, string][] = [
    ['none', 'Unassigned'],
    ['key', 'Key'],
    ['media', 'Media'],
    ['firmware', 'Device'],
    ['macro', 'Macro'],
  ];
</script>

<div class="editor">
  <div class="head">
    <h4>{title}</h4>
    {#if entry}
      <span class="mono raw">[{entry.map((x) => x.toString(16).padStart(2, '0')).join(', ')}]</span>
    {/if}
  </div>

  <div class="kinds">
    {#each kinds as [k, label] (k)}
      <button class="chip" class:sel={kind === k} onclick={() => (kind = k)}>{label}</button>
    {/each}
  </div>

  {#if kind === 'unknown'}
    <p class="hint">
      Unknown encoding — this slot holds firmware-native data the app does not model. Editing will
      overwrite it.
    </p>
  {:else if kind === 'key'}
    <div class="fields">
      <select bind:value={usage} style="flex:1">
        {#each HID_CODES as [code, label] (code)}
          <option value={code}>{label}</option>
        {/each}
      </select>
      <div class="mods">
        {#each MODS as m (m.bit)}
          {@const on = !!(mods & m.bit)}
          <button
            class="chip"
            class:sel={on}
            onclick={() =>
              (mods = on ? mods & ~m.bit : mods | m.bit)}>{m.side === 'left' ? 'L' : 'R'}​{m.a}</button
          >
        {/each}
      </div>
    </div>
  {:else if kind === 'media'}
    <div class="fields">
      <select bind:value={media}>
        {#each CONSUMER_CODES as [code, label] (code)}
          <option value={code}>{label}</option>
        {/each}
      </select>
    </div>
  {:else if kind === 'firmware'}
    <div class="fields">
      <select bind:value={fwAction}>
        {#each FIRMWARE_ACTIONS as [code, label] (code)}
          <option value={code}>{label}</option>
        {/each}
      </select>
    </div>
  {:else if kind === 'macro'}
    <div class="fields">
      <select bind:value={macroMode}>
        <option value={0}>Repeat while held</option>
        <option value={1}>Toggle</option>
        <option value={2}>Play once (hold)</option>
      </select>
      <select bind:value={macroIdx}>
        {#each Array(50) as _, i (i)}
          <option value={i}>Macro {i + 1}</option>
        {/each}
      </select>
    </div>
  {:else}
    <p class="hint">Leave unassigned.</p>
  {/if}

  <div class="row actions">
    <button class="btn primary sm" onclick={save} disabled={saving}>
      {#if saving}<span class="spinner"></span>{/if} Apply
    </button>
    {#if onCancel}
      <button class="btn sm" onclick={onCancel}>Cancel</button>
    {/if}
  </div>
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
  }
  h4 {
    font-size: 14px;
    font-weight: 650;
  }
  .raw {
    font-size: 10.5px;
    color: var(--text-faint);
  }
  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: 7px;
  }
  .fields {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .mods {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .actions {
    margin-top: 2px;
  }
</style>
