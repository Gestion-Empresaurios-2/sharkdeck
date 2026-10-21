<script lang="ts">
  import * as api from '../lib/tauri';
  import { store, setAutopush, toast, errText, probeDevice, connected } from '../lib/store.svelte';
  import type { KbOptions } from '../lib/types';
  import { onMount } from 'svelte';

  const UDEV = `SUBSYSTEM=="hidraw", ATTRS{idVendor}=="3151", ATTRS{idProduct}=="4015", MODE="0660", TAG+="uaccess"`;

  const OPTS: { key: keyof KbOptions; label: string; hint: string }[] = [
    { key: 'win_lock', label: 'Win key locked', hint: 'Option bit 0x01 — logo key ignored' },
    { key: 'mac_mode', label: 'Mac mode', hint: 'Option bit 0x02 — swapping modifiers' },
    { key: 'wasd_swap', label: 'WASD swap', hint: 'Option bit 0x08' },
    { key: 'keyboard_mode', label: 'Keyboard-mode flag', hint: 'Option bit 0x40' },
    { key: 'keyboard_lock', label: 'Keyboard locked', hint: 'Option bit 0x80 — whole keyboard frozen' },
  ];

  let opts = $state<KbOptions | null>(null);
  let display = $state<{ width: number | null; height: number | null; brightness?: number } | null>(null);
  let logs = $state<string[]>([]);
  let busy = $state('');
  let copied = $state(false);
  let host = $state<api.HostInfo | null>(null);
  let kbBattery = $state<number | null>(null);
  let appVer = $state('');
  let updateInfo = $state<api.UpdateInfo | null>(null);
  let updBusy = $state(false);
  let dlPct = $state(-1);

  async function doCheckUpdate() {
    updBusy = true;
    try {
      appVer = await api.appVersion();
      updateInfo = await api.checkForUpdate();
      toast(
        updateInfo ? 'ok' : 'info',
        updateInfo ? `Update ${updateInfo.version} available` : 'You are on the latest version',
      );
    } catch (e) {
      toast('err', errText(e));
    } finally {
      updBusy = false;
    }
  }

  async function doInstallUpdate() {
    updBusy = true;
    dlPct = 0;
    try {
      await api.installUpdate((p) => (dlPct = p));
      toast('ok', 'Update installed — restarting SharkDeck');
      await api.restartApp();
    } catch (e) {
      toast('err', errText(e));
      updBusy = false;
      dlPct = -1;
    }
  }

  onMount(() => {
    syncOpts();
    api.appVersion().then((v) => (appVer = v)).catch(() => {});
    if (connected()) syncLogs();
    const t = setInterval(syncLogs, 2500);
    refreshHost();
    const h = setInterval(refreshHost, 2000);
    return () => {
      clearInterval(t);
      clearInterval(h);
    };
  });

  async function refreshHost() {
    try {
      host = await api.hostInfo();
    } catch {
      /* host metrics unavailable */
    }
    try {
      kbBattery = await api.batteryProbe();
    } catch {
      kbBattery = null;
    }
  }

  function uptimeText(s: number): string {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    return h > 0 ? `${h}h ${m}m` : `${m}m`;
  }

  function syncOpts() {
    opts =
      (store.device?.options as KbOptions | undefined) ??
      (store.options as KbOptions | undefined) ??
      null;
  }

  async function syncLogs() {
    try {
      logs = await api.logsGet();
    } catch {
      /* left */
    }
  }

  async function probe() {
    await probeDevice();
    syncOpts();
  }

  async function disconnect() {
    try {
      await api.disconnect();
      store.device = null;
      opts = null;
      toast('info', 'Disconnected');
    } catch (e) {
      toast('err', errText(e));
    }
  }

  async function saveOpts() {
    if (!opts) return;
    busy = 'opts';
    try {
      await api.optionsSet(opts);
      toast('ok', 'Option bytes written');
      store.device = { ...(store.device ?? {}), options: opts };
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = '';
    }
  }

  async function probeDisplay() {
    if (!connected()) return toast('err', 'Probe the device first');
    busy = 'display';
    try {
      display = await api.displayParams();
      if (display && display.width) toast('ok', `Panel: ${display.width}×${display.height}`);
      else toast('warn' as 'info', 'Panel read returned no plausible values');
    } catch (e) {
      toast('err', errText(e));
    } finally {
      busy = '';
    }
  }

  function copyUdev() {
    navigator.clipboard?.writeText(UDEV).then(
      () => {
        copied = true;
        setTimeout(() => (copied = false), 1500);
      },
      () => toast('err', 'Clipboard blocked by the webview'),
    );
  }
