<script lang="ts">
  import { onMount } from "svelte";
  import type { Page } from "@honkoku/client-api/types";
  import { relative, user } from "../lib";
  import { lockAge } from "../lock-age";

  let { page }: { page: Page } = $props();
  let name = $state("名前を確認中");
  let now = $state(Date.now());
  let age = $derived(lockAge(page.updatedAt, now));

  $effect(() => {
    const uid = page.tempEditedBy;
    let cancelled = false;
    name = uid ? "名前を確認中" : "名前不明";
    if (uid)
      void user(uid)
        .then((u) => {
          if (!cancelled) name = u.displayName;
        })
        .catch(() => {
          if (!cancelled) name = "名前不明";
        });
    return () => {
      cancelled = true;
    };
  });

  onMount(() => {
    const timer = setInterval(() => (now = Date.now()), 60000);
    return () => clearInterval(timer);
  });
</script>

<span class="lock-status caption muted" class:idle={age.idle}>
  <span class="holder" title={name}>{name}</span>
  {#if age.ageMs !== null && page.updatedAt}<time
      datetime={page.updatedAt}
      title={new Date(page.updatedAt).toLocaleString("ja-JP")}
      >最終更新{relative(page.updatedAt, now)}</time
    >{:else}<span>更新日時不明</span>{/if}
  {#if age.idle}<span class="idle-label">放置中</span>{/if}
</span>

<style>
  .lock-status {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px 8px;
    min-width: 0;
  }
  .holder {
    max-width: 12em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .idle {
    color: color-mix(in srgb, var(--gold) 65%, var(--text-muted));
  }
  .idle-label {
    padding: 0 4px;
    border-radius: var(--control-radius);
    background: color-mix(in srgb, var(--gold) 10%, var(--surface));
  }
</style>
