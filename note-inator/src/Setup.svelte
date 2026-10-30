<script lang="ts">
  import { api, errMsg } from './api';
  import { app } from './store.svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { confirmBox } from './dialogs.svelte';
  import AppId from './AppId.svelte';

  let { onready, onrestored }: { onready: () => void; onrestored: () => void } = $props();
  let pw = $state('');
  let pw2 = $state('');
  let understood = $state(false);
  let error = $state('');
  let busy = $state(false);

  async function create() {
    error = '';
    if (pw.length < 8) return (error = 'Use at least 8 characters.');
    if (pw !== pw2) return (error = 'The passwords do not match.');
    if (!understood) return (error = 'Please confirm you understand there is no password recovery.');
    busy = true;
    try {
      app.settings = await api.createVault(pw);
      pw = pw2 = '';
      onready();
    } catch (e) { error = errMsg(e); }
    busy = false;
  }

  async function restore() {
    const file = await open({ multiple: false, title: 'Choose a vault backup' });
    if (!file || typeof file !== 'string') return;
    if (!(await confirmBox('Restore this backup? You will need the password it was created with.'))) return;
    try { await api.importBackup(file); onrestored(); } catch (e) { error = errMsg(e); }
  }
</script>

<div class="center">
  <form class="card" onsubmit={(e) => { e.preventDefault(); create(); }}>
    <AppId size={64} hero />
    <h1>Welcome</h1>
    <p>Choose a password to protect your notes. Everything is encrypted on this computer.</p>
    <div class="warn">
      <strong>There is no password recovery.</strong> If you forget this password, your notes cannot be
      recovered by anyone. Keep it somewhere safe.
    </div>
    <label>Password <input type="password" bind:value={pw} autocomplete="new-password" /></label>
    <label>Repeat password <input type="password" bind:value={pw2} autocomplete="new-password" /></label>
    <label class="check"><input type="checkbox" bind:checked={understood} /> I understand there is no way to recover a forgotten password.</label>
    {#if error}<p class="error">{error}</p>{/if}
    <button class="primary" disabled={busy}>{busy ? 'Creating…' : 'Create vault'}</button>
    <button type="button" class="link" onclick={restore}>Restore from a backup instead</button>
  </form>
</div>