</script>

<div class="page">
  <div class="page-head">
    <h2>Device</h2>
    <p>Identity, protocol inspection and app preferences.</p>
  </div>

  <div class="cols">
    <div class="grid" style="gap:16px">
      <div class="panel">
        <h3>Identity</h3>
        {#if store.device && !store.device.error}
          <div class="kv"><span class="k">Hidraw path</span><span class="v">{store.device.path}</span></div>
          <div class="kv"><span class="k">Handshake id</span><span class="v">{store.device.handshakeId} <span class="muted">(K86 → 1168)</span></span></div>
          <div class="kv"><span class="k">Firmware</span><span class="v">{store.device.firmware}</span></div>
          <div class="kv"><span class="k">Profile</span><span class="v">{Number(store.device.profile) + 1} / 3</span></div>
        {:else}
          <p class="hint">Nothing connected yet.</p>
        {/if}
        <div class="row" style="margin-top:14px">
          <button class="btn primary" onclick={probe} disabled={store.busy}>
            {#if store.busy}<span class="spinner"></span>{/if} Probe
          </button>
          <button class="btn" onclick={disconnect} disabled={!connected()}>Disconnect</button>
        </div>
      </div>

      <div class="panel">
        <h3>Automatic pushes</h3>
        <label class="switch-row">
          <input type="checkbox" bind:checked={store.autopush} onchange={(e) => setAutopush((e.currentTarget as HTMLInputElement).checked)} />
          <span>Let SharkDeck refresh the keyboard screen on its own</span>
        </label>
        <p class="hint">
          {#if store.autopush}
            On: the FRAME OS screen and the clock are re-pushed on a timer, so the
            keyboard's display is continuously overwritten.
          {:else}
            Off (recommended): the keyboard keeps whatever it is showing. SharkDeck
            only writes to the panel when you press <em>Push</em>, upload an image or
            use the knob.
          {/if}
        </p>
      </div>

      <div class="panel">
        <h3>Updates</h3>
        <div class="kv"><span class="k">Installed version</span><span class="v">{appVer || '—'}</span></div>
        {#if updateInfo}
          <div class="kv"><span class="k">Available</span><span class="v">{updateInfo.version}</span></div>
          {#if updateInfo.date}
            <div class="kv"><span class="k">Released</span><span class="v">{updateInfo.date}</span></div>
          {/if}
          {#if updateInfo.notes}
            <pre class="notes">{updateInfo.notes}</pre>
          {/if}
        {/if}
        <div class="row" style="margin-top:12px">
          <button class="btn" onclick={doCheckUpdate} disabled={updBusy}>
            {#if updBusy}<span class="spinner"></span>{/if} Check for updates
          </button>
          {#if updateInfo}
            <button class="btn tint" onclick={doInstallUpdate} disabled={updBusy}>
              {dlPct >= 0 ? `Downloading ${Math.round(dlPct * 100)}%` : 'Install & restart'}
            </button>
          {/if}
        </div>
        <p class="hint" style="margin-top:8px">
          Updates are signed and served from the project's GitHub release feed. A release feed and signing
          key must be published before an update can appear here.
        </p>
      </div>

      <div class="panel">
        <h3>Desk &amp; keyboard</h3>
        <div class="kv"><span class="k">Host</span><span class="v">{host?.hostname ?? '—'}</span></div>
        <div class="kv">
          <span class="k">Host battery</span>
          <span class="v">{host?.battery != null ? `${host.battery}%${host.charging ? ' charging' : ''}` : '—'}</span>
        </div>
        <div class="kv"><span class="k">CPU load</span><span class="v">{host ? `${host.cpu.toFixed(0)}%` : '—'}</span></div>
        <div class="kv"><span class="k">Memory used</span><span class="v">{host ? `${host.ram.toFixed(0)}%` : '—'}</span></div>
        <div class="kv"><span class="k">Host uptime</span><span class="v">{host ? uptimeText(host.uptime) : '—'}</span></div>
        <div class="kv">
          <span class="k">Keyboard battery</span>
          <span class="v">{kbBattery != null ? `${kbBattery}%` : 'not reported'}</span>
        </div>
        <p class="hint" style="margin-top:8px">
          The K86 has no battery read command — it only answers while USB-powered, so this stays empty on a wired board.
          The Host battery / CPU / RAM / Uptime widgets can be pushed onto the screen from the Templates page.
        </p>
      </div>

      <div class="panel">
        <h3>Display calibration</h3>
        <p class="hint">
          Press the button, quickly watch the panel and compare against the firmware's answer:
        </p>
        <div class="row" style="margin-top:12px">
          <button class="btn" onclick={() => (store.page = 'screen')}>Patterns page →</button>
          <button class="btn" onclick={probeDisplay} disabled={busy === 'display'}>
            {#if busy === 'display'}<span class="spinner"></span>{/if} Read panel params (0x0E)
          </button>
          {#if display}
            <span class="mono" style="font-size:12px">
              {display.width ?? '?'}×{display.height ?? '?'}
              {display.brightness != null ? ` · brightness ${display.brightness}` : ''}
            </span>
          {/if}
        </div>
        <p class="hint" style="margin-top:10px">
          The sheared/orientation pattern page is under Screen → Built-in patterns.
        </p>
      </div>

      <div class="panel">
        <h3>udev rule (Linux)</h3>
        <p class="hint">No sudo UI hacks — a uaccess tag gives your seat direct HID access:</p>
        <pre class="mono">{UDEV}</pre>
        <div class="row" style="margin-top:10px">
          <button class="btn sm" onclick={copyUdev}>{copied ? 'Copied ✓' : 'Copy'}</button>
          <span class="hint">→ /etc/udev/rules.d/99-sharkdeck.rules, then `udevadm control --reload && udevadm trigger`</span>
        </div>
      </div>
    </div>

    <div class="grid" style="gap:16px">
      <div class="panel">
        <h3>Option bytes</h3>
        {#if opts}
          {#each OPTS as o (o.key)}
            <label class="opt">
              <input type="checkbox" bind:checked={opts[o.key]} />
              <div>
                <div>{o.label}</div>
                <div class="hint">{o.hint}</div>
              </div>
            </label>
          {/each}
          <div class="kv" style="margin-top:8px">
            <span class="k">fn_matrix flag</span><span class="v">{opts.fn_matrix}</span>
          </div>
          <div class="kv">
            <span class="k">power_save</span><span class="v">{opts.power_save}</span>
          </div>
          <button class="btn tint" style="margin-top:14px" onclick={saveOpts} disabled={busy === 'opts'}>
            {#if busy === 'opts'}<span class="spinner"></span>{/if} Write options
          </button>
        {:else}
          <p class="hint">Probe the device to read option bits.</p>
        {/if}
      </div>

      <div class="panel">
        <h3>Log</h3>
        <div class="log mono">
          {#each logs as l, i (i)}
            <div>{l}</div>
          {:else}
            <div class="muted">empty</div>
          {/each}
        </div>
      </div>

      <div class="panel">
        <h3>About SharkDeck</h3>
        <p class="hint" style="line-height:1.65">
          Open-source control panel for the Attack Shark K86 (3151:4015) — hardware-verified
          protocol thanks to these MIT projects:<br />
          · <b>RaphaelCaputo2/shark-k86-mac</b> · <b>mehmetnadir/keyboard-studio</b> ·
          <b>dniminenn/sharkfin</b> (dnim.dev)<br /><br />
          <b>Safety by design:</b> destructive vendor opcodes (flash erase 0xAC, display erase
          0x2C, boot entry 0x7F/0x30, factory reset 0x02) are never wired — the app only touches
          keymap, options, lighting, macros and image slots. Writes are read-verified with
          automatic rollback.
        </p>
      </div>
    </div>
  </div>
</div>

<style>
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
    gap: 16px;
    align-items: start;
  }
  pre.mono {
    background: rgba(0, 0, 0, 0.3);
    border-radius: 9px;
    padding: 10px 12px;
    font-size: 11.5px;
    color: #9fe3b0;
    white-space: pre-wrap;
    user-select: all;
  box-shadow: var(--elev-1);
}
  .log {
    max-height: 220px;
    overflow-y: auto;
    font-size: 11px;
    color: var(--text-dim);
    background: rgba(0, 0, 0, 0.25);
    border-radius: 9px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column-reverse;
    gap: 3px;
    user-select: all;
  box-shadow: var(--elev-1);
}
  .opt {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    padding: 9px 0;
    font-size: 13px;
  }
  .notes {
    margin: 8px 0 0;
    padding: 8px 10px;
    background: var(--panel-2);
    border-radius: 8px;
    font-family: 'Ubuntu Mono', 'DejaVu Sans Mono', monospace;
    font-size: 11.5px;
    color: var(--dim);
    white-space: pre-wrap;
    max-height: 120px;
    overflow: auto;
  box-shadow: var(--elev-1);
}
  .opt:last-of-type {
    border-bottom: none;
}
  .opt input {
    margin-top: 2px;
  }
</style>
