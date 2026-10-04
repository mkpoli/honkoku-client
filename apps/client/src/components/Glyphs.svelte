<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    glyphAttestations,
    atlasGlyphs,
    searchStatus,
    listProjects,
    isTauri,
  } from "@honkoku/client-api/invoke";
  import type {
    AtlasGlyphs,
    GlyphAttestations,
    SearchStatus,
  } from "@honkoku/client-api/types";
  import { glyphCards, singleGlyph } from "./glyph-regions";
  import { errorMessage } from "../lib";
  import GlyphCard from "./GlyphCard.svelte";
  import AtlasCard from "./AtlasCard.svelte";
  import ExternalLink from "./ExternalLink.svelte";
  import Clips from "./Clips.svelte";
  import Skeleton from "./Skeleton.svelte";
  let { character, compact = false }: { character: string; compact?: boolean } =
    $props();
  let input = $state(""),
    project = $state<string | null>(null),
    limit = $state(100);
  let tab = $state("corpus"),
    result = $state<GlyphAttestations>(),
    status = $state<SearchStatus>();
  let loading = $state(false),
    error = $state(""),
    revision = $state(0);
  let names = $state<Record<string, string>>({});
  let atlas = $state<AtlasGlyphs>(),
    atlasLoading = $state(false),
    atlasError = $state(""),
    atlasRevision = $state(0);
  let generation = 0;
  let code = $derived(
    [...character]
      .map(
        (c) =>
          `U+${c.codePointAt(0)!.toString(16).toUpperCase().padStart(4, "0")}`,
      )
      .join(" "),
  );
  let codh = $derived(
    `http://codh.rois.ac.jp/char-shape/unicode/U+${character.codePointAt(0)?.toString(16).toUpperCase().padStart(4, "0")}/`,
  );
  let cards = $derived(
    (
      result?.pages
        .map((p) => ({
          ...p,
          projectTitle: names[p.projectId] || p.projectTitle,
        }))
        .flatMap(glyphCards) ?? []
    )
      .sort((a, b) => Number(!!b.crop) - Number(!!a.crop))
      .slice(0, compact ? 24 : limit),
  );
  let located = $derived(cards.filter((c) => c.crop));
  let unknown = $derived(cards.filter((c) => !c.crop));
  $effect(() => {
    input = character;
    project = null;
  });
  $effect(() => {
    const char = character,
      scope = project,
      count = compact ? 24 : limit;
    revision;
    const request = ++generation;
    result = undefined;
    error = "";
    if (!status?.present || !singleGlyph(char)) {
      loading = false;
      return;
    }
    loading = true;
    void glyphAttestations(char, scope, count)
      .then((value) => {
        if (request === generation) {
          result = value;
          for (const p of value.pages)
            if (!names[p.projectId]) names[p.projectId] = p.projectTitle;
        }
      })
      .catch((e) => {
        if (request === generation) error = errorMessage(e);
      })
      .finally(() => {
        if (request === generation) loading = false;
      });
    return () => {
      generation++;
    };
  });
  $effect(() => {
    const char = character,
      count = compact ? 24 : limit;
    atlasRevision;
    let alive = true;
    atlas = undefined;
    atlasError = "";
    if (!singleGlyph(char)) return;
    atlasLoading = true;
    void atlasGlyphs(char, count)
      .then((value) => {
        if (alive) atlas = value;
      })
      .catch((e) => {
        if (alive) atlasError = errorMessage(e);
      })
      .finally(() => {
        if (alive) atlasLoading = false;
      });
    return () => {
      alive = false;
    };
  });
  function change(event: Event) {
    const node = event.currentTarget as HTMLInputElement;
    if (singleGlyph(node.value))
      location.hash = `#/glyphs/${encodeURIComponent(node.value)}`;
    else if (node.value) node.value = character;
  }
  async function setup() {
    try {
      status = await searchStatus();
    } catch (e) {
      error = errorMessage(e);
    }
  }
  onMount(() => {
    let alive = true;
    void searchStatus()
      .then((s) => {
        if (alive) status = s;
      })
      .catch((e) => {
        if (alive) error = errorMessage(e);
      });
    void listProjects()
      .then((p) => {
        if (alive)
          names = {
            ...names,
            ...Object.fromEntries(p.map((p) => [p.id, p.title])),
          };
      })
      .catch(() => {});
    return () => {
      alive = false;
      generation++;
    };
  });
