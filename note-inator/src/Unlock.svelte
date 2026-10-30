<script lang="ts">
  import { api, errMsg } from './api';
  import { app } from './store.svelte';
  import AppId from './AppId.svelte';

  let { onunlocked }: { onunlocked: () => void } = $props();
  let pw = $state('');
  let error = $state('');
  let busy = $state(false);

  async function unlock() {
    error = '';
    busy = true;
    try {
      app.settings = await api.unlock(pw);
      pw = '';
      onunlocked();
    } catch (e) { error = errMsg(e); pw = ''; }
    busy = false;
  }
  function focus(el: HTMLInputElement) { el.focus(); }
</script>

<div class="center">
  <form class="card" onsubmit={(e) => { e.preventDefault(); unlock(); }}>
    <h1 class="sr-only">Note-inator</h1>
    <AppId size={64} hero />
    <p>Enter your password to unlock.</p>
    <input use:focus type="password" bind:value={pw} autocomplete="current-password" placeholder="Password" />
    {#if error}<p class="error">{error}</p>{/if}
    <button class="primary" disabled={busy || !pw}>{busy ? 'Unlocking…' : 'Unlock'}</button>
  </form>
</div>
