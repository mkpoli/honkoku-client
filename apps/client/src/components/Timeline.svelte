<script lang="ts">
  import { untrack } from "svelte";
  import { Region } from "../region.svelte";
  import Skeleton from "./Skeleton.svelte";
  import RegionNotice from "./RegionNotice.svelte";
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
  let reviewOnly = $state(false);
  $effect(() => {
    if (projectId) tab = "project";
  });
  const region = new Region<TimelineItem[]>();
  let items = $derived(region.value ?? []);
  let visible = $derived(
    items.filter((item) => !reviewOnly || item.event.requestReview === true),
  );
  let loading = $derived(region.pending);
  let error = $derived(region.error);
  let more = $state(true);
  let now = $state(Date.now());
  function load(append = false) {
    const previous = append ? items.at(-1)?.event : undefined;
    const retained = items;
    const filter = {
      project_id:
        projectId && (compact || tab === "project") ? projectId : undefined,
      joined: tab === "joined",
      before: previous?.createdAt,
      before_id: previous?.id,
    };
    return region.load(
      `timeline:${session?.uid}:${projectId}:${tab}`,
      async () => {
        const batch = await homeTimeline(filter, 20);
        more = batch.length === 20;
        return [
          ...new Map(
            (append ? [...retained, ...batch] : batch).map((i) => [
              i.event.id,
              i,
            ]),
          ).values(),
        ];
      },
      (value) => {
        now = Date.now();
        onitems?.(value);
      },
    );
  }
  $effect(() => {
    projectId;
    session?.uid;
    tab;
    void untrack(() => load());
    return () => region.cancel();
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
  {#if !projectId}<div class="chips">
      <button
        aria-pressed={reviewOnly}
        class:active={reviewOnly}
        onclick={() => (reviewOnly = !reviewOnly)}>添削希望のみ表示</button
      >
    </div>{/if}
  <div class="timeline-items scroll">
    {#each visible as item (item.event.id)}
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
            {#if item.event.isReview || item.event.requestReview}<span
                class="caption review-chip">添削希望</span
              >{/if}
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
        {#if item.excerpt}<blockquote>
            <span class="excerpt-text">{item.excerpt}</span>
          </blockquote>{/if}
      </article>
    {:else}{#if !loading && !error}<p class="empty">
          活動はまだありません。
        </p>{/if}{/each}
    <RegionNotice {region} />
    {#if loading && !items.length}<Skeleton label="活動を取得中" count={5} />
    {:else if more && !error}<button
        class="load-more"
        disabled={loading}
        onclick={() => load(true)}>さらに表示⌄</button
      >{/if}
  </div>
</section>
