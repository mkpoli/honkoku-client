<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type {
    SearchMode,
    SearchProgress,
    SearchResults,
    SearchStatus,
  } from "@honkoku/client-api/types";
  import {
    isTauri,
    listProjects,
    onSearchError,
    onSearchProgress,
    searchBuild,
    searchChooseDump,
    searchQuery,
    searchStatus,
    searchSync,
  } from "@honkoku/client-api/invoke";
  import { errorMessage } from "../lib";
  import { href } from "../routes";

  let { query }: { query: string } = $props();
  let input = $state("");
  let mode = $state<SearchMode>("Strict");
  let project = $state<string | null>(null);
  let vertical = $state(false);
  let status = $state<SearchStatus>();
  let result = $state<SearchResults>();
  let projectNames = $state<Record<string, string>>({});
  let busy = $state(false),
    building = $state(false),
    syncing = $state(false);
  let error = $state("");
  let progress = $state<SearchProgress>();
  let generation = 0;
  const desktop = isTauri();
  const displayText = (text: string) => text.replace(/[\r\n\u2028\u2029\t]/g, " ");
  const number = (n: number) => n.toLocaleString("ja-JP");
  let website = $derived(
    `https://app.honkoku.org/search?keyword=${encodeURIComponent(query)}`,
  );

  $effect(() => {
    input = query;
    project = null;
  });
  $effect(() => {
    const text = query,
      selectedMode = mode,
      selectedProject = project;
    if (!status?.present || building) return;
    untrack(() => void run(text, selectedMode, selectedProject));
  });
  async function run(
    text: string,
    selectedMode: SearchMode,
    selectedProject: string | null,
    more = false,
  ) {
    const request = ++generation;
    const cursor = more ? (result?.next_cursor ?? null) : null;
    if (!more) result = undefined;
    busy = true;
    error = "";
    try {
      const next = await searchQuery({
        text,
        mode: selectedMode,
        project: selectedProject,
        entry: null,
        limit: 20,
        cursor,
      });
      if (request !== generation) return;
      for (const hit of next.hits)
        if (!projectNames[hit.project_id]) projectNames[hit.project_id] = hit.project_title || hit.project_id;
      result =
        more && result
          ? { ...next, hits: [...result.hits, ...next.hits] }
          : next;
    } catch (e) {
      if (request === generation) error = errorMessage(e);
    } finally {
      if (request === generation) busy = false;
    }
  }
  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (query === input) void run(query, mode, project);
    else location.hash = href({ search: true, query: input });
  }
  async function build(choose = false, clone = false) {
    error = "";
    try {
      const folder = choose ? await searchChooseDump() : undefined;
      if (choose && !folder) return;
      building = true;
      progress = undefined;
      status = await searchBuild(folder ?? undefined, clone);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      building = false;
    }
  }
  async function sync() {
    syncing = true;
    error = "";
    try {
      await searchSync();
      status = await searchStatus();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      syncing = false;
    }
  }
  async function openWebsite(event: MouseEvent) {
    if (!desktop) return;
    event.preventDefault();
    try {
      await openUrl(website);
    } catch (e) {
      error = errorMessage(e);
    }
  }
  onMount(() => {
    let alive = true;
    const stops: (() => void)[] = [];
    void (async () => {
      try {
        for (const register of [
          onSearchProgress((p) => {
            progress = p;
          }),
          onSearchError((e) => {
            error = errorMessage(e);
          }),
        ]) {
          const stop = await register;
          if (alive) stops.push(stop);
          else stop();
        }
        const value = await searchStatus();
        if (alive) status = value;
      } catch (e) {
        if (alive) error = errorMessage(e);
      }
    })();
    void listProjects()
      .then((projects) => {
        if (alive)
          projectNames = {
            ...projectNames,
            ...Object.fromEntries(projects.map((p) => [p.id, p.title])),
          };
      })
      .catch(() => {});
    return () => {
      alive = false;
      generation++;
      stops.forEach((stop) => stop());
    };
  });
</script>

