<script lang="ts">
  import Skeleton from "./Skeleton.svelte";
  let { workbench = false }: { workbench?: boolean } = $props();
</script>

{#if workbench}<div class="workbench">
    <div class="workbench-toolbar"><Skeleton count={1} /></div>
    <div class="workbench-panes">
      <section class="panel transcription-panel">
        <Skeleton shape="columns" count={8} />
      </section>
      <section class="panel facsimile-panel">
        <Skeleton shape="frame" count={1} />
      </section>
    </div>
    <div class="panel filmstrip"><Skeleton count={1} /></div>
  </div>
{:else}<div class="entry-screen scroll">
    <section class="panel entry-header"><Skeleton count={1} /></section>
    <section class="panel page-grid-panel">
      <h2>コマ一覧</h2>
      <Skeleton shape="cards" count={12} />
    </section>
  </div>{/if}
