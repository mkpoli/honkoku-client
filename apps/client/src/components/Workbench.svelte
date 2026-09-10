<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import type {
    Canvas,
    Entry,
    Page,
    SessionInfo,
    SaveOptions,
  } from "@honkoku/client-api/types";
  import { parseGroups as parse, renderInline } from "@honkoku/markup";
  import { date, notes, status, statusClass, user } from "../lib";
  import { href, parseRoute } from "../routes";
  import VerticalEditor from "@honkoku/editor/VerticalEditor.svelte";
  import type { EditorUpdate } from "@honkoku/editor";
  import {
    pageLock,
    pageDraft,
    pageSave,
    pageDiscard,
    pageLockState,
    listPages,
    homeTimeline,
  } from "@honkoku/client-api/invoke";
  import { errorMessage } from "../lib";
  import { Drafts } from "../drafts";
  import { prepareSound, completionSound } from "../sound";
  import Facsimile from "./Facsimile.svelte";
  import Thumbnail from "./Thumbnail.svelte";
  let {
    entry,
    pages,
    canvases,
    index,
    session,
    onpage,
    registerLeave,
  }: {
    entry: Entry;
    pages: Page[];
    canvases: Canvas[];
    index: number;
    session: SessionInfo | null;
    onpage: (page: Page) => void;
    registerLeave: (guard: (() => Promise<void>) | undefined) => void;
  } = $props();
  let page = $derived(pages.find((p) => p.index === index)!);
  let groups = $derived(
    parse(
      page.status === "editing" && page.syncMode
        ? (page.tempText ?? page.text)
        : page.text,
    ),
  );
  let pageNotes = $derived(notes(page));
  let swapped = $state(false),
    horizontal = $state(false),
    half = $state(""),
    expanded = $state(false),
    noteIndex = $state<number | null>(null),
    authors = $state<Record<string, string>>({});
  let editing = $state(false),
    busy = $state(false),
    composing = $state(false);
  let source = $state(""),
    saveState = $state(""),
    notice = $state("");
  let savePopover = $state(false),
    discardPopover = $state(false);
  let completed = $state(false),
    share = $state(false),
    requestReview = $state(false),
    comment = $state("");
  let resumable = $state(false),
    lockName = $state("名前を確認中");
  let celebration = $state<number | null | undefined>();
  let recovered = $state("");
  let queue: Drafts | undefined;
  let operation: Promise<void> | undefined;
  let toastTimer: ReturnType<typeof setTimeout>;
  let lastOptions: SaveOptions | undefined;
  function savedOptions(): SaveOptions | undefined {
    try {
      const value = JSON.parse(
        sessionStorage.getItem(`honkoku.save.${session?.uid}`) ?? "null",
      );
      if (value && ["initiated", "completed"].includes(value.status))
        return {
          status: value.status,
          share: value.share === true,
          requestReview: value.requestReview === true,
          comment: typeof value.comment === "string" ? value.comment : "",
        };
    } catch {}
  }
  const storageKey = () => `honkoku.edit.${session?.uid}.${entry.id}.${index}`;
  function localDraft():
    { source: string; draft: string; updatedAt?: string | null } | undefined {
    try {
      return (
        JSON.parse(localStorage.getItem(storageKey()) ?? "null") ?? undefined
      );
    } catch {
      return;
    }
  }
  let acknowledged = "";
  let draftUpdatedAt: string | null | undefined;
  function remember() {
    try {
      localStorage.setItem(
        storageKey(),
        JSON.stringify({
          source,
          draft: acknowledged,
          updatedAt: draftUpdatedAt,
        }),
      );
    } catch {
      notice =
        "端末に下書きを保存できません。画面を閉じる前に保存してください。";
    }
  }
  function forget() {
    try {
      localStorage.removeItem(storageKey());
    } catch {}
    resumable = false;
  }
  async function reread() {
    const pageIndex = index;
    const fresh = (await listPages(entry.id)).find(
      (p) => p.index === pageIndex,
    );
    if (fresh) onpage(fresh);
    return fresh;
  }
  async function failure(error: unknown) {
    const pageIndex = index;
    notice = errorMessage(error);
    if (editing) saveState = "未保存の変更";
    const kind =
      error && typeof error === "object" && "kind" in error ? error.kind : "";
    if (kind === "conflict" || /編集中|already being edited/i.test(notice)) {
      const fresh = await reread().catch(() => undefined);
      if (pageIndex !== index) return;
      if (
        editing &&
        fresh &&
        (fresh.status !== "editing" || fresh.tempEditedBy !== session?.uid)
      ) {
        recovered = source;
        editing = false;
        void queue?.stop();
        queue = undefined;
      }
    }
  }
  function act(action: () => Promise<void>) {
    if (busy) return;
    busy = true;
    notice = "";
    operation = action()
      .catch(failure)
      .finally(() => {
        busy = false;
        operation = undefined;
      });
  }
  function begin(locked: Page, restore = false) {
    celebration = undefined;
    clearTimeout(toastTimer);
    lastOptions = savedOptions();
    onpage(locked);
    const local = restore ? localDraft() : undefined;
    acknowledged = locked.tempText ?? "";
    draftUpdatedAt = locked.updatedAt;
    source =
      local && local.source !== local.draft ? local.source : acknowledged;
    editing = true;
    recovered = "";
    saveState = locked.tempTextChanged
      ? `下書き保存${new Date(locked.updatedAt ?? Date.now()).toLocaleTimeString("ja-JP", { hour: "2-digit", minute: "2-digit" })}`
      : "未保存の変更";
    share = locked.share ?? false;
    completed = lastOptions?.status === "completed";
    requestReview = lastOptions?.requestReview ?? false;
    comment = lastOptions?.comment ?? "";
    const entryId = entry.id,
      pageIndex = index;
    queue = new Drafts(
      async (text) => {
        saveState = "送信中";
        const draft = await pageDraft(entryId, pageIndex, text);
        acknowledged = text;
        draftUpdatedAt = draft.updatedAt;
        onpage(draft);
        remember();
        saveState =
          source === text
            ? `下書き保存${new Date().toLocaleTimeString("ja-JP", { hour: "2-digit", minute: "2-digit" })}`
            : "未保存の変更";
        notice = "";
      },
      (error) => {
        saveState = "未保存の変更";
        void failure(error);
      },
    );
    remember();
    if (source !== acknowledged) queue.request(source);
  }
  function update(value: EditorUpdate) {
    composing = value.composing;
    if (!value.patches.length) return;
    source = value.source;
    saveState = "未保存の変更";
    remember();
    queue?.request(source);
  }
  function start() {
    act(async () => begin(await pageLock(entry.id, index, false)));
  }
  function resume() {
    act(async () => {
      const state = await pageLockState(entry.id, index);
      const fresh = await reread();
      if (
        !state.isMine ||
        !fresh ||
        fresh.status !== "editing" ||
        fresh.tempEditedBy !== session?.uid
      )
        throw { kind: "conflict", message: "編集状態が変わりました。" };
      if (localDraft()?.updatedAt !== fresh.updatedAt) {
        resumable = false;
        throw {
          kind: "conflict",
          message: "この端末以外で編集内容が更新されました。",
        };
      }
      begin(fresh, true);
    });
  }
  function takeOver() {
    act(async () => {
      await pageDiscard(entry.id, index);
      forget();
      await reread();
      begin(await pageLock(entry.id, index, false));
    });
  }
  function save(options: SaveOptions) {
    if (composing || !editing) return;
    prepareSound();
    act(async () => {
      savePopover = false;
      lastOptions = { ...options };
      try {
        sessionStorage.setItem(
          `honkoku.save.${session?.uid}`,
          JSON.stringify(options),
        );
      } catch {}
      await queue!.flush(source);
      saveState = "送信中";
      const saved = await pageSave(entry.id, index, options);
      await queue!.stop();
      queue = undefined;
      onpage(saved.page);
      editing = false;
      forget();
      saveState = "保存済み";
      if (saved.page.status === "completed") {
        completionSound();
        celebration = saved.count ?? null;
        toastTimer = setTimeout(() => {
          celebration = undefined;
        }, 2000);
        if (saved.count === undefined) {
          const savedIndex = index;
          void homeTimeline({ project_id: entry.projectId }, 100)
            .then((items) => {
              const event = items.find(
                (item) => item.event.id === saved.timelineEventId,
              )?.event;
              if (index === savedIndex && celebration !== undefined && event)
                celebration = event.count;
            })
            .catch(() => {});
        }
      }
    });
  }
  function discard() {
    act(async () => {
      discardPopover = false;
      await queue?.stop();
      try {
        await pageDiscard(entry.id, index);
      } catch (error) {
        begin(page, true);
        throw error;
      }
      editing = false;
      queue = undefined;
      forget();
      saveState = "";
      await reread();
    });
  }
  function appendOcr(text: string) {
    if (!editing || composing || busy) return;
    source += `${source && !/[\r\n]$/.test(source) ? (source.match(/\r\n|\r|\n/)?.[0] ?? "\n") : ""}${text}`;
    saveState = "未保存の変更";
    remember();
    queue?.request(source);
  }
  async function leave() {
    await operation;
    if (!editing) return;
    if (composing) throw Error("文字の変換を確定してから移動してください。");
    busy = true;
    try {
      await queue?.flush();
      await queue?.stop();
      queue = undefined;
      editing = false;
      resumable = true;
    } catch (error) {
      await failure(error);
      throw error;
    } finally {
      busy = false;
    }
  }
  $effect(() => {
    index;
    untrack(() => {
      editing = false;
      source = "";
      recovered = "";
      saveState = "";
      notice = "";
      savePopover = false;
      discardPopover = false;
      celebration = undefined;
      clearTimeout(toastTimer);
      void queue?.stop();
      queue = undefined;
      const local = localDraft();
      resumable = !!local && local.updatedAt === page.updatedAt;
    });
  });
  $effect(() => {
    if (!session && editing) {
      editing = false;
      void queue?.stop();
      queue = undefined;
      saveState = "";
    }
  });
  $effect(() => {
    registerLeave(editing || busy ? leave : undefined);
  });
  $effect(() => {
    const uid = page.status === "editing" ? page.tempEditedBy : null;
    let cancelled = false;
    lockName = "名前を確認中";
    if (uid)
      void user(uid)
        .then((u) => {
          if (!cancelled) lockName = u.displayName;
        })
        .catch(() => {
          if (!cancelled) lockName = "名前不明";
        });
    return () => {
      cancelled = true;
    };
  });
  let strip: HTMLDivElement;
  let notePanel: HTMLElement;
  const ocrText = (v: Page["ocr"]) => ({
    minna: typeof v?.minna === "string" ? v.minna : v?.minna?.text,
    ndl: typeof v?.ndl === "string" ? v.ndl : v?.ndl?.text,
  });
  let ocr = $derived(ocrText(page.ocr));
  $effect(() => {
    index;
    noteIndex = null;
    half = "";
  });
  $effect(() => {
    const values = pageNotes;
    let cancelled = false;
    for (const n of values)
      if (n?.createdBy)
        void user(n.createdBy)
          .then((u) => {
            if (!cancelled) authors = { ...authors, [u.uid]: u.displayName };
          })
          .catch(() => {
            if (!cancelled && n?.createdBy)
              authors = { ...authors, [n.createdBy]: "名前不明" };
          });
    return () => {
      cancelled = true;
    };
  });
  $effect(() => {
    index;
    if (expanded)
      void tick().then(() =>
        strip?.querySelector('[aria-current="page"]')?.scrollIntoView({
          block: "nearest",
          inline: "center",
          behavior: "instant",
        }),
      );
  });
  function go(next: number) {
    if (next >= 0 && next < pages.length)
      location.hash = href({ entryId: entry.id, pageIndex: next });
  }
  function references(element: HTMLElement) {
    const click = (event: MouseEvent) => {
      const target = (event.target as HTMLElement).closest<HTMLButtonElement>(
        "[data-note]",
      );
      if (target) {
        noteIndex = Number(target.dataset.note) - 1;
        void tick().then(() =>
          notePanel
            .querySelector(`[data-note-index="${noteIndex}"]`)
            ?.scrollIntoView({ block: "nearest" }),
        );
      }
    };
    element.addEventListener("click", click);
    return { destroy: () => element.removeEventListener("click", click) };
  }
  function measureColumns(element: HTMLElement) {
    const apply = () => {
      const available = element.clientHeight;
      if (available > 0)
        element.style.setProperty("--column-height", `${available}px`);
    };
    const observer = new ResizeObserver(apply);
    observer.observe(element);
    if (element.parentElement) observer.observe(element.parentElement);
    apply();
    return { destroy: () => observer.disconnect() };
  }
  onMount(() => {
    const unload = () => {
      if (editing) {
        remember();
        void queue?.flush().catch(() => {});
      }
    };
    window.addEventListener("pagehide", unload);
    window.addEventListener("beforeunload", unload);
    const keys = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s" && editing) {
        e.preventDefault();
        save(
          lastOptions ?? {
            status: "initiated",
            share: page.share ?? false,
            requestReview: false,
            comment: "",
          },
        );
        return;
      }
      if (editing || busy) return;
      if (
        e.altKey ||
        e.ctrlKey ||
        e.metaKey ||
        e.shiftKey ||
        (e.target instanceof HTMLElement &&
          e.target.closest('input,textarea,select,[contenteditable="true"]'))
      )
        return;
      const current = parseRoute(location.hash).pageIndex ?? index;
      const next = (
        {
          ArrowLeft: current - 1,
          ArrowRight: current + 1,
          Home: 0,
          End: pages.length - 1,
        } as Record<string, number>
      )[e.key];
      if (next !== undefined) {
        e.preventDefault();
        go(next);
      }
    };
    window.addEventListener("keydown", keys);
    return () => {
      registerLeave(undefined);
      window.removeEventListener("keydown", keys);
      window.removeEventListener("pagehide", unload);
      window.removeEventListener("beforeunload", unload);
      clearTimeout(toastTimer);
      void queue?.stop();
    };
  });
