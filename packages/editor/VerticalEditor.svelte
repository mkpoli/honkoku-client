<script lang="ts">
  import { onMount, untrack, tick } from "svelte";
  import {
    historyKey,
    createEditor,
    textareaSource,
    wrapSelection,
    insertText,
    insertSource,
    undo,
    redo,
    type EditorUpdate,
  } from "./index";
  import { palette } from "./palette";
  import { transcriptionColumns } from "@honkoku/markup";
  import "prosemirror-view/style/prosemirror.css";
  import "./style.css";
  let {
    source = $bindable(""),
    onupdate,
    onready,
    oncolumnchange,
    highlightedColumn = -1,
    onnote,
  }: {
    source: string;
    onnote?: (content: string | null, index?: number) => number;
    oncolumnchange?: (index: number) => void;
    highlightedColumn?: number;
    onupdate?: (update: EditorUpdate) => void;
    onready?: (editor: ReturnType<typeof createEditor>) => void;
  } = $props();
  let host: HTMLDivElement;
  let editor = $state<ReturnType<typeof createEditor>>();
  let raw = $state(false),
    composing = $state(false);
  let rawInput = $state<HTMLTextAreaElement>(null!);
  let rawRange = { from: 0, to: 0 };
  type Construct = "振り仮名" | "割書" | "見せ消ち" | "注記";
  const commands: Construct[] = ["振り仮名", "割書", "見せ消ち", "注記"];
  let status = $state("");
  let note = $state<{index: number; content: string; x: number; y: number}>();
  let noteInput = $state<HTMLTextAreaElement>(null!);
  let openCategory = $state<string | null>(null);
  let localNoteCount = 0;
  function captureRaw() {
    if (rawInput)
      rawRange = { from: rawInput.selectionStart, to: rawInput.selectionEnd };
  }
  function selectedText() {
    if (raw) {
      captureRaw();
      return rawInput.value.slice(rawRange.from, rawRange.to);
    }
    const selection = editor!.view.state.selection;
    return editor!.view.state.doc.textBetween(selection.from, selection.to);
  }
  async function insertRaw(text: string, focus = true, offset = text.length) {
    const value = rawInput.value;
    const position = rawRange.from + offset;
    editor?.setSource(
      textareaSource(
        source,
        value.slice(0, rawRange.from) + text + value.slice(rawRange.to),
      ),
    );
    await tick();
    if (focus) rawInput.focus();
    rawInput.setSelectionRange(position, position);
    captureRaw();
  }
  function insertGlyph(text: string, structured: boolean, focus = true) {
    if (raw) {
      captureRaw();
      void insertRaw(text, focus);
    } else
      editor?.run(structured ? insertSource(text) : insertText(text), focus);
  }
  async function insertConstruct(title: Construct) {
    if (composing) return;
    status = "";
    openCategory = null;
    const selection = selectedText();
    if (title === "注記") {
      if (!raw && !insertSource("＃1")(editor!.view.state)) {
        status = "ここには注記を入れられません。";
        return;
      }
      const index = onnote?.("") ?? localNoteCount++;
      if (raw) await insertRaw(`＃${index + 1}`);
      else editor?.run(insertSource(`＃${index + 1}`));
      await tick();
      editNote(index, "");
      return;
    }
    if (raw) {
      const base = title === "割書" ? "" : selection;
      await insertRaw(`《${title}：${base}｜》`, true, title.length + 2);
    } else {
      const kind = ({振り仮名: "ruby", 割書: "warigaki", 見せ消ち: "misekechi"} as const)[title];
      if (!editor?.run(wrapSelection(kind)))
        status = "ここにはこの記号を入れられません。";
    }
  }
  export function editNote(index: number, content: string) {
    if (composing) return;
    const anchor = host.closest(".editor-workspace")?.querySelector<HTMLElement>(`[data-note="${index + 1}"]`) ?? rawInput ?? host;
    const rect = anchor.getBoundingClientRect();
    note = {index, content, x: Math.max(8, Math.min(rect.left, innerWidth - 336)),
      y: Math.max(8, Math.min(rect.bottom + 4, innerHeight - 250))};
    void tick().then(() => noteInput?.focus());
  }
  function closeNote() {
    note = undefined;
    if (raw) rawInput?.focus();
    else editor?.view.focus();
  }
  function constructKey(event: KeyboardEvent) {
    if (event.isComposing || composing || event.altKey || !(event.ctrlKey || event.metaKey)) return;
    const title = ({r: "振り仮名", w: "割書", m: "見せ消ち", n: "注記"} as const)[event.key.toLowerCase() as "r"];
    if (title) {
      event.preventDefault();
      event.stopPropagation();
      void insertConstruct(title);
    }
  }
  function shortcuts(element: HTMLElement) {
    element.addEventListener("keydown", constructKey, true);
    return {destroy: () => element.removeEventListener("keydown", constructKey, true)};
  }
  function rawComposition(value: boolean) {
    composing = value;
    onupdate?.({
      source,
      serialized: source,
      patches: [],
      column: editor?.view.state.selection.$head.index(0) ?? 0,
      composing,
    });
  }
  function paletteKey(event: KeyboardEvent, label: string) {
    const group = event.currentTarget as HTMLElement;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      group.querySelector<HTMLButtonElement>(".palette-chip")?.focus();
      openCategory = null;
    } else if (
      ["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown"].includes(event.key)
    ) {
      event.preventDefault();
      openCategory = label;
      void tick().then(() => {
        const buttons = [
          ...group.querySelectorAll<HTMLButtonElement>(
            ".palette-glyphs button",
          ),
        ];
        const current = buttons.indexOf(
          document.activeElement as HTMLButtonElement,
        );
        const delta = ["ArrowLeft", "ArrowUp"].includes(event.key) ? -1 : 1;
        buttons[(current + delta + buttons.length) % buttons.length]?.focus();
      });
    }
  }
  let canUndo = $state(false),
    canRedo = $state(false);
  let lastNotifiedColumn = -1;
  function notifyColumn(index: number) {
    if (index === lastNotifiedColumn) return;
    lastNotifiedColumn = index;
    oncolumnchange?.(index);
  }
  function rawSelection(element: HTMLTextAreaElement) {
    const sourceIndex =
      element.value.slice(0, element.selectionStart).split("\n").length - 1;
    notifyColumn(
      transcriptionColumns(source).findIndex(
        (column) => column.sourceIndex === sourceIndex,
      ),
    );
  }
  onMount(() => {
    let ready = false;
    const initialColumn = untrack(() => highlightedColumn);
    const instance = createEditor(
      host,
      untrack(() => source),
      (update) => {
        source = update.source;
        composing = update.composing;
        if (editor) {
          canUndo = undo(editor.view.state);
          canRedo = redo(editor.view.state);
        }
        onupdate?.(update);
      },
      (index) => {
        if (ready) notifyColumn(index);
      },
      (message) => (status = message),
    );
    editor = instance;
    ready = true;
    if (initialColumn >= 0) instance.focusColumn(initialColumn);
    else
      notifyColumn(
        transcriptionColumns(source).findIndex(
          (column) =>
            column.sourceIndex === instance.view.state.selection.$head.index(0),
        ),
      );
    onready?.(instance);
    return () => instance.destroy();
  });
  $effect(() => {
    editor?.setSource(source);
  });
  export function focusColumn(index: number) {
    raw = false;
    requestAnimationFrame(() => editor?.focusColumn(index));
  }
  $effect(() => {
    editor?.setHighlightedColumn(highlightedColumn);
  });
