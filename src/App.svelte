<script lang="ts">
  import { store, applyTheme } from './lib/store.svelte';
  import Screen from './pages/Screen.svelte';
  import Knob from './pages/Knob.svelte';
  import Lighting from './pages/Lighting.svelte';
  import Keymap from './pages/Keymap.svelte';
  import Macros from './pages/Macros.svelte';
  import Templates from './pages/Templates.svelte';
  import Os from './pages/Os.svelte';
  import Settings from './pages/Settings.svelte';
  import { onMount } from 'svelte';

  const PAGES = [
    { id: 'os', label: 'Screen OS' },
    { id: 'screen', label: 'Screen' },
    { id: 'templates', label: 'Templates' },
    { id: 'knob', label: 'Knob' },
    { id: 'lighting', label: 'Lighting' },
    { id: 'keymap', label: 'Keymap' },
    { id: 'macros', label: 'Macros' },
    { id: 'settings', label: 'Device' },
  ];

  // Minimal flat line icons (16x16, stroke) — GNOME-ish, no fills, no gradients.
  const ICONS: Record<string, string> = {
    os: 'M2 2h12v12H2z M5 5h6v6H5z',
    screen: 'M2 3h12v8H2z M6 14h4 M8 11v3',
    templates: 'M2 2h5v5H2z M9 2h5v5H9z M2 9h5v5H2z M9 9h5v5H9z',
    knob: 'M8 2a6 6 0 016 6 6 6 0 01-6 6 6 6 0 01-6-6 6 6 0 016-6z M8 8m-2 0a2 2 0 104 0 2 2 0 10-4 0',
    lighting: 'M8 1v2 M8 13v2 M2 8h2 M12 8h2 M4 4l1.4 1.4 M10.6 10.6L12 12 M12 4l-1.4 1.4 M5.4 10.6L4 12 M8 5a3 3 0 100 6 3 3 0 000-6',
    keymap: 'M1.5 4h13v8h-13z M4 6.5h1 M7 6.5h1 M10 6.5h1 M12 6.5h1 M5 9.5h6',
    macros: 'M3 2h10v12H3z M5.5 5h5 M5.5 8h5 M5.5 11h3',
    settings: 'M8 5a3 3 0 103 3 3 3 0 00-3-3z M13.6 9.5l1.8-1.5-1.8-1.5.3-2.3-2.2-.7L10.4 2 8.2 2.9 6 2l-1.3 1.5-2.2.7.3 2.3L1 8l1.8 1.5-.3 2.3 2.2.7L6 14l2.2-.9 2.2.9 1.3-1.5 2.2-.7z',
  };

  onMount(() => {
    applyTheme(store.theme);
    void store.probeDevice();
  });
</script>

