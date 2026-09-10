<script lang="ts">
  import { onMount } from "svelte";
  import {
    clipsList,
    clipDelete,
    sessionCurrent,
  } from "@honkoku/client-api/invoke";
  import type { Clip } from "@honkoku/client-api/types";
  import { errorMessage } from "../lib";
  import { href } from "../routes";
  import QueuedImage from "./QueuedImage.svelte";
  import Skeleton from "./Skeleton.svelte";
  let {
    character = "",
    embedded = false,
  }: { character?: string; embedded?: boolean } = $props();
  let clips = $state<Clip[]>([]),
    filter = $state(""),
    loading = $state(true),
    error = $state("");
  let signedIn = $state(true),
    deleting = $state<string[]>([]);
  let generation = 0;
  let shown = $derived(
    clips.filter(
      (c) =>
        c.reading.includes(character) &&
        [c.reading, ...c.tags, c.comment]
          .join("\n")
          .toLocaleLowerCase()
          .includes(filter.trim().toLocaleLowerCase()),
    ),
  );
  async function refresh() {
    const request = ++generation;
    loading = true;
    error = "";
    try {
      const session = await sessionCurrent();
      const rows = session ? await clipsList() : [];
      if (request === generation) {
        signedIn = !!session;
        clips = rows;
      }
    } catch (e) {
      if (request === generation) error = errorMessage(e);
    } finally {
      if (request === generation) loading = false;
    }
  }
  async function remove(id: string) {
    deleting = [...deleting, id];
    error = "";
    try {
      await clipDelete(id);
      clips = clips.filter((c) => c.id !== id);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      deleting = deleting.filter((value) => value !== id);
    }
  }
  onMount(() => {
    void refresh();
    return () => {
      generation++;
    };
  });
</script>

<section class="clips" class:embedded aria-label="クリップ">
  {#if !embedded}<h1>クリップ</h1>{/if}
  <label class="filter"
    >読み・タグ・コメント<input
      aria-label="クリップを絞り込み"
      bind:value={filter}
      placeholder="クリップを絞り込み"
    /></label
  >
  {#if error}<p role="alert">
      {error}<button onclick={refresh}>再試行</button>
    </p>{/if}
  {#if loading}<Skeleton count={4} />{:else if !signedIn}<p>
      ログインすると、自分のクリップを表示できます。
    </p>
  {:else}<p class="caption muted">{shown.length}件</p>
    <div class="clip-grid">
      {#each shown as clip (clip.id)}<article class="panel clip-card">
          <a
            class="clip-image"
            href={href({ entryId: clip.entryId, pageIndex: clip.index })}
            ><QueuedImage url={clip.uri} alt={clip.reading} /></a
          >
          <div class="clip-meta">
            <strong>{clip.reading}</strong>{#if clip.isPrivate}<span
                class="privacy">非公開</span
              >{/if}
          </div>
          <p class="tags">{clip.tags.map((t) => `#${t}`).join("　")}</p>
          <p class="comment">{clip.comment}</p>
          <footer>
            <a href={href({ entryId: clip.entryId, pageIndex: clip.index })}
              >コマ{clip.index + 1}へ</a
            ><button
              disabled={deleting.includes(clip.id)}
              onclick={() => remove(clip.id)}>削除</button
            >
          </footer>
        </article>{:else}<p>一致するクリップはありません。</p>{/each}
    </div>
  {/if}
</section>

<style>
  .clips {
    padding: 24px;
    height: 100%;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .embedded {
    padding: 0;
    height: auto;
    overflow: visible;
  }
  .filter {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-width: 440px;
    font-size: 13px;
  }
  .clip-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
    gap: 16px;
  }
  .clip-card {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .clip-image {
    display: block;
    height: 160px;
  }
  .clip-meta,
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .privacy {
    font-size: 11px;
    color: var(--text-muted);
    border: 1px solid var(--border);
    padding: 2px 6px;
    border-radius: 4px;
  }
  .tags,
  footer {
    font-size: 13px;
    color: var(--text-muted);
  }
  .comment {
    font-size: 14px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    flex: 1;
  }
</style>
