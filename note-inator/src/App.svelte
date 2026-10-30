<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { api } from './api';
  import { app } from './store.svelte';
  import { flushNow } from './flush';
  import Setup from './Setup.svelte';
  import Unlock from './Unlock.svelte';
  import Main from './Main.svelte';
  import DialogHost from './DialogHost.svelte';
  import { dlg } from './dialogs.svelte';

  let phase = $state<'loading' | 'setup' | 'locked' | 'main'>('loading');

  async function refreshPhase() {
    const s = await api.status();
    phase = s.unlocked ? 'main' : s.exists ? 'locked' : 'setup';
  }

  // Save pending edits, then lock. The Rust side drops the key and vault.
  // `minimize` is set for the Lock button and Ctrl+L, not for idle/system locks.
  async function lockNow(minimize = false) {
    if (phase !== 'main') return;
    try { await flushNow(); } catch { /* lock anyway */ }
    await api.lock(minimize);
    dlg.current = null;
    phase = 'locked';
  }

  $effect(() => {
    document.documentElement.dataset.theme = app.settings.theme;
  });

  onMount(() => {
    refreshPhase();
    const unlisten: Promise<UnlistenFn>[] = [
      listen('lock-now', () => lockNow(false)),
      listen('locked', () => { dlg.current = null; if (phase === 'main') phase = 'locked'; }),
      listen('flush-and-quit', async () => {
        try { await flushNow(); } catch { /* quit anyway */ }
        await api.quit();
      }),
    ];

    // Tell the backend we are active (it owns the idle timer), at most every 3 s.
    let last = 0;
    const ping = () => {
      const now = Date.now();
      if (phase === 'main' && now - last > 3000) { last = now; api.activity().catch(() => {}); }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'l') {
        e.preventDefault();
        lockNow(true);
      } else ping();
    };
    window.addEventListener('keydown', onKey, true);
    for (const ev of ['mousemove', 'mousedown', 'wheel', 'touchstart']) window.addEventListener(ev, ping, { passive: true });

    return () => {
      unlisten.forEach((p) => p.then((f) => f()));
      window.removeEventListener('keydown', onKey, true);
      for (const ev of ['mousemove', 'mousedown', 'wheel', 'touchstart']) window.removeEventListener(ev, ping);
    };
  });
</script>

{#if phase === 'loading'}
  <div class="center"></div>
{:else if phase === 'setup'}
  <Setup onready={() => (phase = 'main')} onrestored={refreshPhase} />
{:else if phase === 'locked'}
  <Unlock onunlocked={() => (phase = 'main')} />
{:else}
  <Main onlock={() => lockNow(true)} onrestored={() => (phase = 'locked')} />
{/if}
<DialogHost />
