<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Canvas, Entry, Page } from "@honkoku/client-api/types";
  import { parseGroups as parse, renderInline } from "@honkoku/markup";
  import { date, notes, status, statusClass, user } from "../lib";
  import { href, parseRoute } from "../routes";
  import Facsimile from "./Facsimile.svelte";
  import Thumbnail from "./Thumbnail.svelte";
  let {
    entry,
    pages,
    canvases,
    index,
  }: { entry: Entry; pages: Page[]; canvases: Canvas[]; index: number } =
    $props();
  let page = $derived(pages.find((p) => p.index === index)!);
  let groups = $derived(parse(page.text));
  let pageNotes = $derived(notes(page));
  let swapped = $state(false),
    horizontal = $state(false),
    half = $state(""),
    expanded = $state(false),
    noteIndex = $state<number | null>(null),
    authors = $state<Record<string, string>>({});
  let strip: HTMLDivElement;
  let notePanel: HTMLElement;
  const ocrText = (v: Page["ocr"]) => ({
    minna: typeof v?.minna === "string" ? v.minna : v?.minna?.text,
    ndl: typeof v?.ndl === "string" ? v.ndl : v?.ndl?.text,
  });
  let ocr = $derived(ocrText(page.ocr));
  $effect(() => {
    index;
    noteIndex = null;
    half = "";
  });
  $effect(() => {
    const values = pageNotes;
    let cancelled = false;
    for (const n of values)
      if (n?.createdBy)
        void user(n.createdBy)
          .then((u) => {
            if (!cancelled) authors = { ...authors, [u.uid]: u.displayName };
          })
          .catch(() => {
            if (!cancelled && n?.createdBy)
              authors = { ...authors, [n.createdBy]: "名前不明" };
          });
    return () => {
      cancelled = true;
    };
  });
  $effect(() => {
    index;
    if (expanded)
      void tick().then(() =>
        strip?.querySelector('[aria-current="page"]')?.scrollIntoView({
          block: "nearest",
          inline: "center",
          behavior: "instant",
        }),
      );
  });
  function go(next: number) {
    if (next >= 0 && next < pages.length)
      location.hash = href({ entryId: entry.id, pageIndex: next });
  }
  function references(element: HTMLElement) {
    const click = (event: MouseEvent) => {
      const target = (event.target as HTMLElement).closest<HTMLButtonElement>(
        "[data-note]",
      );
      if (target) {
        noteIndex = Number(target.dataset.note) - 1;
        void tick().then(() =>
          notePanel
            .querySelector(`[data-note-index="${noteIndex}"]`)
            ?.scrollIntoView({ block: "nearest" }),
        );
      }
    };
    element.addEventListener("click", click);
    return { destroy: () => element.removeEventListener("click", click) };
  }
  function measureColumns(element: HTMLElement) {
    const apply = () => {
      const available = element.clientHeight;
      if (available > 0)
        element.style.setProperty("--column-height", `${available}px`);
    };
    const observer = new ResizeObserver(apply);
    observer.observe(element);
    if (element.parentElement) observer.observe(element.parentElement);
    apply();
    return { destroy: () => observer.disconnect() };
  }
  onMount(() => {
    const keys = (e: KeyboardEvent) => {
      if (
        e.altKey ||
        e.ctrlKey ||
        e.metaKey ||
        e.shiftKey ||
        (e.target instanceof HTMLElement &&
          e.target.closest('input,textarea,select,[contenteditable="true"]'))
      )
        return;
      const current = parseRoute(location.hash).pageIndex ?? index;
      const next = (
        {
          ArrowLeft: current - 1,
          ArrowRight: current + 1,
          Home: 0,
          End: pages.length - 1,
        } as Record<string, number>
      )[e.key];
      if (next !== undefined) {
        e.preventDefault();
        go(next);
      }
    };
    window.addEventListener("keydown", keys);
    return () => window.removeEventListener("keydown", keys);
  });
</script>

