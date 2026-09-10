<script module lang="ts">
  import { iiifLocalUrl } from "@honkoku/client-api/iiif";
  let active = 0;
  const jobs: (() => void)[] = [];
  function pump() {
    while (active < 6 && jobs.length) jobs.shift()!();
  }
  function schedule(start: () => void) {
    let running = false,
      finished = false;
    const job = () => {
      running = true;
      active++;
      start();
    };
    jobs.push(job);
    pump();
    return () => {
      if (finished) return;
      finished = true;
      if (running) active--;
      else {
        const i = jobs.indexOf(job);
        if (i >= 0) jobs.splice(i, 1);
      }
      pump();
    };
  }
</script>

<script lang="ts">
  import { glyphImageUrl } from "@honkoku/client-api/invoke";
  let {
    url,
    alt,
    infoUrl,
  }: { url: string; alt: string; infoUrl?: string | null } = $props();
  let ready = $state(false),
    failed = $state(false),
    retry = $state(0);
  function load(node: HTMLImageElement, upstream: string) {
    let alive = true,
      pending = true;
    let timeout: ReturnType<typeof setTimeout>;
    const finish = () => {
      pending = false;
      clearTimeout(timeout);
      release();
    };
    const loaded = () => {
      ready = true;
      finish();
    };
    const error = () => {
      failed = true;
      finish();
    };
    node.addEventListener("load", loaded);
    node.addEventListener("error", error);
    const release = schedule(() => {
      timeout = setTimeout(() => {
        node.removeAttribute("src");
        error();
      }, 30000);
      void (infoUrl ? glyphImageUrl(infoUrl, upstream) : iiifLocalUrl(upstream))
        .then((local) => {
          if (alive && pending) node.src = local;
        })
        .catch(() => {
          if (alive) error();
        });
    });
    return {
      destroy() {
        alive = false;
        node.removeEventListener("load", loaded);
        node.removeEventListener("error", error);
        node.removeAttribute("src");
        finish();
      },
    };
  }
  $effect(() => {
    url;
    retry;
    ready = false;
    failed = false;
  });
</script>

<div
  class="queued-image"
  class:loading={!ready && !failed}
  aria-busy={!ready && !failed}
>
  {#key `${url}-${retry}`}<img
      use:load={url}
      {alt}
      class:ready
      referrerpolicy="no-referrer"
    />{/key}
  {#if failed}<button
      onclick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        retry++;
      }}
      aria-label={`${alt}を再試行`}>再試行</button
    >{/if}
</div>

<style>
  .queued-image {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 40px;
    background: var(--surface-inset);
    border-radius: 6px;
    overflow: hidden;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    opacity: 0;
  }
  img.ready {
    opacity: 1;
  }
  .loading {
    background: linear-gradient(
      100deg,
      var(--surface-inset) 30%,
      var(--border) 50%,
      var(--surface-inset) 70%
    );
    background-size: 200% 100%;
    animation: shimmer 1.5s infinite;
  }
  button {
    position: absolute;
    inset: 35% 4px auto;
    font-size: 12px;
  }
  @keyframes shimmer {
    to {
      background-position: -200% 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .loading {
      animation: none;
    }
  }
</style>
