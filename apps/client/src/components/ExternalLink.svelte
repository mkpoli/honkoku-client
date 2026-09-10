<script lang="ts">
  import type { Snippet } from "svelte";
  import { isTauri } from "@honkoku/client-api/invoke";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { safeUrl } from "../lib";
  let { href, children }: { href: string; children: Snippet } = $props();
  let failed = $state(false);
  async function open(event: MouseEvent) {
    if (!isTauri()) return;
    event.preventDefault();
    try {
      await openUrl(href);
      failed = false;
    } catch {
      failed = true;
    }
  }
</script>

{#if safeUrl(href)}<a
    {href}
    target="_blank"
    rel="noopener noreferrer"
    onclick={open}>{@render children()}</a
  >{:else}{@render children()}{/if}
{#if failed}<span class="error" role="alert"
    >ブラウザーを開けませんでした。</span
  >{/if}
