import { parseLine } from "./syntax";
export type ExportFormat = "txt" | "xml" | "tex";
export interface ExportPage {
  index: number;
  text: string;
}
const xml = (s: string) =>
  s.replace(
    /[&<>"']/g,
    (c) =>
      ({
        "&": "&amp;",
        "<": "&lt;",
        ">": "&gt;",
        '"': "&quot;",
        "'": "&apos;",
      })[c]!,
  );
const tex = (s: string) =>
  s.replace(
    /[\\{}$&#%_^~]/g,
    (c) =>
      ({
        "\\": "\\textbackslash{}",
        "^": "\\textasciicircum{}",
        "~": "\\textasciitilde{}",
      })[c] ?? `\\${c}`,
  );
function inline(source: string, format: "xml" | "tex"): string {
  const escape = format === "xml" ? xml : tex;
  // These site wrappers predate the bracket notation used by the editor.
  const parsed = parseLine(source);
  const wrapper =
    /｛＿([レ一二三上中下甲乙丙丁天地人])｝|｛([^｛｝]+)｝|〔([^〔〕]+)〕|＜([^＜＞]+)＞/gu;
  let output = "",
    start = 0;
  for (const m of source.matchAll(wrapper)) {
    if (
      parsed.some(
        (n) =>
          n.from <= m.index &&
          n.to >= m.index + m[0].length &&
          n.kind !== "text" &&
          n.kind !== "raw",
      )
    )
      continue;
    output += nodes(source.slice(start, m.index), format);
    const content = m[1] ?? m[2] ?? m[3] ?? m[4];
    const body = inline(content, format);
    output += m[1]
      ? format === "xml"
        ? `<metamark function="kaeriten">${body}</metamark>`
        : `\\kaeriten{ ${body} }`
      : format === "xml"
        ? `<${m[2] ? "persName" : m[3] ? "placeName" : "date"}>${body}</${m[2] ? "persName" : m[3] ? "placeName" : "date"}>`
        : escape(m[0]);
    start = m.index + m[0].length;
  }
  return output + nodes(source.slice(start), format);
}
function nodes(source: string, format: "xml" | "tex"): string {
  const escape = format === "xml" ? xml : tex;
  const grouped = parseLine(source).reduce<ReturnType<typeof parseLine>>(
    (nodes, node) => {
      const previous = nodes.at(-1);
      if (
        node.kind === "gap" &&
        previous?.kind === "gap" &&
        previous.source[0] === node.source[0]
      )
        previous.source += node.source;
      else nodes.push({ ...node });
      return nodes;
    },
    [],
  );
  return grouped
    .map((n) => {
      const p = (n.segments ?? []).map((s) => inline(s, format));
      const macro = (name: string, fields = p) =>
        `\\${name}${fields.map((s) => `{ ${s} }`).join("")}`;
      if (format === "xml")
        switch (n.kind) {
          case "ruby": {
            let value = `<ruby><rb>${p[0]}</rb><rt>${p[1]}</rt></ruby>`;
            if (p[2]) value = `<ruby><rb>${value}</rb><rt>${p[2]}</rt></ruby>`;
            return value;
          }
          case "warigaki":
            return `<note type="wari">${p.map((s) => `${s}<milestone unit="wrb"/>`).join("")}</note>`;
          case "misekechi":
            return `<subst><del>${p[0]}</del><add>${p[1]}</add></subst>`;
          case "kenten":
            return `<seg style="text-emphasis: filled sesame">${p[0]}</seg>`;
          case "return":
            return `<metamark function="kaeriten">${p[0]}</metamark>`;
          case "okurigana":
            return `<note type="okuri">${p[0]}</note>`;
          case "editorial":
            return `<!-- ${xml(n.source.slice(1, -1))
              .replace(/-+/g, (s) => [...s].join(" "))
              .replace(/-$/, "- ")} -->`;
          case "reference":
            return `<note n="${xml(n.source.slice(1))}">${xml(n.source.slice(1))}</note>`;
          case "gap":
            return `<gap quantity="${Array.from(n.source).length}" unit="chars" reason="${n.source[0] === "□" ? "wormhole" : "illegible"}"/>`;
          case "title":
            return `<docTitle>${p[0]}</docTitle>`;
          case "place":
            return `<placeName>${p[0]}</placeName>`;
        }
      else
        switch (n.kind) {
          case "ruby": {
            const right = macro("MigiNakaTn", p.slice(0, 2));
            return p[2] ? macro("HidariNakaTn", [right, p[2]]) : right;
          }
          case "warigaki":
            return macro(
              p.length === 4
                ? "yongyouwari"
                : p.length === 3
                  ? "sangyouwari"
                  : "sougyou",
            );
          case "misekechi":
            return macro("sout", [macro("MigiKataTn")]);
          case "kenten":
            return macro("bou", p.slice(0, 1));
          case "return":
            return macro("kaeriten");
          case "okurigana":
            return macro("kokana") + "{}";
          case "editorial":
            return `\\textcolor{red}{${tex(n.source)}}`;
          case "reference":
            return tex(n.source.slice(1));
        }
      return escape(n.source);
    })
    .join("");
}
function pageBody(source: string, format: "xml" | "tex") {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const stack: string[] = [];
  const body = lines
    .map((line) => {
      const block = /^％(表紙|字下げ[一二三])$/.exec(line);
      if (block) {
        if (format === "tex") {
          stack.push("");
          return "";
        }
        const tag = block[1] === "表紙" ? "titlePage" : "div";
        stack.push(tag);
        return tag === "titlePage"
          ? "<titlePage>"
          : `<div rend="indent(-${{ 一: 1, 二: 2, 三: 3 }[block[1].slice(-1)]})">`;
      }
      if (line === "％" && stack.length) {
        const tag = stack.pop();
        return format === "xml" ? `</${tag}>` : "";
      }
      return (format === "xml" ? "<lb/>" : "") + inline(line, format);
    })
    .join("\n");
  return (
    body +
    (format === "xml"
      ? stack
          .reverse()
          .map((tag) => `</${tag}>`)
          .join("")
      : "")
  );
}
export function exportTranscription(
  pages: readonly ExportPage[],
  format: ExportFormat,
): string {
  const ordered = [...pages].sort((a, b) => a.index - b.index);
  if (format === "txt")
    return ordered.map((p) => p.text.replace(/\r\n?/g, "\n")).join("\n");
  if (format === "xml")
    return `<?xml version="1.0" encoding="UTF-8"?>\n<text xmlns="http://www.tei-c.org/ns/1.0"><body>\n${ordered.map((p) => `<pb n="${p.index + 1}"/>\n${pageBody(p.text, format)}`).join("\n")}\n</body></text>\n`;
  return (
    "% kunten2e.styを利用して組版してください。\n\\documentclass[dvipdfmx,autodetect-engine]{utbook}\n\\usepackage{kunten2e,color}\n\\usepackage[normalem]{ulem}\n\\begin{document}\n" +
    ordered
      .map((p) => `% ${p.index + 1}コマ\n${pageBody(p.text, format)}`)
      .join("\n\\newpage\n") +
    "\n\\end{document}\n"
  );
}
