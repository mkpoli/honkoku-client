import {
  Slice,
  Schema,
  type Node as PMNode,
  type NodeSpec,
} from "prosemirror-model";
import {
  EditorState,
  Plugin,
  TextSelection,
  NodeSelection,
  type Command,
  type Transaction,
} from "prosemirror-state";
import { Decoration, DecorationSet, EditorView } from "prosemirror-view";
import { history, undo, redo, closeHistory } from "prosemirror-history";
import { keymap } from "prosemirror-keymap";
import { baseKeymap } from "prosemirror-commands";
import {
  transcriptionColumns,
  parse,
  parseLine,
  allowsChild,
  type SyntaxNode,
} from "@honkoku/markup";
export { undo, redo, TextSelection, NodeSelection };
import {
  caretPlugin,
  contextName,
  contextNames,
  selectContext,
  escapeContext,
  enterContext,
  deleteContext,
  moveCharacter,
  characterPositions,
  contextSelection,
} from "./caret";
export * from "./caret";

const annotationNames = Object.fromEntries(
  Object.entries(contextNames).filter(
    ([kind]) => !["return", "okurigana"].includes(kind),
  ),
);
const nodes: Record<string, NodeSpec> = {
  doc: { content: "column+", attrs: { newline: { default: "\n" } } },
  column: {
    content: "inline*",
    attrs: { ending: { default: "" } },
    toDOM: () => ["div", { class: "transcription-column" }, 0],
    parseDOM: [{ tag: "div.transcription-column" }],
  },
  text: { group: "inline" },
  segment: {
    inline: true,
    content: "inline*",
    attrs: {
      role: { default: "base" },
      reading: { default: false },
      placeholder: { default: false },
    },
    toDOM: (node) => [
      "span",
      {
        class: `editor-segment editor-${node.attrs.role}`,
        "data-segment": node.attrs.role,
        "data-placeholder": String(node.attrs.placeholder),
      },
      node.attrs.reading ? ["rt", { class: "editor-reading" }, 0] : 0,
    ],
    parseDOM: [
      {
        tag: "[data-segment]",
        getAttrs: (e) => ({
          role: e.getAttribute("data-segment"),
          reading: !!e.querySelector("rt"),
          placeholder: e.getAttribute("data-placeholder") === "true",
        }),
      },
    ],
  },
  raw: {
    inline: true,
    group: "inline",
    content: "text*",
    toDOM: () => ["span", { class: "editor-raw", "data-raw": "true" }, 0],
    parseDOM: [{ tag: "span[data-raw]" }],
  },
  source: {
    inline: true,
    group: "inline",
    atom: true,
    attrs: { source: { default: "" }, kind: { default: "gap" } },
    toDOM: (node) => [
      node.attrs.kind === "reference" ? "button" : "span",
      {
        ...(node.attrs.kind === "reference"
          ? {
              type: "button",
              "data-note": String(
                Number(node.attrs.source.slice(1).normalize("NFKC")),
              ),
              "aria-label": `注記${Number(node.attrs.source.slice(1).normalize("NFKC"))}`,
            }
          : {}),
        class: `editor-source-anchor editor-token markup-${node.attrs.kind === "gap" ? "glyph" : node.attrs.kind}`,
        "data-source": node.attrs.source,
        "data-kind": node.attrs.kind,
        "data-context-label": contextName(node),
        contenteditable: "false",
      },
      node.attrs.source,
    ],
    parseDOM: [
      {
        tag: "span[data-source]",
        getAttrs: (e) => ({
          source: e.getAttribute("data-source"),
          kind: e.getAttribute("data-kind"),
        }),
      },
    ],
  },
};
for (const name of [...Object.keys(annotationNames), "return", "okurigana"]) {
  const count =
    name === "ruby"
      ? "{2,3}"
      : name === "warigaki"
        ? "{2,4}"
        : ["misekechi", "kenten"].includes(name)
          ? "{2}"
          : "{1}";
  nodes[name] = {
    inline: true,
    group: "inline",
    content: `segment${count}`,
    isolating: true,
    atom: name === "return",
    attrs: {
      original: { default: null },
      originalParts: { default: null },
      form: { default: "bracket" },
    },
    toDOM: (node) => [
      name === "ruby" ? "ruby" : "span",
      {
        class: `editor-annotation editor-${name}`,
        "data-annotation": name,
        "data-form": node.attrs.form,
        "data-context-label": contextName(node),
        ...(name === "return" ? { contenteditable: "false" } : {}),
      },
      0,
    ],
    parseDOM: [
      {
        tag: `[data-annotation="${name}"]`,
        getAttrs: (e) => ({ form: e.getAttribute("data-form") ?? "bracket" }),
      },
    ],
  };
}
export const schema = new Schema({ nodes });
function parts(node: PMNode): string[] {
  const values: string[] = [];
  node.forEach((child) => values.push(columnSource(child)));
  return values;
}
export function annotation(
  kind: string,
  values: string[],
  attrs: Record<string, unknown> = {},
): PMNode {
  const roles =
    kind === "warigaki"
      ? values.map((_, i) => `line-${i}`)
      : values.map((_, i) => (i === 0 ? "base" : i === 1 ? "right" : "left"));
  return schema.nodes[kind].createChecked(
    attrs,
    values.map((text, i) =>
      schema.nodes.segment.create(
        {
          role: roles[i],
          reading: kind === "ruby" && i > 0,
          placeholder: !text,
        },
        text ? parseLine(text).map(project) : schema.text("\u200b"),
      ),
    ),
  );
}
function project(node: SyntaxNode): PMNode {
  if (node.kind === "text") return schema.text(node.source);
  if (node.kind === "raw")
    return schema.nodes.raw.create(null, schema.text(node.source));
  if (node.segments)
    return annotation(node.kind, node.segments, {
      original: node.source,
      originalParts: node.segments,
      form: node.form ?? "bracket",
    });
  return schema.nodes.source.create({ kind: node.kind, source: node.source });
}
export function fromMarkup(source: string): PMNode {
  const tree = parse(source);
  return schema.nodes.doc.create(
    { newline: source.match(/\r\n|\r|\n/)?.[0] ?? "\n" },
    tree.columns.map((c) =>
      schema.nodes.column.create(
        { ending: c.ending.source },
        c.nodes.map(project),
      ),
    ),
  );
}
function inlineSource(node: PMNode): string {
  if (node.isText || node.type.name === "raw") return node.textContent;
  if (node.type.name === "segment") return columnSource(node);
  if (node.type.name === "source") return node.attrs.source;
  const values = parts(node);
  if (
    node.attrs.original !== null &&
    JSON.stringify(values) === JSON.stringify(node.attrs.originalParts)
  )
    return node.attrs.original;
  if (node.type.name === "return") return `＿${values[0]}`;
  if (node.type.name === "okurigana") return `￣${values[0]}`;
  if (node.type.name === "ruby" && node.attrs.form === "legacy")
    return `${node.attrs.original?.startsWith("／") ? "／" : ""}${values[0]}（${values.slice(1).join("｜")}）`;
  return `《${annotationNames[node.type.name]}：${values.join("｜")}》`;
}
export function columnSource(column: PMNode): string {
  let value = "";
  column.forEach((node) => (value += inlineSource(node)));
  return column.attrs.placeholder ? value.replace("\u200b", "") : value;
}
export function toMarkup(doc: PMNode): string {
  let source = "";
  doc.forEach(
    (column, _, index) =>
      (source +=
        columnSource(column) +
        (index < doc.childCount - 1
          ? column.attrs.ending || doc.attrs.newline
          : "")),
  );
  return source;
}
/** Preserve wrappers only for complete shells; any selected kunten retains its prefix. */
export function selectedMarkup(doc: PMNode, from: number, to: number): string {
  function selected(node: PMNode, pos: number): string {
    const end = pos + node.nodeSize;
    if (to <= pos || from >= end) return "";
    if (node.isText)
      return node.text!.slice(
        Math.max(0, from - pos),
        Math.min(node.nodeSize, to - pos),
      );
    if (from <= pos && to >= end && node.type.name !== "column")
      return inlineSource(node);
    const values: string[] = [];
    node.forEach((child, offset) => {
      const start = pos + 1 + offset;
      if (to > start && from < start + child.nodeSize)
        values.push(selected(child, start));
    });
    const text = values.join(
      node.firstChild?.type.name === "segment" ? "｜" : "",
    );
    if (text && node.type.name === "okurigana") return `￣${text}`;
    if (text && node.type.name === "return") return `＿${text}`;
    return node.attrs.placeholder ? text.replace("\u200b", "") : text;
  }
  let result = "";
  doc.forEach((column, pos, index) => {
    result += selected(column, pos);
    if (
      index < doc.childCount - 1 &&
      from < pos + column.nodeSize &&
      to > pos + column.nodeSize
    )
      result += column.attrs.ending || doc.attrs.newline;
  });
  return result;
}
export function clipboardMarkup(text: string): Slice {
  return new Slice(fromMarkup(text).content, 1, 1);
}
export interface SourcePatch {
  from: number;
  to: number;
  text: string;
}
export function applyPatches(source: string, patches: SourcePatch[]): string {
  for (const patch of [...patches].sort((a, b) => b.from - a.from))
    source = source.slice(0, patch.from) + patch.text + source.slice(patch.to);
  return source;
}
export function sourcePatches(tr: Transaction): SourcePatch[] {
  if (!tr.docChanged) return [];
  const before = tr.before,
    after = tr.doc;
  const lines = (doc: PMNode) => {
    const result: { node: PMNode; value: string; offset: number }[] = [];
    let offset = 0;
    doc.forEach((node, _, i) => {
      const value =
        columnSource(node) +
        (i < doc.childCount - 1 ? node.attrs.ending || doc.attrs.newline : "");
      result.push({ node, value, offset });
      offset += value.length;
    });
    return result;
  };
  const a = lines(before),
    b = lines(after),
    patches: SourcePatch[] = [];
  // Unchanged persistent nodes are anchors even when columns are inserted or removed.
  let ai = 0,
    bi = 0;
  while (ai < a.length || bi < b.length) {
    if (ai < a.length && bi < b.length && a[ai].value === b[bi].value) {
      ai++;
      bi++;
      continue;
    }
    let nextA = a.length,
      nextB = b.length;
    outer: for (let j = bi; j < b.length; j++)
      for (let i = ai; i < a.length; i++) {
        if (a[i].node === b[j].node && a[i].value === b[j].value) {
          nextA = i;
          nextB = j;
          break outer;
        }
      }
    const old = a
      .slice(ai, nextA)
      .map((x) => x.value)
      .join("");
    const replacement = b
      .slice(bi, nextB)
      .map((x) => x.value)
      .join("");
    let prefix = 0,
      suffix = 0;
    while (
      prefix < old.length &&
      prefix < replacement.length &&
      old[prefix] === replacement[prefix]
    )
      prefix++;
    while (
      suffix < old.length - prefix &&
      suffix < replacement.length - prefix &&
      old[old.length - 1 - suffix] ===
        replacement[replacement.length - 1 - suffix]
    )
      suffix++;
    if (old !== replacement) {
      const offset = a[ai]?.offset ?? toMarkup(before).length;
      patches.push({
        from: offset + prefix,
        to: offset + old.length - suffix,
        text: replacement.slice(prefix, replacement.length - suffix),
      });
    }
    ai = nextA;
    bi = nextB;
  }
  return patches;
}
export type ConstructKind = "ruby" | "warigaki" | "misekechi";
function canInsert(state: EditorState, kind: string): boolean {
  const { $from, $to } = state.selection;
  if (!$from.sameParent($to)) return false;
  const parent =
    $from.parent.type.name === "segment"
      ? $from.node($from.depth - 1).type.name
      : $from.parent.type.name;
  if (!allowsChild(parent, kind)) return false;
  let valid = true;
  state.selection.content().content.descendants((node) => {
    if (node.type.name === "column" || node.type.name === "segment") return;
    if (
      !allowsChild(
        kind,
        node.type.name === "source" ? node.attrs.kind : node.type.name,
      )
    )
      valid = false;
  });
  return valid;
}
export const insertAnnotation =
  (kind: ConstructKind, values: string[]): Command =>
  (state, dispatch) => {
    if (!canInsert(state, kind)) return false;
    const node = annotation(kind, values);
    const from = state.selection.from;
    const tr = closeHistory(state.tr).replaceSelectionWith(node, false);
    dispatch?.(
      tr.setSelection(TextSelection.create(tr.doc, from + 2)).scrollIntoView(),
    );
    return true;
  };
