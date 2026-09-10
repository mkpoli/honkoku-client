import { parseLine } from "./syntax";
export { parse, serialize, parseLine } from "./syntax";
export type {
  SyntaxTree,
  SyntaxNode,
  SyntaxKind,
  SourceColumn,
} from "./syntax";
export type Inline =
  | {
      kind:
        | "text"
        | "raw"
        | "editorial"
        | "comment"
        | "glyph"
        | "rightLine"
        | "title"
        | "box"
        | "place";
      text: string;
    }
  | {
      kind: "ruby" | "warichu" | "correction" | "emphasis";
      base: string;
      annotation: string;
      segments?: string[];
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
  for (const node of parseLine(text)) {
    const parts = node.segments ?? [];
    const names = {
      ruby: "ruby",
      warigaki: "warichu",
      misekechi: "correction",
      kenten: "emphasis",
    } as const;
    if (node.kind in names) {
      nodes.push({
        kind: names[node.kind as keyof typeof names],
        base: parts[0],
        annotation: parts[1],
        ...(parts.length > 2 ? { segments: parts } : {}),
      });
    } else if (node.kind === "return" || node.kind === "okurigana") {
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
        if (node.kind === "return") previous.returnMark += parts[0];
        else previous.okurigana += parts[0];
      } else nodes.push({ kind: "text", text: node.source });
    } else if (node.kind === "reference")
      nodes.push({
        kind: "reference",
        number: Number(node.source.slice(1).normalize("NFKC")),
      });
    else
      nodes.push({
        kind:
          node.kind === "divider"
            ? "editorial"
            : node.kind === "gap"
              ? "glyph"
              : node.kind,
        text: parts[0] ?? node.source,
      } as Inline);
  }
  return nodes;
}
export function parseGroups(text: string): ColumnGroup[] {
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
        case "raw":
        case "text":
          return escape(node.text);
        case "rightLine":
        case "title":
        case "box":
        case "place":
        case "editorial":
        case "comment":
        case "glyph":
          return `<span class="markup-${node.kind}">${escape(node.text)}</span>`;
        case "ruby": {
          const ruby = `<ruby>${escape(node.base)}<rp>（</rp><rt>${escape(node.annotation)}</rt><rp>）</rp></ruby>`;
          return node.segments?.[2] === undefined
            ? ruby
            : `<ruby class="markup-double-ruby">${ruby}<rt class="markup-left-ruby">${escape(node.segments[2])}</rt></ruby>`;
        }
        case "warichu":
          return `<span class="markup-warichu">${(node.segments ?? [node.base, node.annotation]).map((part) => `<span>${escape(part)}</span>`).join("")}</span>`;
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
