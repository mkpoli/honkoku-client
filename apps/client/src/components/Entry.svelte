<script lang="ts">
  import type { Canvas, Entry, Page } from "@honkoku/client-api/types";
  import { label, number, status, statusClass } from "../lib";
  import { href } from "../routes";
  import Thumbnail from "./Thumbnail.svelte";
  import ExternalLink from "./ExternalLink.svelte";
  let {
    entry,
    pages,
    canvases,
  }: { entry: Entry; pages: Page[]; canvases: Canvas[] } = $props();
  let counts = $derived(
    [...new Set(pages.map((p) => p.status))].map((s) => ({
      s,
      count: pages.filter((p) => p.status === s).length,
    })),
  );
</script>

<div class="entry-screen scroll">
  <section class="panel entry-header">
    <h1 class="serif">{label(entry.label)}</h1>
    <p>
      {label(entry.attribution)}{#if entry.license}<span class="licence"
          ><ExternalLink href={entry.license}>利用条件↗</ExternalLink></span
        >{/if}
    </p>
    <div class="entry-summary">
      <strong>{number(entry.size)}コマ</strong>{#each counts as c}<span
          class="status {statusClass(c.s)}"
          >{status(c.s).symbol}{status(c.s).label}{c.count}</span
        >{/each}
    </div>
  </section>
  <section class="panel page-grid-panel">
    <h2>コマ一覧</h2>
    <div class="page-grid">
      {#each pages as p (p.id)}<a
          class="page-card"
          href={href({ entryId: entry.id, pageIndex: p.index })}
          ><Thumbnail
            url={canvases[p.index]?.thumbnailUrl ?? canvases[p.index]?.imageUrl}
            alt={`コマ${p.index + 1}の原本`}
          />
          <div>
            <strong>{p.index + 1}</strong><span
              class="status {statusClass(p.status)}"
              >{status(p.status).symbol}{status(p.status).label}</span
            >
          </div></a
        >{/each}
    </div>
  </section>
</div>
