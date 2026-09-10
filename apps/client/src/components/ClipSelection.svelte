<script lang="ts">
  import OpenSeadragon from "openseadragon";
  import { clipCreate } from "@honkoku/client-api/invoke";
  import type { Canvas } from "@honkoku/client-api/types";
  import { clampRegion, type Rectangle } from "./glyph-regions";
  import { errorMessage } from "../lib";
  let {
    viewer,
    canvas,
    entryId,
    index,
    onclose,
    onsaved,
  }: {
    viewer?: OpenSeadragon.Viewer;
    canvas?: Canvas;
    entryId: string;
    index: number;
    onclose: () => void;
    onsaved: () => void;
  } = $props();
  let rect = $state<Rectangle>(),
    position = $state({ x: 16, y: 48 });
  let reading = $state(""),
    tags = $state(""),
    comment = $state(""),
    isPrivate = $state(false);
  let saving = $state(false),
    error = $state("");
  let form = $state<HTMLFormElement>();
  $effect(() => {
    const v = viewer,
      size = canvas;
    if (!v || !size || !v.world.getItemCount()) return;
    const item = v.world.getItemAt(0);
    const overlay = document.createElement("div");
    overlay.className = "clip-selection-rectangle";
    Object.assign(overlay.style, {
      border: "2px solid var(--accent)",
      background: "rgba(217,89,54,.12)",
      pointerEvents: "none",
    });
    let start: OpenSeadragon.Point | undefined,
      added = false;
    const oldNav = (
      v as OpenSeadragon.Viewer & { innerTracker: OpenSeadragon.MouseTracker }
    ).innerTracker.isTracking();
    v.setMouseNavEnabled(false);
    function draw(end: OpenSeadragon.Point) {
      if (!start || !size) return;
      const a = item.viewerElementToImageCoordinates(start),
        b = item.viewerElementToImageCoordinates(end);
      const region = clampRegion(
        [
          Math.round(Math.min(a.x, b.x)),
          Math.round(Math.min(a.y, b.y)),
          Math.round(Math.abs(b.x - a.x)),
          Math.round(Math.abs(b.y - a.y)),
        ],
        size,
      );
      const bounds = item.imageToViewportRectangle(...region);
      if (!added) {
        v!.addOverlay({ element: overlay, location: bounds });
        added = true;
      } else v!.updateOverlay(overlay, bounds);
      return region;
    }
    function anchor() {
      if (!rect || !v) return;
      const bounds = item.imageToViewportRectangle(...rect);
      const p = v.viewport.pixelFromPoint(bounds.getTopRight(), true);
      const container = v.viewport.getContainerSize();
      position = {
        x: Math.max(8, Math.min(p.x + 8, container.x - 288)),
        y: Math.max(8, Math.min(p.y, container.y - 340)),
      };
    }
    const tracker = new OpenSeadragon.MouseTracker({
      element: v.canvas,
      pressHandler: (event) => {
        if (saving || rect) return;
        start = event.position;
      },
      dragHandler: (event) => {
        if (start) draw(event.position);
      },
      releaseHandler: (event) => {
        if (!start) return;
        const value = draw(event.position);
        start = undefined;
        if (value && value[2] >= 2 && value[3] >= 2) {
          rect = value;
          anchor();
          requestAnimationFrame(() => form?.querySelector("input")?.focus());
        }
      },
    });
    v.addHandler("animation", anchor);
    v.addHandler("resize", anchor);
    return () => {
      tracker.destroy();
      v.setMouseNavEnabled(oldNav);
      v.removeOverlay(overlay);
      overlay.remove();
      v.removeHandler("animation", anchor);
      v.removeHandler("resize", anchor);
    };
  });
  async function save() {
    if (!rect || saving) return;
    saving = true;
    error = "";
    try {
      await clipCreate({
        entryId,
        index,
        reading,
        tags: tags.split(/[,、]/),
        comment,
        isPrivate,
        xywh: rect,
      });
      onsaved();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="clip-mode" role="status">
  原本をドラッグして切り抜く<button disabled={saving} onclick={onclose}
    >取消</button
  >
</div>
{#if rect}<form
    bind:this={form}
    class="clip-form panel"
    style:left={`${position.x}px`}
    style:top={`${position.y}px`}
    aria-label="切り抜きを保存"
    onsubmit={(e) => {
      e.preventDefault();
      void save();
    }}
  >
    <label>読み<input bind:value={reading} required disabled={saving} /></label>
    <label
      >タグ<input
        bind:value={tags}
        placeholder="カンマ区切り"
        disabled={saving}
      /></label
    >
    <label
      >コメント<textarea bind:value={comment} disabled={saving}
      ></textarea></label
    >
    <label class="privacy"
      ><input
        type="checkbox"
        bind:checked={isPrivate}
        disabled={saving}
      />非公開</label
    >
    {#if error}<p role="alert">{error}</p>{/if}
    <div>
      <button class="primary" disabled={saving || !reading.trim()}>保存</button
      ><button type="button" disabled={saving} onclick={onclose}>取消</button>
    </div>
  </form>{/if}

<style>
  .clip-mode {
    position: absolute;
    top: 8px;
    left: 8px;
    z-index: 8;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 8px;
    font-size: 13px;
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .clip-form {
    position: absolute;
    width: 272px;
    max-height: calc(100% - 16px);
    overflow: auto;
    padding: 12px;
    z-index: 9;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
  }
  .privacy {
    flex-direction: row;
    align-items: center;
  }
  textarea {
    min-height: 60px;
    resize: vertical;
  }
  form > div {
    display: flex;
    gap: 8px;
  }
  p {
    font-size: 13px;
    color: var(--accent);
  }
</style>
