<script lang="ts">
  import { Region } from "../region.svelte";
  import Skeleton from "./Skeleton.svelte";
  import RegionNotice from "./RegionNotice.svelte";
  import { untrack } from "svelte";
  import type {
    Collection,
    EntrySummary,
    EntryProgress,
    CollectionProgress,
    Project,
    SessionInfo,
    TimelineItem,
  } from "@honkoku/client-api/types";
  import {
    projectPageActivity,
    listCollections,
    listEntrySummaries,
    collectionProgress,
    entryProgress,
    isTauri,
  } from "@honkoku/client-api/invoke";
  import { concurrentEach } from "../data";
  import { aggregate, errorMessage, label, number, user } from "../lib";
  import { href } from "../routes";
  import SortControl from "./SortControl.svelte";
  import Progress from "./Progress.svelte";
  import Thumbnail from "./Thumbnail.svelte";
  import Timeline from "./Timeline.svelte";
  import Markdown from "./Markdown.svelte";
  import Avatar from "./Avatar.svelte";
  let {
    project,
    metadataPending = false,
    projectRegion,
    collectionId,
    session,
    oncollection,
  }: {
    project: Project;
    metadataPending?: boolean;
    projectRegion: Region<Project>;
    collectionId?: string;
    session: SessionInfo | null;
    oncollection: (c: Collection | null) => void;
  } = $props();
  const activityRegion = new Region<Record<string, string>>();
  const collectionRegion = new Region<Collection[]>();
  const entriesRegion = new Region<EntrySummary[]>();
  const figuresRegion = new Region<EntryProgress[]>();
  let progressRegions = $state<Record<string, Region<CollectionProgress>>>({});
  let collections = $state<Collection[]>([]),
    selected = $state<Collection | null>(null),
    entries = $state<EntrySummary[]>([]),
    activity = $state<TimelineItem[]>([]);
  let tab = $state("collections"),
    organizer = $state("主催者を確認中"),
    search = $state(""),
    error = $state(""),
    loading = $derived(entriesRegion.pending),
    collectionsLoading = $derived(collectionRegion.pending);
  let progress = $state<Record<string, CollectionProgress>>({});
  let entryFigures = $state<Record<string, EntryProgress>>({});
  let collectionSort = $state("updated"),
    entrySort = $state("platform");
  const completion = (p?: { completed: number; size: number }) =>
    p && p.size > 0 ? p.completed / p.size : 0;
  let latest = $derived.by(() => {
    const result: Record<string, number> = {};
    const parents = new Map(
      collections.flatMap((c) =>
        (c.entries ?? []).map((id) => [id, c.id] as const),
      ),
    );
    for (const [entryId, updatedAt] of Object.entries(
      activityRegion.value ?? {},
    )) {
      const id = parents.get(entryId);
      if (id)
        result[id] = Math.max(result[id] ?? 0, Date.parse(updatedAt) || 0);
    }
    return result;
  });
  let sortedCollections = $derived(
    [...collections].sort((a, b) => {
      if (collectionSort === "name")
        return a.title.localeCompare(b.title, "ja");
      if (collectionSort.startsWith("progress"))
        return (
          (completion(progress[a.id]) - completion(progress[b.id])) *
          (collectionSort === "progress-desc" ? -1 : 1)
        );
      if (collectionSort === "size")
        return (
          (progress[b.id]?.entries ?? b.entryCount ?? 0) -
          (progress[a.id]?.entries ?? a.entryCount ?? 0)
        );
      if (collectionSort === "updated")
        return (latest[b.id] ?? 0) - (latest[a.id] ?? 0);
      return 0;
    }),
  );
  let sortedEntries = $derived(
    [...entries].sort((a, b) => {
      if (entrySort === "name")
        return label(a.label).localeCompare(label(b.label), "ja");
      if (entrySort.startsWith("progress"))
        return (
          (completion(entryFigures[a.id]) - completion(entryFigures[b.id])) *
          (entrySort === "progress-desc" ? -1 : 1)
        );
      return 0;
    }),
  );
  let projectGeneration = 0;
  let selectionGeneration = 0;
  let totals = $derived(selected ? progress[selected.id] : undefined);
  let summed = $derived(
    Object.values(progress).reduce(
      (sum, p) => ({
        size: sum.size + p.size,
        completed: sum.completed + p.completed,
        entries: sum.entries + p.entries,
      }),
      { size: 0, completed: 0, entries: 0 },
    ),
  );
  function differs(actual: number, reported: number | null | undefined) {
    return (
      reported != null &&
      Math.abs(actual - reported) > Math.abs(reported) * 0.01
    );
  }
  let discrepancy = $derived(
    collections.length > 0 &&
      collections.every((c) => progress[c.id]) &&
      (differs(summed.size, project.totalImageCount) ||
        differs(summed.completed, project.completedImageCount) ||
        differs(summed.entries, project.totalEntryCount)),
  );
  let contributors = $derived(aggregate(activity));
  async function loadProgress(c: Collection, g: number, refresh = false) {
    if (g !== projectGeneration) return;
    const region = (progressRegions[c.id] ??= new Region<CollectionProgress>());
    await region.load(
      `progress:${session?.uid}:${c.id}`,
      () => collectionProgress(c.id, refresh),
      (value) => {
        if (g === projectGeneration) progress = { ...progress, [c.id]: value };
      },
      { kind: "collectionProgress", id: c.id },
    );
  }
  function loadProject(projectId: string) {
    const g = ++projectGeneration;
    activity = [];
    progress = {};
    Object.values(progressRegions).forEach((r) => r.cancel());
    progressRegions = {};
    return collectionRegion.load(
      `collections:${session?.uid}:${projectId}`,
      () => listCollections(projectId),
      (list) => {
        collections = list.filter((c) => c.display !== false);
        void concurrentEach(collections, (c) => loadProgress(c, g));
      },
      { kind: "collections", id: projectId },
    );
  }
  function loadSelection(c: Collection | undefined, explicit: boolean) {
    const g = ++selectionGeneration;
    selected = c ?? null;
    entries = [];
    entryFigures = {};
    entriesRegion.cancel();
    figuresRegion.cancel();
    oncollection(explicit ? (c ?? null) : null);
    if (!c) return;
    void entriesRegion.load(
      `entries:${session?.uid}:${c.id}`,
      () => listEntrySummaries(c.id),
      (rows) => {
        if (g !== selectionGeneration) return;
        entries = rows;
        void figuresRegion.load(
          `figures:${session?.uid}:${c.id}`,
          () => entryProgress(rows.map((e) => e.id)),
          (figures) => {
            if (g === selectionGeneration)
              entryFigures = Object.fromEntries(
                figures.map((p) => [p.entryId, p]),
              );
          },
          { kind: "entryProgress", ids: rows.map((e) => e.id) },
        );
      },
      { kind: "entries", id: c.id },
    );
  }
  $effect(() => {
    const id = project.id;
    tab = "collections";
    search = "";
    void untrack(() => {
      void activityRegion.load(`page-activity:${session?.uid}:${id}`, () =>
        projectPageActivity(id),
      );
      return loadProject(id);
    });
    return () => {
      projectGeneration++;
      collectionRegion.cancel();
      activityRegion.cancel();
      Object.values(progressRegions).forEach((r) => r.cancel());
    };
  });
  $effect(() => {
    const id = collectionId;
    const list = collections;
    const c = id ? list.find((c) => c.id === id) : list[0];
    void untrack(() => loadSelection(c, !!id));
    if (id && list.length && !c)
      error = "このプロジェクトにコレクションがありません。";
    return () => {
      selectionGeneration++;
      entriesRegion.cancel();
      figuresRegion.cancel();
    };
  });
  $effect(() => {
    const p = project;
    organizer =
      p.projectType === "official" ? "みんなで翻刻" : "主催者を確認中";
    let cancelled = false;
    if (p.projectType !== "official" && p.ownerId)
      void user(p.ownerId)
        .then((u) => {
          if (!cancelled) organizer = u.displayName;
        })
        .catch(() => {
          if (!cancelled) organizer = "主催者不明";
        });
    return () => {
      cancelled = true;
    };
  });