</script>

<div class="workbench">
  <div class="workbench-toolbar">
    <div class="page-position">
      <button
        disabled={index === 0}
        onclick={() => go(index - 1)}
        aria-label="前のコマ">‹</button
      ><strong>{index + 1}／{entry.size}コマ</strong><button
        disabled={index === pages.length - 1}
        onclick={() => go(index + 1)}
        aria-label="次のコマ">›</button
      ><span class="status {statusClass(page.status)}"
        >{status(page.status).symbol}{status(page.status).label}</span
      ><span class="caption muted edit-status" role="status"
        >{saveState || (editing ? "未保存の変更" : "閲覧のみ")}</span
      >
    </div>
    <div class="toolbar-actions">
      {#if editing}
        <div class="save-control">
          <button
            class="primary"
            disabled={busy || composing}
            onclick={() => {
              savePopover = !savePopover;
              discardPopover = false;
            }}>保存</button
          >
          {#if savePopover}<form
              class="save-popover"
              onsubmit={(e) => {
                e.preventDefault();
                save({
                  status: completed ? "completed" : "initiated",
                  share,
                  requestReview,
                  comment,
                });
              }}
            >
              <label
                ><input
                  type="checkbox"
                  bind:checked={completed}
                />このコマを完了</label
              >
              <label><input type="checkbox" bind:checked={share} />共有</label>
              <label
                ><input
                  type="checkbox"
                  bind:checked={requestReview}
                />レビュー依頼</label
              >
              <label class="save-comment"
                >コメント<input type="text" bind:value={comment} /></label
              >
              <div>
                <button
                  type="submit"
                  class="primary"
                  disabled={busy || composing}>保存を確定</button
                ><button type="button" onclick={() => (savePopover = false)}
                  >閉じる</button
                >
              </div>
            </form>{/if}
        </div>
        <div class="save-control">
          <button
            disabled={busy || composing}
            onclick={() => {
              discardPopover = !discardPopover;
              savePopover = false;
            }}>破棄</button
          >
          {#if discardPopover}<div class="save-popover">
              <p>変更を破棄しますか</p>
              <div>
                <button onclick={discard} disabled={busy}>破棄する</button
                ><button onclick={() => (discardPopover = false)}
                  >キャンセル</button
                >
              </div>
            </div>{/if}
        </div>
      {:else if page.status === "editing"}
        {#if session && page.tempEditedBy === session.uid}
          {#if resumable}<button disabled={busy} onclick={resume}
              >編集を再開</button
            >
          {:else}<span class="caption muted">この端末以外で編集中</span><button
              disabled={busy}
              onclick={takeOver}>破棄して引き継ぐ</button
            >{/if}
        {:else}<span class="caption muted"
            >他のユーザーが編集中・{lockName}</span
          >{/if}
      {:else if session}<button class="primary" disabled={busy} onclick={start}
          >編集開始</button
        >{/if}
      <button onclick={() => (swapped = !swapped)}>⇄左右を入れ替え</button
      ><button
        disabled={editing}
        aria-pressed={!horizontal}
        onclick={() => (horizontal = !horizontal)}
        >{horizontal ? "横書き" : "縦書き"}⌄</button
      >
    </div>
  </div>
  {#if notice}<div class="edit-notice caption" role="status">{notice}</div>{/if}
  {#if recovered}<details class="edit-recovery">
      <summary>端末に残っている本文</summary><textarea
        readonly
        value={recovered}
        aria-label="端末に残っている本文"></textarea>
    </details>{/if}
  {#if celebration !== undefined}<div class="completion-toast" role="status">
      ✓完了{celebration !== null
        ? `・${celebration.toLocaleString("ja-JP")}文字`
        : ""}
    </div>{/if}
  <div class="workbench-panes" class:swapped>
    <section class="panel transcription-panel">
      <div class="pane-toolbar">
        <h2>翻刻</h2>
        <span class="caption muted"
          >{horizontal ? "横書き" : "縦書き"}{half ? `・${half}` : ""}</span
        >
      </div>
      {#if editing}<div class="workbench-editor" inert={busy}>
          <VerticalEditor bind:source onupdate={update} />
        </div>{:else}
        <div class="transcription" class:horizontal use:references>
          {#each groups as g, i}<section
              class="column-group"
              class:half-selected={half === g.label && !!g.label}
              aria-label={g.label || `本文${i + 1}`}
            >
              {#if g.label}<button
                  class="column-label"
                  class:active={half === g.label}
                  onclick={() => (half = half === g.label ? "" : g.label)}
                  >{g.label}</button
                >{/if}
              <div class="columns" use:measureColumns>
                {#each g.columns as column}<div class="transcription-column">
                    {@html renderInline(column)}
                  </div>{/each}
              </div>
            </section>{:else}<p class="empty">本文はありません。</p>{/each}
        </div>
      {/if}
    </section>
    <Facsimile canvas={canvases[index]} pageNumber={index + 1} bind:half />
  </div>
  <div class="workbench-supplement">
    <section class="panel ocr-panel">
      <h2>OCR</h2>
      <div class="ocr-columns scroll">
        {#if ocr.minna}<div>
            <h3>みんなで翻刻</h3>
            {#if editing}<button
                disabled={busy || composing}
                onclick={() => appendOcr(ocr.minna!)}>本文に挿入</button
              >{/if}
            <pre>{ocr.minna}</pre>
          </div>{/if}{#if ocr.ndl}<div>
            <h3>国立国会図書館</h3>
            {#if editing}<button
                disabled={busy || composing}
                onclick={() => appendOcr(ocr.ndl!)}>本文に挿入</button
              >{/if}
            <pre>{ocr.ndl}</pre>
          </div>{/if}{#if !ocr.minna && !ocr.ndl}<p class="muted">
            OCRの記録はありません。
          </p>{/if}
      </div>
    </section>
    <section class="panel notes-panel" bind:this={notePanel}>
      <h2>注記</h2>
      <div class="scroll">
        {#each pageNotes as n, i}{#if n}<article
              class="note"
              class:highlighted={noteIndex === i}
              data-note-index={i}
            >
              <strong>＃{i + 1}</strong>
              <p>{n.content}</p>
              <div class="caption muted">
                {n.createdBy
                  ? (authors[n.createdBy] ?? "名前を確認中")
                  : "名前不明"}・{date(n.createdAt)}
              </div>
            </article>{/if}{/each}{#if !pageNotes.some(Boolean)}<p
            class="muted"
          >
            注記はありません。
          </p>{/if}{#if noteIndex !== null && !pageNotes[noteIndex]}<p
            class="muted"
            role="status"
          >
            この番号の注記はありません。
          </p>{/if}
      </div>
    </section>
  </div>
  <nav
    class="panel filmstrip"
    class:expanded
    aria-label="コマを選択"
    onmouseenter={() => (expanded = true)}
    onmouseleave={(e) => {
      if (!e.currentTarget.contains(document.activeElement)) expanded = false;
    }}
    onfocusin={() => (expanded = true)}
    onfocusout={(e) => {
      if (!e.currentTarget.contains(e.relatedTarget as Node)) expanded = false;
    }}
  >
    <div class="filmstrip-heading">
      <strong>コマ</strong><span class="status completed caption">✓完了</span
      ><span class="status initiated caption">◐翻刻中</span><span
        class="status default caption">○未着手</span
      ><span class="caption muted">←→で移動</span>
    </div>
    <div class="status-strip">
      {#each pages as p (p.id)}<a
          class={statusClass(p.status)}
          class:current={p.index === index}
          href={href({ entryId: entry.id, pageIndex: p.index })}
          aria-label={`コマ${p.index + 1}・${status(p.status).label}`}
          aria-current={p.index === index ? "page" : undefined}
        ></a>{/each}
    </div>
    <div class="filmstrip-thumbnails" bind:this={strip} inert={!expanded}>
      {#each pages as p (p.id)}<a
          href={href({ entryId: entry.id, pageIndex: p.index })}
          class:current={p.index === index}
          aria-current={p.index === index ? "page" : undefined}
          ><Thumbnail
            url={expanded ? canvases[p.index]?.thumbnailUrl : null}
          /><span
            >{p.index + 1}・<span class="status {statusClass(p.status)}"
              >{status(p.status).symbol}{status(p.status).label}</span
            ></span
          ></a
        >{/each}
    </div>
  </nav>
</div>
