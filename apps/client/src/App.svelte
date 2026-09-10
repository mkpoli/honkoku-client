<script lang="ts">
  import { onMount } from "svelte";
  import type {
    Canvas,
    Collection,
    Entry,
    Page,
    Project,
    SessionInfo,
    User,
  } from "@honkoku/client-api/types";
  import {
    isTauri,
    onWindowClose,
    getCollection,
    getProject,
    homeDailyProgress,
    listPages,
    listProjects,
    me,
    sessionClear,
    sessionCurrent,
    sessionImport,
    unreadNotificationCount,
  } from "@honkoku/client-api/invoke";
  import type { ConnectionOutcome } from "@honkoku/client-api/invoke";
  import { allPages, errorMessage, label } from "./lib";
  import { entryCanvases, entryData } from "./data";
  import { parseRoute, href } from "./routes";
  import type { Route } from "./routes";
  import { savedTheme, setTheme } from "./theme";
  import { soundEnabled, setSound } from "./sound";
  import Home from "./components/Home.svelte";
  import ProjectScreen from "./components/Project.svelte";
  import EntryScreen from "./components/Entry.svelte";
  import Workbench from "./components/Workbench.svelte";
  import Avatar from "./components/Avatar.svelte";
  let EditorSpike = $state<typeof import("./dev/EditorSpike.svelte").default>();
  let route = $state<Route>(parseRoute(location.hash)),
    theme = $state(savedTheme()),
    search = $state("");
  let projects = $state<Project[]>([]),
    project = $state<Project | null>(null),
    collection = $state<Collection | null>(null),
    entry = $state<Entry | null>(null),
    pages = $state<Page[]>([]),
    canvases = $state<Canvas[]>([]);
  let session = $state<SessionInfo | null>(null),
    profile = $state<User | null>(null),
    unread = $state(0),
    connected = $state(false),
    syncedAt = $state<number>(),
    error = $state(""),
    loginError = $state(""),
    loading = $state(true),
    signingIn = $state(false),
    direction = $state("forward");
  let generation = 0;
  let sound = $state(soundEnabled());
  let leaveWorkbench = $state<() => Promise<void>>();
  let navigation = 0;
  let acceptedHash = location.hash;
  let isHome = $derived(
    !route.projectId && !route.entryId && !route.invalid && !route.editorSpike,
  );
  let workbench = $derived(route.pageIndex !== undefined);
  async function identity() {
    session = await sessionCurrent();
    if (session) {
      [profile, unread] = await Promise.all([me(), unreadNotificationCount()]);
      if (!profile) session = null;
    } else {
      profile = null;
      unread = 0;
    }
  }
  async function login() {
    signingIn = true;
    loginError = "";
    try {
      session = await sessionImport();
      await identity();
    } catch {
      loginError =
        "ログイン情報を読み込めませんでした。端末でbun tools/session/login.tsを実行し、ログイン情報を取得してから再試行してください。";
    } finally {
      signingIn = false;
    }
  }
  async function logout() {
    try {
      await leaveWorkbench?.();
      await sessionClear();
      session = null;
      profile = null;
      unread = 0;
    } catch (e) {
      loginError = errorMessage(e);
    }
  }
  async function load(next: Route) {
    const g = ++generation;
    loading = true;
    error = "";
    project = null;
    collection = null;
    entry = null;
    pages = [];
    canvases = [];
    try {
      if (next.editorSpike && import.meta.env.DEV) {
        EditorSpike = (await import("./dev/EditorSpike.svelte")).default;
        return;
      }
      if (next.invalid)
        throw Error("ページが見つかりません。ホームから選び直してください。");
      if (!next.projectId && !next.entryId) {
        const ps = await listProjects();
        if (g !== generation) return;
        projects = ps;
      }
      if (next.entryId) {
        const e = await entryData(next.entryId);
        const [p, c, ps, cs] = await Promise.all([
          getProject(e.projectId),
          getCollection(e.collectionId),
          listPages(e.id),
          entryCanvases(e.id),
        ]);
        if (g !== generation) return;
        const all = allPages(e, ps);
        if (
          next.pageIndex !== undefined &&
          !all.some((p) => p.index === next.pageIndex)
        )
          throw Error("指定されたコマがありません。");
        entry = e;
        project = p;
        collection = c;
        pages = all;
        canvases = cs;
      } else if (next.projectId) {
        const p = await getProject(next.projectId);
        if (g !== generation) return;
        project = p;
      }
    } catch (e) {
      if (g === generation) error = errorMessage(e);
    } finally {
      if (g === generation) loading = false;
    }
  }
  onMount(() => {
    const connection = (e: Event) => {
      const outcome = (e as CustomEvent<ConnectionOutcome>).detail;
      connected = outcome.connected;
      if (outcome.syncedAt) syncedAt = outcome.syncedAt;
    };
    window.addEventListener("honkoku:connection", connection);
    void (async () => {
      try {
        await identity();
      } catch (e) {
        loginError = errorMessage(e);
      }
      await load(route);
      void homeDailyProgress().catch(() => {});
    })();
    const navigate = async () => {
      const hash = location.hash;
      if (hash === acceptedHash) return;
      const attempt = ++navigation;
      try {
        await leaveWorkbench?.();
      } catch (error) {
        if (attempt === navigation) {
          loginError = errorMessage(error);
          history.replaceState(null, "", acceptedHash || "#/");
        }
        return;
      }
      if (attempt !== navigation) return;
      acceptedHash = hash;
      const next = parseRoute(hash);
      direction =
        (!next.entryId && route.entryId) ||
        (!next.collectionId && route.collectionId) ||
        (!next.projectId && route.projectId)
          ? "back"
          : "forward";
      const sameEntry = next.entryId && next.entryId === route.entryId && entry;
      route = next;
      if (sameEntry) {
        error =
          next.pageIndex !== undefined &&
          !pages.some((p) => p.index === next.pageIndex)
            ? "指定されたコマがありません。"
            : "";
      } else void load(next);
    };
    window.addEventListener("hashchange", navigate);
    return () => {
      generation++;
      window.removeEventListener("hashchange", navigate);
      window.removeEventListener("honkoku:connection", connection);
    };
  });
  $effect(() => {
    const guard = leaveWorkbench;
    if (!guard || !isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onWindowClose(async (event, close) => {
      event.preventDefault();
      if (disposed) return;
      try {
        await guard();
        unlisten?.();
        await close().catch(() => {
          loginError =
            "下書きを保存しました。もう一度ウィンドウを閉じてください。";
        });
      } catch (error) {
        loginError = errorMessage(error);
      }
    })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch((error) => {
        loginError = errorMessage(error);
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  });
</script>

<div class="app-shell" class:reading={workbench}>
  <header class="topbar">
    <a class="brand" href="#/">みんなで翻刻</a><label class="search top-search"
      ><span aria-hidden="true">⌕</span><input
        aria-label="全プロジェクトを検索"
        placeholder="プロジェクトを検索"
        disabled={!isHome}
        bind:value={search}
      /></label
    >
    <div class="top-controls">
      <details class="theme-menu">
        <summary>テーマ</summary>
        <div class="theme-options">
          <label class="theme-control"
            ><span class="caption muted">テーマ</span><select
              aria-label="表示テーマ"
              bind:value={theme}
              onchange={() => setTheme(theme)}
              ><option value="system">システム</option><option value="light"
                >ライト</option
              ><option value="dark">ダーク</option></select
            ></label
          ><label class="sound-control"
            ><input
              type="checkbox"
              bind:checked={sound}
              onchange={() => setSound(sound)}
            />効果音</label
          >
        </div>
      </details>
      {#if session}<div class="signed-user">
          <Avatar
            user={profile ?? {
              uid: session.uid,
              displayName: session.display_name ?? "利用者",
            }}
            small
          /><strong>{profile?.displayName ?? session.display_name}</strong><span
            class="notification-badge"
            aria-label={`未読通知${unread}件`}>♧{unread}</span
          ><button class="logout" onclick={logout} aria-label="ログアウト"
            >ログアウト</button
          >
        </div>{:else}<button
          class="primary"
          disabled={signingIn}
          onclick={login}>ログイン</button
        >{/if}
    </div>
  </header>
  <nav class="breadcrumb" aria-label="パンくず">
    <a href="#/" aria-current={isHome ? "page" : undefined}>⌂ホーム</a
    >{#if project}<span>›</span><a href={href({ projectId: project.id })}
        >{project.title}</a
      >{/if}{#if collection && project}<span>›</span><a
        href={href({ projectId: project.id, collectionId: collection.id })}
        >{collection.title}</a
      >{/if}{#if entry}<span>›</span><a href={href({ entryId: entry.id })}
        >{label(entry.label)}</a
      >{/if}
  </nav>
  {#if loginError}<div class="message error" role="alert">
      {loginError}<button onclick={() => (loginError = "")} aria-label="閉じる"
        >×</button
      >
    </div>{/if}
  <main class:back={direction === "back"} aria-busy={loading}>
    {#if error}<div class="panel error" role="alert">
        {error}<button onclick={() => load(route)}>再試行</button><a href="#/"
          >ホームへ</a
        >
      </div>{:else if loading}<div class="panel empty" role="status">
        読み込み中…
      </div>{:else}{#key route.editorSpike ? "editor" : (route.entryId ?? route.projectId ?? "home")}<div
          class="route-screen"
        >
          {#if route.editorSpike && EditorSpike}<EditorSpike
            />{:else if isHome}<Home
              {projects}
              {session}
              {profile}
              bind:search
            />{:else if entry}{#if workbench}<Workbench
                {entry}
                {pages}
                {canvases}
                {session}
                onpage={(updated) => {
                  pages = pages.map((p) => (p.id === updated.id ? updated : p));
                }}
                registerLeave={(guard) => {
                  leaveWorkbench = guard;
                }}
                index={route.pageIndex!}
              />{:else}<EntryScreen
                {entry}
                {pages}
                {canvases}
              />{/if}{:else if project}<ProjectScreen
              {project}
              collectionId={route.collectionId}
              {session}
              oncollection={(c) => (collection = c)}
            />{/if}
        </div>{/key}{/if}
  </main>
  <footer class="statusbar">
    {#if !isTauri()}<span>閲覧データ</span>{/if}
    <span
      ><i class:online={connected}></i>{connected
        ? "接続済み"
        : "オフライン"}</span
    ><span
      >{#if syncedAt}<i class="online"></i>同期済み{new Date(
          syncedAt,
        ).toLocaleTimeString("ja-JP", {
          hour: "2-digit",
          minute: "2-digit",
        })}{:else}未同期{/if}</span
    >
  </footer>
</div>
