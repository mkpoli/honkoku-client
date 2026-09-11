<script lang="ts">
  import { elements } from "@honkoku/markup";
  import Transcription from "./Transcription.svelte";
</script>

<article class="panel markup-help reference-page">
  <h1>特殊記法の解説</h1>
  <p class="muted">記法を入力すると、閲覧画面に組版した本文が表示されます。</p>
  <nav aria-label="特殊記法の一覧">
    {#each elements as element}<button
        onclick={() =>
          document
            .getElementById(`markup-${element.name}`)
            ?.scrollIntoView({ block: "start" })}>{element.name}</button
      >{/each}
  </nav>
  {#each elements as element}<section
      id={`markup-${element.name}`}
      class="markup-element"
      aria-label={element.name}
    >
      <div>
        <h2>{element.name}</h2>
        <p>{element.doc}</p>
        <pre>{element.example}</pre>
      </div>
      <div class="rendered-example">
        <Transcription source={element.example} />
      </div>
    </section>{/each}
</article>

<style>
  nav {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }
  .markup-element {
    display: grid;
    grid-template-columns: 1fr 300px;
    gap: 24px;
    border-top: 1px solid var(--border);
    padding: 24px 0;
  }
  .rendered-example {
    height: 220px;
    overflow: auto;
  }
  pre {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    background: var(--surface-inset);
    padding: 12px;
    border-radius: 7px;
  }
</style>
