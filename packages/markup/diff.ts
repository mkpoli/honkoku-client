export interface Change {
  kind: "equal" | "insert" | "delete";
  text: string;
  widthOnly?: boolean;
}

export interface DiffToken {
  text: string;
  mark?: string;
  label?: string;
  lineBreak?: boolean;
}

/** Text that follows change `i` in the version it belongs to. */
export function textAfter(changes: Change[], i: number): string {
  const side = changes[i].kind;
  for (const change of changes.slice(i + 1))
    if (change.kind === "equal" || change.kind === side) return change.text;
  return "";
}

/**
 * Keep source text intact; only changed whitespace gets a display mark.
 * `after` is the text that follows in the same version, so a CR whose LF is
 * unchanged marks a line-ending change instead of starting a line.
 */
export function diffTokens(change: Change, after = ""): DiffToken[] {
  if (change.kind === "equal") return [{ text: change.text }];
  const tokens: DiffToken[] = [];
  let offset = 0;
  for (const match of change.text.matchAll(/\r\n|[\s\u200b]/gu)) {
    if (match.index > offset)
      tokens.push({ text: change.text.slice(offset, match.index) });
    const text = match[0];
    const end = match.index + text.length;
    const lineBreak =
      /^[\r\n\u2028\u2029]/u.test(text) &&
      !(
        text === "\r" &&
        (end < change.text.length ? change.text[end] : after[0]) === "\n"
      );
    const [mark, label] = lineBreak
      ? ["↵", "改行"]
      : text === "\r"
        ? ["␍", "改行コードの変更（CR）"]
        : text === " "
          ? ["·", "半角空白"]
          : text === "\u3000"
            ? ["⬚", "全角空白"]
            : text === "\t"
              ? ["⇥", "タブ"]
              : [
                  "·",
                  `空白（U+${text.codePointAt(0)!.toString(16).toUpperCase().padStart(4, "0")}）`,
                ];
    tokens.push({ text, mark, label, lineBreak });
    offset = match.index + text.length;
  }
  if (offset < change.text.length)
    tokens.push({ text: change.text.slice(offset) });
  return tokens;
}

/** Half-width forms: ASCII and the U+FF61–FFEF half-width block. */
const narrow = /^[\u0020-\u007e\uff61-\uffdc\uffe8-\uffee]+$/u;
/** 半 or 全 for the side of a width-only change, 混 when it mixes both. */
export function widthLabel(text: string): "半" | "全" | "混" {
  if (narrow.test(text)) return "半";
  return [...text].some((ch) => narrow.test(ch)) ? "混" : "全";
}

const foldWidth = (text: string) =>
  text.replace(/[\u3000\uff01-\uffef]+/gu, (part) => part.normalize("NFKC"));

/** Linear-space LCS over Unicode code points; common edges avoid work on long saves. */
export function diffSource(before: string, after: string): Change[] {
  const a = Array.from(before),
    b = Array.from(after),
    result: Change[] = [];
  const emit = (kind: Change["kind"], text: string) => {
    if (!text) return;
    const last = result.at(-1);
    if (last?.kind === kind) last.text += text;
    else result.push({ kind, text });
  };
  function scores(x: string[], y: string[]) {
    let row = new Uint32Array(y.length + 1);
    for (const c of x) {
      const next = new Uint32Array(y.length + 1);
      for (let j = 0; j < y.length; j++)
        next[j + 1] = c === y[j] ? row[j] + 1 : Math.max(row[j + 1], next[j]);
      row = next;
    }
    return row;
  }
  function walk(x: string[], y: string[]) {
    let prefix = 0;
    while (prefix < x.length && prefix < y.length && x[prefix] === y[prefix])
      prefix++;
    emit("equal", x.slice(0, prefix).join(""));
    x = x.slice(prefix);
    y = y.slice(prefix);
    let suffix = 0;
    while (
      suffix < x.length &&
      suffix < y.length &&
      x[x.length - 1 - suffix] === y[y.length - 1 - suffix]
    )
      suffix++;
    const tail = x.slice(x.length - suffix).join("");
    x = x.slice(0, x.length - suffix);
    y = y.slice(0, y.length - suffix);
    const alphabet = new Set(x);
    if (!x.length) emit("insert", y.join(""));
    else if (!y.length) emit("delete", x.join(""));
    else if (x.length === 1) {
      const match = y.indexOf(x[0]);
      if (match < 0) {
        emit("delete", x[0]);
        emit("insert", y.join(""));
      } else {
        emit("insert", y.slice(0, match).join(""));
        emit("equal", x[0]);
        emit("insert", y.slice(match + 1).join(""));
      }
    } else if (!y.some((c) => alphabet.has(c))) {
      emit("delete", x.join(""));
      emit("insert", y.join(""));
    } else {
      const mid = Math.floor(x.length / 2);
      const left = scores(x.slice(0, mid), y),
        right = scores(x.slice(mid).reverse(), [...y].reverse());
      let split = 0;
      for (let j = 1; j <= y.length; j++)
        if (
          left[j] + right[y.length - j] >
          left[split] + right[y.length - split]
        )
          split = j;
      walk(x.slice(0, mid), y.slice(0, split));
      walk(x.slice(mid), y.slice(split));
    }
    emit("equal", tail);
  }
  walk(a, b);
  for (let start = 0; start < result.length; ) {
    if (result[start].kind === "equal") {
      start++;
      continue;
    }
    let end = start;
    while (end < result.length && result[end].kind !== "equal") end++;
    const run = result.slice(start, end);
    const removed = run
      .filter((c) => c.kind === "delete")
      .map((c) => c.text)
      .join("");
    const added = run
      .filter((c) => c.kind === "insert")
      .map((c) => c.text)
      .join("");
    // Restrict normalization to width forms: ligatures and circled digits are not width edits.
    if (removed && added && foldWidth(removed) === foldWidth(added))
      for (const change of run) change.widthOnly = true;
    start = end;
  }
  return result;
}
