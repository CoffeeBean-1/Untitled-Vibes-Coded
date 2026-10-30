<script lang="ts">
  // Note-inator identity: the app icon and name, with a small "by CAi" lockup.
  import Brand from './Brand.svelte';

  let { size = 28, hero = false }: { size?: number; hero?: boolean } = $props();
</script>

<div class="app-id" class:hero>
  <img class="app-icon" src="/branding/noteinator/icon.svg" width={size} height={size} alt="" draggable="false" />
  <div class="app-text">
    <span class="app-name">Note-inator</span>
    <span class="by">by <Brand size={20} /></span>
  </div>
</div>
