import type { CanvasLine } from "../client-api/ocr";
import { parseLine, splitFields } from "./syntax";

export function plainColumn(source: string): string {
  if (/^\s*％/.test(source)) return "";
  return parseLine(source)
    .map((node) => {
      // Older transcriptions include single-part or otherwise noneditable shells.
      if (node.kind === "raw") {
        const shell =
          /^《(割書|振り仮名|見せ消ち|圏点|右線|題|箱|場所)：([\s\S]*)》$/.exec(
            node.source,
          );
        if (shell) {
          const fields = splitFields(shell[2]);
          return shell[1] === "割書"
            ? fields.map(plainColumn).join("")
            : plainColumn(fields[0]);
        }
      }
      if (
        [
          "divider",
          "editorial",
          "comment",
          "reference",
          "return",
          "okurigana",
        ].includes(node.kind)
      )
        return "";
      if (node.kind === "warigaki")
        return node.segments!.map(plainColumn).join("");
      return node.segments ? plainColumn(node.segments[0]) : node.source;
    })
    .join("");
}

/** The source index is retained for editor caret positions, including blank lines. */
export function transcriptionColumns(
  source: string,
): { sourceIndex: number; text: string }[] {
  return source
    .split(/\r\n|\r|\n/)
    .flatMap((line, sourceIndex) =>
      !line.trim() || /^\s*【[右左]丁】\s*$/.test(line)
        ? []
        : [{ sourceIndex, text: plainColumn(line) }],
    );
}

export function foldText(text: string): string {
  return text
    .normalize("NFKC")
    .replace(/[ァ-ヶ]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0x60))
    .replace(/[\p{P}\p{Z}\p{S}\s]/gu, "");
}
export function dice(a: string, b: string): number {
  const x = [...foldText(a)],
    y = [...foldText(b)];
  if (!x.length || !y.length) return 0;
  if (x.join("") === y.join("")) return 1;
  if (x.length < 2 || y.length < 2) return 0;
  const grams = new Map<string, number>();
  for (let i = 1; i < x.length; i++) {
    const key = x[i - 1] + x[i];
    grams.set(key, (grams.get(key) ?? 0) + 1);
  }
  let common = 0;
  for (let i = 1; i < y.length; i++) {
    const key = y[i - 1] + y[i],
      count = grams.get(key) ?? 0;
    if (count) {
      common++;
      grams.set(key, count - 1);
    }
  }
  return (2 * common) / (x.length + y.length - 2);
}

/** A column spanning two OCR lines maps to the first line of that span. */
export function alignColumns(
  columns: string[],
  lines: CanvasLine[],
): (number | null)[] {
  const n = columns.length,
    m = lines.length;
  const scores = Array.from({ length: n + 1 }, () =>
    new Float64Array(m + 1).fill(-Infinity),
  );
  const steps = Array.from({ length: n + 1 }, () => new Uint8Array(m + 1));
  scores[0][0] = 0;
  function update(i: number, j: number, score: number, step: number) {
    if (score > scores[i][j]) {
      scores[i][j] = score;
      steps[i][j] = step;
    }
  }
  for (let i = 0; i <= n; i++)
    for (let j = 0; j <= m; j++) {
      const score = scores[i][j];
      if (i < n) update(i + 1, j, score, 1);
      if (j < m) update(i, j + 1, score, 2);
      if (i === n || j === m) continue;
      const similarity = dice(columns[i], lines[j].text);
      if (similarity > 0.3) update(i + 1, j + 1, score + similarity - 0.3, 3);
      if (j + 1 < m && lines[j].half === lines[j + 1].half) {
        const combined = dice(columns[i], lines[j].text + lines[j + 1].text);
        if (
          combined > 0.3 &&
          similarity > 0.15 &&
          dice(columns[i], lines[j + 1].text) > 0.15
        )
          update(i + 1, j + 2, score + combined - 0.38, 4);
      }
      if (i + 1 < n) {
        const combined = dice(columns[i] + columns[i + 1], lines[j].text);
        if (
          combined > 0.3 &&
          similarity > 0.15 &&
          dice(columns[i + 1], lines[j].text) > 0.15
        )
          update(i + 2, j + 1, score + 2 * (combined - 0.3) - 0.08, 5);
      }
    }
  const matches: (number | null)[] = Array(n).fill(null);
  let i = n,
    j = m;
  while (i || j) {
    switch (steps[i][j]) {
      case 1:
        i--;
        break;
      case 2:
        j--;
        break;
      case 3:
        matches[--i] = lines[--j].index;
        break;
      case 4:
        j -= 2;
        matches[--i] = lines[j].index;
        break;
      case 5:
        j--;
        matches[--i] = lines[j].index;
        matches[--i] = lines[j].index;
        break;
      default:
        throw Error("Alignment path is incomplete");
    }
  }
  return matches;
}