<div class="workbench">
  <div class="workbench-toolbar">
    <div class="page-position">
      <button
        disabled={index === 0}
        onclick={() => go(index - 1)}
        aria-label="前のコマ">‹</button
      ><strong>{index + 1}／{entry.size}コマ</strong><button
        disabled={index === pages.length - 1}
        onclick={() => go(index + 1)}
        aria-label="次のコマ">›</button
      ><span class="status {statusClass(page.status)}"
        >{status(page.status).symbol}{status(page.status).label}</span
      ><span class="caption muted">閲覧のみ</span>
    </div>
    <div class="toolbar-actions">
      <button onclick={() => (swapped = !swapped)}>⇄左右を入れ替え</button
      ><button
        aria-pressed={!horizontal}
        onclick={() => (horizontal = !horizontal)}
        >{horizontal ? "横書き" : "縦書き"}⌄</button
      >
    </div>
  </div>
  <div class="workbench-panes" class:swapped>
    <section class="panel transcription-panel">
      <div class="pane-toolbar">
        <h2>翻刻</h2>
        <span class="caption muted"
          >{horizontal ? "横書き" : "縦書き"}{half ? `・${half}` : ""}</span
        >
      </div>
      <div class="transcription" class:horizontal use:references>
        {#each groups as g, i}<section
            class="column-group"
            class:half-selected={half === g.label && !!g.label}
            aria-label={g.label || `本文${i + 1}`}
          >
            {#if g.label}<button
                class="column-label"
                class:active={half === g.label}
                onclick={() => (half = half === g.label ? "" : g.label)}
                >{g.label}</button
              >{/if}
            <div class="columns" use:measureColumns>
              {#each g.columns as column}<div class="transcription-column">
                  {@html renderInline(column)}
                </div>{/each}
            </div>
          </section>{:else}<p class="empty">本文はありません。</p>{/each}
      </div>
    </section>
    <Facsimile canvas={canvases[index]} pageNumber={index + 1} bind:half />
  </div>
  <div class="workbench-supplement">
    <section class="panel ocr-panel">
      <h2>OCR</h2>
      <div class="ocr-columns scroll">
        {#if ocr.minna}<div>
            <h3>みんなで翻刻</h3>
            <pre>{ocr.minna}</pre>
          </div>{/if}{#if ocr.ndl}<div>
            <h3>国立国会図書館</h3>
            <pre>{ocr.ndl}</pre>
          </div>{/if}{#if !ocr.minna && !ocr.ndl}<p class="muted">
            OCRの記録はありません。
          </p>{/if}
      </div>
    </section>
    <section class="panel notes-panel" bind:this={notePanel}>
      <h2>注記</h2>
      <div class="scroll">
        {#each pageNotes as n, i}{#if n}<article
              class="note"
              class:highlighted={noteIndex === i}
              data-note-index={i}
            >
              <strong>＃{i + 1}</strong>
              <p>{n.content}</p>
              <div class="caption muted">
                {n.createdBy
                  ? (authors[n.createdBy] ?? "名前を確認中")
                  : "名前不明"}・{date(n.createdAt)}
              </div>
            </article>{/if}{/each}{#if !pageNotes.some(Boolean)}<p
            class="muted"
          >
            注記はありません。
          </p>{/if}{#if noteIndex !== null && !pageNotes[noteIndex]}<p
            class="muted"
            role="status"
          >
            この番号の注記はありません。
          </p>{/if}
      </div>
    </section>
  </div>
  <nav
    class="panel filmstrip"
    class:expanded
    aria-label="コマを選択"
    onmouseenter={() => (expanded = true)}
    onmouseleave={(e) => {
      if (!e.currentTarget.contains(document.activeElement)) expanded = false;
    }}
    onfocusin={() => (expanded = true)}
    onfocusout={(e) => {
      if (!e.currentTarget.contains(e.relatedTarget as Node)) expanded = false;
    }}
  >
    <div class="filmstrip-heading">
      <strong>コマ</strong><span class="status completed caption">✓完了</span
      ><span class="status initiated caption">◐翻刻中</span><span
        class="status default caption">○未着手</span
      ><span class="caption muted">←→で移動</span>
    </div>
    <div class="status-strip">
      {#each pages as p (p.id)}<a
          class={statusClass(p.status)}
          class:current={p.index === index}
          href={href({ entryId: entry.id, pageIndex: p.index })}
          aria-label={`コマ${p.index + 1}・${status(p.status).label}`}
          aria-current={p.index === index ? "page" : undefined}
        ></a>{/each}
    </div>
    <div class="filmstrip-thumbnails" bind:this={strip} inert={!expanded}>
      {#each pages as p (p.id)}<a
          href={href({ entryId: entry.id, pageIndex: p.index })}
          class:current={p.index === index}
          aria-current={p.index === index ? "page" : undefined}
          ><Thumbnail
            url={expanded ? canvases[p.index]?.thumbnailUrl : null}
          /><span
            >{p.index + 1}・<span class="status {statusClass(p.status)}"
              >{status(p.status).symbol}{status(p.status).label}</span
            ></span
          ></a
        >{/each}
    </div>
  </nav>
</div>
