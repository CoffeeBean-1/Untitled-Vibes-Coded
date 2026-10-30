<script lang="ts">
  // CAi lockup: local mark + traced wordmark. Both wordmark variants are rendered;
  // app.css shows the one that contrasts with the resolved theme.
  let { size = 32, product = '' }: { size?: number; product?: string } = $props();
</script>

<div class="brand" style:--mark="{size}px">
  <img class="mark" src="/branding/cai/cai-mark-128.png" width={size} height={size} alt="" draggable="false" />
  <img class="wm on-light" src="/branding/cai/cai-wordmark-light.svg" alt="CAi" draggable="false" />
  <img class="wm on-dark" src="/branding/cai/cai-wordmark-dark.svg" alt="CAi" draggable="false" />
  {#if product}<span class="product">{product}</span>{/if}
</div>
