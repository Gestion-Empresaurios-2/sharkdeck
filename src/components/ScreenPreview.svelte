<script lang="ts">
  import { onMount } from 'svelte';

  let { preview, width = 128, height = 128, zoom = 3, label } = $props();

  let canvas: HTMLCanvasElement;

  $effect(() => {
    const cv = canvas;
    if (!cv) return;
    const w = width;
    const h = height;
    const z = Math.max(1, Math.floor(zoom));
    cv.width = w * z;
    cv.height = h * z;
    const ctx = cv.getContext('2d');
    if (!ctx) return;
    // very subtle checker so the panel size is evident on the dark canvas
    ctx.fillStyle = '#0a0c11';
    ctx.fillRect(0, 0, cv.width, cv.height);
    ctx.fillStyle = '#12161d';
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        if ((x + y) % 2 === 0) ctx.fillRect(x * z, y * z, z, z);
      }
    }
    if (!preview) return;
    try {
      const bin = atob(preview);
      if (bin.length !== w * h * 4) {
        ctx.fillStyle = bgFor(h);
        ctx.fillRect(0, 0, cv.width, cv.height);
        ctx.fillStyle = '#d71c2d';
        ctx.font = `600 ${Math.min(cv.width, cv.height) / 10 + 6}px sans-serif`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText('bad preview data', cv.width / 2, cv.height / 2);
        return;
      }
      const px = new Uint8ClampedArray(bin.length);
      for (let i = 0; i < bin.length; i++) px[i] = bin.charCodeAt(i);
      const img = new ImageData(px, w, h);
      const tmp = new OffscreenCanvas(w, h);
      tmp.getContext('2d')?.putImageData(img, 0, 0);
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(tmp as unknown as CanvasImageSource, 0, 0, cv.width, cv.height);
    } catch {
      /* leave checker */
    }
  });

  function bgFor(h: number) {
    return `hsl(0 0% ${Math.min(96, 88 + (h % 5))}%)`;
  }

  onMount(() => {/* draw happens via effect */});
</script>

<div class="wrap">
  <canvas bind:this={canvas} class="frame"></canvas>
  {#if label}
    <div class="cap">{label}</div>
  {/if}
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .frame {
    background: #0a0c11;
    border-radius: 10px;
    image-rendering: pixelated;
    box-shadow: 0 0 0 1px var(--line, #252a33), 0 8px 24px rgba(0, 0, 0, 0.45);
    max-width: 100%;
    height: auto;
  }
  .cap {
    font-size: 11px;
    color: var(--muted, #838895);
  }
</style>
