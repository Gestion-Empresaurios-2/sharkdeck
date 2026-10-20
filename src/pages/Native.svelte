<script lang="ts">
  import { onMount } from 'svelte';
  import * as api from '../lib/tauri';
  import { store, toast, errText, connected } from '../lib/store.svelte.ts';
  import type { KbOptions, LightingState } from '../lib/types';

  let profile = $state(0);
  let options = $state<KbOptions | null>(null);
  let light = $state<LightingState | null>(null);
  let busy = $state(false);
  let probeRows = $state<api.OpcodeRow[]>([]);
  let fullHex = $state<{ op: number; hex: string } | null>(null);

  async function runProbe() {
    busy = true;
    try {
      const res = await api.opcodeProbe();
      probeRows = res.rows;
      toast('ok', `${res.rows.length} opcodes probed`);
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  async function showFull(op: number) {
    try {
      fullHex = await api.opcodeFull(op);
    } catch (e) {
      toast('err', errText(e));
    }
  }

  async function backupSettings() {
    busy = true;
    try {
      const stamp = new Date().toISOString().slice(0, 10);
      const path = await api.pickSaveFile(`sharkdeck-settings-${stamp}.json`);
      if (!path) return;
      const written = await api.settingsBackup(path);
      toast('ok', `Settings backed up · ${written.split('/').pop()}`);
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  async function restoreSettings() {
    busy = true;
    try {
      const path = await api.pickOpenJson();
      if (!path) return;
      const report = await api.settingsRestore(path);
      toast('ok', `Restored — ${report}`);
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

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

  async function refresh() {
    try {
      profile = await api.profileGet();
    } catch (e) {
      /* not fatal */
    }
    try {
      options = await api.optionsGet();
      if (options) store.options = options;
    } catch (e) {
      /* not fatal */
    }
    try {
      light = await api.lightGet(false);
    } catch (e) {
      /* not fatal */
    }
  }

  onMount(refresh);

  async function setProfile(p: number) {
    if (busy) return;
    busy = true;
    try {
      await api.profileSet(p);
      profile = p;
      toast('ok', `Profile ${p + 1} active`);
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  async function toggleOption(key: keyof KbOptions, label: string) {
    if (!options || busy) return;
    busy = true;
    const next: KbOptions = { ...options, [key]: options[key] ? 0 : 1 } as KbOptions;
    try {
      await api.optionsSet(next);
      options = next;
      store.options = next;
      toast('ok', `${label} ${next[key] ? 'on' : 'off'}`);
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  async function setEffect(effect: number) {
    if (!light || busy) return;
    busy = true;
    const next: LightingState = { ...light, effect };
    try {
      await api.lightSet(false, next);
      light = next;
      toast('ok', `Lighting → ${EFFECTS.find((x) => x[0] === effect)?.[1] ?? effect}`);
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  async function setBrightness(value: number) {
    if (!light || busy) return;
    busy = true;
    const next: LightingState = { ...light, brightness: value };
    try {
      await api.lightSet(false, next);
      light = next;
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  function goto(page: string) {
    store.page = page as never;
  }

  const NATIVE_ROWS = [
    { id: 'knob', title: 'Knob & shortcuts', desc: 'Turn left / turn right / press — rebind to media, keys, firmware actions or macros.', cta: 'Open Knob' },
    { id: 'keymap', title: 'Keymap & Fn layer', desc: 'All 128 slots across 3 profiles, per-key overrides, JSON backup.', cta: 'Open Keymap' },
    { id: 'perkey', title: 'Per-key RGB', desc: 'Paint the board per key (flash write, rate limited, replug on stall).', cta: 'Open Per-key RGB' },
    { id: 'macros', title: 'Macros', desc: '50 macro slots with the on-device sequence format.', cta: 'Open Macros' },
  ];
</script>

<div class="page">
  <header class="page-head">
    <div>
      <p class="eyebrow">FIRMWARE &amp; HARDWARE CONTROL</p>
      <h2>Native OS</h2>
      <p>
        Everything the keyboard's <strong>own firmware</strong> exposes. The panel content is emulated by the
        host; these controls are the device's real registers.
      </p>
    </div>
    <div class="row">
      <button class="btn sm" onclick={() => void refresh()}>Read state</button>
      <button class="btn sm primary" onclick={() => goto('os')}>Open Screen OS</button>
    </div>
  </header>

  <div class="grid two">
    <section class="panel">
      <h3>Device</h3>
      <div class="kv"><span>Model</span><b>Attack Shark K86</b></div>
      <div class="kv"><span>Handshake ID</span><b>1168</b></div>
      <div class="kv"><span>Firmware</span><b>{store.device?.firmware ?? '—'}</b></div>
      <div class="kv"><span>Interface</span><b>{store.device?.path ?? 'not connected'}</b></div>
      <div class="kv"><span>Panel</span><b>240 × 135 RGB565</b></div>
      <div class="kv"><span>Profiles</span><b>3</b></div>
      <p class="hint">
        The keyboard runs a closed firmware with a framebuffer player. SharkDeck talks to it over HID feature
        reports; the OS itself is stored in the board's flash.
      </p>
    </section>

    <section class="panel">
      <h3>Profile</h3>
      <div class="row">
        {#each [0, 1, 2] as p}
          <button class="chip" class:sel={profile === p} onclick={() => void setProfile(p)} disabled={busy}>
            {p + 1}
          </button>
        {/each}
      </div>
      <p class="hint">Profile stores the keymap and Fn layer. Lighting, debounce and sleep are shared.</p>

      <h3 style="margin-top:20px">Hardware options</h3>
      <div class="row">
        <button class="chip" class:sel={!!options?.win_lock} onclick={() => void toggleOption('win_lock', 'Win lock')} disabled={busy}>Win lock</button>
        <button class="chip" class:sel={!!options?.mac_mode} onclick={() => void toggleOption('mac_mode', 'Mac mode')} disabled={busy}>Mac mode</button>
        <button class="chip" class:sel={!!options?.wasd_swap} onclick={() => void toggleOption('wasd_swap', 'WASD swap')} disabled={busy}>WASD swap</button>
        <button class="chip" class:sel={!!options?.keyboard_mode} onclick={() => void toggleOption('keyboard_mode', 'Keyboard mode')} disabled={busy}>Keyboard mode</button>
        <button class="chip" class:sel={!!options?.keyboard_lock} onclick={() => void toggleOption('keyboard_lock', 'Keyboard lock')} disabled={busy}>Keyboard lock</button>
      </div>
    </section>

    <section class="panel">
      <h3>Lighting (native)</h3>
      <div class="row wrap">
        {#each EFFECTS as [id, name]}
          <button class="chip tiny" class:sel={light?.effect === id} onclick={() => void setEffect(id)} disabled={busy}>
            {name}
          </button>
        {/each}
      </div>
      <div class="row" style="margin-top:12px">
        <span class="hint" style="width:64px">Brightness</span>
        {#each [0, 1, 2, 3, 4] as b}
          <button class="chip tiny" class:sel={light?.brightness === b} onclick={() => void setBrightness(b)} disabled={busy}>{b}</button>
        {/each}
      </div>
      <p class="hint">Written through the keyboard's own lighting registers (0x07 / 0x08).</p>
    </section>

    <section class="panel">
      <h3>Native surfaces</h3>
      {#each NATIVE_ROWS as r}
        <div class="row between native-row">
          <div>
            <b>{r.title}</b>
            <p class="hint">{r.desc}</p>
          </div>
          <button class="btn sm" onclick={() => goto(r.id)}>{r.cta}</button>
        </div>
      {/each}
    </section>

    <section class="panel">
      <h3>Panel content (emulated)</h3>
      <p class="hint">
        The keyboard's screen has no host-side read-back and no scripting: it can only replay frames that were
        pushed to it. SharkDeck therefore renders the panel on the host and streams frames, which is what the
        Screen OS and Templates pages do.
      </p>
      <div class="row">
        <button class="btn sm" onclick={() => goto('os')}>Screen OS</button>
        <button class="btn sm" onclick={() => goto('templates')}>Templates</button>
        <button class="btn sm" onclick={() => goto('screen')}>Screen</button>
      </div>
      <p class="hint" style="margin-top:12px">
        Five image slots live on the device; which one is shown is chosen by the keyboard's own menu (the knob),
        not by the host.
      </p>
    </section>

    <section class="panel">
      <h3>Backup &amp; restore</h3>
      <p class="hint">
        The keyboard's flash never reads back over USB — no opcode in the whole GET family (0x80–0x9F) returns
        firmware bytes, and the boot loader is write-and-verify only. Everything that <em>is</em> readable is
        backed up here: the keymap and Fn layer of all three profiles, all 50 macro slots, the option bytes and
        both lighting states.
      </p>
      <div class="row">
        <button class="btn sm" onclick={backupSettings} disabled={busy}>Backup settings…</button>
        <button class="btn sm" onclick={restoreSettings} disabled={busy}>Restore settings…</button>
      </div>
      <p class="hint" style="margin-top:12px">
        Restore writes only the slots that differ and verifies every keymap write by read-back. Destructive
        commands (flash erase, display erase, boot entry, factory reset) are deliberately never exposed.
        The reverse-engineering programme for a real firmware image lives in <code>re/PLAN.md</code>.
      </p>
    </section>

    <section class="panel">
      <h3>Opcode lab</h3>
      <p class="hint">
        Read-only reconnaissance of the keyboard's command set. Each opcode is probed against a
        known-good baseline (device id <code>0x8F</code> → 1168): a reply that merely repeats the
        baseline means the firmware is <em>not</em> implementing that opcode. Destructive opcodes are
        blocked in the transport layer and are never sent.
      </p>
      <div class="row">
        <button class="btn sm" onclick={runProbe} disabled={busy || !connected()}>
          Scan 0x80–0x9F
        </button>
        <span class="hint">{probeRows.length ? `${probeRows.length} opcodes probed` : 'not scanned yet'}</span>
      </div>
      {#if probeRows.length}
        <div class="op-grid">
          {#each probeRows as r}
            <button
              class="op-row"
              class:live={r.alive && r.verdict.startsWith('implemented')}
              class:blocked={r.verdict.startsWith('blocked')}
              onclick={() => showFull(r.op)}
              title="click for the full 64-byte reply"
            >
              <span class="mono">0x{r.op.toString(16).toUpperCase().padStart(2, '0')}</span>
              <span class="op-verdict">{r.verdict}</span>
              {#if r.head}<span class="mono op-head">{r.head}</span>{/if}
            </button>
          {/each}
        </div>
      {/if}
      {#if fullHex}
        <p class="hint" style="margin-top:10px">
          <span class="mono">0x{fullHex.op.toString(16).toUpperCase().padStart(2, '0')}</span>
          <span class="mono op-head">{fullHex.hex}</span>
        </p>
      {/if}
    </section>
  </div>
</div>

<style>

  .op-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 4px;
    margin-top: 12px;
  }
  .op-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    text-align: left;
    padding: 6px 8px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--ink-60, #757575);
    font-size: 12px;
    cursor: pointer;
  }
  .op-row:hover {
    background: var(--surface-hover, rgba(0, 0, 0, 0.04));
  }
  .op-row.live {
    color: var(--ink, #000);
  }
  .op-row.live .op-verdict {
    color: var(--ok, #0f7b6c);
  }
  .op-row.blocked {
    color: var(--err, #d44c47);
  }
  .op-verdict {
    flex: 1;
    font-size: 12px;
  }
  .op-head {
    font-size: 10.5px;
    color: var(--ink-40, rgba(0, 0, 0, 0.4));
    letter-spacing: 0.02em;
  }

  .grid.two {
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    align-items: start;
  }
  .row.wrap {
    flex-wrap: wrap;
  }
  .row.between {
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    padding: 10px 0;
    border-top: 1px solid var(--line, rgba(0, 0, 0, 0.08));
  }
  .native-row:first-of-type {
    border-top: none;
  }
  .native-row b {
    display: block;
    margin-bottom: 2px;
  }
  .eyebrow {
    font-size: 11px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--stone, #757575);
    margin: 0 0 6px;
  }
  code {
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 12px;
  }
</style>
