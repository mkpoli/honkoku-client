<script lang="ts">
  import { onMount } from "svelte";
  import type { RecentWork } from "@honkoku/client-api/types";
  import { historyRecent } from "@honkoku/client-api/invoke";
  import { errorMessage, label, relative, status, statusClass } from "../lib";
  import { href } from "../routes";
  import Thumbnail from "./Thumbnail.svelte";
  let items = $state<RecentWork[]>([]),
    loading = $state(true),
    error = $state("");
  async function load() {
    loading = true;
    error = "";
    try {
      items = await historyRecent(8);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }
  onMount(() => {
    void load();
  });
</script>

<section class="panel recent-work" aria-label="最近の作業">
  <h2>最近の作業</h2>
  <div class="recent-items scroll">
    {#each items as item (item.entryId)}
      {@const next =
        item.statusAfter === "completed" ? item.nextUnfinishedIndex : null}
      {@const when =
        item.savedAt && item.savedAt > item.openedAt
          ? item.savedAt
          : item.openedAt}
      <article class="recent-row" data-entry-id={item.entryId}>
        <a
          class="recent-thumbnail"
          href={href({ entryId: item.entryId, pageIndex: item.index })}
          aria-label={`コマ${item.index + 1}を開く`}
          ><Thumbnail entryId={item.entryId} index={item.index} /></a
        >
        <div class="recent-copy">
          <h3>
            <a href={href({ entryId: item.entryId })}
              >{label(item.entryLabel) || item.entryId}</a
            >
          </h3>
          <p class="caption muted">{item.projectTitle ?? item.projectId}</p>
          <p class="caption">
            コマ{item.index + 1}<span
              class="status {statusClass(item.statusAfter)}"
              >{status(item.statusAfter).symbol}{status(item.statusAfter)
                .label}</span
            >
          </p>
          <time class="caption muted" datetime={when}>{relative(when)}</time>
        </div>
        <a
          class="button recent-resume"
          href={href({ entryId: item.entryId, pageIndex: next ?? item.index })}
          >{next !== null ? "次の未着手へ" : "続きから"}</a
        >
      </article>
    {:else}{#if !loading && !error}<p class="empty">
          最近の作業はありません。
        </p>{/if}{/each}
    {#if loading}<p class="empty" role="status">履歴を読み込み中…</p>{/if}
    {#if error}<p class="error" role="alert">
        {error}<button onclick={load}>再試行</button>
      </p>{/if}
  </div>
</section>
