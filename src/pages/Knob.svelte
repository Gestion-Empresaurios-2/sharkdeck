<script lang="ts">
  import * as api from '../lib/tauri';
  import { toast, errText, connected } from '../lib/store.svelte';
  import {
    CONSUMER_CODES,
    FIRMWARE_ACTIONS,
    usageLabel,
  } from '../lib/types';
  import type { Entry4 } from '../lib/types';
  import BindingEditor from '../components/BindingEditor.svelte';

  // Hardware-verified on K86 fw 1.19 (read-back of the factory layer):
  // turn-left = slot 96 (consumer 0xEA vol-down), turn-right = slot 97
  // (0xE9 vol-up), press = slot 78 (0xE2 mute — "back among the function
  // keys", exactly as keyboard-studio's Keymap.swift comments). We still
  // read everything back and show reality, never assume.
  const KNOB_CARDS: { slot: number; label: string; hint: string }[] = [
    { slot: 96, label: 'Turn left ↺', hint: 'Factory: volume down' },
    { slot: 97, label: 'Turn right ↻', hint: 'Factory: volume up' },
    { slot: 78, label: 'Press ⊙', hint: 'Factory: mute' },
  ];

  let profile = $state(0);
  let entries = $state<Record<number, Entry4 | null>>({ 96: null, 97: null, 78: null });
  let editing = $state<{ slot: number; entry: Entry4 } | null>(null);
  let loading = $state(false);

  // extra candidate slots that appear in real firmware dumps
  const EXTRA_SLOTS = [88, 89, 90, 91, 92, 93, 94, 95, 99, 100, 101, 102, 103];

  async function readSlot(slot: number, tryIndex = 0): Promise<Entry4 | null> {
    const target = tryIndex === 0 ? slot : EXTRA_SLOTS[tryIndex - 1];
    if (target == null) return null;
    try {
      const blob = await api.keymapRead(profile, false);
      const e = [blob[target * 4], blob[target * 4 + 1], blob[target * 4 + 2], blob[target * 4 + 3]] as Entry4;
      return e;
    } catch (e) {
      if (tryIndex === 0) toast('err', errText(e));
      return null;
    }
  }

  async function refresh() {
    if (!connected()) return;
    loading = true;
    try {
      for (const card of KNOB_CARDS) {
        const e = await readSlot(card.slot, 0);
        entries[card.slot] = e;
      }
    } finally {
      loading = false;
    }
  }

  async function save(slot: number, entry: Entry4) {
    try {
      const res = await api.keymapWrite(profile, false, slot, entry);
      if (res.ok) toast('ok', `Slot ${slot} updated`);
      else toast('err', `Firmware refused (got [${res.got?.join(',') ?? ''}]) — rolled back`);
      await refresh();
      editing = null;
    } catch (e) {
      toast('err', errText(e));
    }
  }

  function label(e: Entry4 | null): string {
    if (!e || (!e[0] && !e[1] && !e[2] && !e[3])) return 'Unassigned';
    if (e[0] === 0) {
      const parts = [];
      if (e[1]) {
        if (e[1] & 0x01) parts.push('LCtrl');
        if (e[1] & 0x02) parts.push('LShift');
        if (e[1] & 0x04) parts.push('LAlt');
        if (e[1] & 0x08) parts.push('LGui');
        if (e[1] & 0x10) parts.push('RCtrl');
        if (e[1] & 0x20) parts.push('RShift');
        if (e[1] & 0x40) parts.push('RAlt');
        if (e[1] & 0x80) parts.push('RGui');
      }
      parts.push(usageLabel(e[2]));
      return parts.join(' + ');
    }
    if (e[0] === 3) {
      const code = e[2] | (e[3] << 8);
      const hit = CONSUMER_CODES.find(([c]) => c === hex4(code));
      return hit ? hit[1] : `Media 0x${hex4(code)}`;
    }
    if (e[0] === 9) return `Macro ${e[2] + 1}`;
    if (e[0] === 10) {
      const hit = FIRMWARE_ACTIONS.find(
        ([c]) => c === (e[1] < 16 ? '0' + e[1].toString(16) : e[1].toString(16)),
      );
      return hit ? hit[1] : `Action ${e[1]}`;
    }
    return `Raw [${e.join(',')}]`;
  }

  function hex4(n: number) {
    return n.toString(16).padStart(4, '0');
  }

</script>

<div class="page">
  <div class="page-head">
    <h2>Knob shortcuts</h2>
    <p>
      Re-bind the rotary knob beyond volume: media, key chords, firmware device actions (including
      the hidden <b>wheel swap</b> that remaps turns to brightness) and macros. Reads are verified
      from the device's base-layer keymap in profile {profile + 1}.
    </p>
  </div>

  <div class="row" style="margin-bottom:16px">
    <span class="muted" style="font-size:12.5px">Profile</span>
    {#each [0, 1, 2] as p (p)}
      <button class="chip" class:sel={profile === p} onclick={() => ((profile = p), refresh())}>
        Profile {p + 1}
      </button>
    {/each}
    <span class="grow"></span>
    <button class="btn sm" onclick={refresh} disabled={!connected() || loading}>
      {#if loading}<span class="spinner"></span>{/if} Re-read
    </button>
  </div>

  <div class="cards">
    {#each KNOB_CARDS as card (card.slot)}
      <div class="panel card">
        <div class="card-head">
          <div class="icon">{card.label.split(' ')[1] ?? ''}</div>
          <div>
            <div class="name">{card.label.replace(/[^A-Za-z ]/g, '').trim()}</div>
            <div class="hint">{card.hint}</div>
          </div>
        </div>
        <div class="current">
          {#if entries[card.slot] === null}
            <span class="muted">slot {card.slot}: unreadable</span>
          {:else}
            <span class="mono" style="font-size:12px">slot {card.slot} ·</span>
            <span>{label(entries[card.slot])}</span>
          {/if}
        </div>
        <button
          class="btn sm"
          onclick={() => (editing = { slot: card.slot, entry: entries[card.slot] ?? [0, 0, 0, 0] })}
          disabled={!connected()}
        >
          Edit binding
        </button>
      </div>
    {/each}
  </div>

  {#if editing}
    <div class="panel editbox">
      <BindingEditor
        entry={editing.entry}
        onSave={(e) => save(editing!.slot, e)}
        onCancel={() => (editing = null)}
        title={`Slot ${editing.slot} → ${KNOB_CARDS.find((c) => c.slot === editing?.slot)?.label ?? ''}`}
      />
    </div>
  {/if}

  <div class="panel">
    <h3>Why the knob can do more</h3>
    <ul class="hint">
      <li>
        <b>Wheel swap (action 14)</b> remaps turns from volume to brightness at firmware level.
      </li>
      <li>Bind turns to media keys or macro index for custom behaviour.</li>
      <li>
        Encoders send indistinguishable step sequences — only left/right slots differ.
      </li>
    </ul>
  </div>
</div>

<style>
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 14px;
    margin-bottom: 16px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .card-head {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .icon {
    width: 42px;
    height: 42px;
    border-radius: 12px;
    background: var(--grad);
    display: grid;
    place-items: center;
    font-size: 18px;
    color: #fff;
  }
  .name {
    font-weight: 650;
    font-size: 15px;
  }
  .current {
    padding: 10px 12px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--border);
    font-size: 13px;
  }
  .editbox {
    margin-bottom: 16px;
    border-color: rgba(91, 140, 255, 0.35);
  }
</style>
