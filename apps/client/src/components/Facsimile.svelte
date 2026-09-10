<script lang="ts">
  import { untrack } from "svelte";
  import type { PageLines, LocalPageLines } from "../../../../packages/client-api/ocr";
  import OpenSeadragon from "openseadragon";
  import type { Canvas } from "@honkoku/client-api/types";
  let {
    canvas,
    pageNumber,
    half = $bindable(""),
    lineModel = { engine: null, lines: [], estimated: false },
    highlightedLine = null,
    onlinehover,
    onlineselect,
  }: {
    canvas?: Canvas;
    pageNumber: number;
    half?: string;
    lineModel?: PageLines | LocalPageLines;
    highlightedLine?: number | null;
    onlinehover?: (index: number | null) => void;
    onlineselect?: (index: number) => void;
  } = $props();
  let host: HTMLDivElement;
  let viewer = $state<OpenSeadragon.Viewer>();
  let opened = $state(false);
  let showLines = $state(false);
  let overlayElements = $state<HTMLButtonElement[]>([]);
  let fallback = $state(false),
    imageFailed = $state(false),
    scale = $state(100),
    plainScale = $state(1);
  export function zoom(factor: number) {
    if (fallback) {
      plainScale = Math.min(8, Math.max(0.25, plainScale * factor));
      scale = Math.round(plainScale * 100);
    } else viewer?.viewport.zoomBy(factor);
  }
  export function reset() {
    viewer?.viewport.goHome();
    plainScale = 1;
    scale = 100;
    half = "";
  }
  $effect(() => {
    const current = canvas;
    fallback = !current?.infoJsonUrl && !current?.imageUrl;
    opened = false;
    showLines = false;
    imageFailed = false;
    plainScale = 1;
    scale = 100;
    half = "";
    if (!host || !current || (!current.infoJsonUrl && !current.imageUrl))
      return;
    const v = OpenSeadragon({
      element: host,
      tileSources: current.infoJsonUrl ?? {
        type: "image",
        url: current.imageUrl!,
      },
      showNavigationControl: false,
      showNavigator: false,
      crossOriginPolicy: "Anonymous",
      animationTime: window.matchMedia("(prefers-reduced-motion: reduce)")
        .matches
        ? 0
        : 0.2,
      blendTime: window.matchMedia("(prefers-reduced-motion: reduce)").matches
        ? 0
        : 0.1,
      visibilityRatio: 1,
      constrainDuringPan: true,
      gestureSettingsMouse: { clickToZoom: false },
      maxZoomPixelRatio: 3,
    });
    viewer = v;
    v.addHandler("open-failed", () => (fallback = true));
    v.addHandler("open", () => (opened = true));
    v.addHandler("canvas-click", (event) => {
      if (!event.quick || !v.world.getItemCount()) return;
      const point = v.viewport.viewportToImageCoordinates(
        v.viewport.pointFromPixel(event.position),
      );
      const size = v.world.getItemAt(0).getContentSize();
      if (
        point.x >= 0 &&
        point.x <= size.x &&
        point.y >= 0 &&
        point.y <= size.y
      )
        half = point.x < size.x / 2 ? "左丁" : "右丁";
    });
    v.addHandler("zoom", () => {
      if (v.viewport)
        scale = Math.round(
          (v.viewport.getZoom() / v.viewport.getHomeZoom()) * 100,
        );
    });
    return () => {
      v.destroy();
      if (untrack(() => viewer) === v) viewer = undefined;
    };
  });
  $effect(() => {
    const region = half;
    if (!opened || !viewer?.world.getItemCount()) return;
    if (!region) {
      viewer.viewport.goHome();
      return;
    }
    const size = viewer.world.getItemAt(0).getContentSize();
    viewer.viewport.fitBounds(
      viewer.viewport.imageToViewportRectangle(
        region === "右丁" ? size.x / 2 : 0,
        0,
        size.x / 2,
        size.y,
      ),
    );
  });
  $effect(() => {
    const v = viewer,
      ready = opened,
      lines = lineModel.lines;
    if (!v || !ready || !v.world.getItemCount()) return;
    const trackers: OpenSeadragon.MouseTracker[] = [];
    const elements = lines.map((line) => {
      const element = document.createElement("button");
      element.type = "button";
      element.className = "line-overlay";
      element.dataset.lineIndex = String(line.index);
      element.setAttribute(
        "aria-label",
        `原本の行${line.index + 1}：${line.text}`,
      );
      const tracker = new OpenSeadragon.MouseTracker({
        element,
        enterHandler: () => onlinehover?.(line.index),
        leaveHandler: () => onlinehover?.(null),
        clickHandler: (event) => {
          if (event.quick) onlineselect?.(line.index);
        },
      });
      trackers.push(tracker);
      element.addEventListener("keydown", (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onlineselect?.(line.index);
        }
      });
      v.addOverlay({
        element,
        location: v.world
          .getItemAt(0)
          .imageToViewportRectangle(line.x, line.y, line.width, line.height),
        checkResize: false,
        rotationMode: OpenSeadragon.OverlayRotationMode.EXACT,
      });
      return element;
    });
    overlayElements = elements;
    return () => {
      trackers.forEach((tracker) => tracker.destroy());
      elements.forEach((element) => {
        v.removeOverlay(element);
        element.remove();
      });
    };
  });
  $effect(() => {
    for (const element of overlayElements) {
      const active = Number(element.dataset.lineIndex) === highlightedLine;
      element.classList.toggle("highlighted", active);
      element.classList.toggle("line-hidden", !showLines && !active);
      element.setAttribute("aria-pressed", String(active));
      element.tabIndex = showLines || active ? 0 : -1;
    }
  });
  $effect(() => {
    const v = viewer,
      line = lineModel.lines.find((line) => line.index === highlightedLine);
    if (!v || !opened || !line || !v.world.getItemCount()) return;
    const rect = v.world
      .getItemAt(0)
      .imageToViewportRectangle(line.x, line.y, line.width, line.height);
    const points = [
      rect.getTopLeft(),
      rect.getTopRight(),
      rect.getBottomLeft(),
      rect.getBottomRight(),
    ].map((p) => v.viewport.pixelFromPoint(p, true));
    const size = v.viewport.getContainerSize();
    if (
      points.every(
        (p) => p.x >= 0 && p.x <= size.x && p.y >= 0 && p.y <= size.y,
      )
    )
      return;
    const lowX = Math.min(...points.map((p) => p.x)),
      highX = Math.max(...points.map((p) => p.x));
    const lowY = Math.min(...points.map((p) => p.y)),
      highY = Math.max(...points.map((p) => p.y));
    const offset = (low: number, high: number, extent: number) =>
      high - low > extent
        ? (low + high - extent) / 2
        : low < 0
          ? low
          : high > extent
            ? high - extent
            : 0;
    const delta = v.viewport.deltaPointsFromPixels(
      new OpenSeadragon.Point(
        offset(lowX, highX, size.x),
        offset(lowY, highY, size.y),
      ),
      true,
    );
    v.viewport.panTo(v.viewport.getCenter(true).plus(delta));
  });
