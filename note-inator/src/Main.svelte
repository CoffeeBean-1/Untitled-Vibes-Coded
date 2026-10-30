<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errMsg, type Notebook, type Kind, type PageMeta, type PageFull } from './api';
  import { confirmBox, promptBox } from './dialogs.svelte';
  import { flushNow } from './flush';
  import Editor from './Editor.svelte';
  import Settings from './Settings.svelte';
  import AppId from './AppId.svelte';

  let { onlock, onrestored }: { onlock: () => void; onrestored: () => void } = $props();

  let tree = $state<Notebook[]>([]);
  let collapsed = $state<Record<string, boolean>>({});
  let selSection = $state<string | null>(null);
  let page = $state<PageFull | null>(null);
  let showSettings = $state(false);
  let error = $state('');
  // On a small window start with the sidebar tucked away.
  let navOpen = $state(window.innerWidth >= 900);

  const fmt = (ms: number) =>
    new Date(ms).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });

  async function run(fn: () => Promise<void>) {
    error = '';
    try { await fn(); } catch (e) { error = errMsg(e); }
  }

  async function refresh() {
    tree = await api.getTree();
    const sections = tree.flatMap((n) => n.sections);
    if (!sections.some((s) => s.id === selSection)) selSection = sections[0]?.id ?? null;
    if (page && !sections.some((s) => s.pages.some((p) => p.id === page!.id))) page = null;
  }

  async function openPage(id: string) {
    await flushNow();
    page = await api.getPage(id);
  }

  async function selectSection(id: string) {
    selSection = id;
    collapsed[id] = false;
  }

  onMount(() => run(async () => {
    await refresh();
    const first = tree.flatMap((n) => n.sections).find((s) => s.id === selSection)?.pages[0];
    if (first) await openPage(first.id);
  }));

  const onsaved = (m: PageMeta) => {
    for (const n of tree) for (const s of n.sections) for (const p of s.pages)
      if (p.id === m.id) { p.title = m.title; p.modified = m.modified; }
  };

  const addNotebook = () => run(async () => {
    const name = (await promptBox('Notebook name'))?.trim();
    if (!name) return;
    await api.createNode('notebook', '', name);
    await refresh();
  });

  const addSection = (nb: string) => run(async () => {
    const name = (await promptBox('Section name'))?.trim();
    if (!name) return;
    const id = await api.createNode('section', nb, name);
    collapsed[nb] = false;
    await refresh();
    await selectSection(id);
  });

  const addPage = (sec: string) => run(async () => {
    await flushNow();
    const id = await api.createNode('page', sec, 'Untitled page');
    selSection = sec;
    collapsed[sec] = false;
    await refresh();
    await openPage(id);
  });

  const rename = (kind: Kind, id: string, current: string) => run(async () => {
    const name = (await promptBox(`Rename ${kind}`, current))?.trim();
    if (!name || name === current) return;
    await flushNow();
    await api.renameNode(kind, id, name);
    await refresh();
    if (page && kind === 'page' && page.id === id) page = await api.getPage(id);
  });

  const remove = (kind: Kind, id: string, name: string) => run(async () => {
    const extra = kind === 'page' ? '' : ' and everything inside it';
    if (!(await confirmBox(`Delete ${kind} "${name}"${extra}? This cannot be undone.`))) return;
    await flushNow();
    if (kind !== 'page' || page?.id === id) page = null; // drop the editor without re-saving a deleted page
    await api.deleteNode(kind, id);
    await refresh();
  });

  // ---- drag and drop ----
  let drag: { kind: Kind; id: string; parent: string } | null = null;

  function dragStart(e: DragEvent, kind: Kind, id: string, parent: string) {
    drag = { kind, id, parent };
    e.dataTransfer!.effectAllowed = 'move';
    e.dataTransfer!.setData('text/plain', id);
  }
  function canDrop(kind: Kind, parent: string, acceptPages = false) {
    return !!drag && ((drag.kind === kind && drag.parent === parent) || (acceptPages && drag.kind === 'page'));
  }
  function over(e: DragEvent, kind: Kind, parent: string, acceptPages = false) {
    if (canDrop(kind, parent, acceptPages)) e.preventDefault();
  }
  const drop = (e: DragEvent, kind: Kind, targetId: string, parent: string, siblings: string[], acceptPages = false) =>
    run(async () => {
      e.preventDefault();
      const d = drag;
      drag = null;
      if (!d) return;
      if (acceptPages && d.kind === 'page') {
        await flushNow();
        await api.movePage(d.id, targetId);
      } else if (d.kind === kind && d.parent === parent && d.id !== targetId) {
        const ids = [...siblings];
        const from = ids.indexOf(d.id);
        const to = ids.indexOf(targetId);
        ids.splice(from, 1);
        ids.splice(to, 0, d.id);
        await api.reorder(kind, parent, ids);
      } else return;
      await refresh();
    });
</script>