export const wrapSelection =
  (kind: ConstructKind, reading = ""): Command =>
  (state, dispatch) => {
    if (!canInsert(state, kind)) return false;
    const { from, to } = state.selection;
    const value = columnSource(
      state.selection.$from.parent.cut(
        state.selection.$from.parentOffset,
        state.selection.$to.parentOffset,
      ),
    );
    const node = annotation(kind, [kind === "warigaki" ? "" : value, reading]);
    const tr = closeHistory(state.tr).replaceWith(from, to, node);
    const first = from + 2;
    const target = first;
    dispatch?.(
      tr.setSelection(TextSelection.create(tr.doc, target)).scrollIntoView(),
    );
    return true;
  };
export const selectColumn: Command = (state, dispatch) => {
  const { $head, from, to } = state.selection;
  const start = $head.start(1),
    end = $head.end(1);
  dispatch?.(
    state.tr.setSelection(
      TextSelection.create(
        state.doc,
        from === start && to === end ? 1 : start,
        from === start && to === end ? state.doc.content.size - 1 : end,
      ),
    ),
  );
  return true;
};
/** Navigate the closest editable compound without splitting its source column. */
export function shellKey(key: string, backwards = false): Command {
  return (state, dispatch) => {
    const { $head } = state.selection;
    if ($head.parent.type.name !== "segment") return false;
    const depth = $head.depth - 1,
      node = $head.node(depth);
    const index = $head.index(depth),
      start = $head.before(depth);
    const move = (pos: number, tr = state.tr) => {
      dispatch?.(
        tr.setSelection(TextSelection.create(tr.doc, pos)).scrollIntoView(),
      );
      return true;
    };

    if (key === "Tab" || (key === "Enter" && node.type.name === "warigaki")) {
      const next = index + (backwards ? -1 : 1);
      if (next < 0) return move(start);
      if (next < node.childCount) {
        let pos = start + 2;
        for (let i = 0; i < next; i++) pos += node.child(i).nodeSize;
        return move(pos);
      }
      if (key === "Enter" && node.childCount < 4) {
        const pos = start + node.nodeSize - 1;
        const tr = state.tr.insert(
          pos,
          schema.nodes.segment.create(
            { role: `line-${next}`, placeholder: true },
            schema.text("\u200b"),
          ),
        );
        return move(pos + 1, tr);
      }
      if (key === "Enter") return true;
      return move(start + node.nodeSize);
    }
    return false;
  };
}
export const insertText =
  (text: string): Command =>
  (state, dispatch) => {
    dispatch?.(closeHistory(state.tr).insertText(text).scrollIntoView());
    return true;
  };
