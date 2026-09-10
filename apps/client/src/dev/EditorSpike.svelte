<script lang="ts">
  import { onMount } from "svelte";
  import Transcription from "../components/Transcription.svelte";
  import VerticalEditor from "@honkoku/editor/VerticalEditor.svelte";
  import {
    historyKey,
    fromMarkup,
    toMarkup,
    textareaSource,
    type EditorUpdate,
  } from "./editor-harness";
  import { editorFixture } from "./editor-fixture";
  let source = $state(editorFixture.text);
  let reading = $state(false);
  let serialized = $state(toMarkup(fromMarkup(editorFixture.text)));
  let column = $state(0),
    composing = $state(false);
  function update(value: EditorUpdate) {
    serialized = value.serialized;
    column = value.column;
    composing = value.composing;
  }
  onMount(() => () => {
    delete window.editorSpike;
  });
</script>

<div class="editor-spike">
  <div class="workbench-toolbar">
    <div>
      <strong>{editorFixture.title}</strong><span class="caption muted"
        >　{editorFixture.index + 1}コマ</span
      >
    </div>
    <span class="caption muted"
      >{column + 1}列目・{composing ? "変換中" : "縦書き"}</span
    >
  </div>
  <div class="editor-spike-panes">
    <section class="panel transcription-panel">
      <div class="pane-toolbar">
        <h2>翻刻</h2>
        <span class="caption muted">↑↓文字　←→列　Enter改行</span>
      </div>
      <VerticalEditor
        accountId="fixture-editor"
        bind:source
        onupdate={update}
        onready={(editor) => {
          window.editorSpike = {
            ...editor,
            get source() {
              return source;
            },
            fixture: editorFixture,
          };
        }}
      />
    </section>
    <section class="panel editor-source-panel">
      <div class="pane-toolbar">
        <h2>{reading ? "表示" : "原文"}</h2>
        <button aria-pressed={reading} onclick={() => (reading = !reading)}
          >表示を確認</button
        >
        <output
          class="editor-equality"
          class:equal={serialized === source}
          aria-live="polite">{serialized === source ? "一致" : "不一致"}</output
        >
      </div>
      {#if reading}<Transcription {source} />{:else}<textarea
          class="editor-raw-textarea"
          aria-label="原文"
          onkeydown={(event) => historyKey(event, window.editorSpike)}
          value={source}
          disabled={composing}
          oninput={(event) =>
            window.editorSpike?.setSource(
              textareaSource(source, event.currentTarget.value),
            )}></textarea>{/if}
      <div class="editor-source-footer caption muted">
        {source.length.toLocaleString("ja-JP")}文字・{source.split(/\r\n|\r|\n/)
          .length}列
      </div>
    </section>
  </div>
</div>

<style>
  .editor-spike {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .editor-spike-panes {
    display: grid;
    grid-template-columns: minmax(0, 3fr) minmax(0, 2fr);
    gap: 16px;
    flex: 1;
    min-height: 0;
  }
  .editor-spike-panes > .panel {
    padding: 0;
    overflow: hidden;
  }
  .editor-source-panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .editor-equality {
    font-size: 13px;
    color: var(--accent);
  }
  .editor-equality.equal {
    color: var(--success);
  }
  .editor-source-footer {
    padding: 12px 16px;
    border-top: 1px solid var(--border);
  }
</style>
