<script lang="ts">
  import { entryBibliography } from "@honkoku/client-api/invoke";
  import { manifestLabel as label, manifestLinks } from "../bibliography";
  import { errorMessage, safeUrl } from "../lib";
  import ExternalLink from "./ExternalLink.svelte";
  import Markdown from "./Markdown.svelte";
  let {
    entryId,
    manifestUrl,
    onclose,
  }: { entryId: string; manifestUrl: string; onclose: () => void } = $props();
  let manifest = $state<Record<string, unknown>>(),
    error = $state(""),
    reload = $state(0);
  let metadata = $derived(
    Array.isArray(manifest?.metadata)
      ? (manifest.metadata as { label: unknown; value: unknown }[])
      : [],
  );
  let required = $derived(
    manifest?.requiredStatement as
      { label?: unknown; value?: unknown } | undefined,
  );
  $effect(() => {
    const id = entryId;
    reload;
    let alive = true;
    manifest = undefined;
    error = "";
    void entryBibliography(id)
      .then((m) => {
        if (alive) manifest = m;
      })
      .catch((e) => {
        if (alive) error = errorMessage(e);
      });
    return () => {
      alive = false;
    };
  });
</script>

<aside class="bibliography-drawer side-drawer" aria-label="書誌情報">
  <header>
    <h2>書誌情報</h2>
    <button aria-label="書誌情報を閉じる" onclick={onclose}>×</button>
  </header>
  <div class="bibliography-content">
    {#if error}<p role="alert">{error}</p>
      <button onclick={() => reload++}>再試行</button>{:else if !manifest}<p
        role="status"
      >
        書誌情報を読み込み中…
      </p>{:else}
      <h3>{label(manifest.label)}</h3>
      <dl>
        {#each metadata as row}<dt>{label(row.label)}</dt>
          <dd><Markdown text={label(row.value)} /></dd>{/each}
        {#if manifest.attribution}<dt>提供</dt>
          <dd><Markdown text={label(manifest.attribution)} /></dd>{/if}
        {#if required}<dt>{label(required.label) || "提供条件"}</dt>
          <dd><Markdown text={label(required.value)} /></dd>{/if}
        {#each [manifest.license, manifest.rights] as rights}{#if rights}<dt>
              利用条件
            </dt>
            <dd>
              {#if manifestLinks(rights).length}{#each manifestLinks(rights) as link}<ExternalLink
                    href={link.url}>{link.label}</ExternalLink
                  >{/each}{:else}{label(rights)}{/if}
            </dd>{/if}{/each}
      </dl>
      {#each manifestLinks(manifest.logo) as logo}{#if safeUrl(logo.url)}<img
            class="manifest-logo"
            src={logo.url}
            alt="提供機関のロゴ"
            referrerpolicy="no-referrer"
          />{/if}{/each}
      {#each [...manifestLinks(manifest.seeAlso), ...manifestLinks(manifest.homepage)] as link}<p
        >
          <ExternalLink href={link.url}>{link.label}</ExternalLink>
        </p>{/each}
    {/if}
    <p><ExternalLink href={manifestUrl}>IIIFマニフェスト</ExternalLink></p>
  </div>
</aside>

<style>
  .bibliography-content {
    overflow: auto;
    padding: 16px;
    overflow-wrap: anywhere;
  }
  dt {
    color: var(--text-muted);
    font-size: 13px;
    margin-top: 16px;
  }
  dd {
    margin: 4px 0 0;
  }
  h3 {
    font: 600 20px var(--font-serif);
  }
  .manifest-logo {
    max-width: 160px;
    max-height: 80px;
    object-fit: contain;
  }
</style>