const isKunten = (node: PMNode | null | undefined) =>
  !!node && ["return", "okurigana"].includes(node.type.name);
export const insertOkurigana =
  (kana: string): Command =>
  (state, dispatch) => {
    if (!/^[ァ-ヶ]{1,8}$/u.test(kana) || !canInsert(state, "okurigana"))
      return false;
    const { $to, to } = state.selection;
    if (!$to.nodeBefore) return false;
    const node = annotation("okurigana", [kana]);
    const tr = closeHistory(state.tr).insert(to, node);
    dispatch?.(
      tr.setSelection(TextSelection.create(tr.doc, to + 2)).scrollIntoView(),
    );
    return true;
  };
export const okuriganaFromSelection: Command = (state, dispatch) => {
  const { empty, $from, $to, from, to } = state.selection;
  if (empty || !$from.sameParent($to) || !canInsert(state, "okurigana"))
    return false;
  const kana = state.doc
    .textBetween(from, to, "\ufffc", "\ufffc")
    .normalize("NFC")
    .replace(/[ぁ-ゖ]/gu, (c) => String.fromCodePoint(c.codePointAt(0)! + 0x60));
  if (!/^[ァ-ヶ]{1,8}$/u.test(kana) || !$from.nodeBefore) return false;
  const node = annotation("okurigana", [kana]);
  const tr = closeHistory(state.tr).replaceWith(from, to, node);
  dispatch?.(
    tr
      .setSelection(TextSelection.create(tr.doc, from + node.nodeSize))
      .scrollIntoView(),
  );
  return true;
};
export const extendOkurigana =
  (text: string): Command =>
  (state, dispatch) => {
    const { empty, $from, from } = state.selection;
    if (!empty || !/^[ァ-ヶ]+$/u.test(text)) return false;
    const previous = $from.nodeBefore;
    if (previous?.type.name === "okurigana") {
      const node = annotation("okurigana", [parts(previous)[0] + text]);
      const tr = state.tr.replaceWith(from - previous.nodeSize, from, node);
      dispatch?.(
        tr
          .setSelection(
            TextSelection.create(
              tr.doc,
              from - previous.nodeSize + node.nodeSize,
            ),
          )
          .scrollIntoView(),
      );
      return true;
    }
    if (
      (previous?.isText || previous?.type.name === "raw") &&
      previous.textContent.endsWith("￣")
    ) {
      const start =
        from - (previous.type.name === "raw" ? previous.nodeSize : 1);
      const node = annotation("okurigana", [text]);
      const tr = state.tr.replaceWith(start, from, node);
      dispatch?.(
        tr
          .setSelection(TextSelection.create(tr.doc, start + node.nodeSize))
          .scrollIntoView(),
      );
      return true;
    }
    return false;
  };
