<script lang="ts">
  import { onMount } from "svelte";
  import type {
    Announcement,
    Project,
    SessionInfo,
    TimelineItem,
    User,
  } from "@honkoku/client-api/types";
  import { homeAnnouncements, homeRanking } from "@honkoku/client-api/invoke";
  import { aggregate, date, errorMessage, number, percent, user } from "../lib";
  import { href } from "../routes";
  import Avatar from "./Avatar.svelte";
  import Progress from "./Progress.svelte";
  import Timeline from "./Timeline.svelte";
  import ExternalLink from "./ExternalLink.svelte";
  let {
    projects,
    session,
    profile,
    search = $bindable(""),
  }: {
    projects: Project[];
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
  let ranking = $state<User[]>([]),
    announcements = $state<Announcement[]>([]),
    activity = $state<TimelineItem[]>([]),
    error = $state("");
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
      ? ranking.map((user) => ({ user, count: user.exp ?? 0 }))
      : aggregate(activity, rankTab === "today" ? 24 : 168),
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
  async function loadSide() {
    error = "";
    const result = await Promise.allSettled([
      homeRanking(),
      homeAnnouncements(),
    ]);
    if (result[0].status === "fulfilled") ranking = result[0].value;
    else error = errorMessage(result[0].reason);
    if (result[1].status === "fulfilled") announcements = result[1].value;
    else error = errorMessage(result[1].reason);
  }
  onMount(() => {
    void loadSide();
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
    <div class="scroll project-groups">
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
      {:else}<p class="empty">該当するプロジェクトはありません。</p>{/each}
    </div>
  </section>
  <Timeline {session} onitems={(items) => (activity = items)} />
  <aside class="home-side scroll">
    <section class="panel ranking">
      <h2>ランキング</h2>
      <div class="tabs" aria-label="集計期間">
        {#each [["today", "今日"], ["week", "今週"], ["total", "累計"]] as [value, text]}<button
            class:active={rankTab === value}
            onclick={() => (rankTab = value)}>{text}</button
          >{/each}
      </div>
      {#if rankTab !== "total"}<p class="caption muted">
          読み込んだ活動から集計
        </p>{/if}
      <ol class="ranking-list">
        {#each ranked as record, i (record.user.uid)}<li>
            <span class="rank" class:leading={i < 3}>{i + 1}</span><Avatar
              user={record.user}
              small
            />
            <div class="rank-copy">
              <strong>{record.user.displayName}</strong>
              <div>
                <span class="numeric"
                  >{number(record.count)}{rankTab === "total"
                    ? "pt"
                    : "字"}</span
                ><span class="muted">Lv.{number(record.user.level)}</span>
              </div>
            </div>
          </li>{:else}<li class="empty">この期間の活動はありません。</li>{/each}
      </ol>
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
      {#each announcements as item (item.id)}<div class="announcement">
          <time datetime={item.createdAt}>{date(item.createdAt)}</time
          ><ExternalLink href={"https://app.honkoku.org/AdminAnnouncements"}
            >{item.title}</ExternalLink
          >
        </div>{:else}<p class="empty">お知らせはありません。</p>{/each}
    </section>
    {#if error}<p class="error" role="alert">
        {error}<button onclick={loadSide}>再試行</button>
      </p>{/if}
  </aside>
</div>
