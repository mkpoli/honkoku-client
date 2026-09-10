<script lang="ts">
  let {
    region,
  }: { region: { slow: boolean; error: string; retry: () => void } } = $props();
</script>

{#if region.error || region.slow}<div
    class="region-notice caption"
    role="status"
  >
    {region.error || "応答に時間がかかっています。"}<button
      onclick={region.retry}>再試行</button
    >
  </div>{/if}