</script>

<section class="panel facsimile-panel">
  <div class="pane-toolbar">
    <h2>原本</h2>
    {#if (showLines || highlightedLine !== null) && lineModel.lines.length}<span
        class="caption muted"
        >{lineModel.engine === "local"
          ? "ローカルOCR"
          : lineModel.engine === "minna"
          ? "みんなで翻刻"
          : "国立国会図書館"}{lineModel.estimated ? "・推定" : ""}</span
      >{/if}
    <div class="zoom-controls">
      {#if lineModel.lines.length}<button
          aria-pressed={showLines}
          onclick={() => (showLines = !showLines)}>行枠</button
        >{/if}
      <button aria-label="縮小" onclick={() => zoom(1 / 1.25)}>−</button><span
        class="numeric">{scale}%</span
      ><button aria-label="拡大" onclick={() => zoom(1.25)}>＋</button><button
        onclick={reset}>全体</button
      >
    </div>
  </div>
  <div class="half-tabs" aria-label="原本の見開き">
    <button
      class:active={half === "左丁"}
      onclick={() => (half = half === "左丁" ? "" : "左丁")}>左丁</button
    ><button
      class:active={half === "右丁"}
      onclick={() => (half = half === "右丁" ? "" : "右丁")}>右丁</button
    >
  </div>
  <div class="facsimile-canvas">
    <div
      class="osd"
      class:hidden={fallback}
      bind:this={host}
      aria-label={`コマ${pageNumber}の拡大画像`}
    ></div>
    {#if fallback}{#if canvas?.imageUrl && !imageFailed}<div
          class="plain-image"
        >
          <img
            src={canvas.imageUrl}
            alt={`コマ${pageNumber}の原本`}
            style:transform={`scale(${plainScale})`}
            referrerpolicy="no-referrer"
            onerror={() => (imageFailed = true)}
          />
        </div>{:else}<p class="empty">
          原本画像を読み込めませんでした。
        </p>{/if}{/if}
  </div>
</section>
