<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { SavedPage } from "@honkoku/client-api/types";
  import { saveBonus, savePoints, shareUrl } from "../save-result";
  let {
    saved,
    entryLabel,
    onclose,
  }: {
    saved: SavedPage;
    entryLabel: string;
    onclose: () => void;
  } = $props();
  let dialog: HTMLDialogElement;
  const completed = $derived(saved.page.status === "completed");
  const points = $derived(savePoints(saved));
  const bonus = $derived(saveBonus(saved));
  let shown = $state(0);
  onMount(() => {
    dialog.showModal();
    const target = saved.count;
    if (matchMedia("(prefers-reduced-motion: reduce)").matches || !target) {
      shown = target;
      return;
    }
    const start = performance.now();
    let frame = requestAnimationFrame(function step(now) {
      const t = Math.min(1, (now - start) / 700);
      shown = Math.round(target * (1 - (1 - t) ** 3));
      if (t < 1) frame = requestAnimationFrame(step);
    });
    return () => cancelAnimationFrame(frame);
  });
  function share() {
    void openUrl(
      shareUrl(entryLabel, saved.page.entryId, saved.page.index, saved.count),
    ).catch(() => {});
  }
</script>

<dialog
  bind:this={dialog}
  class="save-result"
  class:completed
  {onclose}
  aria-labelledby="save-result-title"
>
  <button class="close" aria-label="閉じる" onclick={() => dialog.close()}
    >×</button
  >
  <p class="kicker">{completed ? "翻刻完了" : "保存完了"}</p>
  <h2 id="save-result-title">おつかれさま！</h2>
  <p class="where">『{entryLabel}』コマ{saved.page.index + 1}</p>
  <div class="tally">
    <span class="spark" aria-hidden="true"></span>
    <span class="spark" aria-hidden="true"></span>
    <span class="spark" aria-hidden="true"></span>
    <p class="chars">
      <span class="figure" aria-hidden="true"
        >{shown.toLocaleString("ja-JP")}</span
      ><span class="visually-hidden">{saved.count.toLocaleString("ja-JP")}</span
      ><span class="unit">文字</span>
    </p>
    <p class="caption">今回翻刻した文字数</p>
  </div>
  <p class="points">
    {points.toLocaleString("ja-JP")}ポイント獲得しました！
  </p>
  {#if bonus}<p class="bonus caption">
      翻刻文を{bonus}した場合はポイントが増えます
    </p>{/if}
  <div class="actions">
    <button onclick={share}>Xでシェアする</button>
    <!-- svelte-ignore a11y_autofocus -->
    <button class="primary" autofocus onclick={() => dialog.close()}
      >閉じる</button
    >
  </div>
</dialog>

<style>
  dialog {
    position: fixed;
    width: min(380px, calc(100vw - 48px));
    padding: 28px 28px 24px;
    border: 1px solid var(--border);
    border-radius: 12px;
    color: var(--text);
    background: var(--surface);
    box-shadow: 0 16px 64px #0003;
    text-align: center;
  }
  dialog[open] {
    animation: arrive 320ms cubic-bezier(0.2, 0.9, 0.3, 1.2);
  }
  dialog::backdrop {
    background: #0005;
  }
  .close {
    position: absolute;
    top: 12px;
    right: 12px;
    border: 0;
    background: transparent;
    font-size: 22px;
    line-height: 1;
    padding: 4px 8px;
    color: var(--text-muted);
  }
  p {
    margin: 0;
  }
  .kicker {
    display: inline-block;
    padding: 2px 10px;
    border-radius: 999px;
    font-size: 12px;
    color: var(--accent);
    background: var(--accent-soft);
  }
  .completed .kicker {
    color: var(--surface);
    background: var(--gold);
  }
  h2 {
    margin: 12px 0 4px;
    font: 600 22px/1.4 var(--font-serif);
  }
  .where {
    font-size: 13px;
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
  .tally {
    position: relative;
    margin: 20px auto 16px;
    padding: 16px 0 14px;
    border-block: 1px solid var(--border);
  }
  .chars {
    font-family: var(--font-serif);
    color: var(--gold);
  }
  .figure {
    font-size: 56px;
    font-weight: 600;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    margin-left: 4px;
    font-size: 18px;
  }
  .caption {
    font-size: 12px;
    color: var(--text-muted);
  }
  .points {
    font-weight: 600;
    color: var(--accent);
    animation: rise 600ms 450ms ease-out both;
  }
  .bonus {
    margin-top: 4px;
  }
  .spark {
    position: absolute;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--gold);
    opacity: 0;
    animation: sparkle 1.8s ease-in-out infinite;
  }
  .spark:nth-child(1) {
    top: 14px;
    left: 22%;
  }
  .spark:nth-child(2) {
    top: 40%;
    right: 20%;
    width: 4px;
    height: 4px;
    animation-delay: 0.6s;
  }
  .spark:nth-child(3) {
    bottom: 28px;
    left: 28%;
    width: 3px;
    height: 3px;
    animation-delay: 1.2s;
  }
  .actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    margin-top: 24px;
  }
  .actions button {
    padding: 10px 16px;
  }
  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  @keyframes arrive {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.96);
    }
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
  @keyframes sparkle {
    0%,
    100% {
      opacity: 0;
      transform: scale(0);
    }
    50% {
      opacity: 1;
      transform: scale(1);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    dialog[open],
    .points,
    .spark {
      animation: none;
    }
  }
</style>
