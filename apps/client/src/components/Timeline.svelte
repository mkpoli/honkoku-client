<script lang="ts">
  import type { SessionInfo, TimelineItem } from "@honkoku/client-api/types";
  import { homeTimeline } from "@honkoku/client-api/invoke";
  import { action, errorMessage, label, relative } from "../lib";
  import { href } from "../routes";
  import Avatar from "./Avatar.svelte";
  import Thumbnail from "./Thumbnail.svelte";
  let {
    projectId,
    session,
    compact = false,
    title = "タイムライン",
    onitems,
  }: {
    projectId?: string;
    session: SessionInfo | null;
    compact?: boolean;
    title?: string;
    onitems?: (items: TimelineItem[]) => void;
  } = $props();
  let tab = $state("all");
  $effect(() => {
    if (projectId) tab = "project";
  });
  let items = $state<TimelineItem[]>([]);
  let loading = $state(false),
    error = $state(""),
    more = $state(true);
  let generation = 0;
  let now = $state(Date.now());
  async function load(append = false) {
    const current = ++generation;
    const previous = append ? items.at(-1)?.event : undefined;
    loading = true;
    error = "";
    try {
      const batch = await homeTimeline(
        {
          project_id:
            projectId && (compact || tab === "project") ? projectId : undefined,
          joined: tab === "joined",
          before: previous?.createdAt,
          before_id: previous?.id,
        },
        20,
      );
      if (current !== generation) return;
      const combined = append ? [...items, ...batch] : batch;
      items = [...new Map(combined.map((i) => [i.event.id, i])).values()];
      more = batch.length === 20;
      now = Date.now();
      onitems?.(items);
    } catch (e) {
      if (current === generation) error = errorMessage(e);
    } finally {
      if (current === generation) loading = false;
    }
  }
  $effect(() => {
    projectId;
    session?.uid;
    tab;
    items = [];
    more = true;
    void load();
    return () => {
      generation++;
    };
  });
</script>

<section class="panel timeline-panel" class:compact aria-label={title}>
  <h2>{title}</h2>
  <div class="tabs" aria-label="活動の範囲">
    {#if !compact}<button
        class:active={tab === "all"}
        onclick={() => (tab = "all")}>すべて</button
      ><button
        class:active={tab === "joined"}
        disabled={!session}
        onclick={() => (tab = "joined")}>参加中</button
      >{/if}
    {#if projectId}<button
        class:active={compact || tab === "project"}
        onclick={() => (tab = "project")}>このプロジェクト</button
      >{/if}
  </div>
  <div class="timeline-items scroll">
    {#each items as item (item.event.id)}
      <article class="activity">
        <div class="activity-main">
          <Avatar user={item.actor} small={compact} />
          <div class="activity-copy">
            <div class="actor-line">
              <strong>{item.actor?.displayName ?? "名前不明"}</strong><time
                datetime={item.event.createdAt}
                >{relative(item.event.createdAt, now)}</time
              >
            </div>
            <a
              class="activity-action"
              href={href({
                entryId: item.event.entryId,
                pageIndex: item.event.index,
              })}>{action(item)}</a
            >
            <a
              class="activity-label"
              href={href({ entryId: item.event.entryId })}
              >{label(item.entryLabel) || "資料"}<span
                >コマ{item.event.index + 1}</span
              ></a
            >
          </div>
          <a
            class="activity-image"
            href={href({
              entryId: item.event.entryId,
              pageIndex: item.event.index,
            })}
            aria-label={`コマ${item.event.index + 1}を開く`}
            ><Thumbnail
              entryId={item.event.entryId}
              index={item.event.index}
            /></a
          >
        </div>
        {#if item.excerpt}<blockquote>{item.excerpt}</blockquote>{/if}
      </article>
    {:else}{#if !loading && !error}<p class="empty">
          活動はまだありません。
        </p>{/if}{/each}
    {#if error}<p class="error" role="alert">{error}</p>
      <button onclick={() => load(items.length > 0)}>再試行</button>{/if}
    {#if loading}<p class="empty" role="status">
        読み込み中…
      </p>{:else if more && !error}<button
        class="load-more"
        onclick={() => load(true)}>さらに表示⌄</button
      >{/if}
  </div>
</section>
