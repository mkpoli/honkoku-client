/** Measure usable pixels after toolbar layout, scrollbars, and the label gutter. */
export function measureColumns(
  element: HTMLElement,
  options: { onheight?: (height: number) => void; lineNumbers: boolean },
) {
  const apply = () => {
    const style = getComputedStyle(element);
    const gutter =
      parseFloat(style.getPropertyValue("--line-number-gutter")) || 0;
    const inset = element.classList.contains("editor-scroll") ? 8 : 0;
    const height = Math.max(
      0,
      element.clientHeight -
        parseFloat(style.paddingTop) -
        parseFloat(style.paddingBottom) -
        inset -
        (inset ? gutter : 0),
    );
    if (!height) return;
    element.style.setProperty("--column-height", `${height}px`);
    options.onheight?.(height);
  };
  const observer = new ResizeObserver(apply);
  observer.observe(element);
  apply();
  return {
    update(next: typeof options) {
      options = next;
      apply();
    },
    destroy() {
      observer.disconnect();
    },
  };
}