</script>

<div class="project-grid">
  <div class="project-main">
    <section class="panel project-header">
      <RegionNotice region={projectRegion} />
      {#if metadataPending}<Skeleton count={2} />{:else}<h1 class="serif">
          {project.title}
        </h1>
        <p>主催：{organizer}</p>
        <a href={href({projectId:project.id,guidelines:true})}>翻刻ガイドライン</a>
        {#if project.description}<p class="description muted">
            {project.description}
          </p>{/if}
        <div class="project-figures">
          <span
            >{number(project.completedImageCount)}／{number(
              project.totalImageCount,
            )}コマ</span
          ><span>{number(project.charCount)}字</span><span
            >{number(project.collections?.length)}コレクション</span
          ><span>{number(project.totalEntryCount)}資料</span>
        </div>
        <Progress
          done={project.completedImageCount ?? 0}
          total={project.totalImageCount ?? 0}
          caption
        />
        {#if discrepancy}<p class="caption muted collection-total">
            集計：{number(summed.completed)}／{number(
              summed.size,
            )}コマ・{number(summed.entries)}資料
          </p>{/if}
      {/if}
      <div class="tabs project-tabs" aria-label="プロジェクトの表示">
        {#each [["overview", "概要"], ["collections", "コレクション"], ["timeline", "タイムライン"], ["announcements", "お知らせ"], ["guidelines", "凡例"]] as [value, text]}<button
            class:active={tab === value}
            onclick={() => (tab = value)}>{text}</button
          >{/each}
      </div>
    </section>
    {#if tab === "collections"}<div class="collection-layout">
        <section class="panel collection-list">
          <h2>
            コレクション<span class="count"
              >{collectionsLoading
                ? (project.collections?.length ?? 0)
                : collections.length}</span
            >
          </h2>
          <label class="search"
            ><span aria-hidden="true">⌕</span><input
              aria-label="コレクションを検索"
              placeholder="コレクションを検索"
              bind:value={search}
            /></label
          >
          <SortControl
            storageKey={`honkoku.sort.project.${project.id}`}
            extended
            bind:value={collectionSort}
          />
          <RegionNotice region={collectionRegion} />
          <RegionNotice region={activityRegion} />
          <div class="scroll">
            {#if collectionsLoading && !collections.length}<Skeleton
                label="コレクションを取得中"
                count={7}
              />{/if}
            {#each sortedCollections.filter( (c) => c.title.includes(search), ) as c (c.id)}
              {@const p = progress[c.id]}
              <a
                class="collection-row"
                class:selected={selected?.id === c.id}
                href={href({ projectId: project.id, collectionId: c.id })}
              >
                <span class="folder" aria-hidden="true">▱</span>
                <div>
                  <h3>{c.title}</h3>
                  {#if c.description}<p class="caption muted description-first">
                      {c.description.split(/\r?\n/)[0]}
                    </p>{/if}
                  {#if p}
                    <div class="collection-meta">
                      <span>{number(p.completed)}／{number(p.size)}コマ</span
                      ><span>{number(p.entries)}資料</span>
                    </div>
                    {@render progressBar(p)}
                  {:else}<Skeleton count={1} />{/if}
                  {#if progressRegions[c.id]}<RegionNotice
                      region={progressRegions[c.id]}
                    />{/if}
                </div>
              </a>
            {/each}
          </div>
        </section>
        <section class="panel entries-panel">
          <div class="scroll">
            {#if selected}<h2 class="collection-title serif">
                {selected.title}
              </h2>
              {#if selected.description}<p class="description muted">
                  {selected.description}
                </p>{/if}
              {#if totals}
                <p class="muted">
                  {number(totals.entries)}資料・翻刻済み{number(
                    totals.completed,
                  )}／{number(totals.size)}コマ
                </p>
                {@render progressBar(totals)}
              {:else}<div
                  class="progress-skeleton"
                  aria-label="進捗を読み込み中"
                  aria-busy="true"
                ></div>{/if}
              <SortControl
                storageKey={`honkoku.sort.collection.${selected.id}`}
                bind:value={entrySort}
              />
              <RegionNotice region={figuresRegion} />
              <div class="entry-rows">
                {#each sortedEntries as e (e.id)}{@const p = entryFigures[e.id]}
                  {@const counts = p
                    ? {
                        default: Math.max(
                          0,
                          p.size - p.completed - p.initiated - p.editing,
                        ),
                        initiated: p.initiated + p.editing,
                        completed: p.completed,
                      }
                    : null}
                  <article class="entry-row">
                    <a
                      class="entry-thumbnail"
                      href={href({ entryId: e.id })}
                      aria-label={label(e.label)}
                      ><Thumbnail url={e.thumbnail} /></a
                    >
                    <div class="entry-copy">
                      <h3>
                        <a href={href({ entryId: e.id })}>{label(e.label)}</a>
                      </h3>
                      <p>{number(e.size)}コマ</p>
                      {#if counts}<div
                          class="segmented"
                          aria-label={`未着手${counts.default}、翻刻中${counts.initiated}、完了${counts.completed}`}
                        >
                          {#each Object.entries(counts) as [s, n]}<span
                              class={s}
                              style:flex-grow={n}
                            ></span>{/each}
                        </div>
                        <div class="entry-statuses caption">
                          <span>○未着手{counts.default}</span><span
                            class="status initiated"
                            >◐翻刻中{counts.initiated}</span
                          ><span class="status completed"
                            >✓完了{counts.completed}</span
                          >
                        </div>
                      {:else}<div
                          class="progress-skeleton"
                          aria-label="進捗を読み込み中"
                          aria-busy="true"
                        ></div>{/if}
                    </div>
                    <a
                      class="button primary open-entry"
                      href={href({ entryId: e.id })}>開く</a
                    >
                  </article>{/each}
              </div>
            {:else if !loading && !collectionsLoading}<p class="empty">
                コレクションはありません。
              </p>{/if}
            {#if (loading || collectionsLoading) && !entries.length}<Skeleton
                label="資料を取得中"
                count={4}
              />{/if}
            <RegionNotice region={entriesRegion} />
            {#if error}<p class="error" role="alert">{error}</p>{/if}
          </div>
        </section>
      </div>
    {:else if tab === "timeline"}<Timeline
        projectId={project.id}
        {session}
        onitems={(items) => (activity = items)}
      />
    {:else}<section class="panel project-prose scroll">
        {#if tab === "overview"}<Markdown
            text={project.markdown ??
              project.description ??
              "概要はありません。"}
          />{:else if tab === "guidelines"}<Markdown
            text={project.guidelines ??
              "このプロジェクトの凡例は登録されていません。"}
          />{:else}<h2>お知らせ</h2>
          <p class="empty">準備中</p>{/if}
      </section>{/if}
  </div>
  <aside class="project-side">
    <Timeline
      title="このプロジェクトの動き"
      projectId={project.id}
      {session}
      compact
      onitems={(items) => (activity = items)}
    />
    <section class="panel contributors">
      <h2>貢献者</h2>
      <p class="caption muted">読み込んだ活動から集計</p>
      {#if !activity.length}<Skeleton count={3} />{/if}
      <ol class="scroll">
        {#each contributors as c, i (c.user.uid)}<li>
            <span class="count">{i + 1}</span><Avatar
              user={c.user}
              small
            /><strong>{c.user.displayName}</strong><span
              >{number(c.count)}字</span
            >
          </li>{:else}<li class="empty">活動はまだありません。</li>{/each}
      </ol>
    </section>
  </aside>
</div>

{#snippet progressBar(p: CollectionProgress)}
  <div
    class="segmented collection-progress"
    role="progressbar"
    aria-label="翻刻の進捗"
    aria-valuemin={0}
    aria-valuemax={Math.max(1, p.size)}
    aria-valuenow={Math.min(p.completed, p.size)}
    aria-valuetext={`完了${number(p.completed)}／${number(p.size)}コマ、翻刻中${number(p.initiated + p.editing)}コマ`}
  >
    <span class="completed" style:flex-grow={p.completed}></span>
    <span class="initiated" style:flex-grow={p.initiated + p.editing}></span>
    <span
      class="default"
      style:flex-grow={Math.max(
        0,
        p.size - p.completed - p.initiated - p.editing,
      )}
    ></span>
  </div>
{/snippet}

<style>
  .description {
    white-space: pre-wrap;
    width: 100%;
  }
  .description-first {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin: 4px 0;
  }
  .progress-skeleton {
    height: 7px;
    border-radius: 4px;
    background: var(--surface-inset);
    animation: shimmer 1.5s ease-in-out infinite;
    margin: 8px 0;
    opacity: 0.6;
  }
  .collection-progress {
    margin: 8px 0;
  }
</style>
