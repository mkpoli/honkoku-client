import { parseLine } from "./syntax";
export { parse, serialize, parseLine, allowsChild } from "./syntax";
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
        | "place"
        | "note"
        | "person"
        | "date";
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
        case "note":
        case "person":
        case "date":
          return `<span class="markup-${node.kind} editor-${node.kind}">${renderInline(parseInline(node.text))}</span>`;
        case "editorial":
        case "comment":
        case "glyph":
          return `<span class="markup-${node.kind} editor-${node.kind}">${escape(node.text)}</span>`;
        case "ruby":
        case "warichu":
        case "correction": {
          const kind = {
            ruby: "ruby",
            warichu: "warigaki",
            correction: "misekechi",
          }[node.kind];
          const values = node.segments ?? [node.base, node.annotation];
          const tag = kind === "ruby" ? "ruby" : "span";
          return `<${tag} class="editor-annotation editor-${kind}">${values
            .map((part, i) => {
              const role =
                kind === "warigaki"
                  ? `line-${i}`
                  : i === 0
                    ? "base"
                    : i === 1
                      ? "right"
                      : "left";
              const content = renderInline(parseInline(part));
              return `<span class="editor-segment editor-${role}">${kind === "ruby" && i > 0 ? `<rt class="editor-reading">${content}</rt>` : content}</span>`;
            })
            .join("")}</${tag}>`;
        }
        case "emphasis":
          return `<span class="editor-annotation editor-kenten" style="--kenten-mark: &quot;${escape(node.annotation)}&quot;"><span class="editor-segment editor-base">${renderInline(parseInline(node.base))}</span></span>`;
        case "reference":
          return `<button class="markup-reference" data-note="${node.number}" aria-label="注記${node.number}">＃${node.number}</button>`;
        case "reading":
          return `<span class="markup-reading">${escape(node.base)}${[
            ["return", node.returnMark],
            ["okurigana", node.okurigana],
          ]
            .filter(([, value]) => value)
            .map(([kind, value], i) => {
              const length = Math.max(
                [...node.returnMark].length,
                [...node.okurigana].length,
              );
              return `<span class="markup-${kind} kunten-mark kunten-${kind}" style="--kunten-length:${length};--kunten-back:${i === 0 ? 0 : length};--kunten-offset:0">${escape(value)}</span>`;
            })
            .join("")}</span>`;
      }
    })
    .join("");
}

export { alignColumns, transcriptionColumns, plainColumn } from "./align";
export { diffSource } from "./diff";
export { exportTranscription } from "./export";
export type { ExportFormat, ExportPage } from "./export";
export { elements } from "./elements";

/** Render legacy semantic wrappers without changing the saved source or editor tree. */
export function renderReadingLine(source: string): string {
  const nodes=parseLine(source);
  const normalized=source.replace(/｛＿([レ一二三上中下甲乙丙丁天地人])｝|｛([^｛｝]+)｝|〔([^〔〕]+)〕|＜([^＜＞]+)＞/gu,
    (whole:string, mark:string|undefined, person:string|undefined, place:string|undefined, date:string|undefined, offset:number)=> {
      if(nodes.some(n=>n.from<=offset && n.to>=offset+whole.length && n.kind!=="text" && n.kind!=="raw")) return whole;
      return mark ? `＿${mark}` : person ?? place ?? date ?? whole;
    });
  return renderInline(parseInline(normalized));
}