<div class="app">
  <aside>
    <div class="brand">
      <div class="brand-tile">
        <svg viewBox="0 0 24 24" width="42" height="42" aria-hidden="true">
          <path d="M13.4 1.2 C 11.4 6.4 6.8 12.4 3.6 21.2 C 8 21.4 14.6 21.2 19.4 20.6 C 18.9 13 16.8 5.6 13.4 1.2 Z" fill="#6b76fa"/>
        </svg>
      </div>
      <div>
        <div class="brand-name">SharkDeck</div>
        <div class="brand-sub">Attack Shark K86</div>
      </div>
    </div>

    <nav>
      {#each PAGES as p (p.id)}
        <button class="nav-btn" class:sel={store.page === p.id} onclick={() => (store.page = p.id as any)}>
          <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
            <path d={ICONS[p.id]} fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          {p.label}
        </button>
      {/each}
    </nav>

    <div class="aside-foot">
      <div class="dev-card">
        <div class="dev-row">
          <span class="dot" class:ok={store.device} class:err={!store.device}></span>
          <span class="dev-name">{store.device ? 'Connected' : 'No device'}</span>
        </div>
        <div class="dev-path" title={store.device?.path ?? ''}>{store.device?.path ?? 'via USB cable'}</div>
        <button class="btn sm" style="margin-top:6px" disabled={store.busy} onclick={() => void store.probeDevice()}>
          {store.busy ? 'Probing…' : 'Probe device'}
        </button>
      </div>

      <div class="theme-toggle" role="group" aria-label="Colour scheme">
        <button class={store.theme === 'light' ? 'sel' : ''} title="Light" onclick={() => applyTheme('light')}>
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <path d="M8 4.4a3.6 3.6 0 100 7.2 3.6 3.6 0 100-7.2z M8 0.8v1.7 M8 13.5v1.7 M0.8 8h1.7 M13.5 8h1.7 M2.9 2.9l1.2 1.2 M11.9 11.9l1.2 1.2 M13.1 2.9l-1.2 1.2 M4.1 11.9l-1.2 1.2" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
          </svg>
          Light
        </button>
        <button class={store.theme === 'dark' ? 'sel' : ''} title="Dark" onclick={() => applyTheme('dark')}>
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <path d="M13.5 9.8A5.9 5.9 0 016.2 2.5 5.9 5.9 0 1013.5 9.8z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" />
          </svg>
          Dark
        </button>
      </div>
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
    {:else if store.page === 'keymap'}
      <Keymap />
    {:else if store.page === 'macros'}
      <Macros />
    {:else}
      <Settings />
    {/if}
  </main>

  <div class="toasts">
    {#each store.toasts as t (t.id)}
      <div class="toast {t.kind}">{t.text}</div>
    {/each}
  </div>
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: 232px 1fr;
    height: 100%;
    background: var(--bg);
    color: var(--text);
  }
  aside {
    background: var(--panel-2);
    backdrop-filter: var(--material);
    -webkit-backdrop-filter: var(--material);
    border-right: 1px solid var(--hairline);
    display: flex;
    flex-direction: column;
    padding: 12px 10px;
    gap: 10px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px 10px;
    border-bottom: 1px solid var(--border);
  }
  .brand-tile {
    width: 46px;
    height: 46px;
    border-radius: 11px;
    background: #20253a;
    display: grid;
    place-items: center;
    overflow: hidden;
    flex-shrink: 0;
  }
  .brand-name {
    font-size: 15px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .brand-sub {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--dim);
    margin-top: 1px;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nav-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--dim);
    font-size: 13.5px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    transition: background 0.12s ease, color 0.12s ease;
  }
  .nav-btn:hover {
    background: var(--hover);
    color: var(--text);
  }
  .nav-btn.sel {
    background: var(--panel-2);
    color: var(--text);
    font-weight: 600;
  }
  .aside-foot {
    margin-top: auto;
  }
  .theme-toggle {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0;
    margin-top: 8px;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    background: var(--panel-2);
  }
  .theme-toggle button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 7px 0;
    border: none;
    background: transparent;
    color: var(--dim);
    font-size: 12px;
    cursor: pointer;
  }
  .theme-toggle button + button {
    border-left: 1px solid var(--border);
  }
  .theme-toggle button:hover {
    background: var(--hover);
    color: var(--text);
  }
  .theme-toggle button.sel {
    background: var(--panel);
    color: var(--text);
    font-weight: 600;
  }
  .dev-card {
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px;
  }
  .dev-row {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    font-weight: 600;
  }
  .dev-path {
    margin-top: 3px;
    font-size: 11px;
    color: var(--dim);
    font-family: var(--mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  main {
    overflow-y: auto;
    background: var(--bg);
  }
  .toasts {
    position: fixed;
    right: 14px;
    bottom: 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 50;
  }
  .toast {
    background: rgba(0, 0, 0, 0.85);
    color: #fff;
    font-size: 12.5px;
    padding: 9px 14px;
    border-radius: 8px;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.18);
    max-width: 380px;
  }
  .toast.err { color: #ffc9cd; }
  .toast.ok { color: #c8ecd2; }
</style>
