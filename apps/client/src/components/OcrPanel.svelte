<script lang="ts">
  import { onMount } from "svelte";
  import type {
    Page,
    LocalOcrPage,
    OcrStatus,
    OcrProgress,
  } from "@honkoku/client-api/types";
  import {
    ocrStatus,
    ocrSetup,
    ocrRunPage,
    ocrResult,
    ocrCancel,
    ocrPublishPage,
    onOcrProgress,
  } from "@honkoku/client-api/invoke";
  import { errorMessage } from "../lib";
  let {
    page,
    editing,
    disabled,
    onresult,
    oninsert,
    oninsertall,
  }: {
    page: Page;
    editing: boolean;
    disabled: boolean;
    onresult: (result: LocalOcrPage | null) => void;
    oninsert: (text: string) => void;
    oninsertall: (text: string) => void;
  } = $props();
  let local = $state<LocalOcrPage | null>(null);
  let environment = $state<OcrStatus>();
  let gpu = $state(false),
    running = $state(false),
    settingUp = $state(false),
    publishing = $state(false);
  let confirm = $state(false),
    message = $state("");
  let progress = $state<OcrProgress>();
  let shown = $derived(local ?? page.ocr?.minna ?? page.ocr?.ndl);
  let site = $derived(
    typeof shown === "object" && shown !== null
      ? (shown as Record<string, unknown>)
      : {},
  );
  let lines = $derived(
    local
      ? [...local.lines]
          .sort((a, b) => a.reading_order - b.reading_order)
          .map((l) => ({ text: l.koji, confidence: l.confidence }))
      : Array.isArray(site.lines)
        ? [...site.lines]
            .sort((a, b) => Number(a.readingOrder) - Number(b.readingOrder))
            .map((l) => ({
              text: String(l.raw ?? "").replace(/<[^>]*>/g, ""),
              confidence: Number(l.confidence ?? 0),
            }))
        : [],
  );
  let allText = $derived(
    local
      ? lines.map((l) => l.text).join("\n")
      : typeof shown === "string"
        ? shown
        : String(site.text ?? ""),
  );
  let stamp = $derived(local?.created_at ?? site.createdAt);
  function date(value: unknown) {
    if (typeof value !== "string" || !Number.isFinite(Date.parse(value)))
      return "";
    return new Intl.DateTimeFormat("sv-SE", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    }).format(new Date(value));
  }
  onMount(() => {
    let alive = true;
    let stop: (() => void) | undefined;
    void onOcrProgress((p) => {
      if (alive && (running || settingUp)) progress = p;
    }).then((fn) => {
      if (alive) stop = fn;
      else fn();
    });
    void ocrStatus()
      .then((s) => {
        if (alive) {
          environment = s;
          gpu = s.cuda_available;
        }
      })
      .catch((e) => {
        if (alive) message = errorMessage(e);
      });
    return () => {
      alive = false;
      stop?.();
    };
  });
  $effect(() => {
    const entryId = page.entryId,
      index = page.index;
    let alive = true;
    local = null;
    confirm = false;
    message = "";
    onresult(null);
    void ocrResult(entryId, index)
      .then((result) => {
        if (alive) {
          local = result;
          onresult(result);
        }
      })
      .catch((e) => {
        if (alive) message = errorMessage(e);
      });
    return () => {
      alive = false;
    };
  });
  async function setup() {
    settingUp = true;
    message = "";
    progress = undefined;
    try {
      environment = await ocrSetup(gpu);
    } catch (e) {
      message = errorMessage(e);
    } finally {
      settingUp = false;
    }
  }
  async function run() {
    const entryId = page.entryId,
      index = page.index;
    running = true;
    message = "";
    progress = undefined;
    try {
      const result = await ocrRunPage(entryId, index);
      if (page.entryId === entryId && page.index === index) {
        local = result;
        onresult(result);
      }
    } catch (e) {
      message = errorMessage(e);
    } finally {
      running = false;
    }
  }
  async function publish() {
    publishing = true;
    message = "";
    confirm = false;
    try {
      await ocrPublishPage(page.entryId, page.index);
      message = "翻刻サイトに保存しました。";
    } catch (e) {
      message = errorMessage(e);
    } finally {
      publishing = false;
    }
  }
</script>

