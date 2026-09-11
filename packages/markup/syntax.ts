import { legacyRuby, legacyTextRun } from "./legacy";
export type SyntaxKind =
  | "text"
  | "raw"
  | "ruby"
  | "warigaki"
  | "misekechi"
  | "kenten"
  | "rightLine"
  | "title"
  | "box"
  | "place"
  | "note"
  | "person"
  | "date"
  | "editorial"
  | "divider"
  | "gap"
  | "reference"
  | "comment"
  | "return"
  | "okurigana";
export interface SyntaxNode {
  kind: SyntaxKind;
  source: string;
  from: number;
  to: number;
  segments?: string[];
  form?: "bracket" | "legacy";
}
export interface SourceColumn {
  kind: "column";
  nodes: SyntaxNode[];
  ending: { kind: "newline"; source: string; from: number; to: number };
}
export interface SyntaxTree {
  kind: "document";
  columns: SourceColumn[];
}
const constructs: Record<string, [SyntaxKind, number, number]> = {
  振り仮名: ["ruby", 2, 3],
  割書: ["warigaki", 2, 4],
  見せ消ち: ["misekechi", 2, 2],
  圏点: ["kenten", 2, 2],
  右線: ["rightLine", 1, 1],
  題: ["title", 1, 1],
  箱: ["box", 1, 1],
  場所: ["place", 1, 1],
  注記: ["note", 1, 1],
  人物: ["person", 1, 1],
  日時: ["date", 1, 1],
};
/** Editable children of each compound field. */
export function allowsChild(parent: string, child: string): boolean {
  if (
    [
      "text",
      "raw",
      "gap",
      "reference",
      "return",
      "okurigana",
      "editorial",
    ].includes(child)
  )
    return true;
  if (parent === "column") return child !== "comment" && child !== "divider";
  return (
    Object.values(constructs).some(([kind]) => kind === parent) &&
    Object.values(constructs).some(([kind]) => kind === child)
  );
}
export function splitFields(source: string): string[] {
  const fields: string[] = [];
  let depth = 0,
    start = 0;
  for (let i = 0; i < source.length; i++) {
    if (source[i] === "《" || source[i] === "【" || source[i] === "（") depth++;
    else if (source[i] === "》" || source[i] === "】" || source[i] === "）")
      depth--;
    else if (source[i] === "｜" && depth === 0) {
      fields.push(source.slice(start, i));
      start = i + 1;
    }
  }
  fields.push(source.slice(start));
  return fields;
}
export function parseLine(text: string, start = 0): SyntaxNode[] {
  const nodes: SyntaxNode[] = [];
  let pos = 0;
  function add(
    kind: SyntaxKind,
    size: number,
    segments?: string[],
    form?: SyntaxNode["form"],
  ) {
    const source = text.slice(pos, pos + size);
    const last = nodes.at(-1);
    if (kind === "text" && last?.kind === "text") {
      last.source += source;
      last.to += size;
    } else
      nodes.push({
        kind,
        source,
        from: start + pos,
        to: start + pos + size,
        ...(segments ? { segments } : {}),
        ...(form ? { form } : {}),
      });
    pos += size;
  }
  while (pos < text.length) {
    const rest = text.slice(pos);
    if (rest.startsWith("※")) {
      add("comment", rest.length);
      continue;
    }
    const legacy = legacyRuby(rest);
    if (legacy) {
      add(
        legacy.segments ? "ruby" : "raw",
        legacy.size,
        legacy.segments,
        "legacy",
      );
      continue;
    }
    if (rest[0] === "《" || rest[0] === "【") {
      const open = rest[0],
        close = open === "《" ? "》" : "】";
      let depth = 0,
        end = 0;
      for (; end < rest.length; end++) {
        if (rest[end] === open) {
          depth++;
        }
        if (rest[end] === close && --depth === 0) {
          end++;
          break;
        }
      }
      const token = rest.slice(0, end);
      if (depth !== 0) {
        add("raw", end);
        continue;
      }
      if (open === "【") {
        add(/^【[右左]丁】$/.test(token) ? "divider" : "editorial", end);
        continue;
      }
      const match = /^《([^：]+)：([\s\S]*)》$/.exec(token);
      const rule = match && constructs[match[1]];
      const segments = match ? splitFields(match[2]) : undefined;
      if (
        rule &&
        segments &&
        segments.length >= rule[1] &&
        segments.length <= rule[2] &&
        segments.every((field) =>
          parseLine(field).every(
            (child) =>
              allowsChild(rule[0], child.kind) &&
              !(child.kind === "raw" && /[《》【】]/u.test(child.source)),
          ),
        )
      )
        add(rule[0], end, segments, "bracket");
      else add("raw", end);
      continue;
    }
    const reference = /^＃[0-9０-９]+/u.exec(rest);
    if (reference) {
      add("reference", reference[0].length);
      continue;
    }
    const returning = /^＿[レ一二三上中下甲乙丙丁天地人]/u.exec(rest);
    if (returning) {
      add("return", returning[0].length, [returning[0].slice(1)]);
      continue;
    }
    const okuri = /^￣[\p{Script=Hiragana}\p{Script=Katakana}ー]+/u.exec(rest);
    if (okuri) {
      add("okurigana", okuri[0].length, [okuri[0].slice(1)]);
      continue;
    }
    if (/[■□〓]/u.test(rest[0])) {
      add("gap", 1);
      continue;
    }
    if (/[《》【】＿￣＃（）｜]/u.test(rest[0])) {
      add("raw", 1);
      continue;
    }
    add(
      "text",
      legacyTextRun(rest).length ||
        String.fromCodePoint(rest.codePointAt(0)!).length,
    );
  }
  return nodes;
}
export function parse(source: string): SyntaxTree {
  const columns: SourceColumn[] = [];
  let start = 0;
  for (const match of source.matchAll(/\r\n|\r|\n/g)) {
    const end = match.index!;
    columns.push({
      kind: "column",
      nodes: parseLine(source.slice(start, end), start),
      ending: {
        kind: "newline",
        source: match[0],
        from: end,
        to: end + match[0].length,
      },
    });
    start = end + match[0].length;
  }
  columns.push({
    kind: "column",
    nodes: parseLine(source.slice(start), start),
    ending: {
      kind: "newline",
      source: "",
      from: source.length,
      to: source.length,
    },
  });
  return { kind: "document", columns };
}
export function serialize(tree: SyntaxTree): string {
  return tree.columns
    .map((c) => c.nodes.map((n) => n.source).join("") + c.ending.source)
    .join("");
}
