<script lang="ts">
  import { onMount } from "svelte";
  import type {
    Canvas,
    Collection,
    Entry,
    Page,
    Project,
    SessionInfo,
    SignInProvider,
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
    sessionClearWithSite,
    sessionSignIn,
    onSessionChanged,
    onSignInClosed,
    sessionCurrent,
    sessionImport,
    unreadNotificationCount,
  } from "@honkoku/client-api/invoke";
  import type { ConnectionOutcome } from "@honkoku/client-api/invoke";
  import { allPages, errorMessage, label, status, statusClass } from "./lib";
  import { entryCanvases, entryData } from "./data";
  import { parseRoute, href } from "./routes";
  import type { Route } from "./routes";
  import { savedTheme, setTheme, type Theme } from "./theme";
  import { soundEnabled, setSound } from "./sound";
  import Home from "./components/Home.svelte";
  import ProjectScreen from "./components/Project.svelte";
  import EntryScreen from "./components/Entry.svelte";
  import Workbench from "./components/Workbench.svelte";
  import SignIn from "./components/SignIn.svelte";
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
    fixtureMissing = $state(false),
    loginError = $state(""),
    loading = $state(true),
    signingIn = $state(false),
    direction = $state("forward");
  let signInDialog = $state<"signin" | "signout" | null>(null);
  let sessionListeners: Promise<unknown> = Promise.resolve();
  let generation = 0;
  let sound = $state(soundEnabled());
  const themeOptions: { value: Theme; label: string; glyph: string }[] = [
    { value: "system", label: "システム", glyph: "◐" },
    { value: "light", label: "ライト", glyph: "☀" },
    { value: "dark", label: "ダーク", glyph: "☾" },
  ];
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
  async function login(provider: SignInProvider) {
    signingIn = true;
    loginError = "";
    try {
      await sessionListeners;
      if (!isTauri())
        throw new Error("デスクトップアプリでログインしてください。");
      await sessionSignIn(provider);
    } catch (e) {
      loginError = errorMessage(e);
      signingIn = false;
    }
  }
  async function importSession() {
    signingIn = true;
    loginError = "";
    try {
      await sessionListeners;
      await leaveWorkbench?.();
      session = await sessionImport();
      await identity();
      signInDialog = null;
    } catch {
      loginError =
        "開発用セッションを読み込めませんでした。ログイン情報を取得してから再試行してください。";
    } finally {
      signingIn = false;
    }
  }
  async function logout(clearSiteData: boolean) {
    signingIn = true;
    loginError = "";
    try {
      await leaveWorkbench?.();
      await sessionClearWithSite(clearSiteData);
      session = null;
      profile = null;
      unread = 0;
      signInDialog = null;
    } catch (e) {
      loginError = errorMessage(e);
      // The local session may already be cleared if browser-data removal fails.
      await identity().catch(() => {});
    } finally {
      signingIn = false;
    }
  }
  async function load(next: Route) {
    const g = ++generation;
    loading = true;
    error = "";
    fixtureMissing = false;
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
      if (g === generation) {
        error = errorMessage(e);
        fixtureMissing =
          typeof e === "object" &&
          e !== null &&
          "kind" in e &&
          e.kind === "fixture";
      }
    } finally {
      if (g === generation) loading = false;
    }
  }
  onMount(() => {
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    const updateSession = async () => {
      if (disposed) return;
      signingIn = false;
      signInDialog = null;
      loginError = "";
      try {
        await identity();
      } catch (e) {
        if (!disposed) loginError = errorMessage(e);
      }
    };
    if (isTauri()) {
      sessionListeners = Promise.all(
        [
          onSessionChanged(() => void updateSession()),
          onSignInClosed(() => void updateSession()),
        ].map(async (registration) => {
          const stop = await registration;
          if (disposed) stop();
          else unlisteners.push(stop);
        }),
      );
      void sessionListeners.catch((e) => {
        if (!disposed) loginError = errorMessage(e);
      });
    }
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
      disposed = true;
      unlisteners.forEach((stop) => stop());
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

{#snippet breadcrumbs()}
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
    {#if workbench && entry}<span class="page-count"
        >{route.pageIndex! + 1}／{entry.size}コマ</span
      >{/if}
  </nav>
{/snippet}

<div class="app-shell" class:reading={workbench}>
  <header class="topbar">
    {#if workbench}
      {@render breadcrumbs()}
      {@const current = pages.find((p) => p.index === route.pageIndex)}
      {#if current}<span class="status {statusClass(current.status)}"
          >{status(current.status).symbol}{status(current.status).label}</span
        >{/if}
    {:else}
      <a class="brand" href="#/">みんなで翻刻</a><label
        class="search top-search"
        ><span aria-hidden="true">⌕</span><input
          aria-label="全プロジェクトを検索"
          placeholder="プロジェクトを検索"
          disabled={!isHome}
          bind:value={search}
        /></label
      >
    {/if}
    <div class="top-controls">
      <div class="theme-switch" role="radiogroup" aria-label="表示テーマ">
        {#each themeOptions as option (option.value)}
          <button
            type="button"
            role="radio"
            aria-checked={theme === option.value}
            class:active={theme === option.value}
            title={option.label}
            aria-label={option.label}
            onclick={() => {
              theme = option.value;
              setTheme(theme);
            }}
            ><span class="theme-glyph" aria-hidden="true">{option.glyph}</span
            ><span class="theme-label">{option.label}</span></button
          >
        {/each}
      </div>
      <button
        type="button"
        class="sound-toggle"
        aria-pressed={sound}
        aria-label="効果音"
        title={sound ? "効果音を切る" : "効果音を鳴らす"}
        onclick={() => {
          sound = !sound;
          setSound(sound);
        }}>♪</button
      >
      {#if workbench}<button
          class="account-avatar"
          aria-label={session ? "ログアウト" : "ログイン"}
          onclick={() => {
            loginError = "";
            signInDialog = session ? "signout" : "signin";
          }}
          ><Avatar
            user={profile ?? {
              uid: session?.uid ?? "",
              displayName: session?.display_name ?? "利用者",
            }}
            small
          /></button
        >
      {:else if session}<div class="signed-user">
          <Avatar
            user={profile ?? {
              uid: session.uid,
              displayName: session.display_name ?? "利用者",
            }}
            small
          /><strong>{profile?.displayName ?? session.display_name}</strong><span
            class="notification-badge"
            aria-label={`未読通知${unread}件`}>♧{unread}</span
          ><button
            class="logout"
            onclick={() => {
              loginError = "";
              signInDialog = "signout";
            }}
            aria-label="ログアウト">ログアウト</button
          >
        </div>{:else}<button
          class="primary"
          disabled={signingIn}
          onclick={() => {
            loginError = "";
            signInDialog = "signin";
          }}>ログイン</button
        >{/if}
    </div>
  </header>
  {#if !workbench}{@render breadcrumbs()}{/if}
  {#if loginError && !signInDialog}<div class="message error" role="alert">
      {loginError}<button onclick={() => (loginError = "")} aria-label="閉じる"
        >×</button
      >
    </div>{/if}
  <main class:back={direction === "back"} aria-busy={loading}>
    {#if error}<div class="panel error" role="alert">
        {error}
        {#if fixtureMissing}<p>
            <code>devrun bun run --cwd apps/client tauri dev</code>
          </p>{/if}
        <button onclick={() => load(route)}>再試行</button><a href="#/"
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

{#if signInDialog}
  <SignIn
    mode={signInDialog}
    busy={signingIn}
    error={loginError}
    onprovider={login}
    onimport={importSession}
    onlogout={logout}
    onclose={() => (signInDialog = null)}
  />
{/if}
