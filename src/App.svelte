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
  import { PhysicalSize } from '@tauri-apps/api/dpi';
  import { windowPresentation } from './lib/tauri';

  // Null outside Tauri (e.g. when previewed in a plain browser).
  let win: ReturnType<typeof getCurrentWindow> | null = null;
  try {
    win = getCurrentWindow();
  } catch {
    win = null;
  }

  let version = $state('0.3.1');
  let deviceOpen = $state(false);
  let maximized = $state(false);
  let fullscreen = $state(false);

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
    { id: 'os', label: 'Screen OS', badge: 'LIVE', icon: 'os' },
    { id: 'screen', label: 'Screen', icon: 'screen' },
    { id: 'templates', label: 'Templates', icon: 'grid' },
    { id: 'knob', label: 'Knob', icon: 'dial' },
    { id: 'lighting', label: 'Lighting', icon: 'sun' },
    { id: 'perkey', label: 'Per-key RGB', icon: 'pixels' },
    { id: 'native', label: 'Native OS', icon: 'chip' },
    { id: 'keymap', label: 'Keymap', icon: 'keys' },
    { id: 'macros', label: 'Macros', icon: 'list' },
    { id: 'settings', label: 'Device', icon: 'device' },
  ];

  /** 16×16 stroke icons for the sidebar. */
  const ICONS: Record<string, string> = {
    os: 'M3 4.5h10a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1H3zM6.2 6.8v3.4l2.8-1.7z',
    screen: 'M2.5 3.5h11v7h-11zM6 13h4M8 10.5V13',
    grid: 'M2.5 2.5h4.2v4.2H2.5zM9.3 2.5h4.2v4.2H9.3zM2.5 9.3h4.2v4.2H2.5zM9.3 9.3h4.2v4.2H9.3z',
    dial: 'M8 2.6a5.4 5.4 0 1 0 0 10.8A5.4 5.4 0 0 0 8 2.6zM8 5v3.2l2.2 1.3',
    sun: 'M8 5.2a2.8 2.8 0 1 0 0 5.6 2.8 2.8 0 0 0 0-5.6zM8 1.6v1.4M8 13v1.4M1.6 8h1.4M13 8h1.4M3.5 3.5l1 1M11.5 11.5l1 1M12.5 3.5l-1 1M4.5 11.5l-1 1',
    pixels: 'M2.5 2.5h3v3h-3zM6.5 2.5h3v3h-3zM10.5 2.5h3v3h-3zM2.5 6.5h3v3h-3zM6.5 6.5h3v3h-3zM10.5 6.5h3v3h-3zM2.5 10.5h3v3h-3zM6.5 10.5h3v3h-3zM10.5 10.5h3v3h-3z',
    chip: 'M5 5h6v6H5zM6.5 2.5v2.5M9.5 2.5v2.5M6.5 11v2.5M9.5 11v2.5M2.5 6.5H5M2.5 9.5H5M11 6.5h2.5M11 9.5h2.5',
    keys: 'M1.8 4.5h12.4v7H1.8zM4 6.6h.01M6.2 6.6h.01M8.4 6.6h.01M10.6 6.6h.01M4.6 9h4.4',
    list: 'M5 4h8M5 8h8M5 12h8M2.6 4h.01M2.6 8h.01M2.6 12h.01',
    device: 'M2.5 4.5h11v7h-11zM4.8 13.5h6.4M8 8.6a1.1 1.1 0 1 0 0 2.2 1.1 1.1 0 0 0 0-2.2z',
  };

  /** Close the device popover when clicking outside of it. */
  function onWindowClick(e: MouseEvent) {
    if (!deviceOpen) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest('.device-pop, .status')) return;
    deviceOpen = false;
  }

  function onWindowKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      deviceOpen = false;
      if (fullscreen) void win?.setFullscreen(false).catch(() => {});
    }
    if (e.key === 'F11') {
      e.preventDefault();
      void win?.setFullscreen(!fullscreen).catch(() => {});
    }
  }

  /** Track the window presentation so the shell can drop the corner radius. */
  async function refreshPresentation() {
    if (!win) return;
    try {
      const r = await windowPresentation();
      const wasExpanded = maximized || fullscreen;
      maximized = r.maximized;
      fullscreen = r.fullscreen;
      document.documentElement.dataset.windowExpanded = String(r.maximized || r.fullscreen);
      // Leaving the expanded state leaves square corner pixels behind on
      // WebKitGTK — a cosmetic 1px nudge three times fixes it.
      if (wasExpanded && !r.maximized && !r.fullscreen) void nudgeRepaint();
    } catch {
      /* presentation is cosmetic — never surface a failure */
    }
  }

  async function nudgeRepaint() {
    if (!win) return;
    try {
      const size = await win.innerSize();
      for (let i = 0; i < 3; i++) {
        await win.setSize(new PhysicalSize(size.width + 1, size.height));
        await new Promise((r) => requestAnimationFrame(() => r(null)));
        await win.setSize(new PhysicalSize(size.width, size.height));
        await new Promise((r) => setTimeout(r, 90));
      }
    } catch {
      /* cosmetic only */
    }
  }

  onMount(() => {
    void getVersion().then((v) => (version = v)).catch(() => {});
    applyAccent(store.accent);
    // Keep the DOM in sync with the persisted preference (light is the default).
    applyTheme(store.theme);
    void store.probeDevice();
    document.documentElement.dataset.windowNative = String(!!win);
    void refreshPresentation();
    const unlisten = win?.onResized(() => void refreshPresentation());
    return () => {
      void unlisten?.then((fn) => fn());
    };
  });
