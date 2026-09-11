<script lang="ts">
  import type { PageNote } from "@honkoku/client-api/types";
  import { date, errorMessage } from "../lib";
  import QueuedImage from "./QueuedImage.svelte";
  import Markdown from "./Markdown.svelte";
  let {
    notes,
    editing,
    disabled,
    authors,
    lockName,
    onclose,
    onsave,
    ondelete,
    onhover,
    onpan,
  }: {
    notes: (PageNote | null)[];
    editing: boolean;
    disabled: boolean;
    authors: Record<string, string>;
    lockName?: string;
    onclose: () => void;
    onsave: (note: PageNote, index?: number) => Promise<void>;
    ondelete: (index: number) => Promise<void>;
    onhover: (index: number | null) => void;
    onpan: (index: number) => void;
  } = $props();
  const types = [
    { value: "note", label: "注釈" },
    { value: "memo", label: "メモ" },
    { value: "transcription", label: "翻刻" },
    { value: "other", label: "その他" },
  ];
  let draft = $state<PageNote>();
  let draftIndex = $state<number>();
  let deleting = $state<number>();
  let saving = $state(false),
    error = $state("");
  let textarea = $state<HTMLTextAreaElement>();
  let ordered = $derived(
    notes
      .map((note, index) => ({ note, index }))
      .filter((row) => row.note)
      .sort(
        (a, b) =>
          Date.parse(b.note?.createdAt ?? "") -
            Date.parse(a.note?.createdAt ?? "") || b.index - a.index,
      ),
  );
  export function create(region?: Pick<PageNote, "image" | "xywh">) {
    if (!editing || disabled || saving || draft) return;
    draftIndex = undefined;
    draft = { content: "", type: "note", ...region };
    error = "";
    requestAnimationFrame(() => textarea?.focus());
  }
  function edit(note: PageNote, index: number) {
    if (draft) return;
    draftIndex = index;
    draft = { ...note };
    error = "";
    requestAnimationFrame(() => textarea?.focus());
  }
  function regionClick(element: HTMLElement, index: number) {
    const click = (event: MouseEvent) => {
      if (
        !(event.target as HTMLElement).closest("button,a,input,textarea,form")
      )
        onpan(index);
    };
    element.addEventListener("click", click);
    return { destroy: () => element.removeEventListener("click", click) };
  }
  async function save() {
    if (!draft || saving || disabled) return;
    saving = true;
    error = "";
    try {
      await onsave(draft, draftIndex);
      draft = undefined;
    } catch (e) {
      error = errorMessage(e);
    } finally {
      saving = false;
    }
  }
  async function remove(index: number) {
    if (deleting !== index) {
      deleting = index;
      return;
    }
    saving = true;
    error = "";
    try {
      await ondelete(index);
      deleting = undefined;
    } catch (e) {
      error = errorMessage(e);
    } finally {
      saving = false;
    }
  }
  $effect(() => {
    if (!editing) {
      draft = undefined;
      deleting = undefined;
    }
  });
</script>

