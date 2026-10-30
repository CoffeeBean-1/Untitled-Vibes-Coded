<script lang="ts">
  import { save, open } from '@tauri-apps/plugin-dialog';
  import { api, errMsg } from './api';
  import { app } from './store.svelte';
  import { confirmBox } from './dialogs.svelte';

  let { onclose, onrestored }: { onclose: () => void; onrestored: () => void } = $props();

  let idle = $state(app.settings.idle_minutes === null ? 'off' : String(app.settings.idle_minutes));
  let lockMin = $state(app.settings.lock_on_minimize);
  let theme = $state(app.settings.theme);
  let msg = $state('');
  let err = $state('');

  let oldPw = $state('');
  let newPw = $state('');
  let newPw2 = $state('');
  let pwBusy = $state(false);

  async function persist() {
    err = '';
    const next = {
      idle_minutes: idle === 'off' ? null : Number(idle),
      lock_on_minimize: lockMin,
      theme,
    };
    try {
      await api.setSettings(next);
      app.settings = next;
    } catch (e) { err = errMsg(e); }
  }

  async function changePw() {
    err = msg = '';
    if (newPw.length < 8) return (err = 'The new password must be at least 8 characters.');
    if (newPw !== newPw2) return (err = 'The new passwords do not match.');
    pwBusy = true;
    try {
      await api.changePassword(oldPw, newPw);
      oldPw = newPw = newPw2 = '';
      msg = 'Password changed. Your vault was re-encrypted.';
    } catch (e) { err = errMsg(e); }
    pwBusy = false;
  }

  async function exportBackup() {
    err = msg = '';
    const dest = await save({ title: 'Export backup', defaultPath: 'noteinator-backup.dat' });
    if (!dest) return;
    try { await api.exportBackup(dest); msg = 'Backup saved (still encrypted with your password).'; }
    catch (e) { err = errMsg(e); }
  }

  async function importBackup() {
    err = msg = '';
    const src = await open({ title: 'Import backup', multiple: false });
    if (!src || typeof src !== 'string') return;
    const ok = await confirmBox(
      'Replace the current vault with this backup? The current vault is kept as vault.dat.bak. ' +
      'You will be locked out and must enter the backup’s password.');
    if (!ok) return;
    try { await api.importBackup(src); onclose(); onrestored(); } catch (e) { err = errMsg(e); }
  }
</script>

<div class="overlay" role="presentation">
  <div class="modal" role="dialog" aria-modal="true" aria-label="Settings">
    <div class="row between"><h2>Settings</h2><button class="icon" onclick={onclose} aria-label="Close">✕</button></div>

    <h3>Auto-lock</h3>
    <label>Lock after being idle for
      <select bind:value={idle} onchange={persist}>
        <option value="off">Never</option>
        {#each [1, 2, 5, 10, 15, 30, 60] as m}<option value={String(m)}>{m} minute{m > 1 ? 's' : ''}</option>{/each}
      </select>
    </label>
    <label class="check"><input type="checkbox" bind:checked={lockMin} onchange={persist} /> Lock when the window is minimized</label>
    <p class="hint">The vault also locks when Windows locks or goes to sleep. Press Ctrl+L to lock at any time.</p>

    <h3>Appearance</h3>
    <label>Theme
      <select bind:value={theme} onchange={persist}>
        <option value="system">Follow Windows</option>
        <option value="light">Light</option>
        <option value="dark">Dark</option>
      </select>
    </label>

    <h3>Change password</h3>
    <form onsubmit={(e) => { e.preventDefault(); changePw(); }}>
      <input type="password" placeholder="Current password" bind:value={oldPw} autocomplete="current-password" />
      <input type="password" placeholder="New password" bind:value={newPw} autocomplete="new-password" />
      <input type="password" placeholder="Repeat new password" bind:value={newPw2} autocomplete="new-password" />
      <button class="primary" disabled={pwBusy || !oldPw || !newPw}>{pwBusy ? 'Re-encrypting…' : 'Change password'}</button>
    </form>
    <p class="hint">There is no password recovery. Store the new password safely.</p>

    <h3>Backup</h3>
    <div class="row">
      <button onclick={exportBackup}>Export backup…</button>
      <button onclick={importBackup}>Import backup…</button>
    </div>

    <h3>About</h3>
    <p class="hint about">Note-inator, by CAi (Cursed Assistant-inator). Notes are encrypted on this computer with a key derived from your password.</p>

    {#if msg}<p class="ok">{msg}</p>{/if}
    {#if err}<p class="error">{err}</p>{/if}
  </div>
</div>
