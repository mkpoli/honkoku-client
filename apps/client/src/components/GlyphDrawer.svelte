<script lang="ts">
  import Glyphs from "./Glyphs.svelte";
  let { character, onclose }: { character: string; onclose: () => void } =
    $props();
</script>

<aside class="glyph-drawer panel" aria-label="集字の引き出し">
  <header>
    <h2>集字<span>{character}</span></h2>
    <a href={`#/glyphs/${encodeURIComponent(character)}`}>すべて見る</a><button
      aria-label="集字を閉じる"
      onclick={onclose}>×</button
    >
  </header>
  <div class="drawer-content"><Glyphs {character} compact /></div>
</aside>

<style>
  .glyph-drawer {
    position: absolute;
    inset: 0 0 0 auto;
    width: min(440px, 100%);
    z-index: 6;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    border-bottom: 1px solid var(--border);
    padding: 12px;
  }
  h2 {
    font-size: 16px;
    margin: 0;
    flex: 1;
  }
  span {
    font: 28px var(--font-serif);
    margin-left: 12px;
  }
  a {
    font-size: 13px;
  }
  .drawer-content {
    flex: 1;
    overflow: auto;
  }
</style>