<div class="layout" class:closed={!navOpen}>
  {#if navOpen}
  <aside class="sidebar">
    <div class="brand-head"><AppId size={32} /></div>
    <div class="side-head">
      <strong>Notebooks</strong>
      <span>
        <button class="icon" title="New notebook" onclick={addNotebook}>＋</button>
        <button class="icon" title="Hide sidebar" onclick={() => (navOpen = false)}>«</button>
      </span>
    </div>
    <div class="tree">
      {#each tree as nb (nb.id)}
        <div
          class="row-item nb"
          role="listitem"
          draggable="true"
          ondragstart={(e) => dragStart(e, 'notebook', nb.id, '')}
          ondragover={(e) => over(e, 'notebook', '')}
          ondrop={(e) => drop(e, 'notebook', nb.id, '', tree.map((n) => n.id))}
        >
          <button class="chev" onclick={() => (collapsed[nb.id] = !collapsed[nb.id])} aria-label="Toggle notebook">
            {collapsed[nb.id] ? '▸' : '▾'}
          </button>
          <span class="label" ondblclick={() => rename('notebook', nb.id, nb.name)} role="presentation">{nb.name}</span>
          <span class="actions">
            <button class="icon" title="New section" onclick={() => addSection(nb.id)}>＋</button>
            <button class="icon" title="Rename" onclick={() => rename('notebook', nb.id, nb.name)}>✎</button>
            <button class="icon" title="Delete" onclick={() => remove('notebook', nb.id, nb.name)}>🗑</button>
          </span>
        </div>
        {#if !collapsed[nb.id]}
          {#each nb.sections as s (s.id)}
            <div
              class="row-item sec"
              class:active={s.id === selSection}
              role="listitem"
              draggable="true"
              ondragstart={(e) => dragStart(e, 'section', s.id, nb.id)}
              ondragover={(e) => over(e, 'section', nb.id, true)}
              ondrop={(e) => drop(e, 'section', s.id, nb.id, nb.sections.map((x) => x.id), true)}
            >
              <button class="chev" onclick={() => (collapsed[s.id] = !collapsed[s.id])} aria-label="Toggle section">
                {collapsed[s.id] ? '▸' : '▾'}
              </button>
              <button class="label" onclick={() => { selSection = s.id; collapsed[s.id] = !collapsed[s.id]; }} ondblclick={() => rename('section', s.id, s.name)}>
                {s.name}
              </button>
              <span class="actions">
                <button class="icon" title="New page" onclick={() => addPage(s.id)}>＋</button>
                <button class="icon" title="Rename" onclick={() => rename('section', s.id, s.name)}>✎</button>
                <button class="icon" title="Delete" onclick={() => remove('section', s.id, s.name)}>🗑</button>
              </span>
            </div>
            {#if !collapsed[s.id]}
              {#each s.pages as p (p.id)}
                <div
                  class="page-item"
                  class:active={page?.id === p.id}
                  role="listitem"
                  draggable="true"
                  ondragstart={(e) => dragStart(e, 'page', p.id, s.id)}
                  ondragover={(e) => over(e, 'page', s.id)}
                  ondrop={(e) => drop(e, 'page', p.id, s.id, s.pages.map((x) => x.id))}
                >
                  <button class="page-btn" title={fmt(p.modified)} onclick={() => { selSection = s.id; run(() => openPage(p.id)); }}>
                    <span class="ptitle">{p.title || 'Untitled page'}</span>
                  </button>
                  <button class="icon del" title="Delete page" onclick={() => remove('page', p.id, p.title || 'Untitled page')}>🗑</button>
                </div>
              {/each}
              {#if s.pages.length === 0}<div class="hint indent2">No pages. Click ＋ on the section.</div>{/if}
            {/if}
          {/each}
          {#if nb.sections.length === 0}<div class="hint indent">No sections yet</div>{/if}
        {/if}
      {/each}
      {#if tree.length === 0}<div class="hint">Create a notebook to get started.</div>{/if}
    </div>
    <div class="side-foot">
      <button onclick={onlock} title="Lock (Ctrl+L)">🔒 Lock</button>
      <button onclick={() => (showSettings = true)}>⚙ Settings</button>
    </div>
  </aside>
  {/if}

  <main class="editor-pane">
    {#if error}<div class="banner error">{error}</div>{/if}
    {#if page}
      {#key page.id}
        <Editor pageId={page.id} initialTitle={page.title} initialContent={page.content} {onsaved} />
      {/key}
    {:else}
      <div class="empty">Select a page, or create a new one.</div>
    {/if}
  </main>
</div>

{#if showSettings}
  <Settings onclose={() => (showSettings = false)} {onrestored} />
{/if}

{#if !navOpen}
  <div class="float left"><button title="Show notebooks" onclick={() => (navOpen = true)}>»</button></div>
  <div class="float right"><button title="Lock (Ctrl+L)" onclick={onlock}>🔒</button></div>
{/if}
