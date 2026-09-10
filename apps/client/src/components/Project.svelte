<script lang="ts">
  import type {
    Collection,
    Entry,
    Project,
    SessionInfo,
    TimelineItem,
  } from "@honkoku/client-api/types";
  import { getCollection, listCollections } from "@honkoku/client-api/invoke";
  import { entryData } from "../data";
  import {
    aggregate,
    entryCounts,
    errorMessage,
    label,
    number,
    user,
  } from "../lib";
  import { href } from "../routes";
  import Progress from "./Progress.svelte";
  import Thumbnail from "./Thumbnail.svelte";
  import Timeline from "./Timeline.svelte";
  import Markdown from "./Markdown.svelte";
  import Avatar from "./Avatar.svelte";
  let {
    project,
    collectionId,
    session,
    oncollection,
  }: {
    project: Project;
    collectionId?: string;
    session: SessionInfo | null;
    oncollection: (c: Collection | null) => void;
  } = $props();
  let collections = $state<Collection[]>([]),
    selected = $state<Collection | null>(null),
    entries = $state<Entry[]>([]),
    activity = $state<TimelineItem[]>([]);
  let tab = $state("collections"),
    organizer = $state("主催者を確認中"),
    search = $state(""),
    error = $state(""),
    loading = $state(false);
  let loaded = $state<Record<string, Entry[]>>({});
  let totals = $derived(
    entries.reduce(
      (s, e) => ({
        size: s.size + (e.size ?? 0),
        done: s.done + (e.progress ?? 0),
      }),
      { size: 0, done: 0 },
    ),
  );
  let contributors = $derived(aggregate(activity));
  let generation = 0;
  async function load(id?: string) {
    const g = ++generation;
    loading = true;
    error = "";
    selected = null;
    entries = [];
    oncollection(null);
    try {
      const list = await listCollections(project.id);
      if (g !== generation) return;
      collections = list.filter((c) => c.display !== false);
      const target = id ?? collections[0]?.id;
      if (!target) return;
      if (!collections.some((c) => c.id === target))
        throw Error("このプロジェクトにコレクションがありません。");
      const c = await getCollection(target);
      if (g !== generation) return;
      selected = c;
      if (id) oncollection(c);
      const rows: Entry[] = [];
      for (let i = 0; i < (c.entries?.length ?? 0); i += 4) {
        const batch = await Promise.all(
          c.entries!.slice(i, i + 4).map(entryData),
        );
        if (g !== generation) return;
        rows.push(...batch);
      }
      entries = rows.sort((a, b) => a.index - b.index);
      loaded = { ...loaded, [c.id]: entries };
    } catch (e) {
      if (g === generation) error = errorMessage(e);
    } finally {
      if (g === generation) loading = false;
    }
  }
  $effect(() => {
    project.id;
    collectionId;
    tab = "collections";
    void load(collectionId);
    return () => {
      generation++;
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
      <h1 class="serif">{project.title}</h1>
      <p>主催：{organizer}</p>
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
      <div class="tabs project-tabs" aria-label="プロジェクトの表示">
        {#each [["overview", "概要"], ["collections", "コレクション"], ["timeline", "タイムライン"], ["announcements", "お知らせ"], ["guidelines", "凡例"]] as [value, text]}<button
            class:active={tab === value}
            onclick={() => (tab = value)}>{text}</button
          >{/each}
      </div>
    </section>
    {#if tab === "collections"}<div class="collection-layout">
        <section class="panel collection-list">
          <h2>コレクション<span class="count">{collections.length}</span></h2>
          <label class="search"
            ><span aria-hidden="true">⌕</span><input
              aria-label="コレクションを検索"
              placeholder="コレクションを検索"
              bind:value={search}
            /></label
          >
          <div class="scroll">
            {#each collections.filter( (c) => c.title.includes(search), ) as c (c.id)}{@const rows =
                loaded[c.id]}{@const total =
                rows?.reduce((sum, e) => sum + (e.size ?? 0), 0) ??
                0}{@const done =
                rows?.reduce((sum, e) => sum + (e.progress ?? 0), 0) ?? 0}<a
                class="collection-row"
                class:selected={selected?.id === c.id}
                href={href({ projectId: project.id, collectionId: c.id })}
                ><span class="folder" aria-hidden="true">▱</span>
                <div>
                  <h3>{c.title}</h3>
                  <div class="collection-meta">
                    <span>{number(c.entryCount)}資料</span>{#if rows}<span
                        >{done}／{total}コマ</span
                      >{/if}
                  </div>
                  {#if rows}<Progress {done} {total} />{:else}<span
                      class="caption muted">選択して進捗を表示</span
                    >{/if}
                </div></a
              >{/each}
          </div>
        </section>
        <section class="panel entries-panel">
          <div class="scroll">
            {#if selected}<h2 class="collection-title serif">
                {selected.title}
              </h2>
              <p class="muted">
                {number(
                  selected.entryCount,
                )}資料{#if !loading}・翻刻済み{totals.done}／{totals.size}コマ{/if}
              </p>
              <Progress done={totals.done} total={totals.size} caption />
              <div class="entry-rows">
                {#each entries as e (e.id)}{@const counts = entryCounts(e)}
                  <article class="entry-row">
                    <a
                      class="entry-thumbnail"
                      href={href({ entryId: e.id })}
                      aria-label={label(e.label)}
                      ><Thumbnail entryId={e.id} /></a
                    >
                    <div class="entry-copy">
                      <h3>
                        <a href={href({ entryId: e.id })}>{label(e.label)}</a>
                      </h3>
                      <p class="caption muted">{label(e.attribution)}</p>
                      <p>{number(e.size)}コマ</p>
                      <div
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
                    </div>
                    <a
                      class="button primary open-entry"
                      href={href({ entryId: e.id })}>開く</a
                    >
                  </article>{/each}
              </div>
            {:else if !loading}<p class="empty">
                コレクションはありません。
              </p>{/if}
            {#if loading}<p class="empty" role="status">
                資料を読み込み中…
              </p>{/if}{#if error}<p class="error" role="alert">{error}</p>
              <button onclick={() => load(collectionId)}>再試行</button>{/if}
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
