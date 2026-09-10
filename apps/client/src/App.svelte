<script lang="ts">
  import { onMount } from 'svelte';
  import { listProjects, getProject, listCollections, getCollection, getEntry, listPages } from '@honkoku/client-api/invoke';
  import type { Project, Collection, Entry, Page, Label } from '@honkoku/client-api/types';
  import { href, parseRoute, type Route } from './routes';
  import { savedTheme, setTheme, type Theme } from './theme';

  let route = $state<Route>(parseRoute(location.hash));
  let projects = $state<Project[]>([]);
  let project = $state<Project | null>(null);
  let collections = $state<Collection[]>([]);
  let collection = $state<Collection | null>(null);
  let entries = $state<Entry[]>([]);
  let entry = $state<Entry | null>(null);
  let pages = $state<Page[]>([]);
  let loading = $state(false);
  let error = $state('');
  let theme = $state<Theme>(savedTheme());
  let generation = 0;
  const page = $derived(pages.find(p => p.id === route.pageId) ?? (route.pageId ? undefined : pages[0]));
  const canvas = $derived(page ? entry?.canvases?.find(c => c.id === page.canvasId) ?? entry?.canvases?.[page.index] : undefined);
  const statusLabels: Record<string, string> = { default: '未着手', initiated: '翻刻中', editing: '編集中', completed: '完了', frozen: '凍結' };
  function label(value: Label) {
    if (typeof value === 'string') return value;
    return (value.ja ?? value.en ?? Object.values(value)[0] ?? []).join('／');
  }
  function statusClass(status: string) { return Object.hasOwn(statusLabels, status) ? status : 'default'; }
  function message(cause: unknown): string {
    if (typeof cause === 'object' && cause !== null && 'message' in cause) return String(cause.message);
    return String(cause);
  }
  async function load(next: Route) {
    const current = ++generation;
    loading = true;
    error = '';
    project = null; collections = []; collection = null; entries = []; entry = null; pages = [];
    try {
      const nextProjects = await listProjects();
      if (current !== generation) return;
      projects = nextProjects;
      if (!next.projectId) return;
      const nextProject = await getProject(next.projectId);
      if (current !== generation) return;
      const nextCollections = await listCollections(next.projectId);
      if (current !== generation) return;
      project = nextProject; collections = nextCollections;
      if (!next.collectionId) return;
      if (!nextCollections.some(item => item.id === next.collectionId)) throw new Error('このプロジェクトにコレクションがありません。');
      const nextCollection = await getCollection(next.collectionId);
      if (current !== generation) return;
      collection = nextCollection;
      if (!nextCollection.entries) throw new Error('資料一覧を取得できませんでした。');
      const nextEntries: Entry[] = [];
      // Bound invoke fanout while the Rust transport also enforces its host limit.
      for (let start = 0; start < nextCollection.entries.length; start += 4) {
        const batch = await Promise.all(nextCollection.entries.slice(start, start + 4).map(getEntry));
        if (current !== generation) return;
        nextEntries.push(...batch);
      }
      entries = nextEntries.sort((a, b) => a.index - b.index);
      if (!next.entryId) return;
      const selected = nextEntries.find(item => item.id === next.entryId);
      if (!selected) throw new Error('このコレクションに資料がありません。');
      const nextPages = await listPages(next.entryId);
      if (current !== generation) return;
      entry = selected; pages = nextPages;
      if (next.pageId && !nextPages.some(item => item.id === next.pageId)) throw new Error('指定されたコマがありません。');
    } catch (cause) { if (current === generation) error = message(cause); }
    finally { if (current === generation) loading = false; }
  }
  onMount(() => {
    void load(route);
    const navigate = () => {
      const next = parseRoute(location.hash);
      const pageOnly = next.projectId === route.projectId && next.collectionId === route.collectionId && next.entryId === route.entryId && entry !== null;
      route = next;
      if (pageOnly) error = next.pageId && !pages.some(page => page.id === next.pageId) ? '指定されたコマがありません。' : '';
      else void load(next);
    };
    window.addEventListener('hashchange', navigate);
    return () => { generation++; window.removeEventListener('hashchange', navigate); };
  });
</script>

<header>
  <a class="brand" href="#/">みんなで翻刻</a>
  <label>表示テーマ
    <select bind:value={theme} onchange={() => setTheme(theme)}>
      <option value="system">システム</option><option value="light">ライト</option><option value="dark">ダーク</option>
    </select>
  </label>
</header>
{#if error}<div class="message" role="alert">{error}<button onclick={() => void load(route)}>再試行</button></div>{/if}
{#if loading}<div class="message" role="status">読み込み中…</div>{/if}
<main aria-busy={loading}>
  <nav class="pane" aria-label="プロジェクトとコレクション">
    <h1>プロジェクト</h1>
    <ul>{#each projects as item (item.id)}
      <li><a class:selected={route.projectId === item.id} aria-current={route.projectId === item.id ? 'true' : undefined} href={href({projectId:item.id})}>{item.title}</a></li>
    {/each}</ul>
    {#if project}
      <h2>コレクション</h2>
      <p>{project.description ?? ''}</p>
      <ul>{#each collections as item (item.id)}
        <li><a class:selected={route.collectionId === item.id} href={href({projectId:project.id,collectionId:item.id})}>{item.title}</a></li>
      {:else}<li>コレクションはありません。</li>{/each}</ul>
    {/if}
  </nav>
  <section class="pane" aria-label="資料とコマ">
    <h2>資料</h2>
    {#if collection}
      <p>{collection.title}</p>
      <ul>{#each entries as item (item.id)}
        <li><a class:selected={route.entryId === item.id} href={href({projectId:route.projectId,collectionId:collection.id,entryId:item.id})}>{label(item.label)}</a></li>
      {:else}{#if !loading}<li>資料はありません。</li>{/if}{/each}</ul>
    {:else}<p class="muted">コレクションを選択してください。</p>{/if}
    {#if entry}
      <h2>コマ</h2>
      <ul>{#each pages as item (item.id)}
        <li><a class:selected={page?.id === item.id} href={href({...route,pageId:item.id})}>
          <span>第{item.index + 1}コマ</span><span class="status {statusClass(item.status)}">{statusLabels[item.status] ?? item.status}</span>
        </a></li>
      {:else}<li>コマはありません。</li>{/each}</ul>
    {/if}
  </section>
  <section class="pane reader" aria-label="本文">
    {#if entry && page}
      <h2>{label(entry.label)}</h2>
      <p>第{page.index + 1}コマ・<span class="status {statusClass(page.status)}">{statusLabels[page.status] ?? page.status}</span></p>
      {#if canvas?.imageUrl}<img src={canvas.imageUrl} alt={`第${page.index + 1}コマの原本画像`} referrerpolicy="no-referrer" />{:else}<p class="muted">画像はありません。</p>{/if}
      <pre>{page.text || '本文はありません。'}</pre>
    {:else if project}
      <h2>{project.title}</h2><p>{project.description ?? ''}</p>
      {#if project.guidelines}<h3>翻刻のガイドライン</h3><pre>{project.guidelines}</pre>{/if}
      <p class="muted">資料とコマを選択してください。</p>
    {:else}<p class="muted">プロジェクトを選択してください。</p>{/if}
  </section>
</main>
