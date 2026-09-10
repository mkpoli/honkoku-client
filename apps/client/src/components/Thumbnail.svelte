<script lang="ts">
  import { onMount } from "svelte";
  import { entryCanvases } from "../data";
  let {
    entryId,
    index = 0,
    alt = "",
    url,
  }: {
    entryId?: string;
    index?: number;
    alt?: string;
    url?: string | null;
  } = $props();
  let host: HTMLSpanElement;
  let source = $state<string | null>(null);
  let failed = $state(false);
  let visible = $state(false);
  onMount(() => {
    const observer = new IntersectionObserver(
      (items) => {
        if (items.some((i) => i.isIntersecting)) {
          visible = true;
          observer.disconnect();
        }
      },
      { rootMargin: "100px" },
    );
    observer.observe(host);
    return () => observer.disconnect();
  });
  $effect(() => {
    const id = entryId,
      page = index,
      direct = url;
    source = direct ?? null;
    failed = false;
    let cancelled = false;
    if (visible && id && !direct)
      void entryCanvases(id)
        .then((cs) => {
          if (!cancelled)
            source = cs[page]?.thumbnailUrl ?? cs[page]?.imageUrl ?? null;
        })
        .catch(() => {
          if (!cancelled) failed = true;
        });
    return () => {
      cancelled = true;
    };
  });
</script>

<span class="thumbnail" bind:this={host}
  >{#if source && !failed}<img
      src={source}
      {alt}
      loading="lazy"
      referrerpolicy="no-referrer"
      onerror={() => (failed = true)}
    />{:else}<span class="thumbnail-placeholder" aria-label={alt || "画像なし"}
      >▧</span
    >{/if}</span
>
