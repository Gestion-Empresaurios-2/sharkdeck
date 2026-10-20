<script lang="ts">
  import { onMount } from 'svelte';

  let {
    preview = '',
    width = 128,
    height = 128,
    zoom = 3,
    label = '',
  }: { preview?: string; width?: number; height?: number; zoom?: number; label?: string } = $props();

  let cv: HTMLCanvasElement | undefined = $state();

  $effect(() => {
    if (!cv) return;
    void draw();
  });

  function draw() {
    if (!cv) return;
    const ctx = cv.getContext('2d');
    if (!ctx) return;
    cv.width = width * zoom;
    cv.height = height * zoom;
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, cv.width, cv.height);
    // checkerboard backdrop
    ctx.fillStyle = '#0d1017';
    ctx.fillRect(0, 0, cv.width, cv.height);
    if (!preview) {
      ctx.fillStyle = '#232a38';
      ctx.font = `${13 * Math.max(1, zoom / 2)}px Inter, sans-serif`;
      ctx.textAlign = 'center';
      ctx.fillText('no preview', cv.width / 2, cv.height / 2);
      return;
    }
    try {
      const bin = atob(preview);
      const bytes = new Uint8ClampedArray(width * height * 4);
      const n = Math.min(bytes.length, bin.length);
      for (let i = 0; i < n; i++) bytes[i] = bin.charCodeAt(i);
      const tmp = document.createElement('canvas');
      tmp.width = width;
      tmp.height = height;
      const tctx = tmp.getContext('2d')!;
      tctx.putImageData(new ImageData(bytes, width, height), 0, 0);
      ctx.drawImage(tmp, 0, 0, width, height, 0, 0, cv.width, cv.height);
    } catch {
      ctx.fillStyle = '#f87171';
      ctx.font = '13px Inter, sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('bad preview data', cv.width / 2, cv.height / 2);
    }
  }

  onMount(() => draw());
</script>

<div class="wrap">
  {#if label}<div class="label">{label}</div>{/if}
  <div class="frame">
    <canvas bind:this={cv}></canvas>
  </div>
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: center;
  }
  .label {
    font-size: 12px;
    color: var(--text-dim);
    letter-spacing: 0.1em;
    text-transform: uppercase;
    font-weight: 600;
  }
  .frame {
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    overflow: hidden;
    box-shadow: var(--shadow), 0 0 40px rgba(91, 140, 255, 0.06);
    background: #0d1017;
  }
  canvas {
    display: block;
    width: 100%;
    height: auto;
  }
</style>
