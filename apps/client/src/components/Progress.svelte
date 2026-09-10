<script lang="ts">
  import { percent } from "../lib";
  let {
    done = 0,
    total = 0,
    caption = false,
  }: { done?: number; total?: number; caption?: boolean } = $props();
  let value = $derived(percent(done, total));
</script>

<div class="progress-line">
  <div
    class="progress"
    role="progressbar"
    aria-label="翻刻の進捗"
    aria-valuenow={value}
    aria-valuemin={0}
    aria-valuemax={100}
  >
    <span style:width={`${value}%`}></span>
  </div>
  {#if caption}<span class="numeric caption">{value.toFixed(1)}%</span>{/if}
</div>
