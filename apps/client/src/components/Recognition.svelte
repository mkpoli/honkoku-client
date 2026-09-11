<script lang="ts">
  import OpenSeadragon from "openseadragon";
  import { onMount } from "svelte";
  import {
    recognizeRegion,
    glyphAttestations,
  } from "@honkoku/client-api/invoke";
  import type { Canvas, Prediction } from "@honkoku/client-api/types";
  import { clampRegion, glyphCards, type Rectangle } from "./glyph-regions";
  import GlyphCard from "./GlyphCard.svelte";
  import ExternalLink from "./ExternalLink.svelte";
  import { errorMessage } from "../lib";
  let {
    viewer,
    canvas,
    disabled,
    oninsert,
    selection,
  }: {
    viewer?: OpenSeadragon.Viewer;
    canvas?: Canvas;
    disabled: boolean;
    oninsert: (character: string) => void;
    selection?: Rectangle;
  } = $props();
  let candidates = $state<Prediction[]>([]);
  let cards = $state<ReturnType<typeof glyphCards>>([]);
  let rect = $state<Rectangle>();
  let loading = $state(false),
    error = $state("");
  let generation = 0;
  function clear() {
    generation++;
    candidates = [];
    cards = [];
    error = "";
    loading = false;
  }
  async function recognize(region: Rectangle) {
    if (!canvas?.infoJsonUrl) return;
    rect = region;
    const version = ++generation;
    candidates = [];
    cards = [];
    error = "";
    loading = true;
    try {
      const result = await recognizeRegion(canvas.infoJsonUrl, region);
      if (version !== generation) return;
      candidates = result;
      if (!result.length) error = "候補が見つかりませんでした。";
      const top = result[0]?.character;
      if (top)
        void glyphAttestations(top, null, 20)
          .then((result) => {
            if (version === generation)
              cards = result.pages
                .flatMap(glyphCards)
                .filter((c) => c.crop)
                .slice(0, 4);
          })
          .catch(() => {});
    } catch (e) {
      if (version === generation) error = errorMessage(e);
    } finally {
      if (version === generation) loading = false;
    }
  }
  $effect(() => {
    if (selection) void recognize(selection);
  });
  $effect(() => {
    const v = viewer,
      size = canvas;
    if (!v || !size || !v.world.getItemCount()) return;
    const item = v.world.getItemAt(0),
      overlay = document.createElement("div");
    overlay.className = "recognition-rectangle";
    Object.assign(overlay.style, {
      border: "2px solid var(--accent)",
      background: "rgba(217,89,54,.08)",
      pointerEvents: "none",
    });
    let start: OpenSeadragon.Point | undefined,
      added = false;
    const nav = (
      v as OpenSeadragon.Viewer & { innerTracker: OpenSeadragon.MouseTracker }
    ).innerTracker.isTracking();
    v.setMouseNavEnabled(false);
    const draw = (region: Rectangle) => {
      const bounds = item.imageToViewportRectangle(...region);
      if (added) v.updateOverlay(overlay, bounds);
      else {
        v.addOverlay({ element: overlay, location: bounds });
        added = true;
      }
    };
    const regionAt = (end: OpenSeadragon.Point) => {
      if (!start) return;
      const a = item.viewerElementToImageCoordinates(start),
        b = item.viewerElementToImageCoordinates(end);
      const region = clampRegion(
        [
          Math.min(a.x, b.x),
          Math.min(a.y, b.y),
          Math.abs(a.x - b.x),
          Math.abs(a.y - b.y),
        ],
        size,
      );
      draw(region);
      return region;
    };
    const tracker = new OpenSeadragon.MouseTracker({
      element: v.canvas,
      pressHandler: (e) => {
        start = e.position;
        overlay.style.opacity = "1";
      },
      dragHandler: (e) => {
        regionAt(e.position);
      },
      releaseHandler: (e) => {
        const region = regionAt(e.position);
        start = undefined;
        overlay.style.opacity = ".5";
        if (region && region[2] >= 2 && region[3] >= 2) void recognize(region);
      },
    });
    if (selection) draw(selection);
    return () => {
      tracker.destroy();
      v.setMouseNavEnabled(nav);
      v.removeOverlay(overlay);
      overlay.remove();
    };
  });
  onMount(() => {
    const key = (event: KeyboardEvent) => {
      if (
        event.isComposing ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey ||
        event.shiftKey
      )
        return;
      if (event.key === "Escape") {
        clear();
        event.preventDefault();
        return;
      }
      if (
        disabled ||
        !/^[0-9]$/.test(event.key) ||
        (event.target instanceof HTMLElement &&
          event.target.closest(
            "input,textarea:not(.editor-raw-textarea),select,.save-popover",
          ))
      )
        return;
      const candidate =
        candidates[event.key === "0" ? 9 : Number(event.key) - 1];
      if (candidate) {
        event.preventDefault();
        event.stopImmediatePropagation();
        oninsert(candidate.character);
      }
    };
    window.addEventListener("keydown", key, true);
    return () => {
      generation++;
      window.removeEventListener("keydown", key, true);
    };
  });
