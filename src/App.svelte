<script lang="ts">
  import { store, applyTheme, applyAccent, ACCENTS } from './lib/store.svelte';
  import Os from './pages/Os.svelte';
  import Screen from './pages/Screen.svelte';
  import Templates from './pages/Templates.svelte';
  import Knob from './pages/Knob.svelte';
  import Lighting from './pages/Lighting.svelte';
  import PerKey from './pages/PerKey.svelte';
  import Keymap from './pages/Keymap.svelte';
  import Macros from './pages/Macros.svelte';
  import Native from './pages/Native.svelte';
  import Settings from './pages/Settings.svelte';
  import { onMount } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  // Null outside Tauri (e.g. when previewed in a plain browser).
  let win: ReturnType<typeof getCurrentWindow> | null = null;
  try {
    win = getCurrentWindow();
  } catch {
    win = null;
  }

  let version = $state('0.2.5');
  let deviceOpen = $state(false);

  // Native-style window drag. The webview's own `data-tauri-drag-region`
  // handles the markup, but we also drive the drag ourselves so the bar always
  // moves the window (the bare attribute only reacts to direct hits on the
  // element, not on the text inside it).
  function dragBar(e: MouseEvent) {
    if (!win || e.button !== 0) return;
    // double click = maximise (handled by Tauri's own drag script)
    if (e.detail > 1) return;
    const t = e.target as HTMLElement | null;
    if (
      t?.closest(
        'button, a, input, select, textarea, .win-controls, .accent-swatches, .status, .segmented, .device-pop',
      )
    )
      return;
    e.preventDefault();
    e.stopPropagation();
    void win.startDragging().catch(() => {});
  }

  const PAGES = [
    { id: 'os', label: 'Screen OS', badge: 'LIVE' },
    { id: 'screen', label: 'Screen' },
    { id: 'templates', label: 'Templates' },
    { id: 'knob', label: 'Knob' },
    { id: 'lighting', label: 'Lighting' },
    { id: 'perkey', label: 'Per-key RGB' },
    { id: 'native', label: 'Native OS' },
    { id: 'keymap', label: 'Keymap' },
    { id: 'macros', label: 'Macros' },
    { id: 'settings', label: 'Device' },
  ];

  /** Close the device popover when clicking outside of it. */
  function onWindowClick(e: MouseEvent) {
    if (!deviceOpen) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest('.device-pop, .status')) return;
    deviceOpen = false;
  }

  function onWindowKey(e: KeyboardEvent) {
    if (e.key === 'Escape') deviceOpen = false;
  }

  onMount(() => {
    void getVersion().then((v) => (version = v)).catch(() => {});
    applyAccent(store.accent);
    // Keep the DOM in sync with the persisted preference (light is the default).
    applyTheme(store.theme);
    void store.probeDevice();
  });
</script>

<svelte:window onclick={onWindowClick} onkeydown={onWindowKey} />

