<script lang="ts">
  import { pageHistory } from "@honkoku/client-api/invoke";
  import type { TimelineItem } from "@honkoku/client-api/types";
  import { diffSource } from "@honkoku/markup";
  import { errorMessage, relative } from "../lib";
  import Avatar from "./Avatar.svelte";
  let {
    entryId,
    index,
    current,
    editing,
    disabled,
    onrestore,
    onclose,
  }: {
    entryId: string;
    index: number;
    current: string;
    editing: boolean;
    disabled: boolean;
    onrestore: (source: string) => void;
    onclose: () => void;
  } = $props();
  let items = $state<TimelineItem[]>([]),
    selected = $state(0),
    pending = $state(true),
    error = $state("");
  let horizontal = $state(false),
    compareCurrent = $state(false),
    limit = $state(100),
    reload = $state(0);
  const snapshot = (item?: TimelineItem): Record<string, unknown> =>
    item?.event.data &&
    typeof item.event.data === "object" &&
    !Array.isArray(item.event.data)
      ? item.event.data
      : {};
  const comment = (item: TimelineItem) =>
    typeof item.event.comment === "string"
      ? item.event.comment
      : typeof snapshot(item).comment === "string"
        ? (snapshot(item).comment as string)
        : "";
  const text = (item?: TimelineItem) =>
    typeof snapshot(item).text === "string"
      ? (snapshot(item).text as string)
      : "";
  let changes = $derived(
    diffSource(
      compareCurrent ? text(items[selected]) : text(items[selected + 1]),
      compareCurrent ? current : text(items[selected]),
    ),
  );
  $effect(() => {
    const id = entryId,
      page = index,
      count = limit;
    reload;
    let alive = true;
    pending = true;
    error = "";
    items = [];
    selected = 0;
    void pageHistory(id, page, count + 1)
      .then((values) => {
        if (alive) items = values;
      })
      .catch((e) => {
        if (alive) error = errorMessage(e);
      })
      .finally(() => {
        if (alive) pending = false;
      });
    return () => {
      alive = false;
    };
  });
</script>

<aside
  id="history-side"
  class="history-drawer side-drawer"
  aria-label="編集履歴"
>
  <header>
    <h2>編集履歴</h2>
    <button aria-label="履歴を閉じる" onclick={onclose}>×</button>
  </header>
  {#if pending}<p role="status">履歴を読み込み中…</p>{:else if error}<p
      role="alert"
    >
      {error}
    </p>
    <button onclick={() => reload++}>再試行</button>{:else if !items.length}<p>
      保存された履歴はありません。
    </p>{:else}
    <div class="history-list">
      {#each items.slice(0, limit) as item, i (item.event.id)}
        <button
          class="history-save"
          aria-pressed={selected === i}
          onclick={() => (selected = i)}
        >
          <span class="history-author"
            ><Avatar user={item.actor} small />{item.actor?.displayName ??
              "名前不明"}</span
          >
          <span class="muted"
            >{relative(item.event.createdAt)}・{item.event.count}字変更</span
          >
          <span class="history-tags"
            >{#if snapshot(item).status === "completed"}<span>✓完了</span
              >{/if}{#if item.event.requestReview || snapshot(item).requestReview}<span
                >添削希望</span
              >{/if}{#if item.event.share || snapshot(item).share}<span
                >共有</span
              >{/if}</span
          >
          {#if comment(item)}<span>{comment(item)}</span>{/if}
        </button>
      {/each}
      {#if items.length > limit}<button onclick={() => (limit += 100)}
          >さらに読み込む</button
        >{/if}
    </div>
    <div class="history-controls">
      <label
        ><input type="checkbox" bind:checked={horizontal} />横書きで見る</label
      >
      <label
        ><input
          type="checkbox"
          bind:checked={compareCurrent}
        />現在の内容と比較</label
      >
      <p class="muted">
        {compareCurrent ? "選択した保存→現在の内容" : "前の保存→選択した保存"}
      </p>
      <button
        class="primary"
        {disabled}
        onclick={() => onrestore(text(items[selected]))}
        >{editing ? "復元" : "編集を開始して復元"}</button
      >
    </div>
    <div class="history-diff" class:horizontal aria-label="翻刻文の差分">
      {#each changes as change}{#if change.kind === "insert"}<ins
            >{change.text}</ins
          >{:else if change.kind === "delete"}<del>{change.text}</del
          >{:else}<span>{change.text}</span>{/if}{/each}
    </div>
  {/if}
</aside>

<style>
  .history-list {
    max-height: 42%;
    overflow: auto;
    border-bottom: 1px solid var(--border);
  }
  .history-save {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
    text-align: left;
    border-radius: 0;
    border-width: 0 0 1px;
    padding: 12px;
  }
  .history-save[aria-pressed="true"] {
    background: var(--accent-soft);
    border-inline-start: 3px solid var(--accent);
  }
  .history-author {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .history-tags {
    display: flex;
    gap: 8px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .history-controls {
    padding: 12px;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    font-size: 13px;
  }
  .history-controls p {
    margin: 0;
    width: 100%;
  }
  .history-controls label {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .history-diff {
    padding: 16px;
    writing-mode: vertical-rl;
    white-space: pre-wrap;
    font: 22px/1.8 var(--font-serif);
    color: var(--text-muted);
    overflow: auto;
    flex: 1;
    min-height: 130px;
    overflow-wrap: anywhere;
  }
  .history-diff.horizontal {
    writing-mode: horizontal-tb;
  }
  .history-diff ins {
    color: var(--accent);
    background: var(--accent-soft);
    text-decoration: none;
  }
  .history-diff del {
    color: light-dark(#a34d4d, #da9696);
    text-decoration: line-through;
  }
</style>
