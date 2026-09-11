<script lang="ts">
  import { Region } from "../region.svelte";
  import Skeleton from "./Skeleton.svelte";
  import RegionNotice from "./RegionNotice.svelte";
  import { onMount } from "svelte";
  import type {
    Page,
    LocalOcrPage,
    OcrStatus,
    OcrProgress,
    OcrDiagnostics,
  } from "@honkoku/client-api/types";
  import {
    ocrDiagnostics,
    ocrDoctor,
    ocrRepairModels,
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
  const diagnostics = new Region<OcrDiagnostics>();
  const results = new Region<LocalOcrPage | null>();
  const doctor = new Region<string>();
  let environment = $derived(diagnostics.value?.status);
  let lastError = $state("");
  let copyNotice = $state("");
  const captureError = (error: unknown) => {
    lastError = (
      error && typeof error === "object" && "message" in error
        ? String(error.message)
        : String(error)
    ).replace(/\/home\/[^/\s]+/g, "~");
  };
  function refreshDiagnostics() {
    return diagnostics.load("ocr-diagnostics", ocrDiagnostics, (value) => {
      gpu = value.status?.cuda_available ?? false;
      if (value.last_error) lastError = value.last_error;
    });
  }
  async function copyError() {
    try {
      await navigator.clipboard.writeText(lastError);
      copyNotice = "コピーしました。";
    } catch {
      copyNotice = "本文を選択してコピーしてください。";
    }
  }

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
    void refreshDiagnostics();
    return () => {
      alive = false;
      stop?.();
      diagnostics.cancel();
      results.cancel();
      doctor.cancel();
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
    void results.load(
      `ocr-result:${entryId}:${index}`,
      () => ocrResult(entryId, index),
      (result) => {
        if (alive) {
          local = result;
          onresult(result);
        }
      },
    );
    return () => {
      alive = false;
      results.cancel();
    };
  });
  async function setup() {
    settingUp = true;
    message = "";
    progress = undefined;
    try {
      await ocrSetup(gpu);
      await refreshDiagnostics();
    } catch (e) {
      captureError(e);
    } finally {
      settingUp = false;
    }
  }
  async function repair() {
    settingUp = true;
    try {
      await ocrRepairModels();
      await refreshDiagnostics();
    } catch (e) {
      captureError(e);
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
      captureError(e);
    } finally {
      void refreshDiagnostics();
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
      captureError(e);
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
        >{local ? `ローカルOCR・${local.model}` : "サイトのOCR"}{date(stamp)
          ? `・${date(stamp)}`
          : ""}</span
      >
    </div>
    <div class="actions">
      {#if editing && allText}<button
          {disabled}
          onclick={() => oninsertall(allText)}>本文に挿入</button
        >{/if}
      {#if running}<button
          onclick={() => void ocrCancel().catch((e) => captureError(e))}
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
  {#if diagnostics.value && (!environment?.environment_ready || !environment.models_ready)}
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
  <div class="ocr-content scroll">
    <section class="ocr-diagnostics" aria-label="OCR診断">
      <h3>診断</h3>
      <RegionNotice region={diagnostics} />
      {#if !diagnostics.value && diagnostics.pending}<Skeleton
          count={2}
        />{:else if diagnostics.value}
        <dl>
          <div>
            <dt>環境</dt>
            <dd>{diagnostics.value.environment_ready ? "あり" : "なし"}</dd>
          </div>
          <div>
            <dt>モデル</dt>
            <dd>
              {diagnostics.value.models_present ? "あり" : "なし"}・{(
                diagnostics.value.models_bytes /
                1024 /
                1024
              ).toFixed(1)}MB・{environment?.models_ready
                ? "検証済み"
                : "未確認"}
            </dd>
          </div>
          <div>
            <dt>デバイス</dt>
            <dd>
              {environment
                ? environment.device.toLowerCase() === "cuda"
                  ? "CUDA"
                  : environment.device.toUpperCase()
                : "未確認"}{#if environment?.cuda_error}<span
                  class="caption muted ocr-cuda-error"
                  >CUDAを使えません：{environment.cuda_error}</span
                >{/if}
            </dd>
          </div>
        </dl>
        <p class="caption muted">
          ログ：<code>{diagnostics.value.log_path}</code>
        </p>
        {#if diagnostics.value.models_directory_exists && !environment?.models_ready}<button
            disabled={settingUp || running}
            onclick={repair}>モデルを再取得</button
          >{/if}
      {/if}
      {#if lastError}<label class="ocr-last-error"
          >最後のエラー<textarea
            readonly
            value={lastError}
            aria-label="最後のエラー"></textarea></label
        ><button onclick={copyError}>エラーをコピー</button><span
          class="caption"
          role="status">{copyNotice}</span
        >{/if}
      <button
        disabled={doctor.pending}
        onclick={() =>
          doctor.load("ocr-doctor", async () => {
            try {
              return await ocrDoctor();
            } catch (e) {
              captureError(e);
              throw e;
            }
          })}>診断を実行</button
      >
      <RegionNotice region={doctor} />
      {#if doctor.pending && !doctor.value}<Skeleton count={2} />{/if}
      {#if doctor.value}<textarea
          class="doctor-report"
          readonly
          value={doctor.value}
          aria-label="診断結果"></textarea>{/if}
    </section>
    <RegionNotice region={results} />
    <div class="ocr-lines">
      {#if results.pending && results.value === undefined && !shown}<Skeleton
          count={5}
        />{/if}
      {#each lines as line, i}<div class="ocr-line">
          <span class="line-number numeric">{i + 1}</span><span
            class="line-text">{line.text}</span
          ><span
            class="confidence numeric"
            style:color={line.confidence > 0.9
              ? "var(--status-completed)"
              : line.confidence < 0.7
                ? "var(--accent)"
                : "var(--text-muted)"}
            >{Math.round(line.confidence * 100)}%</span
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
  .ocr-content {
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
  .ocr-diagnostics {
    padding-bottom: 12px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 8px;
  }
  .ocr-diagnostics h3 {
    font: 500 13px/22px var(--font-sans);
  }
  dl {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
    margin: 4px 0;
    font-size: 12px;
  }
  dl > div {
    display: flex;
    gap: 4px;
  }
  dd {
    margin: 0;
  }
  textarea {
    display: block;
    width: 100%;
    min-height: 70px;
    background: var(--surface-inset);
    color: var(--text);
    border: 1px solid var(--border);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .doctor-report {
    min-height: 180px;
  }
  .ocr-last-error {
    font-size: 12px;
  }
  pre {
    white-space: pre-wrap;
    font-family: var(--font-serif);
  }
</style>
