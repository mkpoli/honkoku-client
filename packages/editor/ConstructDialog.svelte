<script lang="ts">
  import { onMount, untrack } from "svelte";
  let {
    title,
    initial,
    labels,
    noteIndex,
    onconfirm,
    ondelete,
    onclose,
  }: {
    title: string;
    initial: string[];
    labels: string[];
    noteIndex?: number;
    onconfirm: (values: string[]) => void;
    ondelete?: () => void;
    onclose: () => void;
  } = $props();
  let values = $state(untrack(() => [...initial]));
  let dialog: HTMLDialogElement;
  onMount(() => {
    dialog.showModal();
  });
</script>

<dialog
  bind:this={dialog}
  class="construct-dialog"
  aria-label={title}
  oncancel={onclose}
  {onclose}
>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      onconfirm(values);
    }}
  >
    <h2>{title}{noteIndex !== undefined ? `＃${noteIndex + 1}` : ""}</h2>
    {#each values as value, i}
      <label
        >{labels[i] ?? `${i + 1}行目`}<input
          aria-label={labels[i] ?? `${i + 1}行目`}
          bind:value={values[i]}
          required={title === "注記" || (title === "振り仮名" && i < 2)}
        /></label
      >
    {/each}
    {#if title === "割書" && values.length < 4}<button
        type="button"
        onclick={() => (values = [...values, ""])}>行を追加</button
      >{/if}
    <div class="dialog-actions">
      <button class="primary" type="submit"
        >{title === "注記"
          ? noteIndex === undefined
            ? "追加"
            : "更新"
          : "挿入"}</button
      >
      <button type="button" onclick={onclose}>キャンセル</button>
      {#if ondelete}<button type="button" onclick={ondelete}>削除</button>{/if}
    </div>
  </form>
</dialog>
