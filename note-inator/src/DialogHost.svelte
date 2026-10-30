<script lang="ts">
  import { dlg } from './dialogs.svelte';

  function done(ok: boolean) {
    const c = dlg.current;
    if (!c) return;
    dlg.current = null;
    c.resolve(c.kind === 'confirm' ? ok : ok ? c.value : null);
  }
  function focus(el: HTMLInputElement) { el.focus(); el.select(); }
</script>

{#if dlg.current}
  <div class="overlay" role="presentation">
    <div class="modal small" role="dialog" aria-modal="true">
      <p>{dlg.current.message}</p>
      {#if dlg.current.kind === 'prompt'}
        <input
          use:focus
          bind:value={dlg.current.value}
          onkeydown={(e) => { if (e.key === 'Enter') done(true); if (e.key === 'Escape') done(false); }}
        />
      {/if}
      <div class="row end">
        <button onclick={() => done(false)}>Cancel</button>
        <button class="primary" onclick={() => done(true)}>OK</button>
      </div>
    </div>
  </div>
{/if}
