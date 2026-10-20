<script lang="ts">
  // Live mirror: renders the exact frames that are pushed to the keyboard
  // panel, so the desktop shows what the TFT shows. The hardware has no
  // read-back command, so this mirrors what we send rather than faking a
  // capture.
  import { onMount } from 'svelte';

  let {
    render,
    w = 240,
    h = 135,
    fps = 2,
    label = 'Live panel',
  }: {
    render: (frame: number) => Uint8ClampedArray;
    w?: number;
    h?: number;
    fps?: number;
    label?: string;
  } = $props();

  let cv: HTMLCanvasElement | undefined = $state();
  let frame = $state(0);

  function paint() {
    if (!cv) return;
    const ctx = cv.getContext('2d');
    if (!ctx) return;
    try {
      const data = render(frame);
      ctx.putImageData(new ImageData(data, w, h), 0, 0);
    } catch {
      /* a transient render error must not kill the loop */
    }
  }

  onMount(() => {
    paint();
    const id = setInterval(() => {
      frame += 1;
      paint();
    }, Math.max(60, Math.round(1000 / fps)));
    return () => clearInterval(id);
  });

  $effect(() => {
    // re-paint when the render source or geometry changes
    void render;
    void w;
    void h;
    paint();
  });
</script>

<div class="live">
  <canvas bind:this={cv} {w} {h} aria-label={label}></canvas>
  <span class="live-dot" aria-hidden="true"></span>
  <span class="live-label">{label}</span>
</div>

<style>
  .live {
    position: relative;
    display: inline-block;
  }
  canvas {
    display: block;
    width: 100%;
    height: auto;
    image-rendering: pixelated;
    border-radius: 8px;
    border: 1px solid var(--border-strong);
    background: #000;
  }
  .live-dot {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--err);
    box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.45);
  }
  .live-label {
    position: absolute;
    left: 8px;
    bottom: 8px;
    font-size: 10px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: #fff;
    background: rgba(0, 0, 0, 0.55);
    padding: 2px 6px;
    border-radius: 4px;
  }
</style>
