import type {
  Canvas,
  GlyphAttestation,
  GlyphOccurrence,
} from "@honkoku/client-api/types";
import { alignColumns, transcriptionColumns } from "@honkoku/markup";
import {
  pageLines,
  type CanvasLine,
} from "../../../../packages/client-api/ocr";
export type Rectangle = [number, number, number, number];
const segmenter = new Intl.Segmenter("ja", { granularity: "grapheme" });
export const graphemes = (text: string) =>
  [...segmenter.segment(text)].map((s) => s.segment);
export const singleGlyph = (text: string) =>
  graphemes(text).length === 1 && !/^\s+$/.test(text);
export function clampRegion(
  rect: Rectangle,
  canvas: Pick<Canvas, "width" | "height">,
): Rectangle {
  const [x, y, w, h] = rect;
  const left = Math.max(0, Math.min(canvas.width, Math.round(x)));
  const top = Math.max(0, Math.min(canvas.height, Math.round(y)));
  const right = Math.max(left, Math.min(canvas.width, Math.round(x + w)));
  const bottom = Math.max(top, Math.min(canvas.height, Math.round(y + h)));
  return [left, top, right - left, bottom - top];
}
export function estimateRegion(
  line: CanvasLine,
  occurrence: GlyphOccurrence,
  canvas: Canvas,
): Rectangle | null {
  const count = graphemes(occurrence.plain).length;
  const offset = graphemes(
    [...occurrence.plain].slice(0, occurrence.offset).join(""),
  ).length;
  if (!count || offset >= count) return null;
  const vertical = line.height >= line.width;
  const advance = (vertical ? line.height : line.width) / count;
  const rect: Rectangle = vertical
    ? [
        line.x - line.width * 0.3,
        line.y + advance * (offset - 0.3),
        line.width * 1.6,
        advance * 1.6,
      ]
    : [
        line.x + advance * (offset - 0.3),
        line.y - line.height * 0.3,
        advance * 1.6,
        line.height * 1.6,
      ];
  const result = clampRegion(rect, canvas);
  return result[2] > 0 && result[3] > 0 ? result : null;
}
export function regionUrl(
  canvas: Canvas,
  rect: Rectangle,
  longSide = 192,
): string | null {
  const info = canvas.infoJsonUrl;
  if (!info?.endsWith("/info.json")) return null;
  const [, , w, h] = rect;
  if (!w || !h) return null;
  const size =
    w >= h ? `${Math.max(longSide, w)},` : `,${Math.max(longSide, h)}`;
  return `${info.slice(0, -10)}/${rect.join(",")}/${size}/0/default.jpg`;
}
export function glyphCards(page: GlyphAttestation) {
  const canvas = page.canvas;
  const lines = pageLines(page, canvas ?? undefined).lines;
  const columns = transcriptionColumns(page.text);
  const alignment = alignColumns(
    columns.map((c) => c.text),
    lines,
  );
  return page.occurrences.map((occurrence, i) => {
    const line = lines.find((l) => l.index === alignment[occurrence.column]);
    const region =
      canvas && line ? estimateRegion(line, occurrence, canvas) : null;
    return {
      key: `${page.pageId}-${i}`,
      page,
      occurrence,
      crop: canvas && region ? regionUrl(canvas, region) : null,
      strip:
        canvas && line
          ? regionUrl(
              canvas,
              clampRegion([line.x, line.y, line.width, line.height], canvas),
              512,
            )
          : null,
    };
  });
}
