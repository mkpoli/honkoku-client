import type { Canvas, Page } from "./types";

export interface CanvasLine {
  index: number;
  x: number;
  y: number;
  width: number;
  height: number;
  text: string;
  confidence: number;
  half: "right" | "left" | null;
}
export interface PageLines {
  engine: "minna" | "ndl" | null;
  lines: CanvasLine[];
  estimated: boolean;
}

function plainHtml(value: unknown): string {
  if (typeof value !== "string") return "";
  return value
    .replace(/<(rt|rp)\b[^>]*>[\s\S]*?<\/\1\s*>/gi, "")
    .replace(/<[^>]*>/g, "")
    .replace(
      /&(#x[\da-f]+|#\d+|amp|lt|gt|quot|apos|nbsp);/gi,
      (_, entity: string) => {
        if (entity[0] === "#") {
          const code =
            entity[1].toLowerCase() === "x"
              ? parseInt(entity.slice(2), 16)
              : Number(entity.slice(1));
          return code > 0 && code <= 0x10ffff
            ? String.fromCodePoint(code)
            : "�";
        }
        return (
          {
            amp: "&",
            lt: "<",
            gt: ">",
            quot: '"',
            apos: "'",
            nbsp: " ",
          } as Record<string, string>
        )[entity.toLowerCase()];
      },
    );
}
const record = (v: unknown): Record<string, unknown> =>
  v && typeof v === "object" ? (v as Record<string, unknown>) : {};

export function pageLines(
  page: Pick<Page, "ocr">,
  canvas?: Pick<Canvas, "width" | "height">,
): PageLines {
  const engine =
    page.ocr?.minna != null ? "minna" : page.ocr?.ndl != null ? "ndl" : null;
  const result: PageLines = { engine, lines: [], estimated: false };
  if (
    !canvas ||
    ![canvas.width, canvas.height].every((n) => Number.isFinite(n) && n > 0) ||
    !engine
  )
    return result;
  const ocr = record(page.ocr?.[engine]);
  const boxes = engine === "minna" ? ocr.lines : ocr.textBlocks;
  if (!Array.isArray(boxes)) return result;
  for (const value of boxes) {
    const b = record(value);
    const [x, y, width, height] = [b.x, b.y, b.width, b.height].map(Number);
    if (
      ![x, y, width, height].every(Number.isFinite) ||
      x < 0 ||
      y < 0 ||
      width <= 0 ||
      height <= 0
    )
      continue;
    result.lines.push({
      index: 0,
      x,
      y,
      width,
      height,
      text: plainHtml(engine === "minna" ? b.raw : record(b.text).text),
      confidence: Number.isFinite(Number(b.confidence))
        ? Number(b.confidence)
        : 0,
      half: null,
    });
  }
  let scale =
    engine === "minna"
      ? Math.max(1, Math.max(canvas.width, canvas.height) / 3500)
      : 1;
  const maxX = Math.max(0, ...result.lines.map((l) => l.x + l.width));
  const maxY = Math.max(0, ...result.lines.map((l) => l.y + l.height));
  if (maxX * scale > canvas.width || maxY * scale > canvas.height) {
    scale = Math.min(canvas.width / maxX, canvas.height / maxY);
    result.estimated = true;
  }
  for (const line of result.lines) {
    line.x *= scale;
    line.y *= scale;
    line.width *= scale;
    line.height *= scale;
  }
  const centre = (l: CanvasLine) => l.x + l.width / 2;
  const ordered = [...result.lines].sort((a, b) => centre(a) - centre(b));
  if (ordered.length > 1) {
    const low = centre(ordered[0]),
      span = centre(ordered.at(-1)!) - low;
    let widest = -1,
      split: number | null = null;
    for (let i = 1; i < ordered.length; i++) {
      const gap = centre(ordered[i]) - centre(ordered[i - 1]);
      const mid = (centre(ordered[i]) + centre(ordered[i - 1])) / 2;
      if (
        gap > 0 &&
        gap > widest &&
        mid >= low + span * 0.25 &&
        mid <= low + span * 0.75
      ) {
        widest = gap;
        split = mid;
      }
    }
    if (split !== null)
      for (const line of ordered)
        line.half = centre(line) < split ? "left" : "right";
  }
  result.lines.sort((a, b) => centre(b) - centre(a) || a.y - b.y);
  result.lines.forEach((line, index) => (line.index = index));
  return result;
}

export interface LocalPageLines extends Omit<PageLines, "engine"> {
  engine: "local";
}

export function pageLinesWithLocal(
  page: Pick<Page, "ocr">,
  canvas?: Pick<Canvas, "width" | "height">,
): PageLines | LocalPageLines {
  if (page.ocr?.local && canvas)
    return localPageLines(page.ocr.local as import("./types").LocalOcrPage, canvas);
  return pageLines(page, canvas);
}

export function localPageLines(
  local: import("./types").LocalOcrPage,
  canvas: Pick<Canvas, "width" | "height">,
): LocalPageLines {
  if (
    ![local.width, local.height, canvas.width, canvas.height].every(
      (n) => Number.isFinite(n) && n > 0,
    )
  )
    return { engine: "local", lines: [], estimated: false };
  const lines = [...local.lines].sort(
    (a, b) => a.reading_order - b.reading_order,
  );
  const sx = canvas.width / local.width,
    sy = canvas.height / local.height;
  return {
    engine: "local",
    estimated: false,
    lines: lines
      .filter(
        (l) =>
          [l.x, l.y, l.width, l.height].every(Number.isFinite) &&
          l.width > 0 &&
          l.height > 0,
      )
      .map((line, index) => ({
        index,
        x: line.x * sx,
        y: line.y * sy,
        width: line.width * sx,
        height: line.height * sy,
        text: line.plain,
        confidence: line.confidence,
        half:
          (line.x + line.width / 2) * sx >= canvas.width / 2 ? "right" : "left",
      })),
  };
}