<div class="app">
  <!-- ============================ fixed 64px top navigation bar ========= -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="topbar" data-tauri-drag-region="deep" onmousedown={dragBar}>
    <!-- left: brand + accent swatches -->
    <div class="topbar-left" data-tauri-drag-region="deep">
      <div class="brand" data-tauri-drag-region="deep">
        <div class="brand-mark" data-tauri-drag-region="deep" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="14" height="14">
            <path
              d="M13.4 1.2 C 11.4 6.4 6.8 12.4 3.6 21.2 C 8 21.4 14.6 21.2 19.4 20.6 C 18.9 13 16.8 5.6 13.4 1.2 Z"
              fill="currentColor"
            />
          </svg>
        </div>
        <span class="brand-word" data-tauri-drag-region="deep">SharkDeck</span>
      </div>

      <span class="hairline" aria-hidden="true"></span>

      <div class="accent-swatches" role="group" aria-label="Accent colour">
        {#each ACCENTS as a (a.key)}
          <button
            class="accent-dot"
            class:sel={store.accent === a.key}
            style={`--sw:${a.color}`}
            title={a.name}
            aria-label={`Accent ${a.name}`}
            onclick={() => applyAccent(a.key)}
          ></button>
        {/each}
      </div>
    </div>

    <!-- centre: muted nav links -->
    <nav class="topbar-nav" aria-label="Sections">
      <div class="nav-track">
        {#each PAGES as p (p.id)}
          <button
            class="nav-btn"
            class:active={store.page === p.id}
            aria-current={store.page === p.id ? 'page' : undefined}
            title={p.label}
            onclick={() => (store.page = p.id)}
          >
            {p.label}
          </button>
        {/each}
      </div>
    </nav>

    <!-- right: device status, colour scheme, native window controls -->
    <div class="topbar-right">
      <button
        class="status"
        class:on={!!store.device}
        onclick={() => (deviceOpen = !deviceOpen)}
        title={store.device ? `${store.device.path} · fw ${store.device.firmware}` : 'No device — click for details'}
        aria-expanded={deviceOpen}
      >
        <span class="status-led" aria-hidden="true"></span>
        <span class="status-label">K86</span>
      </button>

      <span class="hairline" aria-hidden="true"></span>

      <div class="segmented" role="group" aria-label="Colour scheme">
        <button class:active={store.theme === 'light'} onclick={() => applyTheme('light')} title="Light">Light</button>
        <button class:active={store.theme === 'dark'} onclick={() => applyTheme('dark')} title="Dark">Dark</button>
      </div>

      <span class="hairline" aria-hidden="true"></span>

      <div class="win-controls" role="group" aria-label="Window controls">
        <button title="Minimise" aria-label="Minimise" onclick={() => win?.minimize()}><svg viewBox="0 0 12 12" width="12" height="12"><path d="M2 6h8" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" /></svg></button>
        <button title="Maximise" aria-label="Maximise" onclick={() => win?.toggleMaximize()}><svg viewBox="0 0 12 12" width="12" height="12"><rect x="2.5" y="2.5" width="7" height="7" rx="1" fill="none" stroke="currentColor" stroke-width="1.2" /></svg></button>
        <button class="close" title="Close" aria-label="Close" onclick={() => win?.close()}><svg viewBox="0 0 12 12" width="12" height="12"><path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" /></svg></button>
      </div>
    </div>

    <!-- device popover -->
    {#if deviceOpen}
      <div class="device-pop" role="dialog" aria-label="Device">
        <h4>Attack Shark K86</h4>
        <p class="pop-sub">
          {store.device
            ? store.device.error
              ? `Unreadable · ${store.device.error}`
              : `Connected · firmware ${store.device.firmware ?? '—'}`
            : 'No device via USB cable'}
        </p>
        <div class="kv"><span class="k">Path</span><span class="v">{store.device?.path ?? '—'}</span></div>
        <div class="kv"><span class="k">Firmware</span><span class="v">{store.device?.firmware ?? '—'}</span></div>
        <div class="kv"><span class="k">Profile</span><span class="v">{store.device?.profile != null ? `${Number(store.device.profile) + 1} / 3` : '—'}</span></div>
        <div class="row" style="margin-top: 16px">
          <button class="btn tint" disabled={store.busy} onclick={() => void store.probeDevice()}>
            {#if store.busy}<span class="spinner"></span>{/if}
            {store.busy ? 'Probing…' : 'Probe device'}
          </button>
          <span class="hint">SharkDeck v{version}</span>
        </div>
      </div>
    {/if}
  </header>

  <!-- ================================ page branches ====================== -->
  <main>
    {#if store.page === 'os'}
      <Os />
    {:else if store.page === 'screen'}
      <Screen />
    {:else if store.page === 'templates'}
      <Templates />
    {:else if store.page === 'knob'}
      <Knob />
    {:else if store.page === 'lighting'}
      <Lighting />
    {:else if store.page === 'perkey'}
      <PerKey />
    {:else if store.page === 'native'}
      <Native />
    {:else if store.page === 'keymap'}
      <Keymap />
    {:else if store.page === 'macros'}
      <Macros />
    {:else}
      <Settings />
    {/if}
  </main>

  <div
    class="resize-grip"
    role="presentation"
    onmousedown={(e) => {
      e.preventDefault();
      void win?.startResizeDragging('se');
    }}
  ></div>

  <div class="toasts">
    {#each store.toasts as t (t.id)}
      <div class="toast {t.kind}">{t.text}</div>
    {/each}
  </div>
</div>
