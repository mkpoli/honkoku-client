<script lang="ts">
  import type { glyphCards } from "./glyph-regions";
  import QueuedImage from "./QueuedImage.svelte";
  import { href } from "../routes";
  let { card }: { card: ReturnType<typeof glyphCards>[number] } = $props();
  let hover = $state(false);
  let stripPosition = $state({ left: 0, top: 0 });
  function show(event: MouseEvent | FocusEvent) {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    stripPosition = {
      left: Math.max(8, Math.min(rect.right + 4, innerWidth - 136)),
      top: Math.max(8, Math.min(rect.top, innerHeight - 384)),
    };
    hover = true;
  }
</script>

<div
  role="group"
  class="glyph-card"
  class:located={!!card.crop}
  onmouseenter={show}
  onmouseleave={() => (hover = false)}
  onfocusin={show}
  onfocusout={() => (hover = false)}
>
  <a
    href={href({
      entryId: card.page.entryId,
      pageIndex: card.page.index,
      column: card.occurrence.column,
    })}
  >
    {#if card.crop}<div class="glyph-crop">
        <QueuedImage
          url={card.crop}
          infoUrl={card.page.canvas?.infoJsonUrl}
          alt={card.occurrence.matched}
        />
      </div>
    {:else}<p class="kwic">
        {card.occurrence.before}<strong>{card.occurrence.matched}</strong>{card
          .occurrence.after}
      </p>{/if}
    <span class="caption">{card.page.projectTitle || card.page.projectId}</span>
    <span class="caption">{card.page.entryLabel || card.page.entryId}</span>
    <span class="caption numeric">{card.page.index + 1}</span>
  </a>
  {#if card.page.error}<span class="caption error">{card.page.error}</span>{/if}
  {#if hover && card.strip}<div
      class="glyph-strip"
      style:left={`${stripPosition.left}px`}
      style:top={`${stripPosition.top}px`}
    >
      <QueuedImage
        url={card.strip}
        infoUrl={card.page.canvas?.infoJsonUrl}
        alt={`行全体：${card.occurrence.plain}`}
      />
    </div>{/if}
</div>

<style>
  .glyph-card {
    position: relative;
    min-width: 0;
  }
  a {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--text);
    text-decoration: none;
    border-radius: 7px;
    padding: 8px;
    border: 1px solid var(--border);
    background: var(--surface);
    height: 100%;
    box-sizing: border-box;
  }
  a:hover {
    border-color: var(--accent);
  }
  .glyph-crop {
    width: 96px;
    height: 128px;
    margin: 0 auto 4px;
  }
  .caption {
    display: block;
    overflow-wrap: anywhere;
    font-size: 12px;
    line-height: 18px;
  }
  .numeric {
    color: var(--text-muted);
  }
  .kwic {
    font-family: var(--font-serif);
    font-size: 18px;
    line-height: 1.8;
    overflow-wrap: anywhere;
  }
  strong {
    color: var(--accent);
  }
  .glyph-strip {
    position: fixed;
    z-index: 8;
    width: 112px;
    height: 360px;
    border: 1px solid var(--border-strong);
    border-radius: 7px;
    background: var(--surface);
    padding: 8px;
    pointer-events: none;
  }
  .error {
    color: var(--accent);
  }
</style>