</script>

<div class="editor-workspace" use:shortcuts>
  <div class="editor-toolbar" role="toolbar" aria-label="翻刻の編集">
    {#each commands as title}
      <button
        disabled={composing}
        onmousedown={(event) => event.preventDefault()}
        onclick={() => insertConstruct(title)}>{title}</button
      >
    {/each}
    <span class="editor-toolbar-spacer"></span>
    <button disabled={composing || !canUndo} onclick={() => editor?.run(undo)}
      >元に戻す</button
    >
    <button disabled={composing || !canRedo} onclick={() => editor?.run(redo)}
      >やり直す</button
    >
    <button
      disabled={composing}
      aria-pressed={raw}
      onclick={() => {
        editor?.leaveShell();
        raw = !raw;
        if (!raw) requestAnimationFrame(() => editor?.view.focus());
      }}>原文表示</button
    >
  </div>
  {#if status}<p class="editor-status" role="status">{status}</p>{/if}
  <section class="editor-palette" aria-label="特殊記号">
    {#each palette as group}
      <div
        class="palette-category"
        role="toolbar"
        tabindex="-1"
        aria-label={group.label}
        onmouseenter={() => (openCategory = group.label)}
        onmouseleave={() => (openCategory = null)}
        onfocusin={() => (openCategory = group.label)}
        onfocusout={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget as Node))
            openCategory = null;
        }}
        onkeydown={(event) => paletteKey(event, group.label)}
      >
        <button
          class="palette-chip"
          disabled={composing}
          aria-expanded={openCategory === group.label}
          onclick={() => (openCategory = group.label)}>{group.label}</button
        >
        {#if openCategory === group.label}<div
            class="palette-glyphs"
            role="group"
            aria-label={`${group.label}の文字`}
          >
            {#each group.characters as character}<button
                disabled={composing}
                aria-label={`${group.label}${character}`}
                onmousedown={(event) => event.preventDefault()}
                onclick={(event) => {
                  insertGlyph(
                    character,
                    group.label === "欠字" || group.label === "返り点",
                    event.detail !== 0,
                  );
                  openCategory = group.label;
                  if (event.detail === 0) event.currentTarget.focus();
                }}>{character}</button
              >{/each}
          </div>{/if}
      </div>
    {/each}
  </section>
  <div class="editor-body" class:editor-raw-mode={raw}>
    <div class="transcription editor-scroll" hidden={raw}>
      <div bind:this={host} class="editor-mount"></div>
    </div>
    {#if raw}<textarea
        bind:this={rawInput}
        class="editor-raw-textarea"
        aria-label="原文を編集"
        onselect={(event) => {
          captureRaw();
          rawSelection(event.currentTarget);
        }}
        oncompositionstart={() => rawComposition(true)}
        oncompositionend={() => rawComposition(false)}
        onkeydown={(event) => historyKey(event, editor)}
        value={source}
        oninput={(event) =>
          editor?.setSource(textareaSource(source, event.currentTarget.value))}
      ></textarea>{/if}
  </div>
</div>
{#if note}
  <div class="note-popover editor-note-popover" role="dialog" aria-label="注記" tabindex="-1"
    style:left={`${note.x}px`} style:top={`${note.y}px`}
    onkeydown={(event) => { if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); closeNote(); } }}>
    <strong>＃{note.index + 1}</strong>
    <label>注記の内容<textarea bind:this={noteInput} aria-label="注記の内容" value={note.content}
      oninput={(event) => { if (note) { note.content = event.currentTarget.value; onnote?.(note.content, note.index); } }}></textarea></label>
    <div class="dialog-actions">
      <button onclick={closeNote}>更新</button>
      <button onclick={() => { if (note) onnote?.(null, note.index); closeNote(); }}>削除</button>
      <button onclick={closeNote} aria-label="注記を閉じる">閉じる</button>
    </div>
  </div>
{/if}
