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
  type Command,
  type Transaction,
} from "prosemirror-state";
import { Decoration, DecorationSet, EditorView } from "prosemirror-view";
import { history, undo, redo, closeHistory } from "prosemirror-history";
import { keymap } from "prosemirror-keymap";
import { baseKeymap } from "prosemirror-commands";
import { transcriptionColumns, parse, type SyntaxNode } from "@honkoku/markup";
export { undo, redo, TextSelection };

const annotationNames: Record<string, string> = {
  ruby: "振り仮名",
  warigaki: "割書",
  misekechi: "見せ消ち",
  kenten: "圏点",
  rightLine: "右線",
  title: "題",
  box: "箱",
  place: "場所",
};
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
    content: "text*",
    attrs: { role: { default: "base" } },
    toDOM: (node) => [
      "span",
      {
        class: `editor-segment editor-${node.attrs.role}`,
        "data-segment": node.attrs.role,
      },
      0,
    ],
    parseDOM: [
      {
        tag: "span[data-segment]",
        getAttrs: (e) => ({ role: e.getAttribute("data-segment") }),
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
      "span",
      {
        class: "editor-source-anchor",
        "data-source": node.attrs.source,
        "data-kind": node.attrs.kind,
        "aria-hidden": "true",
      },
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
    attrs: {
      original: { default: null },
      originalParts: { default: null },
      form: { default: "bracket" },
    },
    toDOM: (node) => [
      "span",
      {
        class: `editor-annotation editor-${name}`,
        "data-annotation": name,
        "data-form": node.attrs.form,
      },
      0,
    ],
    parseDOM: [
      {
        tag: `span[data-annotation="${name}"]`,
        getAttrs: (e) => ({ form: e.getAttribute("data-form") ?? "bracket" }),
      },
    ],
  };
}
export const schema = new Schema({ nodes });
function parts(node: PMNode): string[] {
  const values: string[] = [];
  node.forEach((child) => values.push(child.textContent));
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
        { role: roles[i] },
        text ? schema.text(text) : undefined,
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
  if (node.isText || node.type.name === "raw" || node.type.name === "segment")
    return node.textContent;
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
  return value;
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
export const wrapSelection =
  (kind: "ruby" | "warigaki" | "misekechi", reading = ""): Command =>
  (state, dispatch) => {
    const { from, to, $from, $to } = state.selection;
    if ($from.depth !== 1 || $to.depth !== 1 || !$from.sameParent($to))
      return false;
    let plain = true;
    state.doc.nodesBetween(from, to, (node) => {
      if (!node.isText && node.type.name !== "column") plain = false;
    });
    if (!plain) return false;
    const value = state.doc.textBetween(from, to);
    if (dispatch)
      dispatch(
        closeHistory(state.tr)
          .replaceSelectionWith(annotation(kind, [value, reading]), false)
          .scrollIntoView(),
      );
    return true;
  };
export const insertText =
  (text: string): Command =>
  (state, dispatch) => {
    dispatch?.(closeHistory(state.tr).insertText(text).scrollIntoView());
    return true;
  };
export const insertSource =
  (text: string): Command =>
  (state, dispatch) => {
    if (
      state.selection.$from.depth !== 1 ||
      !state.selection.$from.sameParent(state.selection.$to)
    )
      return false;
    const node = parse(text).columns[0].nodes[0];
    if (!node) return false;
    dispatch?.(
      closeHistory(state.tr)
        .replaceSelectionWith(project(node), false)
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
      .setDocAttribute("newline", next.attrs.newline);
    dispatch?.(tr);
    return true;
  };
function caretPositions(doc: PMNode, columnIndex: number): number[] {
  let offset = 0;
  for (let i = 0; i < columnIndex; i++) offset += doc.child(i).nodeSize;
  const positions: number[] = [];
  const segmenter = new Intl.Segmenter("ja", { granularity: "grapheme" });
  const column = doc.child(columnIndex);
  if (!column.childCount) positions.push(offset + 1);
  column.descendants((node, pos) => {
    if (node.isText) {
      for (const part of segmenter.segment(node.text!))
        positions.push(offset + 1 + pos + part.index);
      positions.push(offset + 1 + pos + node.nodeSize);
    } else if (node.isLeaf || (node.isTextblock && !node.content.size)) {
      positions.push(offset + 1 + pos + (node.isTextblock ? 1 : 0));
      if (node.isLeaf) positions.push(offset + 1 + pos + node.nodeSize);
    }
  });
  positions.push(offset + 1, offset + column.nodeSize - 1);
  return [...new Set(positions)].sort((a, b) => a - b);
}
export function moveCaret(key: string, extend = false): Command {
  return (state, dispatch) => {
    const { $head, anchor, head } = state.selection;
    const index = $head.index(0),
      positions = caretPositions(state.doc, index);
    let target = head;
    if (key === "Home") target = positions[0];
    else if (key === "End") target = positions.at(-1)!;
    else if (key === "ArrowUp")
      target = positions.filter((p) => p < head).at(-1) ?? head;
    else if (key === "ArrowDown")
      target = positions.find((p) => p > head) ?? head;
    else {
      const next = index + (key === "ArrowLeft" ? 1 : -1);
      if (next >= 0 && next < state.doc.childCount) {
        const other = caretPositions(state.doc, next);
        const ordinal = positions.findIndex((p) => p >= head);
        target = other[Math.min(Math.max(0, ordinal), other.length - 1)];
      }
    }
    dispatch?.(
      state.tr
        .setSelection(
          TextSelection.create(state.doc, extend ? anchor : target, target),
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
  doc.descendants((node, pos) => {
    if (node.type.name !== "source") return;
    decorations.push(
      Decoration.widget(
        pos + 1,
        () => {
          const span = document.createElement("span");
          span.className = `editor-token markup-${node.attrs.kind === "gap" ? "glyph" : node.attrs.kind}`;
          span.textContent = node.attrs.source;
          span.contentEditable = "false";
          span.setAttribute("aria-label", node.attrs.source);
          return span;
        },
        {
          side: -1,
          key: `${pos}:${node.attrs.kind}:${node.attrs.source}`,
          ignoreSelection: true,
        },
      ),
    );
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
) {
  let current = source;
  let composing = false;
  let pendingSource: string | undefined;
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
  const view = new EditorView(element, {
    state: EditorState.create({
      doc: fromMarkup(source),
      plugins: [
        decorationPlugin,
        alignmentPlugin,
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
      kenten(node) {
        const dom = document.createElement("span");
        dom.className = "editor-annotation editor-kenten";
        dom.dataset.annotation = "kenten";
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
      if (event.ctrlKey || event.metaKey || event.altKey) return false;
      if (
        [
          "ArrowUp",
          "ArrowDown",
          "ArrowLeft",
          "ArrowRight",
          "Home",
          "End",
        ].includes(event.key)
      )
        return moveCaret(event.key, event.shiftKey)(view.state, view.dispatch);
      if (event.key === "Enter") return newColumn(view.state, view.dispatch);
      return false;
    },
    handleDOMEvents: {
      compositionstart: () => {
        composing = true;
        publish();
        return false;
      },
      compositionend: () => {
        composing = false;
        setTimeout(() => {
          if (view.isDestroyed) return;
          if (pendingSource !== undefined) {
            const next = pendingSource;
            pendingSource = undefined;
            setSource(next);
          }
          view.dispatch(view.state.tr.setMeta("refreshDecorations", true));
        }, 30);
        return false;
      },
    },
    clipboardTextSerializer: (slice) => {
      if (slice.content.firstChild?.type.name === "column")
        return toMarkup(schema.nodes.doc.create(null, slice.content));
      let result = "";
      slice.content.forEach((node) => (result += inlineSource(node)));
      return result;
    },
    handlePaste: (view, event) => {
      if (composing || view.composing) return false;
      const text = event.clipboardData?.getData("text/plain");
      if (text === undefined) return false;
      if (view.state.selection.$from.depth > 1) {
        if (/[\r\n]/.test(text))
          return replaceSource(toMarkup(view.state.tr.insertText(text).doc))(
            view.state,
            view.dispatch,
          );
        return insertText(text)(view.state, view.dispatch);
      }
      const doc = fromMarkup(text);
      view.dispatch(
        view.state.tr
          .replaceSelection(new Slice(doc.content, 1, 1))
          .scrollIntoView(),
      );
      return true;
    },
    dispatchTransaction(tr) {
      const patches = sourcePatches(tr);
      current = applyPatches(current, patches);
      view.updateState(view.state.apply(tr));
      publish(patches);
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
    destroy: () => view.destroy(),
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
