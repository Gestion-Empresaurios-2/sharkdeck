<script lang="ts">
  import * as api from '../lib/tauri';
  import { toast, errText, connected } from '../lib/store.svelte';
  import type { Entry4 } from '../lib/types';
  import { HID_CODES, usageLabel } from '../lib/types';

  // Blob layout (shark-k86-mac / keyboard-studio):
  // repeat: u16 LE at 0..2, then events until 4 zero bytes:
  //   key/mouse-button: delay<=127: [code, pressed<<7|delay]      (2 bytes)
  //                     else:       [code, pressed?0x80:0, dL, dH] (4 bytes)
  //   mouse move:        [0xF9, delayByte, dx i8, dy i8]          (4 bytes)

  type Step = {
    kind: 'key' | 'mouse' | 'move';
    code: number;
    pressed: boolean;
    delay: number;
    dx: number;
    dy: number;
  };

  let slot = $state(0);
  let repeat = $state(1);
  let steps = $state<Step[]>([]);
  let busy = $state(false);

  // add-step form
  let newKind = $state<'key' | 'mouse' | 'move'>('key');
  let newCode = $state(0x29);
  let newPressed = $state(true);
  let newDelay = $state(50);
  let newDx = $state(0);
  let newDy = $state(0);

  async function load(quiet = false) {
    if (!connected()) return;
    busy = true;
    try {
      const blob = await api.macroRead(slot);
      const [r, parsed] = parseBlob(new Uint8Array(blob));
      repeat = r;
      steps = parsed;
      if (!quiet) toast('info', `Macro ${slot + 1}: ${steps.length} steps, repeat ×${repeat}`);
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  function parseBlob(b: Uint8Array): [number, Step[]] {
    const repeat = b[0] | (b[1] << 8);
    const steps: Step[] = [];
    let i = 2;
    const isZero = (p: number) =>
      b[p] === 0 && b[p + 1] === 0 && b[p + 2] === 0 && b[p + 3] === 0;
    while (i + 2 <= b.byteLength && !isZero(i)) {
      const code = b[i];
      if (code === 0xf9) {
        // mouse move: F9, delay, dx, dy
      steps.push({ kind: 'move', code: 0, pressed: false, delay: b[i + 1], dx: b[i + 2], dy: b[i + 3] });
        i += 4;
        continue;
      }
      const flag = b[i + 1];
      const pressed = (flag & 0x80) !== 0;
      if ((flag & 0x7f) !== 0) {
        // short form: delay lives in the low 7 bits
        steps.push({
          kind: code >= 1 && code <= 5 ? 'mouse' : 'key',
          code,
          pressed,
          delay: flag & 0x7f,
          dx: 0,
          dy: 0,
        });
        i += 2;
        continue;
      }
      const dl = b[i + 2];
      const dh = b[i + 3];
      const delay = dl | (dh << 8);
      steps.push({ kind: code >= 1 && code <= 5 ? 'mouse' : 'key', code, pressed, delay, dx: 0, dy: 0 });
      i += 4;
    }
    return [repeat, steps];
  }

  function buildBlob(): number[] {
    const out: number[] = [];
    const put = (...xs: number[]) => xs.forEach((x) => out.push(x & 0xff));
    put(repeat & 0xff, (repeat >> 8) & 0xff);
    for (const s of steps) {
      if (s.kind === 'move') {
        put(0xf9, s.delay, s.dx, s.dy);
        continue;
      }
      if (s.delay > 0 && s.delay <= 127) {
        put(s.code, (s.pressed ? 0x80 : 0) | s.delay);
        continue;
      }
      put(s.code, s.pressed ? 0x80 : 0, s.delay & 0xff, (s.delay >> 8) & 0xff);
    }
    put(0, 0, 0, 0);
    return out;
  }

  async function save() {
    if (!connected()) return;
    busy = true;
    try {
      const blob = buildBlob();
      if (blob.length > 256) {
        return toast('err', `Sequence too long (${blob.length}/256 bytes)`);
      }
      await api.macroWrite(slot, blob);
      toast('ok', `Macro ${slot + 1} written (${blob.length} bytes)`);
      await load(true);
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = false;
    }
  }

  function addStep() {
    if (newKind === 'move') {
      steps.push({ kind: 'move', code: 0, pressed: false, delay: 0, dx: newDx, dy: newDy });
    } else {
      steps.push({
        kind: newKind,
        code: newPressed ? newCode : newCode,
        pressed: newPressed,
        delay: newDelay,
        dx: 0,
        dy: 0,
      });
    }
  }

  function stepLabel(s: Step): string {
    if (s.kind === 'move') return `mouse move (${s.dx}, ${s.dy})`;
    if (s.kind === 'mouse') return `mouse btn ${s.code} ${s.pressed ? 'down' : 'up'}`;
    const lbl = usageLabel(s.code);
    return `${lbl} ${s.pressed ? '↓' : '↑'} · ${s.delay}ms`;
  }
</script>

<div class="page">
  <div class="page-head">
    <h2>Macros</h2>
    <p>
      50 on-device sequences stored in the keyboard itself — they keep running with the app closed
      (work on any OS once bound).
    </p>
  </div>

  <div class="row" style="margin-bottom:14px; flex-wrap:wrap">
    <span class="muted" style="font-size:12.5px">Slot</span>
    {#each Array(50) as _, i (i)}
      <button class="chip" class:sel={slot === i} onclick={() => ((slot = i), load(true))}>
        {i + 1}
      </button>
    {/each}
  </div>

  <div class="cols">
    <div class="panel">
      <h3>Sequence · Macro {slot + 1}</h3>
      <div class="row" style="margin-bottom:12px">
        <span class="muted" style="font-size:12.5px">Repeat</span>
        <input type="number" min={0} max={65535} bind:value={repeat} style="width:80px" />
        <span class="hint">(0 = run forever)</span>
        <span class="grow"></span>
        <button class="btn sm" onclick={() => load(true)} disabled={!connected() || busy}>Read</button>
        <button class="btn primary sm" onclick={save} disabled={!connected() || busy}>
          {#if busy}<span class="spinner"></span>{/if} Write
        </button>
      </div>

      {#if steps.length === 0}
        <div class="empty">
          Empty — add steps on the right, or press Read to pull the stored sequence.
        </div>
      {:else}
        <div class="steps">
          {#each steps as s, i (i)}
            <div class="step">
              <span class="idx mono">{i + 1}</span>
              <span class="desc">{stepLabel(s)}</span>
              <button class="x" onclick={() => steps.splice(i, 1)}>×</button>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <div class="panel">
      <h3>Add step</h3>
      <div class="row" style="flex-wrap:wrap; margin-bottom:12px">
        {#each [['key', 'Key'], ['mouse', 'Mouse button'], ['move', 'Mouse move']] as [k, label] (k)}
          <button
            class="chip"
            class:sel={newKind === k}
            onclick={() => (newKind = k as 'key' | 'mouse' | 'move')}
          >
            {label}
          </button>
        {/each}
      </div>

      {#if newKind === 'key'}
        <select bind:value={newCode}>
          {#each HID_CODES as [c, label] (c)}
            <option value={c}>{label}</option>
          {/each}
        </select>
        <div class="row" style="margin-top:12px">
          <button class="chip" class:sel={newPressed} onclick={() => (newPressed = true)}>press ↓</button>
          <button class="chip" class:sel={!newPressed} onclick={() => (newPressed = false)}>release ↑</button>
        </div>
      {:else if newKind === 'mouse'}
        <select bind:value={newCode}>
          {#each [1, 2, 3, 4, 5] as b (b)}
            <option value={b}>Button {b}</option>
          {/each}
        </select>
        <div class="row" style="margin-top:12px">
          <button class="chip" class:sel={newPressed} onclick={() => (newPressed = true)}>press ↓</button>
          <button class="chip" class:sel={!newPressed} onclick={() => (newPressed = false)}>release ↑</button>
        </div>
      {:else}
        <div class="row">
          <span class="muted" style="font-size:12.5px">dx</span>
          <input type="number" min={-127} max={127} bind:value={newDx} style="width:70px" />
          <span class="muted" style="font-size:12.5px">dy</span>
          <input type="number" min={-127} max={127} bind:value={newDy} style="width:70px" />
        </div>
      {/if}

      {#if newKind !== 'move'}
        <div class="row" style="margin-top:12px">
          <span class="muted" style="font-size:12.5px">delay</span>
          <input type="number" min={0} max={65535} bind:value={newDelay} style="width:80px" />
          <span class="hint">ms</span>
        </div>
      {/if}
      <button class="btn tint" style="margin-top:14px; width:100%" onclick={addStep}>
        Add step
      </button>
      <p class="hint" style="margin-top:12px">
        Delays over 127ms use the long 4-byte event form automatically. Mouse moves are
        relative, clamped to ±127.
      </p>
    </div>
  </div>

  {#if slot !== null}
    <div class="panel" style="margin-top:16px">
      <h3>Bind macro to a key</h3>
      <p class="hint">
        Use the Keymap page: choose <b>Macro</b> in the binding editor on any key or knob slot.
        Current macro index for this slot is <b>{slot + 1}</b>.
      </p>
    </div>
  {/if}
</div>

<style>
  .cols {
    display: grid;
    grid-template-columns: 1fr 320px;
    gap: 16px;
    align-items: start;
  }
  .empty {
    padding: 28px;
    text-align: center;
    color: var(--text-faint);
    border-radius: 10px;
    font-size: 13px;
}
  .steps {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 380px;
    overflow-y: auto;
  }
  .step {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: 9px;
    background: rgba(255, 255, 255, 0.03);
  box-shadow: var(--elev-1);
}
  .idx {
    color: var(--text-faint);
    font-size: 10.5px;
    width: 18px;
  }
  .desc {
    font-size: 13px;
    flex: 1;
  }
  .x {
    color: var(--text-faint);
    font-size: 15px;
    padding: 2px 6px;
    border-radius: 6px;
}
  .x:hover {
    color: var(--err);
    background: rgba(248, 113, 113, 0.1);
  }
  @media (max-width: 900px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
</style>
