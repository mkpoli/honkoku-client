<script lang="ts">
  import type { AtlasGlyph } from "@honkoku/client-api/types";
  import { href } from "../routes";
  import ExternalLink from "./ExternalLink.svelte";
  let { glyph }: { glyph: AtlasGlyph } = $props();
  let licence = $derived(
    glyph.licence === "bespoke-free"
      ? "所蔵者の条件"
      : glyph.licence.replaceAll("-", " "),
  );
  const states: Record<string, string> = {
    checked: "確認済み",
    flagged: "疑義あり",
    pending: "未確認",
  };
  let page = $derived(
    glyph.entryId && glyph.pageIndex !== null
      ? href({ entryId: glyph.entryId, pageIndex: glyph.pageIndex })
      : null,
  );
</script>

{#snippet body()}
  <div class="glyph-crop">
    <img src={glyph.image} alt={glyph.label} loading="lazy" />
    {#if glyph.state && states[glyph.state]}<span
        class="state"
        data-state={glyph.state}>{states[glyph.state]}</span
      >{/if}
  </div>
  <span class="caption">{glyph.source}</span>
  {#if glyph.pageNumber !== null}<span class="caption numeric"
      >{glyph.pageNumber}</span
    >{/if}
{/snippet}

<div role="group" class="atlas-card">
  {#if page}<a class="card" href={page}>{@render body()}</a>{:else}<div
      class="card"
    >
      {@render body()}
    </div>{/if}
  <span class="caption rights"
    >{#if glyph.rightsUrl}<ExternalLink href={glyph.rightsUrl}
        >{glyph.holder ?? glyph.attribution ?? "所蔵者不明"}</ExternalLink
      >{:else}{glyph.holder ?? glyph.attribution ?? "所蔵者不明"}{/if}・{licence}</span
  >
  <span class="caption"
    ><ExternalLink href={glyph.recordUrl}>記録↗</ExternalLink></span
  >
</div>

<style>
  .atlas-card {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--surface);
    padding: 8px;
  }
  .atlas-card:hover {
    border-color: var(--accent);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--text);
    text-decoration: none;
  }
  .glyph-crop {
    width: 96px;
    height: 128px;
    margin: 0 auto 4px;
    position: relative;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .state {
    position: absolute;
    right: -4px;
    bottom: -4px;
    font-size: 11px;
    line-height: 16px;
    padding: 0 4px;
    border-radius: 4px;
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text-muted);
  }
  .state[data-state="checked"] {
    color: var(--gold);
    border-color: currentColor;
  }
  .state[data-state="flagged"] {
    color: var(--accent);
    border-color: currentColor;
  }
  .caption {
    display: block;
    overflow-wrap: anywhere;
    font-size: 12px;
    line-height: 18px;
  }
  .numeric,
  .rights {
    color: var(--text-muted);
  }
</style>