/** IME commits arrive as document mutations rather than insertText events. */
export const normalizeTypedOkurigana: Command = (state, dispatch) => {
  const { empty, $from, from } = state.selection;
  const text = $from.nodeBefore;
  if (!empty || !text?.isText || $from.parent.type.name === "raw") return false;
  const prefixed = /￣([ァ-ヶ]+)$/u.exec(text.text!);
  let start: number;
  let kana: string;
  if (prefixed) {
    start = from - prefixed[0].length;
    kana = prefixed[1];
  } else {
    if (!/^[ァ-ヶ]+$/u.test(text.text!)) return false;
    const previous = $from.parent.childBefore(
      $from.parentOffset - text.nodeSize,
    ).node;
    if (previous?.type.name !== "okurigana") return false;
    start = from - text.nodeSize - previous.nodeSize;
    kana = parts(previous)[0] + text.text!;
  }
  const node = annotation("okurigana", [kana]);
  const tr = state.tr.replaceWith(start, from, node);
  dispatch?.(
    tr.setSelection(TextSelection.create(tr.doc, start + node.nodeSize)),
  );
  return true;
};
export const insertSource =
  (text: string): Command =>
  (state, dispatch) => {
    if (!canInsert(state, "reference")) return false;
    const node = parse(text).columns[0].nodes[0];
    if (!node) return false;
    const projected = project(node);
    const from = state.selection.from;
    const tr = closeHistory(state.tr).replaceSelectionWith(projected, false);
    dispatch?.(
      tr
        .setSelection(
          TextSelection.create(
            tr.doc,
            from + (projected.isAtom ? projected.nodeSize : 2),
          ),
        )
        .scrollIntoView(),
    );
    return true;
  };
export const replaceSource =
  (text: string): Command =>
  (state, dispatch) => {
    const next = fromMarkup(text);
    if (next.eq(state.doc)) return true;
    const tr = closeHistory(state.tr)
      .replaceWith(0, state.doc.content.size, next.content)
      .setDocAttribute("newline", next.attrs.newline)
      .setMeta("replaceSource", true);
    dispatch?.(tr);
    return true;
  };