</script>

<section class="glyphs" class:compact aria-label="集字">
  {#if !compact}
    <header>
      <div class="glyph-title">
        <h1>{character}</h1>
        <div>
          <strong>集字</strong>
          <p class="caption muted">{code}</p>
          {#if atlas?.jibo.length}<p class="caption">
              字母 {atlas.jibo.join("・")}
            </p>{/if}
          {#if atlas?.readings.length}<p class="caption">
              読み {atlas.readings.join("・")}
            </p>{/if}
        </div>
      </div>
      <label
        >文字<input
          aria-label="集字する文字"
          value={input}
          oninput={(e) => {
            if (!(e instanceof InputEvent && e.isComposing)) change(e);
          }}
          oncompositionend={change}
        /></label
      >
      <label
        >表示件数<select aria-label="集字の表示件数" bind:value={limit}
          ><option value={50}>50</option><option value={100}>100</option><option
            value={200}>200</option
          ></select
        ></label
      >
      <a
        href={codh}
        target="_blank"
        rel="noreferrer"
        onclick={async (e) => {
          if (isTauri()) {
            e.preventDefault();
            try {
              await openUrl(codh);
            } catch (e) {
              error = errorMessage(e);
            }
          }
        }}>くずし字データセット↗</a
      >
    </header>
    {#if atlas?.variants.length}<div class="variants" aria-label="異体字">
        <span class="caption muted">異体字</span>
        {#each atlas.variants as variant (variant.character)}<a
            href={`#/glyphs/${encodeURIComponent(variant.character)}`}
            >{variant.character}</a
          >{/each}
        <details>
          <summary class="caption muted">典拠</summary>
          <ul>
            {#each Object.values(atlas.variantSources) as credit}<li
                class="caption"
              >
                {credit}
              </li>{/each}
          </ul>
        </details>
      </div>{/if}
    <nav aria-label="集字の資料">
      <button aria-pressed={tab === "corpus"} onclick={() => (tab = "corpus")}
        >翻刻資料</button
      ><button aria-pressed={tab === "clips"} onclick={() => (tab = "clips")}
        >クリップ</button
      ><a href="#/clips">すべてのクリップ</a>
    </nav>
  {/if}
  {#if tab === "clips"}<Clips
      {character}
      embedded
    />{:else if !singleGlyph(character)}<p>本文から1文字を選んでください。</p>
  {:else}
    <section class="atlas" aria-label="Glyph Atlas の字形">
      <header class="section-head">
        <h2>Glyph Atlas</h2>
        {#if atlas}<span class="caption muted"
            >{atlas.total.toLocaleString("ja-JP")}字・表示{atlas.items
              .length}字</span
          >{#if atlas.pageUrl}<ExternalLink href={atlas.pageUrl}
              >glyphatlas.org↗</ExternalLink
            >{/if}{/if}
      </header>
      {#if atlasError}<p role="alert">
          {atlasError}<button onclick={() => atlasRevision++}>再試行</button>
        </p>
      {:else if atlasLoading}<div class="glyph-grid" aria-label="Glyph Atlas を取得中">
          {#each Array(compact ? 3 : 6) as _}<Skeleton
              shape="frame"
              count={1}
            />{/each}
        </div>
      {:else if atlas?.items.length}<div
          class="glyph-grid"
          aria-label="Glyph Atlas の画像"
        >
          {#each atlas.items as glyph (glyph.id)}<AtlasCard {glyph} />{/each}
        </div>
      {:else if atlas}<p class="caption muted">
          Glyph Atlas にはこの字の切り出しがまだありません。
        </p>{/if}
    </section>
    <header class="section-head">
      <h2>翻刻資料から推定</h2>
      <span class="caption muted">OCRの行枠を字数で割った位置</span>
    </header>
    {#if status && !status.present}<section class="panel guidance">
      <h2>翻刻を端末で検索する</h2>
      <p>
        公開翻刻データから検索索引を作成します。一度作成すると、通信せずに本文を検索できます。
      </p>
      <p class="muted">公開データ約833MB。検索索引用の空き容量も必要です。</p>
      <a href="#/search">検索索引を作成</a>
    </section>
  {:else}
    {#if !compact && result}<div
        class="facets"
        aria-label="プロジェクトで絞り込み"
      >
        <button aria-pressed={!project} onclick={() => (project = null)}
          >すべて</button
        >{#each result.facets as [id, count]}<button
            aria-pressed={project === id}
            onclick={() => (project = id)}
            >{names[id] || id}<span>{count.toLocaleString("ja-JP")}</span
            ></button
          >{/each}
      </div>{/if}
    {#if error}<p role="alert">
        {error}<button
          onclick={() => {
            if (status) revision++;
            else void setup();
          }}>再試行</button
        >
      </p>{/if}
    {#if loading || (!status && !error)}<div
        class="glyph-grid"
        aria-label="集字を取得中"
      >
        {#each Array(compact ? 6 : 12) as _}<Skeleton
            shape="frame"
            count={1}
          />{/each}
      </div>{/if}
    {#if result}<p class="caption muted">
        {result.total.toLocaleString("ja-JP")}コマ・表示{cards.length}字
      </p>
      <div class="glyph-grid" aria-label="字形の画像">
        {#each located as card (card.key)}<GlyphCard {card} />{/each}
      </div>
      {#if unknown.length}<h2>位置不明</h2>
        <div class="glyph-grid unknown">
          {#each unknown as card (card.key)}<GlyphCard {card} />{/each}
        </div>{/if}
      {#if !cards.length}<p>一致する翻刻はありません。</p>{/if}
      {#if result.pages.some((p) => p.error)}<button onclick={() => revision++}
          >取得できなかった資料を再試行</button
        >{/if}
    {/if}
  {/if}
  {/if}
</section>

<style>
  .glyphs {
    height: 100%;
    overflow: auto;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .glyphs > * {
    flex-shrink: 0;
  }
  .compact {
    padding: 12px;
    height: auto;
    overflow: visible;
  }
  header {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 24px;
  }
  .glyph-title {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  h1 {
    font: 600 64px/1.2 var(--font-serif);
    margin: 0;
  }
  header label {
    display: flex;
    flex-direction: column;
    font-size: 13px;
    gap: 4px;
  }
  header input {
    width: 72px;
    font: 24px var(--font-serif);
  }
  header a {
    margin-left: auto;
    font-size: 13px;
  }
  nav,
  .facets {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  nav {
    border-bottom: 1px solid var(--border);
    padding-bottom: 12px;
  }
  nav a {
    margin-left: auto;
    font-size: 13px;
  }
  .facets button {
    font-size: 13px;
  }
  .facets span {
    margin-left: 8px;
    color: var(--text-muted);
  }
  [aria-pressed="true"] {
    background: var(--accent-soft);
    border-color: var(--accent);
  }
  .glyph-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(128px, 1fr));
    gap: 12px;
    align-items: stretch;
  }
  .compact .glyph-grid {
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: 8px;
  }
  .unknown {
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  }
  h2 {
    font-size: 18px;
    margin: 8px 0 0;
  }
  .section-head {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 12px;
  }
  .section-head h2 {
    margin: 0;
  }
  .section-head :global(a) {
    font-size: 13px;
  }
  .atlas {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .variants {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 8px 12px;
  }
  .variants > a {
    font: 24px var(--font-serif);
    text-decoration: none;
  }
  .variants details {
    flex-basis: 100%;
  }
  .variants ul {
    margin: 4px 0 0;
    padding-left: 16px;
  }
  .guidance {
    padding: 16px;
  }
  .guidance p {
    margin-block: 12px;
  }
</style>
