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
  import { onMount } from 'svelte';

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

  // Native knob: the firmware exposes the knob as a mouse-like HID device,
  // so the kernel gives us a relative-axis input node we can listen to.
  let knobNode = $state<api.InputDevice | null>(null);
  let knobSummary = $state<api.KnobSummary | null>(null);
  let knobError = $state('');
  let knobMode = $state('');
  let knobRaw = $state<string[]>([]);
  let listening = $state(false);

  async function loadKnob() {
    try {
      const r = await api.knobDevices();
      knobNode = r.knob;
    } catch (e) {
      knobError = errText(e);
    }
  }

  async function listenKnob() {
    listening = true;
    knobError = '';
    knobRaw = [];
    try {
      const r = await api.knobEvents(5);
      knobSummary = r.summary;
      knobMode = r.mode ?? 'evdev';
      knobRaw = r.hidraw ?? [];
      if (r.note) knobError = r.note;
    } catch (e) {
      knobError = errText(e);
    } finally {
      listening = false;
    }
  }

  onMount(loadKnob);

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
    <h3>Native knob</h3>
    {#if knobNode}
      <p class="hint">
        The keyboard's own firmware reports the knob as a mouse-like HID device — the kernel
        exposes it as
        <span class="mono">{knobNode.path}</span> ({knobNode.name}). Turn or press the knob
        while listening to see its raw movement.
      </p>
      <div class="row" style="margin: 10px 0">
        <button class="btn" onclick={listenKnob} disabled={listening}>
          {listening ? 'Listening…' : 'Listen 5 s'}
        </button>
      </div>
      {#if knobSummary}
        <div class="kv-grid">
          <div class="kv"><span>Turn left</span><b>{knobSummary.left}</b></div>
          <div class="kv"><span>Turn right</span><b>{knobSummary.right}</b></div>
          <div class="kv"><span>Wheel</span><b>{knobSummary.wheel}</b></div>
          <div class="kv"><span>Presses</span><b>{knobSummary.presses}</b></div>
        </div>
        <p class="hint">Axes seen: {knobSummary.axes.join(', ') || '—'}</p>
      {/if}
      {#if knobMode}
        <p class="hint">
          Source: <span class="mono">{knobMode}</span>{#if knobMode === 'hidraw'}
          — raw HID capture (no 'input' group needed; decode pending){/if}
        </p>
      {/if}
      {#if knobRaw.length}
        <pre class="mono raw-log">{knobRaw.join('\n')}</pre>
      {/if}
    {:else}
      <p class="hint">No knob input node found on this machine.</p>
    {/if}
    {#if knobError}
      <p class="hint err-text">{knobError}</p>
    {/if}
  </div>

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
    background: var(--blue-tint);
    display: grid;
    place-items: center;
    font-size: 18px;
    color: var(--blue);
  }
  .name {
    font-weight: 650;
    font-size: 15px;
  }
  .current {
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--surface-3);
    font-size: 13px;
    color: var(--graphite);
  }
  .editbox {
    margin-bottom: 16px;
    outline: 1px solid var(--blue);
  }
  .kv-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 10px;
  }
  .err-text {
    color: var(--err);
  }

  .raw-log {
    margin: 8px 0 0;
    padding: 10px;
    background: var(--surface-2);
    border-radius: 8px;
    font-size: 11px;
    line-height: 1.5;
    max-height: 140px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }
</style>
