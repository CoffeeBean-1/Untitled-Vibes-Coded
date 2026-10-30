<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import { Editor } from '@tiptap/core';
  import StarterKit from '@tiptap/starter-kit';
  import { Table, TableRow, TableCell, TableHeader } from '@tiptap/extension-table';
  import { TaskList, TaskItem } from '@tiptap/extension-list';
  import { TextStyle, Color } from '@tiptap/extension-text-style';
  import Highlight from '@tiptap/extension-highlight';
  import { api, errMsg, type PageMeta } from './api';
  import { registerFlush, unregisterFlush } from './flush';
  import { promptBox } from './dialogs.svelte';

  let { pageId, initialTitle, initialContent, onsaved }: {
    pageId: string;
    initialTitle: string;
    initialContent: unknown;
    onsaved: (m: PageMeta) => void;
  } = $props();

  const AUTOSAVE_MS = 3000;

  let host: HTMLDivElement;
  let editor: Editor;
  let title = $state(untrack(() => initialTitle));
  let tick = $state(0); // bumped on every transaction so toolbar state re-renders
  let status = $state('');
  let dirty = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  // Pasted content: keep what the schema supports, drop images and Office/browser leftovers.
  const cleanPaste = (html: string) =>
    html
      .replace(/<!--[\s\S]*?-->/g, '')
      .replace(/<img\b[^>]*>/gi, '')
      .replace(/<(picture|svg|object|embed|iframe|script|style)\b[\s\S]*?<\/\1>/gi, '');

  const hasImageOnly = (e: ClipboardEvent) => {
    const d = e.clipboardData;
    if (!d) return false;
    return [...d.items].some((i) => i.type.startsWith('image/')) && !d.getData('text/html') && !d.getData('text/plain');
  };

  function markDirty() {
    dirty = true;
    status = '';
    clearTimeout(timer);
    timer = setTimeout(save, AUTOSAVE_MS);
  }

  async function save() {
    clearTimeout(timer);
    if (!dirty) return;
    dirty = false;
    const content = editor.getJSON(); // captured synchronously, before any await
    try {
      onsaved(await api.savePage(pageId, title, content));
      status = 'Saved';
    } catch (e) {
      dirty = true;
      status = 'Save failed: ' + errMsg(e);
    }
  }

  onMount(() => {
    editor = new Editor({
      element: host,
      content: initialContent as any,
      extensions: [
        StarterKit.configure({ link: { openOnClick: false, autolink: true, protocols: ['http', 'https', 'mailto'] } }),
        Table,
        TableRow,
        TableHeader,
        TableCell,
        TaskList,
        TaskItem.configure({ nested: true }),
        TextStyle,
        Color,
        Highlight.configure({ multicolor: true }),
      ],
      editorProps: {
        transformPastedHTML: cleanPaste,
        handlePaste: (_v, e) => hasImageOnly(e),
        handleDrop: (_v, e) => !!e.dataTransfer?.files?.length,
      },
      onUpdate: markDirty,
      onTransaction: () => { tick++; },
    });
    registerFlush(save);
  });

  onDestroy(() => {
    unregisterFlush();
    void save();
    editor?.destroy();
  });

  const is = (name: string, attrs?: Record<string, unknown>) => { tick; return !!editor?.isActive(name, attrs); };
  const chain = () => editor.chain().focus();

  async function setLink() {
    const prev = (editor.getAttributes('link').href as string) ?? '';
    const url = await promptBox('Link address (http, https or mailto)', prev);
    if (url === null) return;
    if (url.trim() === '') return void chain().extendMarkRange('link').unsetLink().run();
    const href = /^[a-z][a-z0-9+.-]*:/i.test(url.trim()) ? url.trim() : 'https://' + url.trim();
    chain().extendMarkRange('link').setLink({ href }).run();
  }

  const color = (e: Event) => chain().setColor((e.currentTarget as HTMLInputElement).value).run();
  const hl = (e: Event) => chain().setHighlight({ color: (e.currentTarget as HTMLInputElement).value }).run();
</script>

<div class="editor-wrap">
  <input
    class="title"
    bind:value={title}
    oninput={markDirty}
    placeholder="Untitled page"
    spellcheck="true"
  />
  <div class="toolbar">
    <button class:on={is('bold')} onclick={() => chain().toggleBold().run()} title="Bold (Ctrl+B)"><b>B</b></button>
    <button class:on={is('italic')} onclick={() => chain().toggleItalic().run()} title="Italic (Ctrl+I)"><i>I</i></button>
    <button class:on={is('underline')} onclick={() => chain().toggleUnderline().run()} title="Underline (Ctrl+U)"><u>U</u></button>
    <button class:on={is('strike')} onclick={() => chain().toggleStrike().run()} title="Strikethrough"><s>S</s></button>
    <span class="sep"></span>
    <button class:on={is('heading', { level: 1 })} onclick={() => chain().toggleHeading({ level: 1 }).run()}>H1</button>
    <button class:on={is('heading', { level: 2 })} onclick={() => chain().toggleHeading({ level: 2 }).run()}>H2</button>
    <button class:on={is('heading', { level: 3 })} onclick={() => chain().toggleHeading({ level: 3 }).run()}>H3</button>
    <span class="sep"></span>
    <button class:on={is('bulletList')} onclick={() => chain().toggleBulletList().run()} title="Bulleted list">• List</button>
    <button class:on={is('orderedList')} onclick={() => chain().toggleOrderedList().run()} title="Numbered list">1. List</button>
    <button class:on={is('taskList')} onclick={() => chain().toggleTaskList().run()} title="Checklist">☑ List</button>
    <span class="sep"></span>
    <label class="color" title="Text color">A<input type="color" value="#d32f2f" onchange={color} /></label>
    <button onclick={() => chain().unsetColor().run()} title="Clear text color">A✕</button>
    <label class="color hl" title="Highlight">▉<input type="color" value="#fff176" onchange={hl} /></label>
    <button class:on={is('highlight')} onclick={() => chain().unsetHighlight().run()} title="Clear highlight">▉✕</button>
    <span class="sep"></span>
    <button class:on={is('link')} onclick={setLink} title="Link">🔗</button>
    <button class:on={is('codeBlock')} onclick={() => chain().toggleCodeBlock().run()} title="Code block">{'</>'}</button>
    <button onclick={() => chain().setHorizontalRule().run()} title="Horizontal rule">―</button>
    <button onclick={() => chain().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()} title="Insert table">▦</button>
    {#if is('table')}
      <button onclick={() => chain().addRowAfter().run()} title="Add row">+Row</button>
      <button onclick={() => chain().addColumnAfter().run()} title="Add column">+Col</button>
      <button onclick={() => chain().deleteRow().run()} title="Delete row">−Row</button>
      <button onclick={() => chain().deleteColumn().run()} title="Delete column">−Col</button>
      <button onclick={() => chain().deleteTable().run()} title="Delete table">✕Table</button>
    {/if}
    <span class="sep"></span>
    <button onclick={() => chain().undo().run()} title="Undo (Ctrl+Z)">↶</button>
    <button onclick={() => chain().redo().run()} title="Redo (Ctrl+Y)">↷</button>
    <span class="status">{status}</span>
  </div>
  <div class="editor-host" bind:this={host}></div>
</div>
