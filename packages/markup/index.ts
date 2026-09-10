export type Inline =
  | { kind: "text" | "editorial" | "comment" | "glyph"; text: string }
  | {
      kind: "ruby" | "warichu" | "correction" | "emphasis";
      base: string;
      annotation: string;
    }
  | { kind: "reading"; base: string; returnMark: string; okurigana: string }
  | { kind: "reference"; number: number };
export interface ColumnGroup {
  label: "右丁" | "左丁" | "";
  columns: Inline[][];
}
const graphemes = new Intl.Segmenter("ja", { granularity: "grapheme" });
const escape = (text: string) =>
  text.replace(
    /[&<>"']/g,
    (ch) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        ch
      ]!,
  );
export function parseInline(text: string): Inline[] {
  const nodes: Inline[] = [];
  const pattern =
    /《([^《》]*)》|【([^【】]*)】|※[^\n]*|＃([0-9０-９]+)|＿([レ一二三四五六七八九十上中下甲乙丙丁天地人]+)|￣([\p{Script=Hiragana}\p{Script=Katakana}ー]+)|[■□〓]/gu;
  let offset = 0;
  const append = (text: string) => {
    if (text) nodes.push({ kind: "text", text });
  };
  for (const match of text.matchAll(pattern)) {
    append(text.slice(offset, match.index));
    offset = match.index! + match[0].length;
    if (match[1] !== undefined) {
      const parts = /^(振り仮名|割書|見せ消ち|圏点)：([^｜]*)｜([\s\S]*)$/.exec(
        match[1],
      );
      if (parts)
        nodes.push({
          kind: (
            {
              振り仮名: "ruby",
              割書: "warichu",
              見せ消ち: "correction",
              圏点: "emphasis",
            } as const
          )[parts[1] as "振り仮名"],
          base: parts[2],
          annotation: parts[3],
        });
      else append(match[0]);
    } else if (match[2] !== undefined)
      nodes.push({ kind: "editorial", text: match[0] });
    else if (match[3])
      nodes.push({
        kind: "reference",
        number: Number(match[3].normalize("NFKC")),
      });
    else if (match[4] || match[5]) {
      let previous = nodes.at(-1);
      if (previous?.kind === "text") {
        const chars = [...graphemes.segment(previous.text)].map(
          (g) => g.segment,
        );
        const base = chars.pop()!;
        previous.text = chars.join("");
        previous = { kind: "reading", base, returnMark: "", okurigana: "" };
        nodes.push(previous);
      }
      if (previous?.kind === "reading") {
        if (match[4]) previous.returnMark += match[4];
        else previous.okurigana += match[5];
      } else append(match[0]);
    } else
      nodes.push({
        kind: match[0].startsWith("※") ? "comment" : "glyph",
        text: match[0],
      });
  }
  append(text.slice(offset));
  return nodes;
}
export function parse(text: string): ColumnGroup[] {
  const groups: ColumnGroup[] = [];
  let group: ColumnGroup = { label: "", columns: [] };
  const chunks = text.replace(/\r\n?/g, "\n").split(/(【右丁】|【左丁】)/);
  for (const [index, chunk] of chunks.entries()) {
    if (chunk === "【右丁】" || chunk === "【左丁】") {
      if (group.columns.length || group.label) groups.push(group);
      group = { label: chunk === "【右丁】" ? "右丁" : "左丁", columns: [] };
    } else if (chunk) {
      let content = chunk;
      if (index > 0) content = content.replace(/^\n/, "");
      if (index + 1 < chunks.length) content = content.replace(/\n$/, "");
      const lines = content.split("\n");
      group.columns.push(...lines.map(parseInline));
    }
  }
  if (group.columns.length || group.label) groups.push(group);
  return groups;
}
export function renderInline(nodes: Inline[]): string {
  return nodes
    .map((node) => {
      switch (node.kind) {
        case "text":
          return escape(node.text);
        case "editorial":
        case "comment":
        case "glyph":
          return `<span class="markup-${node.kind}">${escape(node.text)}</span>`;
        case "ruby":
          return `<ruby>${escape(node.base)}<rp>（</rp><rt>${escape(node.annotation)}</rt><rp>）</rp></ruby>`;
        case "warichu":
          return `<span class="markup-warichu"><span>${escape(node.base)}</span><span>${escape(node.annotation)}</span></span>`;
        case "correction":
          return `<span class="markup-correction"><s>${escape(node.base)}</s><span>${escape(node.annotation)}</span></span>`;
        case "emphasis":
          return `<span class="markup-emphasis">${[...graphemes.segment(node.base)].map((g) => `<ruby>${escape(g.segment)}<rt>${escape(node.annotation)}</rt></ruby>`).join("")}</span>`;
        case "reference":
          return `<button class="markup-reference" data-note="${node.number}" aria-label="注記${node.number}">＃${node.number}</button>`;
        case "reading":
          return `<span class="markup-reading">${escape(node.base)}<span class="markup-return">${escape(node.returnMark)}</span><span class="markup-okurigana">${escape(node.okurigana)}</span></span>`;
      }
    })
    .join("");
}
