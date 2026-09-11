<script lang="ts">
  import Skeleton from "./Skeleton.svelte";
  import RegionNotice from "./RegionNotice.svelte";
  import type { Region } from "../region.svelte";
  import { tick } from "svelte";
  import type {
    Canvas,
    Entry,
    Page,
    SessionInfo,
  } from "@honkoku/client-api/types";
  import { label, number, status, statusClass, user } from "../lib";
  import { href } from "../routes";
  import Thumbnail from "./Thumbnail.svelte";
  import ExternalLink from "./ExternalLink.svelte";
  let {
    entry,
    pages,
    canvases,
    session,
    pending = false,
    pagesRegion,
    canvasesRegion,
  }: {
    entry: Entry;
    pages: Page[];
    canvases: Canvas[];
    session: SessionInfo | null;
    pending?: boolean;
    pagesRegion: Region<Page[]>;
    canvasesRegion: Region<Canvas[]>;
  } = $props();
  let counts = $derived(
    [...new Set(pages.map((p) => p.status))].map((s) => ({
      s,
      count: pages.filter((p) => p.status === s).length,
    })),
  );
  let filter = $state("all");
  let editors = $state<Record<string, string>>({});
  const filters = [
    ["all", "すべて"],
    ["default", "未着手"],
    ["initiated", "翻刻中"],
    ["completed", "完了"],
  ];
  let visible = $derived(
    pages.filter((p) => filter === "all" || p.status === filter),
  );
  let firstUnfinished = $derived(
    pages.find((p) => p.status === "default" || p.status === "initiated"),
  );
  $effect(() => {
    try {
      const saved = localStorage.getItem(`honkoku.filter.entry.${entry.id}`);
      filter = filters.some(([id]) => id === saved) ? saved! : "all";
    } catch {
      filter = "all";
    }
  });
  function choose(value: string) {
    filter = value;
    try {
      localStorage.setItem(`honkoku.filter.entry.${entry.id}`, value);
    } catch {}
  }
  async function jump() {
    const target = firstUnfinished;
    if (!target) return;
    if (filter !== "all" && filter !== target.status) choose("all");
    await tick();
    const card = document.getElementById(`page-${target.index}`);
    card?.focus({ preventScroll: true });
    card?.scrollIntoView({ block: "center" });
  }
  $effect(() => {
    const ids = [
      ...new Set(
        pages
          .filter((p) => p.status === "editing")
          .map((p) => p.tempEditedBy)
          .filter((id): id is string => !!id),
      ),
    ];
    let cancelled = false;
    for (const id of ids)
      void user(id)
        .then((u) => {
          if (!cancelled) editors[id] = u.displayName;
        })
        .catch(() => {
          if (!cancelled) editors[id] = "名前不明";
        });
    return () => {
      cancelled = true;
    };
  });
</script>

<div class="entry-screen scroll">
  <section class="panel entry-header">
    <h1 class="serif">{label(entry.label)}</h1>
    <p>
      {label(entry.attribution)}{#if entry.license}<span class="licence"
          ><ExternalLink href={entry.license}>利用条件↗</ExternalLink></span
        >{/if}
    </p>
    <div class="entry-summary">
      <strong>{number(entry.size)}コマ</strong>{#each counts as c}<span
          class="status {statusClass(c.s)}"
          >{status(c.s).symbol}{status(c.s).label}{c.count}</span
        >{:else}<p class="empty">この状態のコマはありません。</p>{/each}
    </div>
  </section>
  <section class="panel page-grid-panel">
    <h2>コマ一覧</h2>
    <RegionNotice region={pagesRegion} />
    <RegionNotice region={canvasesRegion} />
    <div class="page-filter-row">
      <div class="chips page-filters" role="group" aria-label="コマの状態">
        {#each filters as [value, text]}<button
            class:active={filter === value}
            aria-pressed={filter === value}
            onclick={() => choose(value)}
            >{text}<span class="count"
              >{value === "all"
                ? pages.length
                : pages.filter((p) => p.status === value).length}</span
            ></button
          >{/each}
      </div>
      <button disabled={!firstUnfinished} onclick={jump}>次の未着手へ</button>
    </div>
    {#if pending}<Skeleton shape="cards" count={12} />{:else}<div
        class="page-grid"
      >
        {#each visible as p (p.id)}<a
            id={`page-${p.index}`}
            data-index={p.index}
            class="page-card {statusClass(p.status)}"
            href={href({ entryId: entry.id, pageIndex: p.index })}
            >{#if p.status === "default" || p.status === "initiated"}<span
                class="page-tag"
                >{p.status === "initiated" ? "◐翻刻中" : "未着手"}</span
              >{:else if p.status === "completed"}<span
                class="page-check"
                aria-label="完了">✓</span
              >{/if}<Thumbnail
              url={canvases[p.index]?.thumbnailUrl ??
                canvases[p.index]?.imageUrl}
              alt={`コマ${p.index + 1}の原本`}
            />
            <div>
              <strong>{p.index + 1}</strong>{#if p.approvedBy?.length}<span
                  class="caption">✓{p.approvedBy.length}／2</span
                >{/if}<span class="status {statusClass(p.status)}"
                >{status(p.status).symbol}{status(p.status).label}</span
              >
            </div>
            {#if p.status === "editing"}<p class="caption locked-editor">
                <span aria-label="ロック中">🔒</span>{p.tempEditedBy ===
                session?.uid
                  ? "あなたが編集中"
                  : p.tempEditedBy
                    ? (editors[p.tempEditedBy] ?? "名前を確認中")
                    : "名前不明"}
              </p>{/if}</a
          >{:else}<p class="empty">この状態のコマはありません。</p>{/each}
      </div>{/if}
  </section>
</div>
