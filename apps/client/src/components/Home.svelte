<script lang="ts">
  import { onMount, tick } from "svelte";
  import { Region } from "../region.svelte";
  import Skeleton from "./Skeleton.svelte";
  import RegionNotice from "./RegionNotice.svelte";
  import type {
    Announcement,
    RankingSelf,
    RankingSort,
    Project,
    SessionInfo,
    TimelineItem,
    User,
  } from "@honkoku/client-api/types";
  import {
    homeAnnouncements,
    homeRanking,
    homeRankingSelf,
  } from "@honkoku/client-api/invoke";
  import { aggregate, date, errorMessage, number, percent, user } from "../lib";
  import { href } from "../routes";
  import Avatar from "./Avatar.svelte";
  import Progress from "./Progress.svelte";
  import RecentWork from "./RecentWork.svelte";
  import Timeline from "./Timeline.svelte";
  import ExternalLink from "./ExternalLink.svelte";
  let {
    projects,
    projectsRegion,
    session,
    profile,
    search = $bindable(""),
  }: {
    projects: Project[];
    projectsRegion: Region<Project[]>;
    session: SessionInfo | null;
    profile: User | null;
    search?: string;
  } = $props();
  let filter = $state("official"),
    sort = $state("updated"),
    group = $state("owner"),
    rankTab = $state("total");
  let collapsed = $state<string[]>([]),
    owners = $state<Record<string, string>>({});
  const rankingRegion = new Region<User[]>();
  let rankSort = $state<RankingSort>("exp");
  let selfRegion = $state(new Region<RankingSelf>());
  let rankingPanel: HTMLElement;
  let pulse = $state(false);
  const rankUnits = { exp: "pt", charCount: "字", likeCount: "いいね" };
  let unit = $derived(rankTab === "total" ? rankUnits[rankSort] : "字");
  async function findSelf() {
    pulse = false;
    await tick();
    const row = rankingPanel.querySelector<HTMLElement>("[data-ranking-self]");
    if (!row) return;
    const bounds = row.getBoundingClientRect();
    const list = row.closest<HTMLElement>(".ranking-list");
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (list)
      list.scrollTo({
        top:
          list.scrollTop +
          bounds.top -
          list.getBoundingClientRect().top -
          (list.clientHeight - row.clientHeight) / 2,
        behavior: reduced ? "auto" : "smooth",
      });
    pulse = true;
  }
  $effect(() => {
    const field = rankSort;
    void rankingRegion.load(`ranking/${field}`, () => homeRanking(field));
    return () => rankingRegion.cancel();
  });
  $effect(() => {
    rankSort;
    rankTab;
    session?.uid;
    pulse = false;
  });
  $effect(() => {
    const uid = session?.uid;
    const field = rankSort;
    const region = new Region<RankingSelf>();
    selfRegion = region;
    if (
      uid &&
      rankTab === "total" &&
      !rankingRegion.pending &&
      rankingRegion.value !== undefined &&
      !ranking.some((user) => user.uid === uid)
    )
      void region.load(`ranking-self/${uid}/${field}`, () =>
        homeRankingSelf(field),
      );
    return () => region.cancel();
  });
  const announcementRegion = new Region<Announcement[]>();
  let ranking = $derived(rankingRegion.value ?? []);
  let announcements = $derived(announcementRegion.value ?? []);
  let activity = $state<TimelineItem[]>([]);
  const joined = (p: Project) =>
    !!session &&
    (!!p.members?.includes(session.uid) || !!p.admins?.includes(session.uid));
  let filtered = $derived(
    projects
      .filter(
        (p) =>
          p.display !== false &&
          (filter === "private"
            ? p.isPrivate
            : !p.isPrivate &&
              (filter === "joined" ? joined(p) : p.projectType === filter)) &&
          `${p.title} ${p.description ?? ""} ${Array.isArray(p.keywords) ? p.keywords.join(" ") : (p.keywords ?? "")}`
            .toLocaleLowerCase()
            .includes(search.toLocaleLowerCase()),
      )
      .sort((a, b) =>
        sort === "progress"
          ? percent(b.completedImageCount ?? 0, b.totalImageCount ?? 0) -
            percent(a.completedImageCount ?? 0, a.totalImageCount ?? 0)
          : sort === "chars"
            ? (b.charCount ?? 0) - (a.charCount ?? 0)
            : Date.parse(b.updatedAt ?? b.createdAt ?? "1970-01-01") -
              Date.parse(a.updatedAt ?? a.createdAt ?? "1970-01-01"),
      ),
  );
  let groups = $derived.by(() => {
    const map = new Map<string, Project[]>();
    for (const p of filtered) {
      const keys =
        group === "owner"
          ? [
              p.projectType === "official"
                ? "みんなで翻刻"
                : (owners[p.ownerId ?? ""] ?? "主催者を確認中"),
            ]
          : (Array.isArray(p.keywords)
              ? p.keywords
              : (p.keywords?.split(/[,、\s]+/) ?? [])
            ).filter(Boolean);
      for (const key of keys.length ? keys : ["キーワードなし"])
        map.set(key, [...(map.get(key) ?? []), p]);
    }
    return [...map];
  });
  let ranked = $derived(
    rankTab === "total"
      ? ranking.map((user) => ({ user, count: user[rankSort] ?? 0 }))
      : aggregate(activity, rankTab === "today" ? 24 : 168),
  );
  let selfLoaded = $derived(
    !!session && ranked.some((record) => record.user.uid === session.uid),
  );
  $effect(() => {
    if (!session && filter === "joined") filter = "official";
  });
  $effect(() => {
    const ids = [
      ...new Set(
        projects
          .filter((p) => p.projectType !== "official")
          .map((p) => p.ownerId)
          .filter((s): s is string => !!s),
      ),
    ];
    let cancelled = false;
    void (async () => {
      for (let i = 0; i < ids.length; i += 4)
        await Promise.all(
          ids.slice(i, i + 4).map(async (id) => {
            try {
              const value = await user(id);
              if (!cancelled) owners = { ...owners, [id]: value.displayName };
            } catch {
              if (!cancelled) owners = { ...owners, [id]: "主催者不明" };
            }
          }),
        );
    })();
    return () => {
      cancelled = true;
    };
  });
  onMount(() => {
    void announcementRegion.load("announcements", homeAnnouncements);
    return () => {
      rankingRegion.cancel();
      announcementRegion.cancel();
    };
  });
