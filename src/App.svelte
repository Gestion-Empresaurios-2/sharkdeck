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

  let version = $state('0.2.3');

  const PAGES = [
    { id: 'os', label: 'Screen OS', badge: 'LIVE' },
    { id: 'screen', label: 'Screen' },
    { id: 'templates', label: 'Templates' },
    { id: 'knob', label: 'Knob' },
    { id: 'lighting', label: 'Lighting' },
    { id: 'perkey', label: 'Per-key RGB' },
    { id: 'keymap', label: 'Keymap' },
    { id: 'macros', label: 'Macros' },
    { id: 'settings', label: 'Device' },
  ];

  const ICONS: Record<string, string> = {
    os: 'M4 4h7v7H4z M13 4h7v5h-7z M4 13h7v7H4z M13 11h7v9h-7z',
    screen: 'M2 3h12v8H2z M6 14h4 M8 11v3',
    templates: 'M2 2h5v5H2z M9 2h5v5H9z M2 9h5v5H2z M9 9h5v5H9z',
    knob: 'M8 2a6 6 0 016 6 6 6 0 01-6 6 6 6 0 01-6-6 6 6 0 016-6z M8 8m-2 0a2 2 0 104 0 2 2 0 10-4 0',
    lighting: 'M8 1v2 M8 13v2 M2 8h2 M12 8h2 M4 4l1.4 1.4 M10.6 10.6L12 12 M12 4l-1.4 1.4 M5.4 10.6L4 12 M8 5a3 3 0 100 6 3 3 0 000-6',
    perkey: 'M2 4h12v8H2z M4.5 6.5h2 M9.5 6.5h2 M4.5 9.5h2 M9.5 9.5h2',
    keymap: 'M1.5 4h13v8h-13z M4 6.5h1 M7 6.5h1 M10 6.5h1 M12 6.5h1 M5 9.5h6',
    macros: 'M3 2h10v12H3z M5.5 5h5 M5.5 8h5 M5.5 11h3',
    settings: 'M8 5a3 3 0 103 3 3 3 0 00-3-3z M13.6 9.5l1.8-1.5-1.8-1.5.3-2.3-2.2-.7L10.4 2 8.2 2.9 6 2l-1.3 1.5-2.2.7.3 2.3L1 8l1.8 1.5-.3 2.3 2.2.7L6 14l2.2-.9 2.2.9 1.3-1.5 2.2-.7z',
  };

  const current = () => PAGES.find((p) => p.id === store.page) ?? PAGES[0];

  onMount(() => {
    void getVersion().then((v) => (version = v)).catch(() => {});
    applyAccent(store.accent);
    void store.probeDevice();
  });
</script>

<div class="app">
  <aside>
    <div class="brand">
      <div class="brand-tile">
        <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true">
          <path
            d="M13.4 1.2 C 11.4 6.4 6.8 12.4 3.6 21.2 C 8 21.4 14.6 21.2 19.4 20.6 C 18.9 13 16.8 5.6 13.4 1.2 Z"
            fill="currentColor"
          />
        </svg>
      </div>
      <div class="brand-name">
        frame
        <span>SharkDeck / Studio</span>
      </div>
    </div>

    <div class="kicker">Workspace</div>

    <nav class="nav-items">
      {#each PAGES as p (p.id)}
        <button class="nav-btn" class:active={store.page === p.id} onclick={() => (store.page = p.id)}>
          <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
            <path d={ICONS[p.id]} fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          {p.label}
          {#if p.badge}<span class="nav-badge">{p.badge}</span>{/if}
        </button>
      {/each}
    </nav>

    <div class="aside-foot">
      <div class="connection-card">
        <div class="connection-top">
          <span class="connection-name">Attack Shark K86</span>
          <span class="live-dot" class:off={!store.device}></span>
        </div>
        <div class="connection-sub">
          {store.device ? `Connected · fw ${store.device.firmware}` : 'No device via USB cable'}
          <br />
          <span class="mono">{store.device?.path ?? '—'}</span>
        </div>
        <button class="btn sm" style="width: 100%; margin-top: 10px" disabled={store.busy} onclick={() => void store.probeDevice()}>
          {store.busy ? 'Probing…' : 'Probe device'}
        </button>
      </div>
      <div class="side-foot">
        FRAME OS / SHARKDECK<br />
        v{version} · 240 × 135 PANEL
      </div>
    </div>
  </aside>

  <div class="shell">
    <header class="topbar" data-tauri-drag-region>
      <div class="dot-grip" aria-hidden="true"><i></i><i></i><i></i></div>
      <div class="crumb" data-tauri-drag-region>
        Workspace<span class="slash">/</span><strong>{current().label}</strong>
      </div>
      <span class="topbar-title" data-tauri-drag-region>SharkDeck · Attack Shark K86</span>
      <div class="topbar-actions">
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
        <button
          class="status"
          class:on={!!store.device}
          onclick={() => void store.probeDevice()}
          title={store.device ? `${store.device.path} · fw ${store.device.firmware}` : 'No device — click to probe'}
        >
          <span class="status-led" aria-hidden="true"></span>
          <span class="status-label">{store.device ? 'K86 · Online' : 'No device'}</span>
        </button>
        <div class="segmented" role="group" aria-label="Colour scheme">
          <button class:active={store.theme === 'light'} onclick={() => applyTheme('light')} title="Light">Light</button>
          <button class:active={store.theme === 'dark'} onclick={() => applyTheme('dark')} title="Dark">Dark</button>
        </div>
        <div class="win-controls" role="group" aria-label="Window controls">
          <button title="Minimise" aria-label="Minimise" onclick={() => win.minimize()}><svg viewBox="0 0 12 12" width="12" height="12"><path d="M2 6h8" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" /></svg></button>
          <button title="Maximise" aria-label="Maximise" onclick={() => win.toggleMaximize()}><svg viewBox="0 0 12 12" width="12" height="12"><rect x="2.5" y="2.5" width="7" height="7" rx="1" fill="none" stroke="currentColor" stroke-width="1.2" /></svg></button>
          <button class="close" title="Close" aria-label="Close" onclick={() => win.close()}><svg viewBox="0 0 12 12" width="12" height="12"><path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" /></svg></button>
        </div>
      </div>
    </header>

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
      {:else if store.page === 'keymap'}
        <Keymap />
      {:else if store.page === 'macros'}
        <Macros />
      {:else}
        <Settings />
      {/if}
    </main>
  </div>

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