<section class="concordance" aria-label="全文検索">
  <div class="search-heading">
    <div>
      <h1>全文検索</h1>
      <p class="muted">翻刻の前後を読み比べる</p>
    </div>
    <a href={website} target="_blank" rel="noreferrer" onclick={openWebsite}
      >みんなで翻刻のサイトで検索↗</a
    >
  </div>
  <form class="panel search-form" onsubmit={submit}>
    <label class="query-label"
      >検索語句<input
        aria-label="検索語句"
        bind:value={input}
        placeholder="蝦夷"
        onkeydown={(e) => {
          if (e.key === "Enter" && e.isComposing) e.preventDefault();
        }}
      /></label
    >
    <div class="search-modes" role="group" aria-label="検索方法">
      <button
        type="button"
        aria-pressed={mode === "Strict"}
        onclick={() => (mode = "Strict")}>厳密</button
      >
      <button
        type="button"
        aria-pressed={mode === "Folded"}
        onclick={() => (mode = "Folded")}>表記ゆれを含む</button
      >
    </div>
    <button class="primary" disabled={building || !status?.present}>検索</button
    >
  </form>
  {#if error}<div class="panel error" role="alert">
      {error}<button
        onclick={() => {
          if (status?.present) void run(query, mode, project);
          else
            void searchStatus()
              .then((s) => (status = s))
              .catch((e) => (error = errorMessage(e)));
        }}>再試行</button
      >
    </div>{/if}
  {#if !status && !error}<p role="status">検索索引を確認中…</p>{/if}
  {#if status && (!status.present || building)}
    <section class="panel search-setup" aria-label="検索索引の作成">
      <h2>翻刻を端末で検索する</h2>
      <p>
        公開翻刻データから検索索引を作成します。一度作成すると、通信せずに本文を検索できます。
      </p>
      <p class="muted">
        公開データ約833MB・約28万コマ。検索索引用の空き容量も必要です。
      </p>
      <div class="setup-actions">
        <button disabled={building} onclick={() => build(true)}
          >フォルダーを選択</button
        >
        {#if status.configured}<button
            disabled={building}
            onclick={() => build()}>設定済みのデータから作成</button
          >{/if}
        {#if status.git_available}<button
            disabled={building}
            onclick={() => build(false, true)}>クローン</button
          >{/if}
      </div>
      {#if building}<div class="build-progress" role="status">
          <p>
            {progress
              ? `${number(progress.done)}／${number(progress.total)}コマ`
              : "データを準備中…"}
          </p>
          <progress
            aria-label="検索索引の作成"
            max={progress?.total || 1}
            value={progress ? progress.done : undefined}
          ></progress>
        </div>{/if}
    </section>
  {:else if status?.present}
    <div class="search-layout">
      <aside class="panel search-facets" aria-label="プロジェクトで絞り込み">
        <h2>プロジェクト</h2>
        <button
          class:chosen={!project}
          onclick={() => (project = null)}
          aria-pressed={!project}>すべて</button
        >
        {#each result?.facets ?? [] as [id, count] (id)}
          <button
            class:chosen={project === id}
            onclick={() => (project = id)}
            aria-pressed={project === id}
            ><span>{projectNames[id] || id}</span><span class="facet-count"
              >{number(count)}</span
            ></button
          >
        {/each}
        {#if project}<button
            class="clear-facet"
            onclick={() => (project = null)}>絞り込みを解除</button
          >{/if}
      </aside>
      <section
        class="panel search-results"
        aria-label="検索結果"
        aria-busy={busy}
      >
        <div class="results-heading">
          <h2>{result ? `${number(result.total)}コマ` : "検索結果"}</h2>
          <button aria-pressed={vertical} onclick={() => (vertical = !vertical)}
            >縦組み</button
          >
        </div>
        {#if !desktop}<p class="caption muted fixture-note">
            ブラウザーでは「蝦夷」の検索結果の一部を表示しています。
          </p>{/if}
        {#if !query.trim()}<p class="search-empty">
            検索する語句を入力してください。
          </p>
        {:else if result?.total === 0}<p class="search-empty">
            一致する翻刻はありません。
          </p>
        {:else if result}
          <div
            class="kwic-list"
            class:vertical
            aria-label={vertical ? "縦組みの検索結果" : "横組みの検索結果"}
          >
            {#each result.hits as hit (hit.page_id)}
              {#each hit.occurrences as occurrence, n (`${hit.page_id}-${n}`)}
                <a
                  class="kwic-row"
                  href={href({
                    entryId: hit.entry_id,
                    pageIndex: hit.index,
                    column: occurrence.column,
                  })}
                >
                  <span class="kwic-context"
                    ><span class="kwic-before">{displayText(occurrence.before)}</span><strong
                      class="kwic-match">{displayText(occurrence.matched)}</strong
                    ><span class="kwic-after">{displayText(occurrence.after)}</span></span
                  >
                  <span class="kwic-caption"
                    >{hit.entry_label ||
                      hit.entry_id}・コマ{hit.index}・{projectNames[hit.project_id] ||
                      hit.project_title ||
                      hit.project_id}</span
                  >
                </a>
              {/each}
            {/each}
          </div>
        {/if}
        {#if busy}<p class="search-loading" role="status">検索中…</p>{/if}
        {#if result?.next_cursor}<button
            class="search-more"
            disabled={busy}
            onclick={() => run(query, mode, project, true)}>さらに表示</button
          >{/if}
      </section>
    </div>
    {#if desktop}<details class="index-status">
        <summary>検索索引・{number(status.page_count)}コマ</summary>
        <p>
          最終作成：{status.last_build
            ? new Date(status.last_build * 1000).toLocaleString("ja-JP")
            : "未作成"}・{number(Math.round(status.size / 1024 / 1024))}MB
        </p>
        {#if status.commit}<p class="caption">
            公開データの版：<code>{status.commit}</code>
          </p>{/if}
        <div class="setup-actions">
          <button disabled={syncing} onclick={sync}
            >{syncing ? "更新中…" : "保存済みのコマを同期"}</button
          ><button onclick={() => build(true)}>公開データを更新</button>
        </div>
      </details>{/if}
  {/if}
</section>

<style>
  .concordance {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
    min-height: 0;
    height: 100%;
    overflow: auto;
  }
  .concordance > * { flex-shrink: 0; }
  .search-heading,
  .results-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .search-heading p {
    margin-block-start: 4px;
  }
  .search-heading > a {
    font-size: 13px;
  }
  .search-form {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: 16px;
    padding: 16px;
  }
  .query-label {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 4px;
    min-inline-size: 180px;
    font-size: 13px;
  }
  .query-label input {
    font-size: 18px;
    line-height: 26px;
  }
  .search-form > button,
  .search-modes button {
    min-block-size: 40px;
  }
  .search-modes {
    display: flex;
    gap: 4px;
  }
  [aria-pressed="true"] {
    background: var(--accent-soft);
    border-color: var(--accent);
  }
  .search-layout {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 16px;
    align-items: start;
    min-height: 0;
  }
  .search-facets,
  .search-results,
  .search-setup {
    padding: 16px;
    min-inline-size: 0;
  }
  .search-facets h2 {
    font-size: 15px;
  }
  .search-facets button {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    inline-size: 100%;
    margin-block-end: 4px;
    text-align: start;
    border-color: transparent;
    background: transparent;
    overflow-wrap: anywhere;
  }
  .search-facets button.chosen {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .facet-count {
    font-size: 13px;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .clear-facet {
    font-size: 13px;
    color: var(--link);
    margin-block-start: 12px;
  }
  .results-heading h2 {
    margin: 0;
  }
  .fixture-note {
    margin-block: 8px 16px;
  }
  .kwic-list {
    margin-block-start: 16px;
  }
  .kwic-row {
    display: block;
    padding-block: 14px;
    border-block-start: 1px solid var(--border);
    color: var(--text);
  }
  .kwic-row:hover {
    background: var(--surface-inset);
    text-decoration: none;
  }
  .kwic-context {
    display: grid;
    grid-template-columns: minmax(0, 1fr) max-content minmax(0, 1fr);
    align-items: baseline;
    gap: 12px;
    font-family: var(--font-serif);
    font-size: 20px;
    line-height: 32px;
  }
  .kwic-before,
  .kwic-after {
    white-space: pre;
    overflow: hidden;
  }
  .kwic-before {
    text-align: end;
    color: var(--text-muted);
  }
  .kwic-match {
    color: var(--accent);
    max-inline-size: 26em;
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }
  .kwic-caption {
    display: block;
    margin-block-start: 6px;
    font-size: 13px;
    line-height: 18px;
    color: var(--text-muted);
    text-align: center;
  }
  .vertical {
    display: flex;
    flex-direction: row-reverse;
    gap: 0;
    overflow-x: auto;
    padding-block-end: 12px;
    max-block-size: 610px;
  }
  .vertical .kwic-row {
    display: flex;
    flex-direction: row-reverse;
    align-items: stretch;
    flex-shrink: 0;
    padding: 16px;
    border-block-start: 0;
    border-inline-start: 1px solid var(--border);
  }
  .vertical .kwic-context {
    writing-mode: vertical-rl;
    grid-template-columns: 220px max-content 220px;
    gap: 8px;
    line-height: 36px;
  }
  .vertical .kwic-before,
  .vertical .kwic-after {
    white-space: pre;
  }
  .vertical .kwic-match {
    max-inline-size: 100px;
  }
  .vertical .kwic-caption {
    writing-mode: vertical-rl;
    text-align: start;
    margin: 0;
    margin-inline-end: 8px;
    max-height: 500px;
  }
  .search-empty,
  .search-loading {
    padding: 32px 16px;
    text-align: center;
    color: var(--text-muted);
  }
  .search-more {
    display: block;
    margin: 16px auto 0;
  }
  .search-setup {
    max-inline-size: 760px;
  }
  .search-setup p {
    margin-block: 12px;
  }
  .setup-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-block-start: 16px;
  }
  .build-progress progress {
    inline-size: 100%;
    accent-color: var(--accent);
  }
  .error {
    display: flex;
    gap: 16px;
    align-items: center;
    padding: 16px;
  }
  .index-status {
    color: var(--text-muted);
    font-size: 13px;
  }
  .index-status p {
    margin-block-start: 8px;
    overflow-wrap: anywhere;
  }
  @media (max-width: 800px) {
    .concordance {
      padding: 16px;
    }
    .search-layout {
      grid-template-columns: 150px minmax(0, 1fr);
    }
    .search-heading {
      align-items: start;
    }
  }
</style>
