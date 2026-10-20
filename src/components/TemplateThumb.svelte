<script lang="ts">
  // Gallery thumbnail drawn straight into a canvas — no base64 round-trip,
  // so it can never end up as a broken image.
  import { renderTemplate, type Template } from '../lib/templates';

  let { tpl, w = 240, h = 135 }: { tpl: Template; w?: number; h?: number } = $props();

  let cv: HTMLCanvasElement | undefined = $state();

  function paint() {
    if (!cv) return;
    const ctx = cv.getContext('2d');
    if (!ctx) return;
    if (cv.width !== w || cv.height !== h) {
      cv.width = w;
      cv.height = h;
    }
    const data = renderTemplate(tpl, w, h, 0);
    ctx.putImageData(new ImageData(data, w, h), 0, 0);
  }

  $effect(() => {
    // re-paint whenever the template (or size) changes
    void tpl;
    void w;
    void h;
    paint();
  });
</script>

<canvas bind:this={cv} width={w} height={h} class="thumb" aria-label={tpl.name}></canvas>

<style>
  .thumb {
    width: 100%;
    height: auto;
    display: block;
    image-rendering: pixelated;
    border-radius: 5px;
    background: #000;
  box-shadow: var(--elev-1);
}
</style>
