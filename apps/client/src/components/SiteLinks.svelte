<script lang="ts">
  import { isTauri } from "@honkoku/client-api/invoke";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { Route } from "../routes";
  import {
    greetingUrl,
    learnUrl,
    siteHref,
    siteOpenLabel,
    siteOrigin,
    wikiUrl,
  } from "../site";

  let { route }: { route: Route } = $props();
  let open = $state(false);
  let failed = $state(false);
  let root: HTMLDivElement | undefined = $state();
  let toggle: HTMLButtonElement | undefined = $state();
  let current = $derived(siteHref(route));
  let links = $derived(
    [
      current && { href: current, label: siteOpenLabel(route) },
      current !== `${siteOrigin}/` && { href: siteOrigin, label: "公式サイト" },
      { href: greetingUrl, label: "ご案内" },
      { href: wikiUrl, label: "Wiki" },
      { href: learnUrl, label: "まなぶ" },
    ].filter((link): link is { href: string; label: string } => !!link),
  );

  function close(refocus = false) {
    open = false;
    failed = false;
    if (refocus) toggle?.focus();
  }
  async function follow(event: MouseEvent, href: string) {
    if (!isTauri()) {
      close();
      return;
    }
    event.preventDefault();
    try {
      await openUrl(href);
      close();
    } catch {
      failed = true;
    }
  }
  $effect(() => {
    if (!open) return;
    const click = (event: MouseEvent) => {
      if (root && !root.contains(event.target as Node)) close();
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") close(root?.contains(document.activeElement));
    };
    document.addEventListener("click", click);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("click", click);
      document.removeEventListener("keydown", key);
    };
  });
</script>

<div
  class="site-links"
  bind:this={root}
  onfocusout={(event) => {
    if (open && !root?.contains(event.relatedTarget as Node | null)) close();
  }}
>
  <button
    bind:this={toggle}
    type="button"
    class="site-links-toggle"
    aria-label="サイトリンク"
    aria-expanded={open}
    aria-controls="site-links-menu"
    title="サイトリンク"
    onclick={() => (open ? close() : (open = true))}>↗</button
  >
  {#if open}<div id="site-links-menu" class="menu-options site-links-menu">
      {#each links as link (link.href)}<a
          href={link.href}
          target="_blank"
          rel="noopener noreferrer"
          onclick={(event) => follow(event, link.href)}>{link.label}↗</a
        >{/each}
      {#if failed}<span class="error" role="alert"
          >ブラウザーを開けませんでした。</span
        >{/if}
    </div>{/if}
</div>