<section class="panel ocr-panel" aria-label="OCR">
  <div class="ocr-header">
    <div>
      <h2>OCR</h2>
      <span class="engine"
        >{local ? `ローカルOCR · ${local.model}` : "サイトのOCR"}{date(stamp)
          ? ` · ${date(stamp)}`
          : ""}</span
      >
    </div>
    <div class="actions">
      {#if editing && allText}<button
          {disabled}
          onclick={() => oninsertall(allText)}>本文に挿入</button
        >{/if}
      {#if running}<button
          onclick={() =>
            void ocrCancel().catch((e) => (message = errorMessage(e)))}
          >中止</button
        >{:else}<button
          disabled={!environment?.environment_ready ||
            !environment.models_ready ||
            settingUp}
          onclick={run}>ローカルOCRを実行</button
        >{/if}
      {#if local}<button
          disabled={publishing || running}
          onclick={() => (confirm = true)}>サイトに保存</button
        >{/if}
    </div>
  </div>
  {#if !environment?.environment_ready || !environment.models_ready}
    <div class="setup">
      <span class="muted">OCR環境とモデルの準備が必要です。</span><label
        ><input
          type="checkbox"
          bind:checked={gpu}
          disabled={!environment?.cuda_available || settingUp}
        />GPUを使う</label
      ><button disabled={settingUp || !environment} onclick={setup}>準備</button
      >
    </div>
  {/if}
  {#if settingUp || running}<div class="progress" role="status">
      {settingUp
        ? "環境を作成 → モデルを取得（289MB） → 完了"
        : "文字を認識"}<progress
        max={progress?.total || 1}
        value={progress?.total ? progress.done : undefined}
      ></progress><span>{progress?.message ?? "開始しています。"}</span>
    </div>{/if}
  {#if message}<p role="status">{message}</p>{/if}
  {#if confirm}<div
      class="publish-confirm"
      role="alertdialog"
      aria-label="翻刻サイトにOCR結果を保存しますか"
    >
      <strong>翻刻サイトにOCR結果を保存しますか</strong>
      <p>保存した結果は他のユーザーにも表示されます。</p>
      <button onclick={publish}>保存する</button><button
        onclick={() => (confirm = false)}>戻る</button
      >
    </div>{/if}
  <div class="ocr-lines scroll">
    {#each lines as line, i}<div class="ocr-line">
        <span class="line-number numeric">{i + 1}</span><span class="line-text"
          >{line.text}</span
        ><span
          class="confidence numeric"
          style:color={line.confidence > 0.9
            ? "var(--status-completed)"
            : line.confidence < 0.7
              ? "var(--accent)"
              : "var(--text-muted)"}>{Math.round(line.confidence * 100)}%</span
        >{#if editing}<button {disabled} onclick={() => oninsert(line.text)}
            >挿入</button
          >{/if}
      </div>
    {:else}{#if allText}<pre>{allText}</pre>{:else}<p class="muted">
          OCRの記録はありません。
        </p>{/if}{/each}
    {#each local?.warnings ?? [] as warning}<p class="muted">
        {warning}
      </p>{/each}
  </div>
</section>

<style>
  .ocr-panel {
    position: relative;
    min-height: 0;
  }
  .ocr-header,
  .actions,
  .setup {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .ocr-header {
    justify-content: space-between;
    gap: 6px;
    margin-bottom: 8px;
  }
  .ocr-header h2 {
    display: inline;
    margin-inline-end: 8px;
  }
  .engine,
  .setup,
  .progress {
    font-size: 12px;
    color: var(--text-muted);
  }
  button {
    font-size: 12px;
    white-space: nowrap;
  }
  .ocr-lines {
    min-height: 0;
    flex: 1;
  }
  .ocr-line {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding-block: 5px;
    border-bottom: 1px solid var(--border);
  }
  .line-number {
    color: var(--text-muted);
    font-size: 11px;
    min-width: 18px;
  }
  .line-text {
    flex: 1;
    font-family: var(--font-serif);
    font-size: 15px;
    overflow-wrap: anywhere;
  }
  .confidence {
    font-size: 11px;
  }
  .setup label {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .progress {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .progress span {
    overflow-wrap: anywhere;
  }
  progress {
    width: 90px;
  }
  .publish-confirm {
    position: absolute;
    inset: 0;
    background: var(--surface);
    padding: 12px;
    z-index: 2;
    overflow: auto;
  }
  pre {
    white-space: pre-wrap;
    font-family: var(--font-serif);
  }
</style>