</script>

<aside class="recognition-strip" aria-label="文字認識" aria-busy={loading}>
  <div class="recognition-heading">
    <span class="caption"
      >{loading ? "認識中…" : "原本をドラッグして文字を認識"}</span
    >
    {#if rect}<button disabled={loading} onclick={() => rect && recognize(rect)}
        >再認識</button
      >{/if}
  </div>
  {#if error}<div class="recognition-error" role="alert">
      {error}<button onclick={() => rect && recognize(rect)}>再試行</button>
    </div>{/if}
  {#if candidates.length}<div class="recognition-results">
      <div class="candidates">
        {#each candidates as candidate, i}<div class="candidate">
            <button
              {disabled}
              aria-label={`${(i + 1) % 10}：${candidate.character}を挿入`}
              onmousedown={(e) => e.preventDefault()}
              onclick={() => oninsert(candidate.character)}
              ><span class="glyph">{candidate.character}</span><span
                class="caption">{(i + 1) % 10}</span
              ></button
            >
            <div
              class="probability"
              title={`${Math.round(candidate.probability * 100)}%`}
            >
              <span style:width={`${candidate.probability * 100}%`}></span>
            </div>
            <ExternalLink
              href={`https://codh.rois.ac.jp/char-shape/unicode/U+${candidate.character.codePointAt(0)!.toString(16).toUpperCase().padStart(4, "0")}/`}
              ><span aria-label={`${candidate.character}のCODH字形`}>字形↗</span
              ></ExternalLink
            >
          </div>{/each}
      </div>
      {#if cards.length}<div
          class="comparisons"
          aria-label={`${candidates[0].character}の集字`}
        >
          {#each cards as card (card.key)}<GlyphCard {card} />{/each}
        </div>{/if}
    </div>{/if}
</aside>

<style>
  .recognition-strip {
    border-bottom: 1px solid var(--border);
    padding: 8px 12px;
    background: var(--surface);
    max-height: 42%;
    overflow: auto;
    flex-shrink: 0;
  }
  .recognition-heading,
  .recognition-error {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-size: 13px;
  }
  .recognition-heading button {
    font-size: 12px;
  }
  .recognition-results {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding-top: 8px;
  }
  .candidates {
    display: grid;
    grid-template-columns: repeat(10, minmax(32px, 1fr));
    gap: 4px;
    flex: 1 1 360px;
  }
  .candidate {
    min-width: 0;
    text-align: center;
  }
  .candidate button {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    padding: 4px 0;
  }
  .glyph {
    font: 32px var(--font-serif);
    line-height: 1.35;
  }
  .probability {
    height: 2px;
    background: var(--border);
    margin: 4px 0;
  }
  .probability span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .candidate :global(a) {
    font-size: 10px;
  }
  .comparisons {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 4px;
    width: 272px;
    flex: 0 0 272px;
  }
  .comparisons :global(.glyph-crop) {
    width: 40px;
    height: 64px;
  }
  .comparisons :global(.caption) {
    display: none;
  }
</style>