</script>

<div class="home-grid">
  <section class="panel project-list" aria-label="プロジェクト">
    <h2>プロジェクト<span class="count">{filtered.length}</span></h2>
    <label class="search"
      ><span aria-hidden="true">⌕</span><input
        aria-label="プロジェクトを検索"
        placeholder="キーワードで検索"
        bind:value={search}
      /></label
    >
    <div class="chips" aria-label="プロジェクトの種類">
      {#each [["official", "公式"], ["user", "ユーザー"], ["private", "非公開"], ["joined", "参加中"]] as [value, text]}<button
          class:active={filter === value}
          disabled={value === "joined" && !session}
          onclick={() => (filter = value)}>{text}</button
        >{/each}
    </div>
    <div class="tabs" aria-label="並び順">
      {#each [["updated", "更新順"], ["progress", "進捗順"], ["chars", "文字数順"]] as [value, text]}<button
          class:active={sort === value}
          onclick={() => (sort = value)}>{text}</button
        >{/each}
    </div>
    <label class="group-control"
      >グループ<select aria-label="グループ" bind:value={group}
        ><option value="owner">主催</option><option value="keyword"
          >キーワード</option
        ></select
      ></label
    >
    <RegionNotice region={projectsRegion} />
    <div class="scroll project-groups">
      {#if !projects.length && projectsRegion.pending}<Skeleton
          label="プロジェクトを取得中"
          count={7}
        />{/if}
      {#each groups as [name, items] (name)}
        <button
          class="group-heading"
          aria-expanded={!collapsed.includes(name)}
          onclick={() =>
            (collapsed = collapsed.includes(name)
              ? collapsed.filter((n) => n !== name)
              : [...collapsed, name])}
          ><span aria-hidden="true">{collapsed.includes(name) ? "›" : "⌄"}</span
          >{group === "owner" ? "主催：" : ""}{name}<span class="count"
            >{items.length}</span
          ></button
        >
        {#if !collapsed.includes(name)}{#each items as p (p.id)}<a
              class="project-row"
              href={href({ projectId: p.id })}
              ><h3>{p.title}</h3>
              <div class="project-meta">
                <Progress
                  done={p.completedImageCount ?? 0}
                  total={p.totalImageCount ?? 0}
                  caption
                /><span
                  >{number(p.completedImageCount)}／{number(
                    p.totalImageCount,
                  )}コマ</span
                ><span>{number(p.charCount)}字</span>
              </div></a
            >{/each}{/if}
      {:else}{#if !projectsRegion.pending}<p class="empty">
            該当するプロジェクトはありません。
          </p>{/if}{/each}
    </div>
  </section>
  <div class="home-centre">
    <RecentWork /><Timeline {session} onitems={(items) => (activity = items)} />
  </div>
  <aside class="home-side scroll">
    <section class="panel ranking" bind:this={rankingPanel}>
      <div class="ranking-heading">
        <h2>ランキング</h2>
        {#if session}<button
            disabled={!selfLoaded && !selfRegion.value}
            onclick={findSelf}>自分の順位</button
          >{/if}
      </div>
      {#if rankTab === "total"}<label class="ranking-sort"
          >集計項目<select
            aria-label="ランキングの集計項目"
            bind:value={rankSort}
          >
            <option value="exp">ポイント</option><option value="charCount"
              >文字数</option
            ><option value="likeCount">いいね</option>
          </select></label
        >{/if}
      <div class="tabs" aria-label="集計期間">
        {#each [["today", "今日"], ["week", "今週"], ["total", "累計"]] as [value, text]}<button
            class:active={rankTab === value}
            onclick={() => (rankTab = value)}>{text}</button
          >{/each}
      </div>
      {#if rankTab !== "total"}<p class="caption muted">
          読み込んだ活動から集計
        </p>{/if}
      <RegionNotice region={rankingRegion} />
      {#if rankingRegion.value === undefined && rankingRegion.pending}<Skeleton
          label="ランキングを取得中"
          count={5}
        />{/if}
      <ol class="ranking-list">
        {#each ranked as record, i (record.user.uid)}
          {@const own = record.user.uid === session?.uid}
          <li
            class:ranking-self={own}
            class:ranking-pulse={own && pulse}
            data-ranking-self={own ? "true" : undefined}
            onanimationend={() => (pulse = false)}
          >
            <span class="rank" class:leading={i < 3}>{i + 1}</span><Avatar
              user={record.user}
              small
            />
            <div class="rank-copy">
              <strong
                ><span class="ranking-name">{record.user.displayName}</span
                >{#if own}<span class="ranking-self-tag">自分</span
                  >{/if}</strong
              >
              <div>
                <span class="numeric">{number(record.count)}{unit}</span><span
                  class="muted">Lv.{number(record.user.level)}</span
                >
              </div>
            </div>
          </li>{:else}{#if !rankingRegion.pending}<li class="empty">
              この期間の活動はありません。
            </li>{/if}{/each}
      </ol>
      {#if session && rankTab === "total" && !selfLoaded}
        <RegionNotice region={selfRegion} />
        {#if selfRegion.pending && !selfRegion.value}<p
            class="caption muted"
            role="status"
          >
            自分の順位を取得中
          </p>{/if}
        {#if selfRegion.value}
          <div
            class="ranking-self ranking-self-footer"
            class:ranking-pulse={pulse}
            data-ranking-self="true"
            onanimationend={() => (pulse = false)}
          >
            <span
              >{selfRegion.value.rank === null
                ? "未集計"
                : `${number(selfRegion.value.rank)}位`}</span
            >
            <strong>あなた</strong><span
              >· {selfRegion.value.value === null
                ? "—"
                : `${number(selfRegion.value.value)}${unit}`}</span
            >
          </div>
        {/if}
      {/if}
    </section>
    {#if session && profile}<section class="panel own-record">
        <h2>あなたの記録</h2>
        <div class="record-head">
          <Avatar user={profile} />
          <div>
            <strong>{profile.displayName}</strong>
            <p>Lv.{number(profile.level)}</p>
          </div>
          <strong class="record-points">{number(profile.exp)}pt</strong>
        </div>
        <div class="record-stats">
          <span>{number(profile.charCount)}字</span><span
            >♡{number(profile.likeCount)}いいね</span
          >
        </div>
      </section>{/if}
    <section class="panel announcements">
      <h2>お知らせ</h2>
      <RegionNotice region={announcementRegion} />
      {#if announcementRegion.value === undefined && announcementRegion.pending}<Skeleton
          count={2}
        />{/if}
      {#each announcements as item (item.id)}<div class="announcement">
          <time datetime={item.createdAt}>{date(item.createdAt)}</time
          ><ExternalLink href={"https://app.honkoku.org/AdminAnnouncements"}
            >{item.title}</ExternalLink
          >
        </div>{:else}{#if !announcementRegion.pending}<p class="empty">
            お知らせはありません。
          </p>{/if}{/each}
    </section>
  </aside>
</div>
