<script lang="ts">
  import { store, probeDevice, connected } from './lib/store.svelte';
  import { onMount } from 'svelte';
  import Screen from './pages/Screen.svelte';
  import Knob from './pages/Knob.svelte';
  import Lighting from './pages/Lighting.svelte';
  import Keymap from './pages/Keymap.svelte';
  import Macros from './pages/Macros.svelte';
  import Settings from './pages/Settings.svelte';

  const nav: { id: string; label: string; icon: string }[] = [
    { id: 'screen', label: 'Screen', icon: 'M5 3h14a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1h-5l1.5 3h-8L9 16H5a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Zm2 13h10' },
    { id: 'knob', label: 'Knob', icon: 'M12 4a8 8 0 1 1 0 16 8 8 0 0 1 0-16Zm0 3v5l4 2' },
    { id: 'lighting', label: 'Lighting', icon: 'M12 4a6 6 0 0 1 4 10.5c-.6.55-1 1.4-1 2.5H9c0-1.1-.4-1.95-1-2.5A6 6 0 0 1 12 4ZM10 20h4' },
    { id: 'keymap', label: 'Keymap', icon: 'M3 6h18v12H3V6Zm3 3h2m2 0h2m2 0h2m2 0h2M8 12h2m2 0h2m2 0h2m-6 3h6' },
    { id: 'macros', label: 'Macros', icon: 'M6 3h12v4H6V3Zm0 14h12v4H6v-4ZM9 9l6 6m-6 0 6-6' },
    { id: 'settings', label: 'Device', icon: 'M12 8a4 4 0 1 1 0 8 4 4 0 0 1 0-8Zm8 4 2-1-1-2.5-2.3.6a7 7 0 0 0-1.5-.9L16.6 5h-2.7l-.4 2.2c-.5.2-1 .5-1.5.9L9.7 7.5 8 10l1.7 1.5a7 7 0 0 0 0 1.8L8 15l1.7 2.5 2.3-.6c.5.4 1 .7 1.5.9l.4 2.2h2.7l.6-2.2c.5-.2 1-.5 1.5-.9l2.3.6L22 15l-2-1.5a7 7 0 0 0 0-1Z' },
  ];

  onMount(() => {
    probeDevice();
  });
</script>

<div class="app">
  <aside class="side">
    <div class="brand">
      <svg viewBox="0 0 24 24" width="26" height="26" stroke="none">
        <defs>
          <linearGradient id="finG" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stop-color="#5b8cff" />
            <stop offset="1" stop-color="#7b61ff" />
          </linearGradient>
        </defs>
        <path d="M3 19 21 4c-1.5 9-6.5 13-11 13.5L4 21l-1-2Z" fill="url(#finG)" />
      </svg>
      <div>
        <div class="brand-name">SharkDeck</div>
        <div class="brand-sub">Attack Shark K86</div>
      </div>
    </div>

    <nav class="nav">
      {#each nav as item (item.id)}
        <button
          class="nav-item"
          class:sel={store.page === item.id}
          onclick={() => (store.page = item.id)}
        >
          <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
            <path d={item.icon} />
          </svg>
          {item.label}
        </button>
      {/each}
    </nav>

    <div class="device-status">
      {#if connected()}
        <span class="pill ok"><span class="dot"></span>Connected</span>
        <div class="hint mono" style="margin-top:6px">{store.device?.path}</div>
      {:else}
        <span class="pill warn"><span class="dot"></span>No device</span>
        <button class="btn sm" style="margin-top:10px; width:100%" onclick={probeDevice} disabled={store.busy}>
          {#if store.busy}<span class="spinner"></span>{:else}Probe{/if}
        </button>
      {/if}
    </div>
  </aside>

  <main class="main">
    {#if store.page === 'screen'}
      <Screen />
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
    grid-template-columns: 236px 1fr;
    height: 100vh;
    background:
      radial-gradient(1000px 500px at 85% -10%, rgba(91, 140, 255, 0.09), transparent 60%),
      radial-gradient(800px 500px at -10% 110%, rgba(123, 97, 255, 0.07), transparent 60%),
      var(--bg);
  }

  .side {
    border-right: 1px solid var(--border);
    background: rgba(11, 14, 20, 0.7);
    backdrop-filter: blur(10px);
    display: flex;
    flex-direction: column;
    padding: 18px 14px;
    gap: 20px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 2px 6px;
  }
  .brand-name {
    font-weight: 700;
    font-size: 15.5px;
    letter-spacing: -0.01em;
  }
  .brand-sub {
    font-size: 10.5px;
    color: var(--text-faint);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .nav {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border-radius: 10px;
    color: var(--text-dim);
    font-weight: 520;
    font-size: 13.5px;
    transition: all 0.14s;
    text-align: left;
  }
  .nav-item:hover {
    background: rgba(255, 255, 255, 0.05);
    color: var(--text);
  }
  .nav-item.sel {
    background: linear-gradient(135deg, rgba(91, 140, 255, 0.18), rgba(123, 97, 255, 0.14));
    color: #fff;
    box-shadow: inset 0 0 0 1px rgba(91, 140, 255, 0.25);
  }

  .device-status {
    margin-top: auto;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel);
  }

  .main {
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .toasts {
    position: fixed;
    right: 18px;
    bottom: 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 50;
    max-width: 420px;
  }
  .toast {
    padding: 10px 14px;
    border-radius: 12px;
    font-size: 12.5px;
    background: rgba(20, 24, 34, 0.95);
    border: 1px solid var(--border-strong);
    box-shadow: var(--shadow);
    animation: rise 0.18s ease-out;
    line-height: 1.45;
  }
  .toast.ok {
    border-color: rgba(74, 222, 128, 0.4);
  }
  .toast.err {
    border-color: rgba(248, 113, 113, 0.45);
    color: #fca5a5;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
</style>
