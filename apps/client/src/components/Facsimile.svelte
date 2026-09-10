<script lang="ts">
  import OpenSeadragon from "openseadragon";
  import type { Canvas } from "@honkoku/client-api/types";
  let {
    canvas,
    pageNumber,
    half = $bindable(""),
  }: { canvas?: Canvas; pageNumber: number; half?: string } = $props();
  let host: HTMLDivElement;
  let viewer: OpenSeadragon.Viewer | undefined;
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
    fallback = !current?.infoJsonUrl;
    imageFailed = false;
    plainScale = 1;
    scale = 100;
    half = "";
    if (!host || !current?.infoJsonUrl) return;
    const v = OpenSeadragon({
      element: host,
      tileSources: current.infoJsonUrl,
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
      if (viewer === v) viewer = undefined;
    };
  });
  $effect(() => {
    const region = half;
    if (!viewer?.world.getItemCount()) return;
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
</script>

<section class="panel facsimile-panel">
  <div class="pane-toolbar">
    <h2>原本</h2>
    <div class="zoom-controls">
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