<aside class="notes-drawer" id="notes-drawer" aria-label="注釈一覧">
  <header>
    <h2>注釈<span class="count">{ordered.length}</span></h2>
    <div>
      {#if editing}<button
          disabled={disabled || saving || !!draft}
          onclick={() => create()}>新規</button
        >{/if}
      <button aria-label="注釈を閉じる" disabled={saving} onclick={onclose}
        >×</button
      >
    </div>
  </header>
  {#if !editing}<p class="caption muted">
      編集を開始すると注釈を追加できます。{#if lockName}<br
        />{lockName}が編集中です。{/if}
    </p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
  <div class="notes-scroll">
    {#if draft && draftIndex === undefined}{@render editor()}{/if}
    {#each ordered as { note, index } (index)}
      {#if note}
        <article
          class="page-note"
          use:regionClick={index}
          data-page-note-index={index}
          onmouseenter={() => onhover(index)}
          onmouseleave={() => onhover(null)}
          onfocusin={() => onhover(index)}
          onfocusout={() => onhover(null)}
        >
          {#if draft && draftIndex === index}{@render editor()}{:else}
            <div class="note-heading">
              <span class="type-chip"
                >{types.find((t) => t.value === note.type)?.label ??
                  "その他"}</span
              ><span class="caption muted">＃{index + 1}</span>
            </div>
            <Markdown
              text={(note.markdown ?? note.content).replace(/\n/g, "  \n")}
            />
            {#if note.image}<button
                class="note-thumbnail"
                aria-label={`注釈${index + 1}の領域へ移動`}
                onclick={() => onpan(index)}
                ><QueuedImage
                  url={note.image.replace(
                    /\/[^/]+\/[^/]+\/[^/]+\.[^/]+$/,
                    "/400,/0/default.jpg",
                  )}
                  alt={`注釈${index + 1}の画像領域`}
                /></button
              >{:else if note.xywh}<button onclick={() => onpan(index)}
                >画像の領域へ移動</button
              >{/if}
            <div class="caption muted">
              {note.createdBy
                ? (authors[note.createdBy] ?? "名前を確認中")
                : "名前不明"}・{date(
                note.createdAt,
              )}{#if note.updatedAt && note.updatedAt !== note.createdAt}<br
                />更新{date(note.updatedAt)}{/if}
            </div>
            {#if editing}<div class="note-actions">
                <button
                  disabled={disabled || saving || !!draft}
                  onclick={() => edit(note, index)}>編集</button
                ><button
                  disabled={disabled || saving || !!draft}
                  onclick={() => remove(index)}
                  >{deleting === index ? "削除する" : "削除"}</button
                >{#if deleting === index}<button
                    disabled={saving}
                    onclick={() => (deleting = undefined)}>取消</button
                  >{/if}
              </div>{/if}
          {/if}
        </article>
      {/if}
    {/each}
    {#if !ordered.length && !draft}<p class="muted">注釈はありません。</p>{/if}
  </div>
</aside>

{#snippet editor()}
  {#if draft}<form
      class="note-editor"
      aria-label="注釈を編集"
      onsubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <fieldset disabled={disabled || saving}>
        <legend>種類</legend>
        <div class="note-types">
          {#each types as type}<button
              type="button"
              aria-pressed={draft.type === type.value}
              onclick={() => {
                if (draft) draft.type = type.value;
              }}>{type.label}</button
            >{/each}
        </div>
        <label
          >本文<textarea bind:this={textarea} bind:value={draft.content}
          ></textarea></label
        >
        {#if draft.image}<div class="note-thumbnail">
            <QueuedImage url={draft.image} alt="選択した画像領域" />
          </div>{/if}
        <div class="note-actions">
          <button class="primary" disabled={!draft.content.trim()}>保存</button
          ><button type="button" onclick={() => (draft = undefined)}
            >取消</button
          >
        </div>
      </fieldset>
    </form>{/if}
{/snippet}

<style>
  .notes-drawer {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(340px, 65%);
    z-index: 10;
    background: var(--surface);
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    padding: 16px;
    gap: 12px;
  }
  header,
  header > div,
  .note-heading,
  .note-actions,
  .note-types {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  header {
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 18px;
  }
  .count {
    margin-left: 8px;
  }
  .notes-scroll {
    overflow: auto;
    min-height: 0;
  }
  .page-note {
    padding: 16px 0;
    border-bottom: 1px solid var(--border);
    overflow-wrap: anywhere;
  }
  .type-chip {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 2px 8px;
    font-size: 13px;
    background: var(--surface-inset);
  }
  .note-thumbnail {
    display: block;
    width: 100%;
    height: 130px;
    padding: 0;
    margin: 8px 0;
    border: 0;
  }
  .note-actions {
    margin-top: 12px;
  }
  fieldset {
    border: 0;
    margin: 0;
    padding: 0;
    min-width: 0;
  }
  legend {
    font-size: 13px;
    margin-bottom: 8px;
  }
  .note-types {
    flex-wrap: wrap;
  }
  label {
    display: block;
    margin-top: 12px;
    font-size: 13px;
  }
  textarea {
    width: 100%;
    min-height: 120px;
    resize: vertical;
    margin-top: 4px;
  }
  .note-editor {
    padding: 12px 0;
  }
  p {
    margin: 0;
  }
  [role="alert"] {
    color: var(--accent);
  }
  .note-types button[aria-pressed="true"] {
    background: var(--accent-soft);
    border-color: var(--accent);
  }
</style>
