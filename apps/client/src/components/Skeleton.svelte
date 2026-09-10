<script lang="ts">
  let {
    count = 4,
    shape = "rows",
    label = "データを取得中",
  }: {
    count?: number;
    shape?: "rows" | "cards" | "columns" | "frame";
    label?: string;
  } = $props();
</script>

<div
  class="skeleton"
  class:skeleton-cards={shape === "cards"}
  class:skeleton-columns={shape === "columns"}
  class:skeleton-frame={shape === "frame"}
  aria-busy="true"
  aria-label={label}
>
  {#each Array(count) as _, i}<div class="skeleton-item" aria-hidden="true">
      <span style:width={`${78 - (i % 3) * 14}%`}></span><span></span><span
        style:width="45%"
      ></span>
    </div>{/each}
</div>
