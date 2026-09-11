<script lang="ts">
  import { openSessions } from "../editing-sessions.svelte";
  import { restoreDraft } from "../editing-draft";
  import type { Region } from "../region.svelte";
  import Skeleton from "./Skeleton.svelte";
  import RegionNotice from "./RegionNotice.svelte";
  import { onMount, tick, untrack } from "svelte";
  import type {
    Canvas,
    Entry,
    Page,
    SessionInfo,
    SaveOptions,
    PageNote,
    JsonValue,
  } from "@honkoku/client-api/types";
  import { alignColumns, transcriptionColumns } from "@honkoku/markup";
  import { pageLinesWithLocal as pageLines } from "../../../../packages/client-api/ocr";
  import OcrPanel from "./OcrPanel.svelte";
  import type { LocalOcrPage } from "@honkoku/client-api/types";
  import {
    createEditor,
    fromMarkup,
    TextSelection,
    textareaSource,
  } from "@honkoku/editor";
  import Transcription from "./Transcription.svelte";
  import { date, label, notes, status, statusClass, user } from "../lib";
  import { href, parseRoute } from "../routes";
  import VerticalEditor from "@honkoku/editor/VerticalEditor.svelte";
  import type { EditorUpdate } from "@honkoku/editor";
  import {
    editingPages,
    pageLock,
    pageDraftTextAndNotes,
    isTauri,
    pageSave,
    pageDiscard,
    pageLockState,
    listPages,
    homeTimeline,
  } from "@honkoku/client-api/invoke";
  import { errorMessage } from "../lib";
  import { Drafts } from "../drafts";
  import { prepareSound, completionSound } from "../sound";
  import GlyphDrawer from "./GlyphDrawer.svelte";
  import ClipSelection from "./ClipSelection.svelte";
  import Facsimile from "./Facsimile.svelte";
  import Thumbnail from "./Thumbnail.svelte";
  let {
    entry,
    pages,
    canvases,
    index,
    column,
    session,
    onpage,
    registerLeave,
    pagesRegion,
    pagesPending = false,
    canvasesRegion,
  }: {
    pagesRegion: Region<Page[]>;
    pagesPending?: boolean;
    canvasesRegion: Region<Canvas[]>;
    entry: Entry;
    pages: Page[];
    canvases: Canvas[];
    index: number;
    column?: number;
    session: SessionInfo | null;
    onpage: (page: Page) => void;
    registerLeave: (guard: (() => Promise<void>) | undefined) => void;
  } = $props();
  let currentEntryId = $derived(entry.id);
  let sessionUid = $derived(session?.uid);
  let page = $derived(pages.find((p) => p.index === index)!);
  let swapped = $state(false),
    horizontal = $state(false),
    half = $state(""),
    expanded = $state(false),
    noteIndex = $state<number | null>(null),
    authors = $state<Record<string, string>>({});
  let editing = $state(false),
    busy = $state(false),
    composing = $state(false);
  let verifying = $state(false),
    lockSlow = $state(false),
    lockFailed = $state(false),
    validationAttempt = $state(0);
  let source = $state(""),
    saveState = $state(""),
    notice = $state("");
  let displayedSource = $derived(
    page.status === "editing" && page.syncMode
      ? (page.tempText ?? page.text)
      : page.text,
  );
  let tempNotes = $state<(JsonValue | null)[]>([]);
  let pageNotes = $derived(
    notes({
      ...page,
      notes: editing
        ? tempNotes
        : page.status === "editing" && page.syncMode
          ? (page.tempNotes ?? page.notes)
          : page.notes,
    }),
  );
  let noteCount = $derived(pageNotes.filter(Boolean).length);
  let hasReferences = $derived(
    /＃[0-9０-９]+/.test(editing ? source : displayedSource),
  );
  let ocrOpen = $state(sessionStorage.getItem("honkoku.ocr.open") === "true");
  let showLines = $state(false);
  let noteList = $state(false);
  let noteAnchor = $state<HTMLElement>();
  let notePosition = $state({ x: 0, y: 0 });
  let noteTimer: ReturnType<typeof setTimeout>;
  function positionNote(target: HTMLElement) {
    noteAnchor = target;
    const rect = target.getBoundingClientRect();
    notePosition = {
      x: Math.max(8, Math.min(rect.left, innerWidth - 336)),
      y: Math.max(8, Math.min(rect.bottom + 4, innerHeight - 250)),
    };
  }
  function closeNotes() {
    clearTimeout(noteTimer);
    noteIndex = null;
    noteList = false;
  }
  function deferClose() {
    clearTimeout(noteTimer);
    noteTimer = setTimeout(() => {
      if (
        document.activeElement !== noteAnchor &&
        !document
          .querySelector(".note-popover")
          ?.contains(document.activeElement)
      )
        closeNotes();
    }, 180);
  }
  function changeNote(content: string | null, index?: number) {
    const position = index ?? tempNotes.length;
    const now = new Date().toISOString();
    const old = pageNotes[position];
    const note: PageNote | null =
      content === null
        ? null
        : {
            id: old?.id ?? "",
            type: "note",
            content,
            markdown: content,
            createdBy: old?.createdBy ?? session!.uid,
            createdAt: old?.createdAt ?? now,
            updatedAt: now,
          };
    tempNotes = [...tempNotes];
    tempNotes[position] = note as JsonValue;
    saveState = "未保存の変更";
    remember();
    queue?.request(draftPayload());
    closeNotes();
    return position;
  }
  const draftPayload = () => JSON.stringify({ text: source, notes: tempNotes });
  const notesPending = () =>
    isTauri() &&
    JSON.stringify(tempNotes) !== JSON.stringify(page.tempNotes ?? page.notes);

  let localOcr = $state<LocalOcrPage | null>(null);
  let lineModel = $derived(
    pageLines({ ocr: { ...page.ocr, local: localOcr } }, canvases[index]),
  );
  let editorInstance: ReturnType<typeof createEditor> | undefined;
  function insertOcr(text: string) {
    if (!editing || busy || composing || !editorInstance) return;
    const raw = document.querySelector<HTMLTextAreaElement>(
      ".editor-raw-textarea",
    );
    if (raw) {
      const end = raw.value.indexOf("\n", raw.selectionStart);
      const position = end < 0 ? raw.value.length : end;
      source = textareaSource(
        source,
        raw.value.slice(0, position) + "\n" + text + raw.value.slice(position),
      );
      saveState = "未保存の変更";
      remember();
      queue?.request(draftPayload());
      return;
    }
    const instance = editorInstance;
    instance.run((state, dispatch) => {
      if (state.selection.$head.depth < 1) return false;
      const position = state.selection.$head.after(1);
      const content = fromMarkup(text).content;
      const transaction = state.tr.insert(position, content);
      transaction.setSelection(
        TextSelection.near(transaction.doc.resolve(position + 1)),
      );
      dispatch?.(transaction.scrollIntoView());
      return true;
    });
  }
  let columns = $derived(
    transcriptionColumns(editing ? source : displayedSource),
  );
  let alignment = $derived(
    alignColumns(
      columns.map((c) => c.text),
      lineModel.lines,
    ),
  );
  let currentColumn = $state(-1);
  let hoveredLine = $state<number | null>(null);
  let selectedLine = $derived(hoveredLine ?? alignment[currentColumn] ?? null);
  let highlightedColumn = $derived(
    hoveredLine === null ? currentColumn : alignment.indexOf(hoveredLine),
  );
  let editor = $state<VerticalEditor>();
  let transcription = $state<Transcription>();
  function columnChange(index: number) {
    currentColumn = index;
    hoveredLine = null;
  }
  function selectLine(lineIndex: number) {
    const column = alignment.indexOf(lineIndex);
    if (column < 0) return;
    columnChange(column);
    if (editing) editor?.focusColumn(column);
    else transcription?.focusColumn(column);
  }
  let savePopover = $state(false),
    discardPopover = $state(false);
  let completed = $state(false),
    share = $state(false),
    requestReview = $state(false),
    comment = $state("");
  let lockName = $state("名前を確認中");
  let menuOpen = $state(false);
  let glyphOpen = $state(false), glyphCharacter = $state("");
  let clipping = $state(false);
  let glyphTimer: ReturnType<typeof setTimeout>;
  function glyphChange(character: string, open: boolean) {
    clearTimeout(glyphTimer);
    if (open) { glyphCharacter=character; glyphOpen=true; clipping=false; }
    else if (glyphOpen) glyphTimer=setTimeout(() => glyphCharacter=character,300);
  }
  $effect(() => { index; currentEntryId; sessionUid; clipping=false; glyphOpen=false; glyphCharacter=""; clearTimeout(glyphTimer); });

  let otherEdits = $derived(
    [
      ...new Map(
        [...Object.values(openSessions.pages), ...pages].map((p) => [p.id, p]),
      ).values(),
    ].filter(
      (p) =>
        p.id !== page.id &&
        p.status === "editing" &&
        !!session &&
        p.tempEditedBy === session.uid,
    ),
  );
  $effect(() => {
    const current = pages;
    untrack(() => current.forEach((p) => openSessions.observe(p)));
  });
  $effect(() => {
    const uid = session?.uid;
    let alive = true;
    if (uid)
      void editingPages()
        .then((values) => {
          if (alive) values.forEach((p) => openSessions.observe(p));
        })
        .catch(() => {});
    return () => {
      alive = false;
    };
  });
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
    | {
        source: string;
        draft: string;
        notes?: (JsonValue | null)[];
        updatedAt?: string | null;
      }
    | undefined {
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
          notes: tempNotes,
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
    if (busy || verifying) return;
    busy = true;
    notice = "";
    operation = action()
      .catch(failure)
      .finally(() => {
        busy = false;
        operation = undefined;
      });
  }
  function begin(locked: Page, restore = false, verified = true) {
    celebration = undefined;
    clearTimeout(toastTimer);
    lastOptions = savedOptions();
    onpage(locked);
    const local = restore ? localDraft() : undefined;
    acknowledged = locked.tempText ?? "";
    draftUpdatedAt = locked.updatedAt;
    const restored = restoreDraft(locked, local);
    source = restored.source;
    tempNotes = restored.notes;
    editing = true;
    recovered = "";
    saveState = locked.tempTextChanged
      ? `下書き保存${new Date(locked.updatedAt ?? Date.now()).toLocaleTimeString("ja-JP", { hour: "2-digit", minute: "2-digit" })}`
      : "未保存の変更";
    share = locked.share ?? false;
    completed = lastOptions?.status === "completed";
    requestReview = lastOptions?.requestReview ?? false;
    comment = lastOptions?.comment ?? "";
    if (!verified) return;
    const entryId = entry.id,
      pageIndex = index;
    queue = new Drafts(
      async (payload) => {
        const { text, notes: pendingNotes } = JSON.parse(payload) as {
          text: string;
          notes: (JsonValue | null)[];
        };
        saveState = "送信中";
        const draft = await pageDraftTextAndNotes(
          entryId,
          pageIndex,
          text,
          pendingNotes,
        );
        acknowledged = text;
        draftUpdatedAt = draft.updatedAt;
        onpage(draft);
        remember();
        saveState =
          source === text &&
          JSON.stringify(tempNotes) === JSON.stringify(pendingNotes) &&
          !notesPending()
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
    if (source !== acknowledged || local?.notes) queue.request(draftPayload());
  }
  function update(value: EditorUpdate) {
    composing = value.composing;
    if (!value.patches.length) return;
    source = value.source;
    saveState = "未保存の変更";
    remember();
    queue?.request(draftPayload());
  }
  function start() {
    act(async () => begin(await pageLock(entry.id, index, false)));
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
      await queue!.flush(draftPayload());
      if (notesPending())
        throw Error(
          "注記の変更はこの端末に保存されています。注記の送信を確認できませんでした。再試行してください。",
        );
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
    queue?.request(draftPayload());
  }
  async function leave() {
    if (verifying) {
      if (editing) remember();
      return;
    }
    await operation;
    if (!editing) return;
    if (composing) throw Error("文字の変換を確定してから移動してください。");
    busy = true;
    try {
      await queue?.flush();
      await queue?.stop();
      queue = undefined;
      editing = false;
      remember();
    } catch (error) {
      await failure(error);
      throw error;
    } finally {
      busy = false;
    }
  }
  $effect(() => {
    const pageIndex = index,
      entryId = currentEntryId,
      uid = sessionUid,
      pending = pagesPending;
    validationAttempt;
    let cancelled = false;
    let validationTimer: ReturnType<typeof setTimeout>;
    untrack(() => {
      verifying = false;
      lockSlow = false;
      lockFailed = false;
      editing = false;
      currentColumn = -1;
      hoveredLine = null;
      showLines = false;
      localOcr = null;
      closeNotes();
      source = "";
      recovered = "";
      saveState = "";
      notice = "";
      savePopover = false;
      discardPopover = false;
      celebration = undefined;
      clearTimeout(toastTimer);
      clearTimeout(glyphTimer);
      void queue?.stop();
      queue = undefined;
      if (!uid || pending) return;
      busy = true;
      verifying = true;
      validationTimer = setTimeout(() => (lockSlow = true), 15000);
      if (page.status === "editing" && page.tempEditedBy === uid)
        begin(page, true, false);
      operation = (async () => {
        const state = await pageLockState(entryId, pageIndex);
        if (cancelled) return;
        const fresh = state.page ?? (await reread());
        if (cancelled || !fresh) return;
        onpage(fresh);
        if (state.isMine) begin(fresh, true);
        else {
          editing = false;
          if (localDraft()) {
            recovered = localDraft()?.source ?? "";
            notice = "編集状態が変わりました。";
          } else if (fresh.status === "editing")
            notice = "他のユーザーが編集中です。";
        }
      })()
        .catch((error) => {
          if (!cancelled) {
            lockFailed = true;
            void failure(error);
          }
        })
        .finally(() => {
          if (!cancelled) {
            busy = false;
            if (!lockFailed) verifying = false;
            lockSlow = false;
            clearTimeout(validationTimer);
            operation = undefined;
          }
        });
    });
    return () => {
      cancelled = true;
      clearTimeout(validationTimer);
    };
  });
  $effect(() => {
    saveState;
    notice;
    const timer = setTimeout(() => {
      saveState = "";
      notice = "";
    }, 6000);
    return () => clearTimeout(timer);
  });
  $effect(() => {
    const requested = column;
    const pageIndex = index;
    const count = columns.length;
    const pending = pagesPending;
    verifying;
    if (requested === undefined || pending) return;
    void tick().then(() => {
      if (pageIndex !== index || requested !== column || requested >= count)
        return;
      columnChange(requested);
      document
        .querySelector(
          `.transcription-reader [data-column-index="${requested}"]`,
        )
        ?.scrollIntoView({ block: "nearest", inline: "nearest" });
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
    const targetOf = (event: Event) =>
      (event.target as HTMLElement).closest<HTMLElement>("[data-note]");
    const show = (event: Event) => {
      const target = targetOf(event);
      if (!target || document.querySelector(".editor-note-popover")) return;
      clearTimeout(noteTimer);
      noteList = false;
      noteIndex = Number(target.dataset.note) - 1;
      positionNote(target);
    };
    const click = (event: MouseEvent) => {
      const target = targetOf(event);
      if (!target) return;
      show(event);
      if (editing && target.closest(".vertical-editor")) return;
      if (editing && pageNotes[noteIndex!]) {
        editor?.editNote(noteIndex!, pageNotes[noteIndex!]!.content);
        closeNotes();
      }
    };
    const leave = (event: Event) => {
      if (targetOf(event)) deferClose();
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeNotes();
    };
    element.addEventListener("pointerover", show);
    element.addEventListener("focusin", show);
    element.addEventListener("pointerout", leave);
    element.addEventListener("focusout", leave);
    element.addEventListener("click", click);
    element.addEventListener("keydown", key);
    element.addEventListener("scroll", closeNotes, true);
    return {
      destroy() {
        element.removeEventListener("pointerover", show);
        element.removeEventListener("focusin", show);
        element.removeEventListener("pointerout", leave);
        element.removeEventListener("focusout", leave);
        element.removeEventListener("click", click);
        element.removeEventListener("keydown", key);
        element.removeEventListener("scroll", closeNotes, true);
        clearTimeout(noteTimer);
      },
    };
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
      if (busy && !verifying) return;
      if (
        e.altKey ||
        e.ctrlKey ||
        e.metaKey ||
        e.shiftKey ||
        (e.target instanceof HTMLElement &&
          e.target.closest(
            'input,textarea,select,[contenteditable="true"],.workbench-editor,.ocr-drawer,.note-popover,.save-popover,.workbench-menu',
          ))
      )
        return;
      const current = parseRoute(location.hash).pageIndex ?? index;
      if (e.key.toLowerCase() === "n") {
        const next = [
          ...pages.filter((p) => p.index > current),
          ...pages.filter((p) => p.index < current),
        ].find((p) => p.status === "default" || p.status === "initiated");
        if (next) {
          e.preventDefault();
          go(next.index);
        }
        return;
      }
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
      clearTimeout(glyphTimer);
      void queue?.stop();
    };
  });
</script>

<div class="workbench">
  <RegionNotice region={pagesRegion} />
  {#if lockSlow || lockFailed}<div class="region-notice caption">
      ロックを確認できません。<button onclick={() => validationAttempt++}
        >再試行</button
      >
    </div>{/if}
  {#if notice || saveState || celebration !== undefined}<div
      class="workbench-notification caption"
      role="status"
    >
      {#if celebration !== undefined}<span class="completion-toast"
          >✓完了{celebration !== null
            ? `・${celebration.toLocaleString("ja-JP")}文字`
            : ""}</span
        >
      {:else if notice}<span class="edit-notice">{notice}</span>{:else}<span
          class="edit-status">{saveState}</span
        >{/if}
    </div>{/if}
  <div class="workbench-toolbar">
    <div class="editing-pages">
      {#if otherEdits.length}<details>
          <summary
            >編集中のコマ<span class="count">{otherEdits.length}</span></summary
          >
          <div class="editing-page-links">
            {#each otherEdits as p}<a
                href={href({ entryId: p.entryId, pageIndex: p.index })}
                >{p.entryId === entry.id ? "" : "別の資料・"}コマ{p.index +
                  1}</a
              >{/each}
          </div>
        </details>{/if}
    </div>
    <div class="page-position">
      <span class="entry-position-title caption muted"
        >{label(entry.label)}</span
      >
      <div>
        <button
          disabled={index === 0}
          onclick={() => go(index - 1)}
          aria-label="前のコマ">‹</button
        ><span class="page-count numeric"
          >{index + 1}／{entry.size ?? pages.length}</span
        ><button
          disabled={index === pages.length - 1}
          onclick={() => go(index + 1)}
          aria-label="次のコマ">›</button
        >
      </div>
    </div>
    <div class="toolbar-actions">
      {#if editing}
        <div class="save-control">
          <button
            class="primary"
            disabled={busy || composing || verifying}
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
                  disabled={busy || composing || verifying}>保存を確定</button
                ><button type="button" onclick={() => (savePopover = false)}
                  >閉じる</button
                >
              </div>
            </form>{/if}
        </div>
        <div class="save-control">
          <button
            disabled={busy || composing || verifying}
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
      {:else if session && page.status !== "editing"}<button
          class="primary"
          disabled={busy || pagesPending || verifying}
          onclick={start}>編集開始</button
        >{/if}
      {#if noteCount && !hasReferences}<button
          class="notes-count"
          aria-expanded={noteList}
          onclick={(event) => {
            noteList = !noteList;
            noteIndex = null;
            positionNote(event.currentTarget);
          }}>注記{noteCount}件</button
        >{/if}
      <button
        aria-pressed={ocrOpen}
        aria-controls="ocr-drawer"
        onclick={() => {
          ocrOpen = !ocrOpen;
          sessionStorage.setItem("honkoku.ocr.open", String(ocrOpen));
        }}>OCR</button
      >
      <button
        aria-pressed={showLines}
        disabled={!lineModel.lines.length}
        onclick={() => (showLines = !showLines)}>行枠</button
      >
      <div class="workbench-menu">
        <button
          aria-label="表示設定"
          aria-expanded={menuOpen}
          onclick={() => (menuOpen = !menuOpen)}>⋯</button
        >
        {#if menuOpen}<div class="menu-options">
            <button disabled={!session || !canvases[index]?.infoJsonUrl} onclick={() => { clipping=true; glyphOpen=false; menuOpen=false; }}>切り抜き</button>
            <button
              onclick={() => {
                swapped = !swapped;
                menuOpen = false;
              }}>⇄左右を入れ替え</button
            ><button
              aria-pressed={!horizontal}
              onclick={() => {
                horizontal = !horizontal;
                menuOpen = false;
              }}>{horizontal ? "横書き" : "縦書き"}</button
            >
          </div>{/if}
      </div>
    </div>
  </div>
  {#if recovered}<details class="edit-recovery">
      <summary>端末に残っている本文</summary><textarea
        readonly
        value={recovered}
        aria-label="端末に残っている本文"></textarea>
    </details>{/if}
  <div class="workbench-panes" class:swapped>
    <section class="panel transcription-panel" use:references>
      {#if pagesPending}<Skeleton
          shape="columns"
          count={8}
        />{:else if editing}<div
          class="workbench-editor"
          inert={busy || verifying}
        >
          <VerticalEditor
            {horizontal}
            accountId={session?.uid}
            bind:this={editor}
            bind:source
            onready={(instance) => (editorInstance = instance)}
            onupdate={update}
            onglyph={glyphChange}
            oncolumnchange={columnChange}
            {highlightedColumn}
            onnote={changeNote}
          />
        </div>{:else}
        <div class="transcription-reader">
          <Transcription
            bind:this={transcription}
            source={displayedSource}
            {horizontal}
            bind:half
            {highlightedColumn}
            oncolumnchange={columnChange}
          />
        </div>
      {/if}
      <div
        id="ocr-drawer"
        class="ocr-drawer"
        hidden={!ocrOpen}
        inert={!ocrOpen}
      >
        <button
          class="drawer-close"
          aria-label="OCRを閉じる"
          onclick={() => {
            ocrOpen = false;
            sessionStorage.setItem("honkoku.ocr.open", "false");
          }}>×</button
        >
        <OcrPanel
          {page}
          {editing}
          disabled={busy || composing || verifying}
          onresult={(result) => (localOcr = result)}
          oninsert={insertOcr}
          oninsertall={appendOcr}
        />
      </div>
    </section>
    <Facsimile
      canvas={canvases[index]}
      pending={canvasesRegion.pending && !canvases[index]}
      region={canvasesRegion}
      pageNumber={index + 1}
      bind:half
      {lineModel}
      {showLines}
      highlightedLine={selectedLine}
      onlineselect={selectLine}
      onlinehover={(line) => (hoveredLine = line)}
    >
      {#snippet children(viewer)}
        {#if glyphOpen}<GlyphDrawer character={glyphCharacter} onclose={() => glyphOpen=false} />{/if}
        {#if clipping}<ClipSelection {viewer} canvas={canvases[index]} entryId={entry.id} {index}
          onclose={() => clipping=false} onsaved={() => { clipping=false; notice="クリップを保存しました。"; }} />{/if}
      {/snippet}
    </Facsimile>
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
    <div class="status-strip">
      {#each pages as p (p.id)}<a
          class={statusClass(p.status)}
          class:current={p.index === index}
          href={href({ entryId: entry.id, pageIndex: p.index })}
          aria-label={`コマ${p.index + 1}・${p.status === "editing" && p.tempEditedBy === session?.uid ? "あなたが編集中" : status(p.status).label}`}
          aria-current={p.index === index ? "page" : undefined}
        ></a>{/each}
    </div>
    <div class="filmstrip-thumbnails" bind:this={strip} inert={!expanded}>
      {#each pages as p (p.id)}<a
          href={href({ entryId: entry.id, pageIndex: p.index })}
          class={statusClass(p.status)}
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

{#if noteList || noteIndex !== null}
  <div
    class="note-popover"
    role="dialog"
    aria-label="注記"
    tabindex="-1"
    style:left={`${notePosition.x}px`}
    style:top={`${notePosition.y}px`}
    onmouseenter={() => clearTimeout(noteTimer)}
    onmouseleave={deferClose}
    onfocusin={() => clearTimeout(noteTimer)}
    onfocusout={(event) => {
      if (!event.currentTarget.contains(event.relatedTarget as Node))
        deferClose();
    }}
    onkeydown={(event) => {
      if (event.key === "Escape") {
        noteAnchor?.focus();
        closeNotes();
      }
    }}
  >
    <button class="note-close" aria-label="注記を閉じる" onclick={closeNotes}
      >×</button
    >
    {#each pageNotes as n, i}{#if n && (noteList || noteIndex === i)}
        <article class="note" data-note-index={i}>
          <strong>＃{i + 1}</strong>
          <p>{n?.content ?? "この番号の注記はありません。"}</p>
          {#if n}<div class="caption muted">
              {n.createdBy
                ? (authors[n.createdBy] ?? "名前を確認中")
                : "名前不明"}・{date(n.createdAt)}
            </div>{/if}
          {#if editing && n}<button
              onclick={() => {
                editor?.editNote(i, n.content);
                closeNotes();
              }}>注記を編集</button
            >{/if}
        </article>
      {/if}{/each}
    {#if noteIndex !== null && !pageNotes[noteIndex]}<p>
        この番号の注記はありません。
      </p>{/if}
  </div>
{/if}