</script>

<svelte:window onclick={onWindowClick} onkeydown={onWindowKey} />

<div class="app-frame">
  <!-- ============================== native title bar ======================= -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="titlebar" data-tauri-drag-region="deep" onmousedown={dragBar}>
    <div class="titlebar-brand" data-tauri-drag-region="deep">
      <div class="brand-mark" data-tauri-drag-region="deep" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="15" height="15" fill="none">
          <!-- FRAME OS mark: a small display with the clock block -->
          <rect
            x="2.4"
            y="4.4"
            width="19.2"
            height="15.2"
            rx="3.4"
            stroke="currentColor"
            stroke-width="1.7"
            opacity=".5"
          />
          <rect x="6" y="9" width="8.6" height="6.2" rx="1.3" fill="currentColor" />
        </svg>
      </div>
      <span class="brand-word" data-tauri-drag-region="deep">SharkDeck</span>
    </div>

    <div class="titlebar-mid" data-tauri-drag-region="deep"></div>

    <div class="titlebar-right">
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

      <span class="hairline" aria-hidden="true"></span>

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

  <!-- ============================ sidebar + workspace ==================== -->
  <div class="shell">
    <aside class="sidebar">
      <div class="kicker">Workspace</div>
      <nav class="nav-items" aria-label="Sections">
        {#each PAGES as p (p.id)}
          <button
            class="nav-item"
            class:active={store.page === p.id}
            aria-current={store.page === p.id ? 'page' : undefined}
            onclick={() => (store.page = p.id)}
          >
            <svg class="nav-icon" viewBox="0 0 16 16" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d={ICONS[p.icon]} />
            </svg>
            <span class="nav-label">{p.label}</span>
            {#if p.badge}<span class="nav-badge">{p.badge}</span>{/if}
          </button>
        {/each}
      </nav>

      <div class="sidebar-foot">
        <div class="dev-card">
          <div class="dev-row">
            <span class="status-led" class:on={!!store.device} aria-hidden="true"></span>
            <span class="dev-name">Attack Shark K86</span>
          </div>
          <div class="dev-sub">{store.device ? store.device.path : 'No device via USB cable'}</div>
          <button class="btn tint sm" disabled={store.busy} onclick={() => void store.probeDevice()}>
            {store.busy ? 'Probing…' : 'Probe device'}
          </button>
        </div>
        <div class="side-foot">FRAME OS · SharkDeck v{version} · 240 × 135 panel</div>
      </div>
    </aside>

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
