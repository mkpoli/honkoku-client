import { parseLine } from "./syntax";

const graphemes = new Intl.Segmenter("ja", { granularity: "grapheme" });
/** Length in full-size character advances; side readings share their base. */
export function visualColumnLength(source: string): number {
  return parseLine(source).reduce((length, node) => {
    if (node.kind === "return" || node.kind === "okurigana") return length;
    if (node.kind === "reference") return length + 18 / 17;
    if (node.kind === "warigaki")
      return length + Math.max(...node.segments!.map(visualColumnLength)) / 2;
    if (node.segments) return length + visualColumnLength(node.segments[0]);
    return length + [...graphemes.segment(node.source)].length;
  }, 0);
}
export function longestColumnLength(source: string): number {
  return Math.max(0, ...source.split(/\r\n|\r|\n/).map(visualColumnLength));
}
export function automaticTextScale(
  paneHeight: number,
  longest: number,
  lineAdvance = 17,
  padding = 34,
): { scale: number; wraps: boolean } {
  const fit =
    Math.max(0, paneHeight) / (Math.max(0, longest) * lineAdvance + padding);
  return { scale: Math.min(1.25, Math.max(0.5, fit)), wraps: fit < 0.5 };
}

/** Inline room used by kunten beside the base, plus bordered annotations. */
function annotationAdvance(source: string): number {
  const nodes = parseLine(source);
  let advance = 0;
  for (let i = 0; i < nodes.length; i++) {
    const node = nodes[i];
    if (node.kind === "return" || node.kind === "okurigana") {
      const sides = { return: 0, okurigana: 0 };
      while (
        i < nodes.length &&
        (nodes[i].kind === "return" || nodes[i].kind === "okurigana")
      ) {
        const mark = nodes[i++];
        sides[mark.kind as keyof typeof sides] += [
          ...graphemes.segment(mark.segments![0]),
        ].length;
      }
      i--;
      advance += Math.max(sides.return, sides.okurigana) * 8.5;
    } else if (node.kind === "gap") {
      advance += 2;
    } else if (node.kind === "warigaki") {
      advance += Math.max(...node.segments!.map(annotationAdvance)) / 2;
    } else if (node.segments) {
      advance += annotationAdvance(node.segments[0]);
      if (node.kind === "box") advance += 6;
    }
  }
  return advance;
}
export function textScalePadding(source: string): number {
  return 34 + Math.max(0, ...source.split(/\r\n|\r|\n/).map(annotationAdvance));
}
