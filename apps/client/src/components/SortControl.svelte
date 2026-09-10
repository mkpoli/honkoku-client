<script lang="ts">
  let {
    storageKey,
    extended = false,
    value = $bindable("platform"),
  }: {
    storageKey: string;
    extended?: boolean;
    value?: string;
  } = $props();
  let choices = $derived([
    ...(extended ? [["updated", "更新順"]] : [["platform", "表示順"]]),
    ["name", "名前順"],
    ["progress", "進捗順"],
    ...(extended ? [["size", "資料数順"]] : []),
  ]);
  $effect(() => {
    const key = storageKey;
    try {
      const saved =
        localStorage.getItem(key) ?? (extended ? "updated" : "platform");
      value =
        choices.some(([id]) => id === saved) || saved === "progress-desc"
          ? saved
          : extended
            ? "updated"
            : "platform";
    } catch {
      value = extended ? "updated" : "platform";
    }
  });
  function choose(id: string) {
    value = id === "progress" && value === "progress" ? "progress-desc" : id;
    try {
      localStorage.setItem(storageKey, value);
    } catch {}
  }
</script>

<div class="sort-control" role="group" aria-label="並び順の選択">
  {#each choices as [id, text]}
    <button
      aria-pressed={value === id ||
        (id === "progress" && value === "progress-desc")}
      onclick={() => choose(id)}
      >{text}{#if id === "progress" && value.startsWith("progress")}<span
          aria-label={value === "progress" ? "昇順" : "降順"}
          >{value === "progress" ? "↑" : "↓"}</span
        >{/if}</button
    >
  {/each}
</div>
