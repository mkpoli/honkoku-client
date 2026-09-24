/** A rendered source line: its unwrapped length at text scale 1 and the room its column offers, in CSS px. */
export interface LineFit {
  length: number;
  room: number;
}

/** Range the automatic fit chooses from. */
export const autoScaleRange = { min: 0.5, max: 1.25 } as const;
/** Range of the manual steps, wider than automatic fitting ever goes. */
export const manualScaleRange = { min: 0.5, max: 2 } as const;

/**
 * Below this scale small kana and ruby stop being comfortable to read, so
 * rather than go lower the fit lets unusually long lines wrap.
 */
const comfortScale = 0.75;
/**
 * A line may wrap instead of setting the size when it is this many times
 * longer than every line kept on one column. 1.5 lets a stray run-on line
 * wrap while a page of uneven but ordinary lines is still fitted whole.
 */
const outlierGain = 1.5;

/** Room kept free in each column for rounding and unscaled borders, as a share of the column. */
const slack = (room: number) => 2 + room * 0.005;

/** Scale each line needs to stay on one visual line. */
function demand(line: LineFit): number {
  return line.length > 0 && line.room > 1
    ? line.length / (line.room - slack(line.room))
    : 0;
}

/**
 * The largest scale at which ordinary lines stay on one visual line.
 * Lines that cannot fit even at the lower bound always wrap. When fitting the
 * rest would still go below `comfortScale`, the few lines standing
 * `outlierGain` above all others (a quarter of the page at most) wrap too.
 */
export function fitTextScale(
  lines: LineFit[],
  { min, max }: { min: number; max: number } = autoScaleRange,
): number {
  const needed = lines
    .map(demand)
    .filter((d) => d > 0)
    .sort((a, b) => b - a);
  if (!needed.length) return max;
  const fits = needed.filter((d) => d * min <= 1);
  if (!fits.length) return comfortScale;
  let kept = 0;
  if (1 / fits[0] < comfortScale) {
    const allowed = Math.max(1, Math.floor(fits.length / 4));
    for (let k = 1; k <= allowed && k < fits.length; k++)
      if (fits[k - 1] > outlierGain * fits[k]) {
        kept = k;
        break;
      }
  }
  return Math.min(max, Math.max(min, Math.floor(100 / fits[kept]) / 100));
}

/** Indices of the lines that need more than one visual line at this scale. */
export function wrappingLines(lines: LineFit[], scale: number): number[] {
  return lines.flatMap((line, i) => (demand(line) * scale > 1 ? [i] : []));
}

let scrollbar: number | undefined;
function scrollbarThickness(): number {
  if (scrollbar === undefined) {
    const probe = document.createElement("div");
    probe.style.cssText =
      "position:absolute;top:-9999px;width:100px;height:100px;overflow:scroll";
    document.body.append(probe);
    scrollbar = probe.offsetHeight - probe.clientHeight;
    probe.remove();
  }
  return scrollbar;
}

/**
 * Read each vertical column's room as laid out, then its unwrapped length at
 * scale 1 under `.text-fit-measuring`, in one synchronous pass so nothing is
 * painted in between. Neither reading depends on the applied scale, which is
 * what keeps measure → rescale → measure from oscillating.
 */
export function measureLines(scroller: HTMLElement): LineFit[] {
  const columns = [
    ...scroller.querySelectorAll<HTMLElement>(".transcription-column"),
  ];
  if (!columns.length) return [];
  const style = getComputedStyle(scroller);
  const barShown =
    scroller.offsetHeight -
      scroller.clientHeight -
      parseFloat(style.borderTopWidth) -
      parseFloat(style.borderBottomWidth) >
    0;
  // The horizontal scrollbar is reserved whether or not it shows, so shrinking the text
  // cannot hide the bar, hand back its height and grow the text again.
  const reserve = barShown ? 0 : scrollbarThickness();
  const rooms = columns.map((column) => column.clientHeight - reserve);
  scroller.classList.add("text-fit-measuring");
  const lengths = columns.map((column) => column.getBoundingClientRect().height);
  scroller.classList.remove("text-fit-measuring");
  return columns.map((_, i) => ({ length: lengths[i], room: rooms[i] }));
}

/** Delay after the last keystroke before the column being typed in is measured again. */
const typingPause = 400;
/** Delay after the last pane resize, so a drag is measured once it stops. */
const resizePause = 100;

export interface FitTextOptions {
  /** Receives every new measurement; `typing` is set when it follows an edit at the caret. */
  onmeasure?: (lines: LineFit[], typing: boolean) => void;
  /** Horizontal text is not fitted. */
  horizontal?: boolean;
}

/**
 * Report the page's line measurements whenever layout can have changed: pane
 * or column size, text content, web fonts, or a switch back from horizontal
 * text.
 * Edits at the caret wait for a pause in typing and never measure during IME
 * composition.
 */
export function fitText(scroller: HTMLElement, options: FitTextOptions) {
  let frame = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let composing = false;
  let typing = false;
  let active = true;
  let last = "";
  const measure = () => {
    frame = 0;
    if (
      !active ||
      composing ||
      !options.onmeasure ||
      options.horizontal ||
      !scroller.isConnected ||
      !scroller.clientHeight
    )
      return;
    const lines = measureLines(scroller);
    const key = lines
      .map((line) => `${Math.round(line.length)}/${Math.round(line.room)}`)
      .join(",");
    if (key === last) return;
    last = key;
    options.onmeasure(lines, typing);
    typing = false;
  };
  const later = (delay: number) => {
    cancelAnimationFrame(frame);
    frame = 0;
    clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      frame = requestAnimationFrame(measure);
    }, delay);
  };
  const now = () => {
    typing = false;
    clearTimeout(timer);
    timer = undefined;
    frame ||= requestAnimationFrame(measure);
  };
  const edited = () => {
    const focus = document.activeElement as HTMLElement | null;
    if (focus?.isContentEditable && scroller.contains(focus)) {
      typing = true;
      later(typingPause);
    } else now();
  };
  const start = () => (composing = true);
  const end = () => {
    composing = false;
    edited();
  };
  // The pane and every column: a column can take its height after the pane has settled.
  const resize = new ResizeObserver(() => later(resizePause));
  const observeBoxes = () => {
    resize.disconnect();
    resize.observe(scroller);
    for (const column of scroller.querySelectorAll(".transcription-column"))
      resize.observe(column);
  };
  observeBoxes();
  const content = new MutationObserver(() => {
    observeBoxes();
    edited();
  });
  content.observe(scroller, {
    childList: true,
    subtree: true,
    characterData: true,
  });
  scroller.addEventListener("compositionstart", start);
  scroller.addEventListener("compositionend", end);
  document.fonts.addEventListener("loadingdone", now);
  void document.fonts.ready.then(() => active && now());
  now();
  return {
    update(next: FitTextOptions) {
      const reshaped = next.horizontal !== options.horizontal;
      options = next;
      if (reshaped) {
        last = "";
        now();
      }
    },
    destroy() {
      active = false;
      cancelAnimationFrame(frame);
      clearTimeout(timer);
      resize.disconnect();
      content.disconnect();
      scroller.removeEventListener("compositionstart", start);
      scroller.removeEventListener("compositionend", end);
      document.fonts.removeEventListener("loadingdone", now);
    },
  };
}
