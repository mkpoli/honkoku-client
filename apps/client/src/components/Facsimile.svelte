<script lang="ts">
  import Recognition from "./Recognition.svelte";
  import type { Rectangle } from "./glyph-regions";
  import Skeleton from "./Skeleton.svelte";
  import RegionNotice from "./RegionNotice.svelte";
  import type { Region } from "../region.svelte";
  import type { Snippet } from "svelte";
  import { untrack } from "svelte";
  import type {
    PageLines,
    LocalPageLines,
  } from "../../../../packages/client-api/ocr";
  import OpenSeadragon from "openseadragon";
  import type { Canvas } from "@honkoku/client-api/types";
  let {
    oninsert,
    lineCharacterCounts = {},
    children,
    modes = [],
    canvas,
    pending = false,
    region,
    pageNumber,
    half = $bindable(""),
    lineModel = { engine: null, lines: [], estimated: false },
    highlightedLine = null,
    showLines = false,
    onlinehover,
    onlineselect,
  }: {
    modes?: {
      label: string;
      active: boolean;
      disabled?: boolean;
      select: () => void;
    }[];
    oninsert?: (character: string) => void;
    lineCharacterCounts?: Record<number, number>;
    children?: Snippet<[OpenSeadragon.Viewer | undefined]>;
    canvas?: Canvas;
    pending?: boolean;
    region?: Region<Canvas[]>;
    pageNumber: number;
    half?: string;
    lineModel?: PageLines | LocalPageLines;
    highlightedLine?: number | null;
    showLines?: boolean;
    onlinehover?: (index: number | null) => void;
    onlineselect?: (index: number) => void;
  } = $props();
  let recognitionMode = $derived(modes.find((mode) => mode.label === "認識"));
  let recognizing = $derived(!!recognitionMode?.active);
  let recognitionSelection = $state<Rectangle>();
  $effect(() => {
    canvas;
    recognizing;
    recognitionSelection = undefined;
  });
  let host: HTMLDivElement;
  let retryVersion = $state(0);
  let imageSlow = $state(false);
  let viewer = $state<OpenSeadragon.Viewer>();
  let opened = $state(false);
  let imageReady = $state(false);
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
    retryVersion;
    imageSlow = false;
    const timer = setTimeout(() => (imageSlow = true), 15000);
    fallback = !current?.infoJsonUrl && !current?.imageUrl;
    opened = false;
    imageReady = false;
    imageFailed = false;
    plainScale = 1;
    scale = 100;
    half = "";
    if (!host || !current || (!current.infoJsonUrl && !current.imageUrl)) {
      clearTimeout(timer);
      return;
    }
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
        : 0.6,
      blendTime: window.matchMedia("(prefers-reduced-motion: reduce)").matches
        ? 0
        : 0.1,
      visibilityRatio: 0.2,
      constrainDuringPan: false,
      springStiffness: 6,
      minZoomImageRatio: 1,
      gestureSettingsMouse: {
        clickToZoom: false,
        dblClickToZoom: true,
        scrollToZoom: true,
        zoomToRefPoint: true,
      },
      maxZoomPixelRatio: 4,
    });
    viewer = v;
    v.addHandler("open-failed", () => {
      fallback = true;
    });
    const fitMinimum = () => {
      (
        v.viewport as OpenSeadragon.Viewport & { minZoomLevel: number }
      ).minZoomLevel = v.viewport.getHomeZoom();
    };
    v.addHandler("open", () => {
      opened = true;
      fitMinimum();
    });
    v.addHandler("tile-loaded", () => {
      imageReady = true;
      imageSlow = false;
      clearTimeout(timer);
    });
    v.addHandler("resize", fitMinimum);
    v.addHandler("canvas-drag-end", () => v.viewport.applyConstraints());
    v.addHandler("zoom", () => {
      if (v.viewport)
        scale = Math.round(
          (v.viewport.getZoom() / v.viewport.getHomeZoom()) * 100,
        );
    });
    return () => {
      clearTimeout(timer);
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
          if (!event.quick) return;
          const count = lineCharacterCounts[line.index];
          if (
            !element.classList.contains("line-hidden") &&
            event.originalEvent.altKey &&
            count &&
            recognitionMode &&
            !recognitionMode.disabled
          ) {
            const vertical = line.height >= line.width;
            const bounds = element.getBoundingClientRect();
            const fraction = vertical
              ? event.position.y / bounds.height
              : event.position.x / bounds.width;
            const offset = Math.max(
              0,
              Math.min(count - 1, Math.floor(fraction * count)),
            );
            const advance = (vertical ? line.height : line.width) / count;
            const region: Rectangle = vertical
              ? [line.x, line.y + offset * advance, line.width, advance]
              : [line.x + offset * advance, line.y, advance, line.height];
            if (!recognizing) recognitionMode.select();
            queueMicrotask(
              () =>
                (recognitionSelection = region.map(Math.round) as Rectangle),
            );
          } else onlineselect?.(line.index);
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
  let pannedLine: number | null = null;
  $effect(() => {
    const v = viewer,
      target = highlightedLine,
      line = lineModel.lines.find((line) => line.index === target);
    if (!v || !opened || !line || !v.world.getItemCount()) return;
    if (target === pannedLine) return;
    pannedLine = target;
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
    const lowX = Math.min(...points.map((p) => p.x)),
      highX = Math.max(...points.map((p) => p.x));
    const lowY = Math.min(...points.map((p) => p.y)),
      highY = Math.max(...points.map((p) => p.y));
    const visibleWidth = Math.min(highX, size.x) - Math.max(lowX, 0),
      visibleHeight = Math.min(highY, size.y) - Math.max(lowY, 0);
    const visible =
      visibleWidth > 0 && visibleHeight > 0
        ? (visibleWidth * visibleHeight) / ((highX - lowX) * (highY - lowY))
        : 0;
    if (visible >= 0.3) return;
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
    <div class="facsimile-modes" role="group" aria-label="原本の操作">
      {#each modes as mode}<button
          aria-pressed={mode.active}
          disabled={mode.disabled || !opened}
          onclick={mode.select}>{mode.label}</button
        >{/each}
    </div>
    {#if (showLines || highlightedLine !== null) && lineModel.lines.length}<span
        class="caption muted"
        >{lineModel.engine === "local"
          ? "ローカルOCR"
          : lineModel.engine === "minna"
            ? "みんなで翻刻"
            : "国立国会図書館"}{lineModel.estimated ? "・推定" : ""}</span
      >{/if}
    <div class="zoom-controls">
      <button aria-label="縮小" onclick={() => zoom(1 / 1.25)}>−</button><span
        class="numeric">{scale}%</span
      ><button aria-label="拡大" onclick={() => zoom(1.25)}>＋</button><button
        onclick={reset}>全体</button
      >
    </div>
  </div>
  {#if recognizing}<Recognition
      viewer={opened ? viewer : undefined}
      {canvas}
      disabled={!oninsert}
      oninsert={(character) => oninsert?.(character)}
      selection={recognitionSelection}
    />{/if}
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
    {#if pending || (!imageReady && !imageFailed && (canvas?.infoJsonUrl || canvas?.imageUrl))}<Skeleton
        shape="frame"
        count={1}
        label="原本を取得中"
      />{/if}
    {#if region}<RegionNotice {region} />{/if}
    {#if imageSlow && !imageReady}<div class="region-notice">
        <button onclick={() => retryVersion++}>再試行</button>
      </div>{/if}
    <div
      class="osd"
      class:hidden={fallback}
      bind:this={host}
      aria-label={`コマ${pageNumber}の拡大画像`}
    ></div>
    {#if fallback && !pending}{#if canvas?.imageUrl && !imageFailed}<div
          class="plain-image"
        >
          <img
            src={canvas.imageUrl}
            alt={`コマ${pageNumber}の原本`}
            style:transform={`scale(${plainScale})`}
            referrerpolicy="no-referrer"
            onload={() => {
              imageReady = true;
              imageSlow = false;
            }}
            onerror={() => (imageFailed = true)}
          />
        </div>{:else}<p class="empty">
          原本画像を読み込めませんでした。<button onclick={() => retryVersion++}
            >再試行</button
          >
        </p>{/if}{/if}
    {@render children?.(opened ? viewer : undefined)}
  </div>
</section>
