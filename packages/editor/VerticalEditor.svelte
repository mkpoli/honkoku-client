<script lang="ts">
  import { onMount, untrack } from "svelte";
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
  import "prosemirror-view/style/prosemirror.css";
  import "./style.css";
  let {
    source = $bindable(""),
    onupdate,
    onready,
  }: {
    source: string;
    onupdate?: (update: EditorUpdate) => void;
    onready?: (editor: ReturnType<typeof createEditor>) => void;
  } = $props();
  let host: HTMLDivElement;
  let editor = $state<ReturnType<typeof createEditor>>();
  let raw = $state(false),
    composing = $state(false),
    reading = $state(""),
    popover = $state(false);
  let readingInput = $state<HTMLInputElement>();
  let canWrap = $state(false),
    canUndo = $state(false),
    canRedo = $state(false);
  onMount(() => {
    const instance = createEditor(
      host,
      untrack(() => source),
      (update) => {
        source = update.source;
        composing = update.composing;
        if (editor) {
          canWrap = wrapSelection("ruby")(editor.view.state);
          canUndo = undo(editor.view.state);
          canRedo = redo(editor.view.state);
        }
        onupdate?.(update);
      },
    );
    editor = instance;
    canWrap = wrapSelection("ruby")(instance.view.state);
    onready?.(instance);
    return () => instance.destroy();
  });
  $effect(() => {
    editor?.setSource(source);
  });
  function submitReading(event: SubmitEvent) {
    event.preventDefault();
    editor?.run(wrapSelection("ruby", reading));
    popover = false;
    reading = "";
  }
</script>

<div class="editor-workspace">
  <div class="editor-toolbar" role="toolbar" aria-label="翻刻の編集">
    <button
      disabled={composing || raw || !canWrap}
      onclick={() => {
        popover = !popover;
        if (popover) requestAnimationFrame(() => readingInput?.focus());
      }}>振り仮名</button
    >
    <button
      disabled={composing || raw || !canWrap}
      onclick={() => editor?.run(wrapSelection("warigaki"))}>割書</button
    >
    <button
      disabled={composing || raw || !canWrap}
      onclick={() => editor?.run(wrapSelection("misekechi"))}>見せ消ち</button
    >
    <button
      disabled={composing || raw}
      onclick={() => editor?.run(insertSource("＃1"))}>注記</button
    >
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
        raw = !raw;
        if (!raw) requestAnimationFrame(() => editor?.view.focus());
      }}>原文表示</button
    >
    {#if popover}
      <form class="editor-reading-popover" onsubmit={submitReading}>
        <label
          >読み<input
            bind:this={readingInput}
            bind:value={reading}
            onkeydown={(event) => {
              if (event.key === "Escape") {
                popover = false;
                editor?.view.focus();
              }
            }}
          /></label
        >
        <button class="primary" type="submit" disabled={composing}>挿入</button>
        <button
          type="button"
          onclick={() => {
            popover = false;
            editor?.view.focus();
          }}>閉じる</button
        >
      </form>
    {/if}
  </div>
  <div class="editor-body" class:editor-raw-mode={raw}>
    <div class="transcription editor-scroll" hidden={raw}>
      <div bind:this={host} class="editor-mount"></div>
    </div>
    {#if raw}<textarea
        class="editor-raw-textarea"
        aria-label="原文を編集"
        onkeydown={(event) => historyKey(event, editor)}
        value={source}
        oninput={(event) =>
          editor?.setSource(textareaSource(source, event.currentTarget.value))}
      ></textarea>{/if}
  </div>
  <section class="editor-palette" aria-label="特殊記号">
    {#each palette as group}
      <div class="editor-palette-row">
        <span>{group.label}</span>{#each group.characters as character}<button
            disabled={composing || raw}
            aria-label={`${group.label} ${character}`}
            onclick={() => editor?.run(insertText(character))}
            >{character}</button
          >{/each}
      </div>
    {/each}
    <div class="editor-palette-row">
      <span>欠字</span>{#each ["■", "□", "〓"] as character}<button
          disabled={composing || raw}
          aria-label={`欠字 ${character}`}
          onclick={() => editor?.run(insertSource(character))}
          >{character}</button
        >{/each}
    </div>
  </section>
</div>