export function moveCaret(
  key: string,
  extend = false,
  horizontal = false,
  word = false,
): Command {
  const forward = horizontal ? "ArrowRight" : "ArrowDown";
  const backward = horizontal ? "ArrowLeft" : "ArrowUp";
  if (key === forward || key === backward)
    return moveCharacter(key === forward, extend, word);
  return (state, dispatch) => {
    const { $head, anchor, head } = state.selection;
    const index = $head.index(0),
      positions = characterPositions(state.doc, index);
    let target = head;
    if (key === "Home") target = positions[0];
    else if (key === "End") target = positions.at(-1)!;
    else {
      const next =
        index + (key === (horizontal ? "ArrowDown" : "ArrowLeft") ? 1 : -1);
      if (next >= 0 && next < state.doc.childCount) {
        const other = characterPositions(state.doc, next);
        const ordinal = positions.findIndex((p) => p >= head);
        target = other[Math.min(Math.max(0, ordinal), other.length - 1)];
      }
    }
    dispatch?.(
      state.tr
        .setSelection(
          contextSelection(state.doc, extend ? anchor : target, target),
        )
        .scrollIntoView(),
    );
    return true;
  };
}
export const newColumn: Command = (state, dispatch) => {
  let tr = state.tr.deleteSelection();
  const { $from } = tr.selection;
  const pos = $from.depth > 1 ? $from.after(2) : $from.pos;
  tr = tr.split(pos, 1);
  dispatch?.(
    tr
      .setSelection(TextSelection.near(tr.doc.resolve(pos + 2)))
      .scrollIntoView(),
  );
  return true;
};
function decorations(doc: PMNode): DecorationSet {
  const decorations: Decoration[] = [];
  doc.descendants((parent, parentPos) => {
    if (!parent.inlineContent || isKunten(parent)) return;
    const children: { node: PMNode; pos: number }[] = [];
    parent.forEach((node, offset) =>
      children.push({ node, pos: parentPos + 1 + offset }),
    );
    for (let i = 0; i < children.length; i++) {
      if (!isKunten(children[i].node)) continue;
      let end = i;
      const sizes = { return: 0, okurigana: 0 };
      while (end < children.length && isKunten(children[end].node)) {
        const node = children[end++].node;
        sizes[node.type.name as keyof typeof sizes] += [
          ...node.textContent,
        ].length;
      }
      const length = Math.max(sizes.return, sizes.okurigana);
      const offsets = { return: 0, okurigana: 0 };
      for (let j = i; j < end; j++) {
        const { node, pos } = children[j];
        const kind = node.type.name as keyof typeof offsets;
        decorations.push(
          Decoration.node(pos, pos + node.nodeSize, {
            class: `kunten-mark kunten-${kind}`,
            style: `--kunten-length:${length};--kunten-back:${j === i ? 0 : length};--kunten-offset:${offsets[kind]}`,
          }),
        );
        offsets[kind] += [...node.textContent].length;
      }
      i = end - 1;
    }
  });
  doc.descendants((node, pos) => {
    if (node.type.name !== "source") return;
    if (node.attrs.kind === "divider") {
      const $pos = doc.resolve(pos);
      decorations.push(
        Decoration.node($pos.before(1), $pos.after(1), {
          class: "editor-half-divider",
        }),
      );
    }
  });
  return DecorationSet.create(doc, decorations);
}
function textPoint(view: EditorView, position: number) {
  const point = view.domAtPos(position);
  if (
    point.node.nodeType === Node.TEXT_NODE &&
    point.node.textContent === "\u200b"
  )
    return { node: point.node, offset: 1 };
  if (point.node.nodeType === Node.ELEMENT_NODE) {
    const next = point.node.childNodes[point.offset];
    const previous = point.node.childNodes[point.offset - 1];
    if (next?.nodeType === Node.TEXT_NODE)
      return { node: next, offset: next.textContent === "\u200b" ? 1 : 0 };
    if (previous?.nodeType === Node.TEXT_NODE)
      return { node: previous, offset: previous.textContent!.length };
  }
  return point;
}
function normalizeFields(state: EditorState): Transaction | null {
  const tr = state.tr;
  const fields: { node: PMNode; pos: number }[] = [];
  state.doc.descendants((node, pos) => {
    if (
      node.type.name === "segment" &&
      ((node.attrs.placeholder && columnSource(node)) ||
        (!node.attrs.placeholder && !node.content.size))
    )
      fields.push({ node, pos });
  });
  for (const { node, pos } of fields.reverse()) {
    if (!node.content.size) {
      tr.insertText("\u200b", pos + 1);
      tr.setNodeMarkup(pos, undefined, { ...node.attrs, placeholder: true });
      if (state.selection.empty && state.selection.head === pos + 1)
        tr.setSelection(TextSelection.create(tr.doc, pos + 1));
      continue;
    }
    let offset = -1;
    node.forEach((child, childPos) => {
      if (child.isText && child.text!.includes("\u200b"))
        offset = childPos + child.text!.indexOf("\u200b");
    });
    if (offset >= 0) tr.delete(pos + 1 + offset, pos + 2 + offset);
    tr.setNodeMarkup(pos, undefined, { ...node.attrs, placeholder: false });
  }
  return fields.length ? tr : null;
}
/** Resolve vertical caret geometry from the adjacent character, including annotation fields. */
export function verticalCaretRect(view: EditorView, position: number) {
  let { node, offset } = view.domAtPos(position);
  if (node.nodeType === Node.ELEMENT_NODE) {
    const next = node.childNodes[offset],
      previous = node.childNodes[offset - 1];
    if (next?.nodeType === Node.TEXT_NODE) {
      node = next;
      offset = 0;
    } else if (previous?.nodeType === Node.TEXT_NODE) {
      node = previous;
      offset = previous.textContent!.length;
    }
  }
  if (node.nodeType === Node.TEXT_NODE && node.textContent?.length) {
    const range = document.createRange();
    const text = node.textContent;
    const segments = [
      ...new Intl.Segmenter("ja", { granularity: "grapheme" }).segment(text),
    ];
    const next = segments.find((part) => part.index >= offset);
    const start = next?.index ?? segments.at(-1)!.index;
    range.setStart(node, start);
    range.setEnd(node, next ? start + next.segment.length : offset);
    const rect = range.getBoundingClientRect();
    return {
      left: rect.left,
      right: rect.right,
      top: next ? rect.top : rect.bottom,
      bottom: next ? rect.top : rect.bottom,
    };
  }
  const rect = view.coordsAtPos(position);
  const parent = view.state.doc.resolve(position).parent;
  const dom =
    node.nodeType === Node.ELEMENT_NODE
      ? (node as HTMLElement)
      : node.parentElement;
  if (dom && !parent.content.size) {
    const box = dom.getBoundingClientRect();
    return {
      left: box.left + 2,
      right: box.right - 2,
      top: box.top + 1,
      bottom: box.top + 1,
    };
  }
  return {
    ...rect,
    right: rect.right > rect.left ? rect.right : rect.left + 22,
  };
}

function horizontalMode(view: EditorView) {
  return !!view.dom.closest(".transcription.horizontal");
}
function pointerPosition(view: EditorView, x: number, y: number): number {
  if (horizontalMode(view))
    return (
      view.posAtCoords({ left: x, top: y })?.pos ?? view.state.selection.head
    );
  const columns = [...view.dom.children] as HTMLElement[];
  const distance = (el: HTMLElement) => {
    const r = el.getBoundingClientRect();
    return Math.max(r.left - x, x - r.right, 0);
  };
  const column = columns.reduce((a, b) => (distance(b) < distance(a) ? b : a));
  let positions = characterPositions(view.state.doc, columns.indexOf(column));
  const segment = view.dom.ownerDocument
    .elementFromPoint(x, y)
    ?.closest(".editor-segment");
  if (segment && view.dom.contains(segment)) {
    const from = view.posAtDOM(segment, 0),
      to = view.posAtDOM(segment, segment.childNodes.length);
    const fields = positions.filter((pos) => pos >= from && pos <= to);
    if (fields.length) positions = fields;
  }
  return positions
    .map((pos) => {
      const r = verticalCaretRect(view, pos);
      const dx = Math.max(r.left - x, x - r.right, 0);
      return { pos, distance: dx * dx + (r.top - y) ** 2 };
    })
    .sort((a, b) => a.distance - b.distance || b.pos - a.pos)[0].pos;
}

