<script lang="ts">
  import type { Route } from "../routes";
  import {
    greetingUrl,
    learnUrl,
    siteHref,
    siteOpenLabel,
    siteOrigin,
    wikiUrl,
  } from "../site";
  import ExternalLink from "./ExternalLink.svelte";

  let { route }: { route: Route } = $props();
  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();
  let current = $derived(siteHref(route));

  function onDocumentClick(event: MouseEvent) {
    if (!open || !root) return;
    const target = event.target as Element | null;
    if (!target || !root.contains(target)) {
      open = false;
      return;
    }
    if (target.closest("a")) open = false;
  }
  function onKey(event: KeyboardEvent) {
    if (event.key === "Escape") open = false;
  }
  $effect(() => {
    if (!open) return;
    document.addEventListener("click", onDocumentClick);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("click", onDocumentClick);
      document.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="site-links" bind:this={root}>
  <button
    type="button"
    class="site-links-toggle"
    aria-label="サイトリンク"
    aria-expanded={open}
    title="サイトリンク"
    onclick={() => (open = !open)}>↗</button
  >
  {#if open}<div class="menu-options site-links-menu">
      {#if current}<ExternalLink href={current}>{siteOpenLabel(route)}↗</ExternalLink>{/if}
      {#if current !== `${siteOrigin}/`}<ExternalLink href={siteOrigin}
          >公式サイト↗</ExternalLink
        >{/if}
      <ExternalLink href={greetingUrl}>ご案内↗</ExternalLink>
      <ExternalLink href={wikiUrl}>Wiki↗</ExternalLink>
      <ExternalLink href={learnUrl}>まなぶ↗</ExternalLink>
    </div>{/if}
</div>
