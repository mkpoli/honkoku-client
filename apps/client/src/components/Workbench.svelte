<script lang="ts">
  import type { Snippet } from "svelte";
  import { openSessions } from "../editing-sessions.svelte";
  import { notesPending, restoreDraft, type LocalDraft } from "../editing-draft";
  import type { Region } from "../region.svelte";
  import HistoryDrawer from "./HistoryDrawer.svelte";
  import BibliographyDrawer from "./BibliographyDrawer.svelte";
  import {
    exportTranscription,
    suggestPageTemplate,
    type ExportFormat,
  } from "@honkoku/markup";
  import { saveTranscription, saveFullImage } from "@honkoku/client-api/invoke";
  import { allPages } from "../lib";
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
    SavedPage,
  } from "@honkoku/client-api/types";
  import {
    alignColumns,
    lineNumbers,
    transcriptionColumns,
  } from "@honkoku/markup";
  import { pageLinesWithLocal as pageLines } from "../../../../packages/client-api/ocr";
  import OcrPanel from "./OcrPanel.svelte";
  import type { LocalOcrPage } from "@honkoku/client-api/types";
  import {
    createEditor,
    insertText,
    fromMarkup,
    TextSelection,
    textareaSource,
  } from "@honkoku/editor";
  import {
    fitTextScale,
    manualScaleRange,
    wrappingLines,
    type LineFit,
  } from "@honkoku/editor/fit-text";
  import Transcription from "./Transcription.svelte";
  import {
    date,
    label as entryLabel,
    notes,
    status,
    statusClass,
    user,
  } from "../lib";
  import { href, parseRoute } from "../routes";
  import VerticalEditor from "@honkoku/editor/VerticalEditor.svelte";
  import type { EditorUpdate } from "@honkoku/editor";
  import {
    editingPages,
    pageLock,
    pageDraftTextAndNotes,
    pageDraftNotes,
    pageNoteDelete,
    pageSave,
    pageDiscard,
    pageLockState,
    listPages,
  } from "@honkoku/client-api/invoke";
  import { errorMessage } from "../lib";
  import { Drafts } from "../drafts";
  import { prepareSound, completionSound } from "../sound";
  import GlyphDrawer from "./GlyphDrawer.svelte";
  import ClipSelection from "./ClipSelection.svelte";
  import NotesDrawer from "./NotesDrawer.svelte";
  import NoteOverlays from "./NoteOverlays.svelte";
  import type { Rectangle } from "./glyph-regions";
  import { graphemes } from "./glyph-regions";
  import Facsimile from "./Facsimile.svelte";
  import Thumbnail from "./Thumbnail.svelte";
  import SaveResult from "./SaveResult.svelte";
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
    leading,
    trailing,
  }: {
    leading?: Snippet;
    trailing?: Snippet;
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
  let annotationNotes = $derived(
    notes({ ...page, notes: editing ? tempNotes : page.notes }),
  );
  let noteCount = $derived(annotationNotes.filter(Boolean).length);
  let ocrOpen = $state(sessionStorage.getItem("honkoku.ocr.open") === "true");
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
            ...old,
            id: old?.id ?? "",
            type: old?.type ?? "note",
            content,
            markdown: content,
            createdBy: old?.createdBy || session!.uid,
            createdAt: old?.createdAt || now,
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
  const unsentNotes = () => notesPending(tempNotes, acknowledgedNotes);

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
  const lineNumbersKey = "honkoku.line-numbers";
  let showLineNumbers = $state(
    (() => {
      try {
        return localStorage.getItem(lineNumbersKey) !== "off";
      } catch {
        return true;
      }
    })(),
  );
  function toggleLineNumbers() {
    showLineNumbers = !showLineNumbers;
    try {
      localStorage.setItem(lineNumbersKey, showLineNumbers ? "on" : "off");
    } catch {}
  }
  /** Each matched OCR line shows the number of the transcription line it belongs to. */
  let overlayNumbers = $derived.by(() => {
    const numbers = lineNumbers(editing ? source : displayedSource);
    const byLine: Record<number, number> = {};
    alignment.forEach((line, column) => {
      const number = numbers[columns[column].sourceIndex];
      // Two columns can share one OCR line: the highlighted one names it, else the first.
      if (line === null || number === null) return;
      if (column === highlightedColumn) byLine[line] = number;
      else byLine[line] ??= number;
    });
    return byLine;
  });
  let selectedColumn = $state(-1);
  let hoveredColumn = $state<number | null>(null);
  const textScaleKey = "honkoku.text-scale";
  const clampScale = (scale: number) =>
    Math.min(
      manualScaleRange.max,
      Math.max(manualScaleRange.min, Math.round(scale * 100) / 100),
    );
  const savedScale = (() => {
    try {
      return JSON.parse(localStorage.getItem(textScaleKey) ?? "null");
    } catch {
      return null;
    }
  })();
  let automaticScale = $state(savedScale?.automatic !== false);
  let manualScale = $state(
    Number.isFinite(savedScale?.scale) ? clampScale(savedScale.scale) : 1,
  );
  let lineFits = $state<LineFit[] | null>(null);
  let autoScale = $state(1);
  /**
   * A new layout (page, pane size, reader or editor) takes its fitted size at
   * once. While typing, the size still shrinks at once so no line wraps, but
   * grows only by a visible step, so typing near the end of the longest line
   * does not keep resizing the page.
   */
  function measured(lines: LineFit[], typing: boolean) {
    lineFits = lines;
    const target = fitTextScale(lines);
    if (!typing || target < autoScale || target - autoScale >= 0.05)
      autoScale = target;
  }
  let textScale = $derived(
    !automaticScale ? manualScale : horizontal ? 1 : autoScale,
  );
  let textWraps = $derived(
    !horizontal && !!lineFits && wrappingLines(lineFits, textScale).length > 0,
  );
  function rememberTextScale() {
    try {
      localStorage.setItem(
        textScaleKey,
        JSON.stringify({ automatic: automaticScale, scale: manualScale }),
      );
    } catch {}
  }
  /** Step the text size by 10% from what is shown now, leaving automatic fitting. */
  function changeScale(delta: number) {
    const step =
      delta > 0
        ? Math.floor(textScale * 10 + 1e-8)
        : Math.ceil(textScale * 10 - 1e-8);
    manualScale = clampScale((step + delta) / 10);
    automaticScale = false;
    rememberTextScale();
  }
  function toggleAutomaticScale() {
    if (automaticScale) manualScale = clampScale(textScale);
    automaticScale = !automaticScale;
    rememberTextScale();
  }
  /**
   * Ctrl+wheel over the text steps once per mouse notch. A trackpad pinch
   * sends many small pixel deltas, which add up to a step; a pause or a change
   * of direction starts the count again.
   */
  function textWheel(element: HTMLElement) {
    let pending = 0;
    let idle: ReturnType<typeof setTimeout> | undefined;
    const wheel = (event: WheelEvent) => {
      if (!event.ctrlKey || !event.deltaY) return;
      if ((event.target as Element).closest(".ocr-drawer")) return;
      event.preventDefault();
      clearTimeout(idle);
      idle = setTimeout(() => (pending = 0), 250);
      if (event.deltaMode !== WheelEvent.DOM_DELTA_PIXEL) {
        changeScale(event.deltaY < 0 ? 1 : -1);
        return;
      }
      if (Math.sign(pending) !== Math.sign(event.deltaY)) pending = 0;
      pending += event.deltaY;
      if (Math.abs(pending) < 100) return;
      changeScale(pending < 0 ? 1 : -1);
      pending = 0;
    };
    element.addEventListener("wheel", wheel, { passive: false });
    return {
      destroy() {
        clearTimeout(idle);
        element.removeEventListener("wheel", wheel);
      },
    };
  }
  let hoveredLine = $state<number | null>(null);
  let highlightedColumn = $derived(
    hoveredColumn !== null
      ? hoveredColumn
      : hoveredLine !== null
        ? alignment.indexOf(hoveredLine)
        : selectedColumn,
  );
  let selectedLine = $derived(
    hoveredLine ??
      (hoveredColumn !== null
        ? (alignment[hoveredColumn] ?? null)
        : selectedColumn >= 0
          ? (alignment[selectedColumn] ?? null)
          : null),
  );
  // A structural text change (draft sync, history restore) shifts column
  // indices, so a view-mode selection is dropped; editing keeps its own.
  $effect(() => {
    displayedSource;
    if (!untrack(() => editing)) {
      selectedColumn = -1;
      hoveredColumn = null;
      hoveredLine = null;
    }
  });
  let editor = $state<VerticalEditor>();
  let notationMode = $state(false);
  let transcription = $state<Transcription>();
  function columnChange(index: number) {
    selectedColumn = index;
    hoveredColumn = null;
    hoveredLine = null;
  }
  function columnHover(index: number | null) {
    hoveredColumn = index !== null && index >= 0 ? index : null;
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
  let menuOpen = $state(false),
    menuRoot = $state<HTMLDivElement>(),
    menuToggle = $state<HTMLButtonElement>();
  $effect(() => {
    if (!menuOpen) return;
    const click = (event: MouseEvent) => {
      if (menuRoot && !menuRoot.contains(event.target as Node)) menuOpen = false;
    };
    const key = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      const inside = menuRoot?.contains(document.activeElement);
      if (!inside && document.activeElement !== document.body) return;
      menuOpen = false;
      if (inside) menuToggle?.focus();
    };
    document.addEventListener("click", click);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("click", click);
      document.removeEventListener("keydown", key);
    };
  });
  let historyOpen = $state(false),
    bibliographyOpen = $state(false),
    exportOpen = $state(false);
  $effect(() => {
    if (!menuOpen) exportOpen = false;
  });
  let exportScope = $state("page"),
    exportFormat = $state<ExportFormat>("txt"),
    exportBusy = $state(false);
  $effect(() => {
    index;
    currentEntryId;
    historyOpen = false;
    bibliographyOpen = false;
  });
  function restoreHistory(text: string) {
    if (busy || verifying || composing) return;
    act(async () => {
      if (!editing) {
        begin(await pageLock(entry.id, index, false));
        await tick();
      }
      editorInstance?.setSource(text);
      source = text;
      saveState = "未保存の変更";
      remember();
      queue?.request(draftPayload());
    });
  }
  async function download(image = false) {
    if (exportBusy) return;
    const selectedEntry = entry,
      selectedIndex = index,
      scope = exportScope,
      format = exportFormat;
    const currentText = editing ? source : displayedSource;
    exportBusy = true;
    notice = "";
    try {
      let saved: boolean;
      if (image) saved = await saveFullImage(selectedEntry.id, selectedIndex);
      else {
        const values =
          scope === "entry"
            ? allPages(selectedEntry, await listPages(selectedEntry.id))
            : [{ index: selectedIndex, text: currentText }];
        const name = `${entryLabel(selectedEntry.label) || selectedEntry.id}${scope === "page" ? `-${selectedIndex + 1}` : ""}`;
        saved = await saveTranscription(
          name,
          format,
          exportTranscription(values, format),
        );
      }
      if (saved) {
        notice = "保存しました。";
        menuOpen = false;
      }
    } catch (error) {
      notice = errorMessage(error);
    } finally {
      exportBusy = false;
    }
  }
  let glyphOpen = $state(false),
    glyphCharacter = $state("");
  let recognizing = $state(false);
  let clipping = $state(false);
  let notesOpen = $state(false),
    annotationMode = $state(false),
    showAnnotations = $state(true);
  let highlightedNote = $state<number | null>(null);
  let notesDrawer = $state<NotesDrawer>(),
    noteOverlays = $state<NoteOverlays>();
  async function savePageNote(draft: PageNote, position?: number) {
    if (!editing || busy || verifying || composing) return;
    busy = true;
    try {
      await queue?.flush();
      const now = new Date().toISOString(),
        old = position === undefined ? undefined : pageNotes[position];
      const note: PageNote = {
        id: old?.id ?? "",
        content: draft.content,
        markdown: draft.content,
        type: draft.type ?? "note",
        createdBy: old?.createdBy || session!.uid,
        createdAt: old?.createdAt || now,
        updatedAt: now,
      };
      if (draft.image) note.image = draft.image;
      if (draft.xywh) note.xywh = draft.xywh;
      const updated = [...tempNotes];
      updated[position ?? updated.length] = note as unknown as JsonValue;
      const fresh = await pageDraftNotes(
        entry.id,
        index,
        JSON.parse(JSON.stringify(updated)) as (JsonValue | null)[],
      );
      tempNotes = fresh.tempNotes ?? updated;
      acknowledgedNotes = JSON.parse(JSON.stringify(tempNotes));
      onpage(fresh);
      remember();
      saveState = "下書き保存";
    } finally {
      busy = false;
    }
  }
  async function deletePageNote(position: number) {
    if (!editing || busy || verifying || composing) return;
    busy = true;
    try {
      await queue?.flush();
      const fresh = await pageNoteDelete(entry.id, index, position);
      if (fresh.tempNotes) {
        tempNotes = fresh.tempNotes;
        acknowledgedNotes = JSON.parse(JSON.stringify(fresh.tempNotes));
      }
      onpage(fresh);
      remember();
      saveState = "下書き保存";
    } finally {
      busy = false;
    }
  }
  async function regionNote(xywh: Rectangle) {
    const info = canvases[index]?.infoJsonUrl;
    if (!info || !editing) return;
    const url = new URL(info, location.href);
    const upstream = url.searchParams.get("url") ?? info;
    const image = `${upstream.replace(/\/info\.json(?:\?.*)?$/, "")}/${xywh.join(",")}/300,/0/default.jpg`;
    annotationMode = false;
    notesOpen = true;
    glyphOpen = false;
    await tick();
    notesDrawer?.create({ xywh, image });
  }
  $effect(() => {
    if (!editing) annotationMode = false;
  });
  let syncMode = $state(false);
  let approvalChange = $state(false);
  let approved = $derived(
    !!session && !!page.approvedBy?.includes(session.uid),
  );
  let canApprove = $derived(
    page.prevStatus === "completed" &&
      page.editedBy !== session?.uid &&
      (page.approvedBy?.length ?? 0) < 2,
  );
  let reviewName = $state("名前を確認中");
  $effect(() => {
    const uid = sessionUid;
    try {
      syncMode = localStorage.getItem(`honkoku.sync.${uid}`) === "true";
    } catch {
      syncMode = false;
    }
  });
  function toggleSync() {
    syncMode = !syncMode;
    try {
      localStorage.setItem(`honkoku.sync.${sessionUid}`, String(syncMode));
    } catch {}
  }
  $effect(() => {
    const uid =
      page.requestReview && page.editedBy !== sessionUid
        ? page.editedBy
        : undefined;
    let cancelled = false;
    reviewName = uid ? "名前を確認中" : "名前不明";
    if (uid)
      void user(uid)
        .then((u) => {
          if (!cancelled) reviewName = u.displayName;
        })
        .catch(() => {
          if (!cancelled) reviewName = "名前不明";
        });
    return () => {
      cancelled = true;
    };
  });
  $effect(() => {
    const entryId = currentEntryId,
      pageIndex = index,
      uid = sessionUid;
    const watching = !editing && !busy && !verifying && !pagesPending;
    if (!watching) return;
    let cancelled = false,
      pending = false;
    const timer = setInterval(async () => {
      if (pending) return;
      pending = true;
      try {
        const state = await pageLockState(entryId, pageIndex);
        if (!cancelled && uid === sessionUid && state.page) onpage(state.page);
      } catch {
        /* The last received text remains visible while reconnecting. */
      } finally {
        pending = false;
      }
    }, 3000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  });
  function insertCandidate(character: string) {
    if (!editing || busy || composing || verifying || !editorInstance) return;
    const raw = document.querySelector<HTMLTextAreaElement>(
      ".editor-raw-textarea",
    );
    if (raw) {
      const position = raw.selectionStart;
      source = textareaSource(
        source,
        raw.value.slice(0, position) +
          character +
          raw.value.slice(raw.selectionEnd),
      );
      saveState = "未保存の変更";
      remember();
      queue?.request(draftPayload());
      void tick().then(() => {
        raw.focus();
        raw.setSelectionRange(
          position + character.length,
          position + character.length,
        );
      });
    } else {
      editorInstance.run(insertText(character));
    }
  }
  let lineCharacterCounts = $derived(
    Object.fromEntries(
      alignment.flatMap((line, i) =>
        line === null ? [] : [[line, graphemes(columns[i].text).length]],
      ),
    ),
  );

  let glyphTimer: ReturnType<typeof setTimeout>;
  function glyphChange(character: string, open: boolean) {
    clearTimeout(glyphTimer);
    if (open) {
      glyphCharacter = character;
      glyphOpen = true;
      clipping = false;
      recognizing = false;
      historyOpen = false;
      bibliographyOpen = false;
    } else if (glyphOpen)
      glyphTimer = setTimeout(() => (glyphCharacter = character), 300);
  }
  $effect(() => {
    index;
    currentEntryId;
    sessionUid;
    clipping = false;
    recognizing = false;
    annotationMode = false;
    notesOpen = false;
    highlightedNote = null;
    showAnnotations = true;
    glyphOpen = false;
    glyphCharacter = "";
    clearTimeout(glyphTimer);
  });

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
  let result = $state<SavedPage>();
  let recovered = $state("");
  let queue: Drafts | undefined;
  let operation: Promise<void> | undefined;
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
  function localDraft(): LocalDraft | undefined {
    try {
      return (
        JSON.parse(localStorage.getItem(storageKey()) ?? "null") ?? undefined
      );
    } catch {
      return;
    }
  }
  let acknowledged = "";
  let acknowledgedNotes: (JsonValue | null)[] = [];
  let draftUpdatedAt: string | null | undefined;
  function remember() {
    try {
      localStorage.setItem(
        storageKey(),
        JSON.stringify({
          source,
          notes: tempNotes,
          draft: acknowledged,
          acknowledgedNotes,
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
    result = undefined;
    approvalChange = false;
    lastOptions = savedOptions();
    onpage(locked);
    const local = restore ? localDraft() : undefined;
    acknowledged = locked.tempText ?? "";
    draftUpdatedAt = locked.updatedAt;
    const restored = restoreDraft(locked, local);
    source = restored.source;
    if (!source.trim() && !restored.unsavedLocal)
      source =
        suggestPageTemplate(
          pages.filter((p) => p.id !== locked.id).map((p) => p.text),
        ) ?? source;
    tempNotes = restored.notes;
    acknowledgedNotes = restored.acknowledgedNotes;
    editing = true;
    hoveredColumn = null;
    hoveredLine = null;
    recovered = "";
    saveState = locked.tempTextChanged
      ? `下書き保存${new Date(locked.updatedAt ?? Date.now()).toLocaleTimeString("ja-JP", { hour: "2-digit", minute: "2-digit" })}`
      : "未保存の変更";
    share = locked.share ?? false;
    completed = locked.prevStatus === "completed";
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
        acknowledgedNotes = pendingNotes;
        draftUpdatedAt = draft.updatedAt;
        onpage(draft);
        remember();
        saveState =
          source === text && !unsentNotes()
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
    if (source !== acknowledged || unsentNotes()) queue.request(draftPayload());
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
    act(async () => begin(await pageLock(entry.id, index, syncMode)));
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
      if (unsentNotes())
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
      if (saved.page.status === "completed") completionSound();
      result = saved;
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
      selectedColumn = -1;
      hoveredColumn = null;
      hoveredLine = null;
      localOcr = null;
      closeNotes();
      source = "";
      recovered = "";
      saveState = "";
      notice = "";
      savePopover = false;
      discardPopover = false;
      result = undefined;
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
  let appliedDeepLink = "";
  $effect(() => {
    const requested = column;
    const pageIndex = index;
    const count = columns.length;
    const pending = pagesPending;
    verifying;
    // A route without a column re-arms the latch, so returning to the same
    // deep link (Back, another search hit) applies it again.
    if (requested === undefined) {
      appliedDeepLink = "";
      return;
    }
    if (pending) return;
    const key = `${pageIndex}:${requested}`;
    // Once per page and column: later source syncs must not yank the caret
    // or the selection back to the deep-linked column.
    if (appliedDeepLink === key) return;
    void tick().then(() => {
      if (pageIndex !== index || requested !== column || requested >= count)
        return;
      appliedDeepLink = key;
      columnChange(requested);
      if (editing) editor?.focusColumn(requested);
      else transcription?.focusColumn(requested);
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
    const values = [...pageNotes, ...annotationNotes];
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
            'input,textarea,select,[contenteditable="true"],.workbench-editor,.ocr-drawer,.notes-drawer,.note-popover,.save-popover,.workbench-menu',
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
  {#if notice}<div class="workbench-notification caption" role="status">
      <span class="edit-notice">{notice}</span>
    </div>{/if}
  {#if result}<SaveResult
      saved={result}
      entryLabel={entryLabel(entry.label)}
      onclose={() => (result = undefined)}
    />{/if}
  <div class="workbench-toolbar">
    <div class="toolbar-leading">
      {#if leading}{@render leading()}{/if}
      <div class="editing-pages">
        {#if otherEdits.length}<details>
            <summary
              >編集中のコマ<span class="count">{otherEdits.length}</span
              ></summary
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
    </div>
    <div class="page-position">
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
      <span class="edit-status caption" aria-live="polite">{saveState}</span>
    </div>
    <div class="toolbar-actions">
      {#if page.requestReview && page.editedBy !== session?.uid}<span
          class="caption review-chip">添削希望・{reviewName}</span
        >{/if}
      {#if page.approvedBy?.length}<span class="caption"
          >✓{page.approvedBy.length}／2</span
        >{/if}
      {#if page.status === "editing" && page.syncMode}<span class="caption"
          >共有中</span
        >{/if}
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
                  isApproval: approvalChange ? !approved : undefined,
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
                  onchange={(event) => {
                    if (event.currentTarget.checked) share = true;
                  }}
                />添削希望</label
              >
              <label
                ><input
                  type="checkbox"
                  bind:checked={approvalChange}
                  disabled={!approved && !canApprove}
                />{approved
                  ? "チェックを取り消す"
                  : "チェック済みにする"}</label
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
      {:else if session && page.status !== "editing"}<div
          class="edit-start-control"
          role="group"
          aria-label="編集の開始"
        >
          <button
            class="primary"
            disabled={busy || pagesPending || verifying}
            onclick={start}>編集開始</button
          >
          <button
            class="caption"
            aria-pressed={syncMode}
            disabled={busy || verifying}
            onclick={toggleSync}>共有モード</button
          >
        </div>{/if}

      <button
        aria-pressed={notesOpen}
        aria-controls="notes-drawer"
        onclick={() => {
          notesOpen = !notesOpen;
          glyphOpen = false;
          annotationMode = false;
          clipping = false;
          recognizing = false;
        }}
        >注釈{#if noteCount}<span class="count">{noteCount}</span>{/if}</button
      >
      <button
        aria-pressed={ocrOpen}
        aria-controls="ocr-drawer"
        onclick={() => {
          ocrOpen = !ocrOpen;
          sessionStorage.setItem("honkoku.ocr.open", String(ocrOpen));
        }}>OCR</button
      >
      <button
        aria-pressed={historyOpen}
        aria-controls="history-side"
        onclick={() => {
          historyOpen = !historyOpen;
          bibliographyOpen = false;
          glyphOpen = false;
          clipping = false;
          recognizing = false;
        }}>履歴</button
      >
      <div class="workbench-menu" bind:this={menuRoot}>
        <button
          bind:this={menuToggle}
          aria-label="表示設定"
          aria-expanded={menuOpen}
          class:has-notice={textWraps}
          title={textWraps ? "長すぎる行は折り返しています" : undefined}
          onclick={() => (menuOpen = !menuOpen)}>⋯</button
        >
        {#if menuOpen}<div class="menu-options">
            <button
              aria-pressed={showAnnotations}
              onclick={() => {
                showAnnotations = !showAnnotations;
                menuOpen = false;
              }}>注釈表示</button
            >
            <button
              aria-expanded={exportOpen}
              onclick={() => (exportOpen = !exportOpen)}
              >翻刻文をダウンロード</button
            >
            {#if exportOpen}<div class="export-submenu">
                <label
                  >範囲<select bind:value={exportScope}
                    ><option value="page">このコマ</option><option value="entry"
                      >この資料</option
                    ></select
                  ></label
                >
                <label
                  >形式<select bind:value={exportFormat}
                    ><option value="txt">テキスト（.txt）</option><option
                      value="xml">TEI XML（.xml）</option
                    ><option value="tex">LaTeX（.tex）</option></select
                  ></label
                >
                <button disabled={exportBusy} onclick={() => download()}
                  >{exportBusy ? "保存の準備中…" : "保存先を選ぶ"}</button
                >
              </div>{/if}
            <button
              disabled={exportBusy || !canvases[index]}
              onclick={() => download(true)}>フルサイズ画像を保存</button
            >
            <button
              onclick={() => {
                bibliographyOpen = true;
                historyOpen = false;
                glyphOpen = false;
                clipping = false;
                recognizing = false;
                menuOpen = false;
              }}>書誌情報</button
            >
            <a href="#/help/markup">特殊記法の解説</a>
            <a href={href({ projectId: entry.projectId, guidelines: true })}
              >翻刻ガイドライン</a
            >
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
            <button
              aria-pressed={showLineNumbers}
              onclick={() => {
                toggleLineNumbers();
                menuOpen = false;
              }}>行番号</button
            >
            <div
              class="text-scale-controls"
              role="group"
              aria-label="本文の大きさ"
              title="Ctrl+ホイールでも変えられます"
            >
              <span>本文</span><button
                aria-label="本文を縮小"
                disabled={textScale <= manualScaleRange.min}
                onclick={() => changeScale(-1)}>−</button
              ><span class="numeric">{Math.round(textScale * 100)}%</span><button
                aria-label="本文を拡大"
                disabled={textScale >= manualScaleRange.max}
                onclick={() => changeScale(1)}>＋</button
              ><button aria-pressed={automaticScale} onclick={toggleAutomaticScale}
                >自動</button
              >
            </div>
            {#if textWraps}<p class="caption muted">長すぎる行は折り返しています</p>{/if}
          </div>{/if}
      </div>
      {#if trailing}{@render trailing()}{/if}
    </div>
  </div>
  {#if recovered}<details class="edit-recovery">
      <summary>端末に残っている本文</summary><textarea
        readonly
        value={recovered}
        aria-label="端末に残っている本文"></textarea>
    </details>{/if}
  <div
    class="workbench-panes"
    class:swapped
    class:line-numbers={showLineNumbers}
  >
    <section
      class="panel transcription-panel"
      style:--text-scale={textScale}
      use:textWheel
      use:references
    >
      {#if pagesPending}<Skeleton
          shape="columns"
          count={8}
        />{:else if editing}<div
          class="workbench-editor"
          inert={busy || verifying}
        >
          {#if notationMode}<div class="pane-toolbar"><span class="caption muted">記法で編集中</span></div>{/if}
          <VerticalEditor
            onmeasure={measured}
            onnotationchange={(active) => (notationMode = active)}
            {horizontal}
            accountId={session?.uid}
            otherPageTexts={pages.filter((p) => p.id !== page.id).map((p) => p.text)}
            pageId={page.id}
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
            onmeasure={measured}
            bind:this={transcription}
            source={displayedSource}
            {horizontal}
            bind:half
            selected={selectedColumn}
            {highlightedColumn}
            oncolumnchange={columnChange}
            onhover={columnHover}
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
      modes={[
        {
          label: "移動",
          active: !clipping && !annotationMode && !recognizing,
          select: () => {
            clipping = false;
            recognizing = false;
            annotationMode = false;
          },
        },
        {
          label: "切り抜き",
          active: clipping,
          disabled: !session || !canvases[index]?.infoJsonUrl,
          select: () => {
            recognizing = false;
            clipping = !clipping;
            annotationMode = false;
            notesOpen = false;
            glyphOpen = false;
          },
        },
        {
          label: "注釈",
          active: annotationMode,
          disabled:
            !editing || busy || verifying || !canvases[index]?.infoJsonUrl,
          select: () => {
            annotationMode = !annotationMode;
            clipping = false;
            recognizing = false;
            notesOpen = false;
            glyphOpen = false;
          },
        },
        {
          label: "認識",
          active: recognizing,
          disabled: !canvases[index]?.infoJsonUrl,
          select: () => {
            clipping = false;
            annotationMode = false;
            recognizing = true;
            notesOpen = false;
            glyphOpen = false;
            historyOpen = false;
            bibliographyOpen = false;
          },
        },
      ]}
      oninsert={editing && !busy && !composing && !verifying
        ? insertCandidate
        : undefined}
      {lineCharacterCounts}
      canvas={canvases[index]}
      pending={canvasesRegion.pending && !canvases[index]}
      region={canvasesRegion}
      pageNumber={index + 1}
      bind:half
      {lineModel}
      highlightedLine={selectedLine}
      lineLabels={overlayNumbers}
      onlineselect={selectLine}
      onlinehover={(line) => (hoveredLine = line)}
    >
      {#snippet children(viewer)}
        <NoteOverlays
          bind:this={noteOverlays}
          {viewer}
          notes={annotationNotes}
          visible={showAnnotations}
          highlighted={highlightedNote}
          selecting={clipping || annotationMode || recognizing}
        />
        {#if notesOpen}{#key page.id}<NotesDrawer
              bind:this={notesDrawer}
              notes={annotationNotes}
              {editing}
              disabled={busy || composing || verifying}
              {authors}
              lockName={page.status === "editing" && !editing
                ? lockName
                : undefined}
              onclose={() => {
                notesOpen = false;
                highlightedNote = null;
              }}
              onsave={savePageNote}
              ondelete={deletePageNote}
              onhover={(position) => (highlightedNote = position)}
              onpan={(position) => noteOverlays?.pan(position)}
            />{/key}{/if}
        {#if annotationMode}<ClipSelection
            {viewer}
            canvas={canvases[index]}
            entryId={entry.id}
            {index}
            onclose={() => (annotationMode = false)}
            onsaved={() => {}}
            onregion={regionNote}
          />{/if}
        {#if historyOpen}<HistoryDrawer
            entryId={entry.id}
            {index}
            current={editing ? source : displayedSource}
            {editing}
            disabled={!session ||
              busy ||
              verifying ||
              composing ||
              pagesPending ||
              (!editing && page.status === "editing")}
            onrestore={restoreHistory}
            onclose={() => (historyOpen = false)}
          />{/if}
        {#if bibliographyOpen}<BibliographyDrawer
            entryId={entry.id}
            manifestUrl={entry.manifestUrl}
            onclose={() => (bibliographyOpen = false)}
          />{/if}
        {#if glyphOpen}<GlyphDrawer
            character={glyphCharacter}
            onclose={() => (glyphOpen = false)}
          />{/if}
        {#if clipping}<ClipSelection
            {viewer}
            canvas={canvases[index]}
            entryId={entry.id}
            {index}
            onclose={() => (clipping = false)}
            onsaved={() => {
              clipping = false;
              recognizing = false;
              notice = "クリップを保存しました。";
            }}
          />{/if}
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

{#if noteIndex !== null}
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
    {#each pageNotes as n, i}{#if n && noteIndex === i}
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
