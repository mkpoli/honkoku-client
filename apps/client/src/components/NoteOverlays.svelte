<script lang="ts">
  import OpenSeadragon from "openseadragon";
  import type { PageNote } from "@honkoku/client-api/types";
  let {
    viewer,
    notes,
    visible,
    highlighted,
    selecting = false,
  }: {
    viewer?: OpenSeadragon.Viewer;
    notes: (PageNote | null)[];
    visible: boolean;
    highlighted: number | null;
    selecting?: boolean;
  } = $props();
  let elements = $state<{ element: HTMLButtonElement; index: number }[]>([]);
  export function pan(index: number) {
    const note = notes[index],
      v = viewer;
    if (!v?.world.getItemCount() || !note?.xywh) return;
    const rect = v.world.getItemAt(0).imageToViewportRectangle(...note.xywh);
    const drawerWidth =
      v.canvas
        .closest(".facsimile-canvas")
        ?.querySelector(".notes-drawer")
        ?.getBoundingClientRect().width ?? 0;
    const size = v.viewport.getContainerSize();
    const target = v.viewport.pointFromPixel(
      new OpenSeadragon.Point((size.x - drawerWidth) / 2, size.y / 2),
      true,
    );
    v.viewport.panTo(
      v.viewport.getCenter(true).plus(rect.getCenter().minus(target)),
    );
  }
  $effect(() => {
    const v = viewer,
      rows = notes;
    if (!v?.world.getItemCount()) return;
    const trackers: OpenSeadragon.MouseTracker[] = [];
    const overlays: { element: HTMLButtonElement; index: number }[] = [];
    rows.forEach((note, index) => {
      if (
        !note?.xywh ||
        note.xywh.length !== 4 ||
        !note.xywh.every(Number.isFinite) ||
        note.xywh[2] <= 0 ||
        note.xywh[3] <= 0
      )
        return;
      const element = document.createElement("button");
      element.type = "button";
      element.className = "note-overlay";
      element.dataset.noteIndex = String(index);
      element.setAttribute("aria-label", `注釈${index + 1}：${note.content}`);
      const tooltip = document.createElement("span");
      tooltip.className = "note-overlay-tooltip";
      tooltip.textContent = note.content;
      tooltip.setAttribute("role", "tooltip");
      element.append(tooltip);
      trackers.push(
        new OpenSeadragon.MouseTracker({
          element,
          enterHandler: () => element.classList.add("hovered"),
          leaveHandler: () => element.classList.remove("hovered"),
        }),
      );
      v.addOverlay({
        element,
        location: v.world.getItemAt(0).imageToViewportRectangle(...note.xywh),
        checkResize: false,
        rotationMode: OpenSeadragon.OverlayRotationMode.EXACT,
      });
      overlays.push({ element, index });
    });
    elements = overlays;
    return () => {
      trackers.forEach((t) => t.destroy());
      overlays.forEach(({ element }) => {
        v.removeOverlay(element);
        element.remove();
      });
    };
  });
  $effect(() => {
    for (const { element, index } of elements) {
      const active = highlighted === index;
      element.style.display = visible || active ? "" : "none";
      element.style.pointerEvents = selecting ? "none" : "auto";
      element.classList.toggle("highlighted", active);
      element.tabIndex = visible && !selecting ? 0 : -1;
    }
  });
</script>

<style>
  :global(.note-overlay) {
    padding: 0;
    border: 1px solid var(--success);
    border-radius: 0;
    background: transparent;
    position: relative;
    min-width: 0;
    min-height: 0;
  }
  :global(.note-overlay.highlighted),
  :global(.note-overlay.hovered),
  :global(.note-overlay:focus) {
    border: 2px solid var(--accent);
    background: rgba(217, 89, 54, 0.12);
  }
  :global(.note-overlay-tooltip) {
    display: none;
    position: absolute;
    left: 0;
    bottom: calc(100% + 4px);
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px;
    width: 220px;
    max-height: 120px;
    overflow: auto;
    white-space: pre-wrap;
    text-align: left;
    font-size: 13px;
    pointer-events: none;
  }
  :global(.note-overlay.hovered .note-overlay-tooltip),
  :global(.note-overlay:focus .note-overlay-tooltip) {
    display: block;
  }
</style>