export interface EditorUpdate {
  source: string;
  serialized: string;
  patches: SourcePatch[];
  column: number;
  composing: boolean;
}
export function createEditor(
  element: HTMLElement,
  source: string,
  onUpdate: (update: EditorUpdate) => void = () => {},
  oncolumnchange: (index: number) => void = () => {},
  onstatus: (message: string) => void = () => {},
) {
  let current = source;
  let composing = false;
  let pendingSource: string | undefined;
  let composingContext:
    | { from: number; node: PMNode; emptyDoc: PMNode }
    | undefined;
  const decorationPlugin: Plugin<DecorationSet> = new Plugin<DecorationSet>({
    state: {
      init: (_, state) => decorations(state.doc),
      apply: (tr, prev) =>
        tr.getMeta("refreshDecorations")
          ? decorations(tr.doc)
          : tr.docChanged
            ? composing
              ? prev.map(tr.mapping, tr.doc)
              : decorations(tr.doc)
            : prev,
    },
    props: { decorations: (state) => decorationPlugin.getState(state) },
  });
  const alignmentPlugin: Plugin<number> = new Plugin<number>({
    state: {
      init: () => -1,
      apply: (tr, previous) => tr.getMeta("alignmentColumn") ?? previous,
    },
    props: {
      decorations(state) {
        const columns = transcriptionColumns(toMarkup(state.doc));
        const active = alignmentPlugin.getState(state);
        const values: Decoration[] = [];
        state.doc.forEach((node, pos, sourceIndex) => {
          const index = columns.findIndex((c) => c.sourceIndex === sourceIndex);
          if (index >= 0)
            values.push(
              Decoration.node(pos, pos + node.nodeSize, {
                "data-column-index": String(index),
                class: index === active ? "alignment-active-column" : "",
              }),
            );
        });
        return DecorationSet.create(state.doc, values);
      },
    },
  });
  let dragAnchor: number | undefined;
  function dragMove(event: MouseEvent) {
    if (dragAnchor === undefined || view.isDestroyed) return;
    const pos = pointerPosition(view, event.clientX, event.clientY);
    view.dispatch(
      view.state.tr
        .setSelection(TextSelection.create(view.state.doc, dragAnchor, pos))
        .scrollIntoView(),
    );
    event.preventDefault();
  }
  function stopDrag() {
    dragAnchor = undefined;
    document.removeEventListener("mousemove", dragMove);
    document.removeEventListener("mouseup", stopDrag);
  }
  const view = new EditorView(element, {
    state: EditorState.create({
      doc: fromMarkup(source),
      plugins: [
        decorationPlugin,
        alignmentPlugin,
        caretPlugin(),
        new Plugin({
          props: {
            decorations(state) {
              const { $head } = state.selection;
              return $head.depth
                ? DecorationSet.create(state.doc, [
                    Decoration.node($head.before(1), $head.after(1), {
                      class: "editor-active-column",
                    }),
                  ])
                : DecorationSet.empty;
            },
          },
        }),
        history(),
        keymap({
          "Mod-z": undo,
          "Mod-Shift-z": redo,
          "Mod-y": redo,
          ...baseKeymap,
        }),
      ],
    }),
    attributes: {
      class: "vertical-editor",
      role: "textbox",
      "aria-label": "翻刻本文",
      "aria-multiline": "true",
      spellcheck: "false",
    },
    nodeViews: {
      segment(node, editorView, getPos) {
        const dom = document.createElement("span");
        dom.className = `editor-segment editor-${node.attrs.role}`;
        dom.dataset.segment = node.attrs.role;
        dom.dataset.placeholder = String(node.attrs.placeholder);
        dom.classList.toggle("editor-empty-field", !!node.attrs.placeholder);
        const contentDOM = node.attrs.reading
          ? document.createElement("rt")
          : dom;
        if (contentDOM !== dom) {
          contentDOM.className = "editor-reading";
          dom.append(contentDOM);
        }
        return {
          dom,
          contentDOM,
          setSelection(anchor, head) {
            const pos = getPos();
            if (pos === undefined) return;
            const a = textPoint(editorView, pos + 1 + anchor);
            const b = textPoint(editorView, pos + 1 + head);
            const selection = dom.ownerDocument.getSelection();
            selection?.setBaseAndExtent(a.node, a.offset, b.node, b.offset);
          },
        };
      },
      kenten(node) {
        const dom = document.createElement("span");
        dom.className = "editor-annotation editor-kenten";
        dom.dataset.annotation = "kenten";
        dom.dataset.contextLabel = "圏点";
        const update = (next: PMNode) => {
          if (next.type.name !== "kenten") return false;
          const mark = parts(next)[1]
            .replace(/[\\"]/g, "\\$&")
            .replace(/[\r\n]/g, " ");
          dom.style.setProperty("--kenten-mark", `"${mark}"`);
          return true;
        };
        update(node);
        return { dom, contentDOM: dom, update };
      },
    },
    handleKeyDown: (view, event) => {
      if (
        composing ||
        view.composing ||
        event.isComposing ||
        event.keyCode === 229
      )
        return false;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a")
        return selectColumn(view.state, view.dispatch);
      const horizontal = horizontalMode(view);
      if (
        [
          "ArrowUp",
          "ArrowDown",
          "ArrowLeft",
          "ArrowRight",
          "Home",
          "End",
        ].includes(event.key)
      ) {
        return moveCaret(
          event.key,
          event.shiftKey,
          horizontal,
          event.ctrlKey || event.altKey || event.metaKey,
        )(view.state, view.dispatch);
      }
      if (event.ctrlKey || event.metaKey || event.altKey) return false;
      if (event.key === "Escape")
        return escapeContext(view.state, view.dispatch);
      if (
        (event.key === "Backspace" || event.key === "Delete") &&
        deleteContext(event.key === "Backspace")(view.state, view.dispatch)
      )
        return true;
      if (event.key === "Enter" && enterContext()(view.state, view.dispatch))
        return true;
      if (event.key === "Tab" || event.key === "Enter") {
        if (shellKey(event.key, event.shiftKey)(view.state, view.dispatch))
          return true;
      }
      if (event.key === "Enter") return newColumn(view.state, view.dispatch);
      return false;
    },
    handleDOMEvents: {
      beforeinput: (view, event) => {
        if (
          event.inputType !== "insertText" ||
          event.data === null ||
          event.isComposing ||
          composing ||
          view.composing
        )
          return false;
        event.preventDefault();
        if (
          view.state.selection.$from.depth > 1 &&
          /[《》｜\r\n]/u.test(event.data)
        ) {
          onstatus("この欄では区切り記号を入力できません。");
          return true;
        }
        onstatus("");
        if (!extendOkurigana(event.data)(view.state, view.dispatch))
          view.dispatch(view.state.tr.insertText(event.data).scrollIntoView());
        return true;
      },
      mousedown: (view, event) => {
        if (event.button !== 0 || composing || view.composing) return false;
        const target = event.target as HTMLElement;
        const shell = target.closest<HTMLElement>(
          "[data-annotation], [data-source]",
        );
        if (
          shell &&
          view.dom.contains(shell) &&
          (!shell.contains(target.closest(".editor-segment")) ||
            shell.dataset.annotation === "return")
        ) {
          const pos =
            view.posAtDOM(shell, 0) -
            (shell.hasAttribute("data-source") ? 0 : 1);
          if (selectContext(pos)(view.state, view.dispatch)) {
            view.focus();
            event.preventDefault();
            return true;
          }
        }
        const columns = [...view.dom.children] as HTMLElement[];
        const last = columns.at(-1)!;
        if (
          !horizontalMode(view) &&
          event.clientX < last.getBoundingClientRect().left - 4
        ) {
          const tr = closeHistory(view.state.tr).insert(
            view.state.doc.content.size,
            schema.nodes.column.create(),
          );
          tr.setSelection(
            TextSelection.create(tr.doc, tr.doc.content.size - 1),
          );
          view.dispatch(tr.scrollIntoView());
        } else {
          const pos = pointerPosition(view, event.clientX, event.clientY);
          if (event.detail === 2) {
            const $pos = view.state.doc.resolve(pos);
            const parent = $pos.parent;
            if (parent.type.name === "segment") {
              const end =
                parent.attrs.placeholder && parent.textContent === "\u200b"
                  ? $pos.start()
                  : $pos.end();
              view.dispatch(
                view.state.tr.setSelection(
                  TextSelection.create(view.state.doc, $pos.start(), end),
                ),
              );
              view.focus();
              event.preventDefault();
              return true;
            }
            const script = (c: string) =>
              /[\p{Script=Hiragana}\p{Script=Katakana}ー]/u.test(c)
                ? "kana"
                : /\p{Script=Han}/u.test(c)
                  ? "kanji"
                  : /[a-z0-9]/i.test(c)
                    ? "latin"
                    : c;
            // Script runs are bounded by inline nodes.
            const child = parent.childAfter($pos.parentOffset);
            const previous = parent.childBefore($pos.parentOffset);
            const run = child.node?.isText ? child : previous;
            if (run.node?.isText) {
              const value = run.node.text!;
              let at = Math.max(0, $pos.parentOffset - run.offset);
              if (verticalCaretRect(view, pos).top > event.clientY) at--;
              const graphemes = [
                ...new Intl.Segmenter("ja", {
                  granularity: "grapheme",
                }).segment(value),
              ];
              let index = graphemes.findIndex(
                (part) => part.index + part.segment.length > at,
              );
              if (index < 0) index = graphemes.length - 1;
              let first = index,
                last = index;
              while (
                first > 0 &&
                script(graphemes[first - 1].segment) ===
                  script(graphemes[index].segment)
              )
                first--;
              while (
                last + 1 < graphemes.length &&
                script(graphemes[last + 1].segment) ===
                  script(graphemes[index].segment)
              )
                last++;
              const from = graphemes[first].index;
              const to = graphemes[last].index + graphemes[last].segment.length;
              view.dispatch(
                view.state.tr.setSelection(
                  TextSelection.create(
                    view.state.doc,
                    $pos.start() + run.offset + from,
                    $pos.start() + run.offset + to,
                  ),
                ),
              );
            }
          } else {
            const anchor = event.shiftKey ? view.state.selection.anchor : pos;
            view.dispatch(
              view.state.tr.setSelection(
                TextSelection.create(view.state.doc, anchor, pos),
              ),
            );
            stopDrag();
            dragAnchor = anchor;
            document.addEventListener("mousemove", dragMove);
            document.addEventListener("mouseup", stopDrag);
          }
        }
        view.focus();
        event.preventDefault();
        return true;
      },
      compositionstart: (view) => {
        if (view.state.selection instanceof NodeSelection) {
          const { from, node } = view.state.selection;
          const tr = closeHistory(view.state.tr).deleteSelection();
          composingContext = { from, node, emptyDoc: tr.doc };
          view.dispatch(tr.setSelection(TextSelection.create(tr.doc, from)));
          composingContext.emptyDoc = view.state.doc;
        }
        composing = true;
        publish();
        return false;
      },
      compositionend: (_view, event) => {
        composing = false;
        setTimeout(() => {
          if (view.isDestroyed) return;
          if (
            composingContext &&
            !event.data &&
            view.state.doc.eq(composingContext.emptyDoc)
          ) {
            const { from, node } = composingContext;
            const tr = view.state.tr.insert(from, node);
            view.dispatch(tr.setSelection(NodeSelection.create(tr.doc, from)));
          }
          composingContext = undefined;
          if (pendingSource !== undefined) {
            const next = pendingSource;
            pendingSource = undefined;
            setSource(next);
          }
          normalizeTypedOkurigana(view.state, view.dispatch);
          view.dispatch(view.state.tr.setMeta("refreshDecorations", true));
        }, 30);
        return false;
      },
    },
    clipboardTextSerializer: (_slice, editorView) => {
      const { from, to } = editorView.state.selection;
      return selectedMarkup(editorView.state.doc, from, to);
    },
    clipboardTextParser: (text) => clipboardMarkup(text),
    handlePaste: (view, event) => {
      if (composing || view.composing) return false;
      const text = event.clipboardData?.getData("text/plain");
      if (text === undefined) return false;
      if (view.state.selection.$from.depth > 1) {
        const children = parseLine(text);
        const parent = view.state.selection.$from.node(
          view.state.selection.$from.depth - 1,
        ).type.name;
        if (
          /[\r\n]/.test(text) ||
          children.some(
            (node) =>
              !allowsChild(parent, node.kind) ||
              (node.kind === "raw" && /[《》｜]/u.test(node.source)),
          )
        ) {
          onstatus("ここにはこの原文を貼り付けられません。");
          return true;
        }
        onstatus("");
        view.dispatch(
          view.state.tr
            .replaceSelection(
              new Slice(
                schema.nodes.column.create(null, children.map(project)).content,
                0,
                0,
              ),
            )
            .scrollIntoView(),
        );
        return true;
      }
      view.dispatch(
        view.state.tr.replaceSelection(clipboardMarkup(text)).scrollIntoView(),
      );
      return true;
    },
    dispatchTransaction(tr) {
      const patches = sourcePatches(tr);
      current = applyPatches(current, patches);
      view.updateState(view.state.apply(tr));
      if (
        tr.selectionSet &&
        view.state.selection instanceof TextSelection &&
        view.hasFocus() &&
        !composing &&
        !view.composing
      ) {
        const { anchor, head } = view.state.selection;
        const a = textPoint(view, anchor),
          b = textPoint(view, head);
        view.dom.ownerDocument
          .getSelection()
          ?.setBaseAndExtent(a.node, a.offset, b.node, b.offset);
      }
      publish(patches);
      if (!composing && !view.composing) {
        const placeholders = normalizeFields(view.state);
        if (placeholders) view.dispatch(placeholders);
      }
    },
  });
  let lastColumn: number | undefined;
  function publish(patches: SourcePatch[] = []) {
    const column = transcriptionColumns(current).findIndex(
      (c) => c.sourceIndex === view.state.selection.$head.index(0),
    );
    if (column !== lastColumn) {
      lastColumn = column;
      oncolumnchange(column);
    }
    onUpdate({
      source: current,
      serialized: toMarkup(view.state.doc),
      patches,
      column: view.state.selection.$head.index(0),
      composing,
    });
  }
  function setSource(text: string) {
    if (text === current) return;
    if (composing || view.composing) {
      pendingSource = text;
      return;
    }
    replaceSource(text)(view.state, view.dispatch);
    view.dispatch(closeHistory(view.state.tr));
  }
  function run(command: Command, focus = true) {
    if (composing || view.composing) return false;
    if (focus) view.focus();
    return command(view.state, view.dispatch, view);
  }
  function focusColumn(index: number) {
    const column = transcriptionColumns(current)[index];
    if (!column || composing || view.composing) return;
    let position = 1;
    for (let i = 0; i < column.sourceIndex; i++)
      position += view.state.doc.child(i).nodeSize;
    view.dispatch(
      view.state.tr
        .setSelection(TextSelection.create(view.state.doc, position))
        .scrollIntoView(),
    );
    view.focus();
    (view.nodeDOM(position - 1) as HTMLElement | null)?.scrollIntoView({
      block: "nearest",
      inline: "nearest",
    });
  }
  function setHighlightedColumn(index: number) {
    if (alignmentPlugin.getState(view.state) !== index)
      view.dispatch(
        view.state.tr
          .setMeta("alignmentColumn", index)
          .setMeta("addToHistory", false),
      );
  }
  publish();
  return {
    view,
    run,
    setSource,
    focusColumn,
    setHighlightedColumn,
    destroy: () => {
      stopDrag();
      view.destroy();
    },
  };
}

export function textareaSource(previous: string, value: string): string {
  const normalized = previous.replace(/\r\n|\r/g, "\n");
  if (normalized === value) return previous;
  let from = 0,
    suffix = 0;
  while (
    from < normalized.length &&
    from < value.length &&
    normalized[from] === value[from]
  )
    from++;
  while (
    suffix < normalized.length - from &&
    suffix < value.length - from &&
    normalized[normalized.length - 1 - suffix] ===
      value[value.length - 1 - suffix]
  )
    suffix++;
  const offsets = [0];
  for (let i = 0; i < previous.length; i++) {
    if (previous[i] === "\r" && previous[i + 1] === "\n") i++;
    offsets.push(i + 1);
  }
  return (
    previous.slice(0, offsets[from]) +
    value
      .slice(from, value.length - suffix)
      .replace(/\n/g, previous.match(/\r\n|\r|\n/)?.[0] ?? "\n") +
    previous.slice(offsets[normalized.length - suffix])
  );
}

export function historyKey(
  event: KeyboardEvent,
  editor: ReturnType<typeof createEditor> | undefined,
): void {
  if (event.isComposing || event.altKey || !(event.ctrlKey || event.metaKey))
    return;
  const key = event.key.toLowerCase();
  const command =
    key === "z"
      ? event.shiftKey
        ? redo
        : undo
      : key === "y"
        ? redo
        : undefined;
  if (!command || !editor) return;
  event.preventDefault();
  editor.run(command, false);
}
