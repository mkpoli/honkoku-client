<script lang="ts">
  import "../../../../packages/editor/style.css";
  import { onDestroy } from "svelte";
  import {
    parseInline,
    renderInline,
    transcriptionColumns,
  } from "@honkoku/markup";
  let {
    source,
    horizontal = false,
    half = $bindable(""),
    highlightedColumn = -1,
    oncolumnchange,
  }: {
    source: string;
    horizontal?: boolean;
    half?: string;
    highlightedColumn?: number;
    oncolumnchange?: (index: number) => void;
  } = $props();
  let host: HTMLDivElement;
  let timer: ReturnType<typeof setTimeout>;
  let pinned = false;
  let currentColumn = $state(-1);
  $effect(() => { source; currentColumn = -1; });
  let groups = $derived.by(() => {
    const indices = new Map(
      transcriptionColumns(source).map((column, index) => [
        column.sourceIndex,
        index,
      ]),
    );
    const groups: {
      label: string;
      columns: { html: string; index: number }[];
    }[] = [];
    let group = { label: "", columns: [] as { html: string; index: number }[] };
    for (const [sourceIndex, line] of source.split(/\r\n|\r|\n/).entries()) {
      const marker = /^\s*【([右左]丁)】\s*$/.exec(line);
      if (marker) {
        if (group.columns.length || group.label) groups.push(group);
        group = { label: marker[1], columns: [] };
      } else
        group.columns.push({
          html: renderInline(parseInline(line)),
          index: indices.get(sourceIndex) ?? -1,
        });
    }
    if (group.columns.length || group.label) groups.push(group);
    return groups;
  });
  function change(index: number) {
    clearTimeout(timer);
    pinned = false;
    oncolumnchange?.(index);
  }
  export function focusColumn(index: number) {
    currentColumn = index;
    change(index);
    pinned = true;
    host
      .querySelector(`[data-column-index="${index}"]`)
      ?.scrollIntoView({ block: "nearest", inline: "nearest" });
    timer = setTimeout(() => {
      pinned = false;
      oncolumnchange?.(-1);
    }, 2000);
  }
  function measureColumns(element: HTMLElement) {
    const apply = () => {
      if (element.clientHeight > 0)
        element.style.setProperty(
          "--column-height",
          `${element.clientHeight}px`,
        );
    };
    const observer = new ResizeObserver(apply);
    observer.observe(element);
    if (element.parentElement) observer.observe(element.parentElement);
    apply();
    return { destroy: () => observer.disconnect() };
  }
  onDestroy(() => clearTimeout(timer));
</script>

<div class="transcription" class:horizontal bind:this={host}>
  {#each groups as group, i}
    <section
      class="column-group"
      class:half-selected={half === group.label && !!group.label}
      aria-label={group.label || `本文${i + 1}`}
    >
      {#if group.label}<button
          class="column-label"
          class:active={half === group.label}
          onclick={() => (half = half === group.label ? "" : group.label)}
          >{group.label}</button
        >{/if}
      <div class="columns" use:measureColumns>
        {#each group.columns as column}
          <div
            class="transcription-column"
            class:editor-active-column={currentColumn === column.index && column.index >= 0}
            class:alignment-active-column={column.index >= 0 &&
              highlightedColumn === column.index}
            data-column-index={column.index >= 0 ? column.index : undefined}
            role="button"
            tabindex={column.index >= 0 ? 0 : -1}
            onmouseenter={() => change(column.index)}
            onmouseleave={() => {
              if (!pinned) change(-1);
            }}
            onfocus={() => { currentColumn = column.index; change(column.index); }}
            onclick={() => focusColumn(column.index)}
            onkeydown={(event) => {
              if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
                event.preventDefault();
                event.stopPropagation();
                const index =
                  column.index + (event.key === "ArrowLeft" ? 1 : -1);
                const element = host.querySelector<HTMLElement>(
                  `[data-column-index="${index}"]`,
                );
                if (element) {
                  element.focus({ preventScroll: true });
                  focusColumn(index);
                }
              }
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                focusColumn(column.index);
              }
            }}
          >
            {@html column.html}
          </div>
        {/each}
      </div>
    </section>
  {:else}<p class="empty">本文はありません。</p>{/each}
</div>
