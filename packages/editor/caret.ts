import type { Node as PMNode } from "prosemirror-model";
import {
  NodeSelection,
  TextSelection,
  Plugin,
  PluginKey,
  type Command,
  type EditorState,
} from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

export const contextNames: Record<string, string> = {
  ruby: "振り仮名",
  warigaki: "割書",
  misekechi: "見せ消ち",
  kenten: "圏点",
  note: "注記",
  place: "場所",
  person: "人物",
  date: "日時",
  title: "題",
  rightLine: "右線",
  box: "箱",
  okurigana: "送り仮名",
  return: "返り点",
};
export function isContext(node: PMNode | null | undefined): boolean {
  return (
    !!node && (!!contextNames[node.type.name] || node.type.name === "source")
  );
}
export function contextName(node: PMNode): string {
  return (
    contextNames[node.type.name] ??
    (node.attrs.kind === "reference"
      ? "脚注"
      : node.attrs.kind === "gap"
        ? node.attrs.source === "□"
          ? "虫損"
          : "難読"
        : "注記")
  );
}
export function contextFields(node: PMNode, pos: number) {
  const fields: { from: number; to: number; index: number }[] = [];
  if (node.isAtom) return fields;
  node.forEach((field, offset, index) => {
    if (field.type.name !== "segment") return;
    const from = pos + offset + 2;
    fields.push({
      from,
      to:
        from +
        (field.attrs.placeholder && field.textContent === "\u200b"
          ? 0
          : field.content.size),
      index,
    });
  });
  return fields;
}
export interface CaretContext {
  pos: number;
  label: string;
  field: number | null;
}
export interface CaretState {
  mode: "text" | "small" | "big";
  path: CaretContext[];
}
export function caretContext(state: EditorState): CaretState {
  const { selection } = state;
  const path: CaretContext[] = [];
  const $pos = selection.$from;
  for (let depth = 1; depth <= $pos.depth; depth++) {
    const node = $pos.node(depth);
    if (!isContext(node)) continue;
    const pos = $pos.before(depth);
    const field =
      contextFields(node, pos).find(
        (f) => selection.head >= f.from && selection.head <= f.to,
      )?.index ?? null;
    const suffix =
      field === null
        ? ""
        : node.type.name === "warigaki"
          ? `${field + 1}行目`
          : node.type.name === "ruby"
            ? ["本文", "読み", "左読み"][field]
            : node.type.name === "misekechi"
              ? ["原文", "訂正"][field]
              : node.type.name === "kenten"
                ? ["本文", "記号"][field]
                : "";
    path.push({
      pos,
      label: contextName(node) + (suffix ? ` ${suffix}` : ""),
      field,
    });
  }
  const big = selection instanceof NodeSelection && isContext(selection.node);
  if (big)
    path.push({
      pos: selection.from,
      label: contextName(selection.node),
      field: null,
    });
  return { mode: big ? "big" : path.length ? "small" : "text", path };
}
export const caretPluginKey = new PluginKey<CaretState>("caretContext");
export function caretPlugin() {
  return new Plugin<CaretState>({
    key: caretPluginKey,
    state: {
      init: (_, state) => caretContext(state),
      apply: (_tr, _previous, _old, state) => caretContext(state),
    },
    props: {
      attributes: (state) => ({
        "data-caret-mode": caretPluginKey.getState(state)!.mode,
      }),
      decorations(state) {
        const caret = caretPluginKey.getState(state)!;
        const active = caret.path.at(-1);
        if (!active) return DecorationSet.empty;
        const node = state.doc.nodeAt(active.pos)!;
        return DecorationSet.create(state.doc, [
          Decoration.node(active.pos, active.pos + node.nodeSize, {
            class: `editor-caret-${caret.mode}`,
            "data-context-label": contextName(node),
          }),
        ]);
      },
    },
  });
}
export const selectContext =
  (pos: number | null): Command =>
  (state, dispatch) => {
    const target = pos === null ? caretContext(state).path[0]?.pos : pos;
    if (target === undefined) return false;
    const node = state.doc.nodeAt(target);
    if (!node || !isContext(node)) return false;
    dispatch?.(
      state.tr
        .setSelection(
          pos === null
            ? TextSelection.create(state.doc, target + node.nodeSize)
            : NodeSelection.create(state.doc, target),
        )
        .scrollIntoView(),
    );
    return true;
  };
export const escapeContext: Command = (state, dispatch) => {
  const caret = caretContext(state);
  if (!caret.path.length) return false;
  if (caret.mode !== "big")
    return selectContext(caret.path.at(-1)!.pos)(state, dispatch);
  return selectContext(caret.path.at(-2)?.pos ?? null)(state, dispatch);
};
export const enterContext =
  (forward = true): Command =>
  (state, dispatch) => {
    const selection = state.selection;
    if (!(selection instanceof NodeSelection) || !isContext(selection.node))
      return false;
    const fields = contextFields(selection.node, selection.from);
    const pos = fields.length
      ? forward
        ? fields[0].from
        : fields.at(-1)!.to
      : forward
        ? selection.to
        : selection.from;
    dispatch?.(
      state.tr
        .setSelection(TextSelection.create(state.doc, pos))
        .scrollIntoView(),
    );
    return true;
  };

/** Expand field-crossing selections until every crossed context is whole. */
export function contextSelection(
  doc: PMNode,
  anchor: number,
  head: number,
): TextSelection {
  let from = Math.min(anchor, head),
    to = Math.max(anchor, head);
  if (from !== to) {
    let changed: boolean;
    do {
      changed = false;
      doc.descendants((node, pos) => {
        if (!isContext(node)) return;
        const end = pos + node.nodeSize;
        if (from >= end || to <= pos) return false;
        if (contextFields(node, pos).some((f) => from >= f.from && to <= f.to))
          return;
        if (from > pos && from < end) {
          from = pos;
          changed = true;
        }
        if (to > pos && to < end) {
          to = end;
          changed = true;
        }
      });
    } while (changed);
  }
  return TextSelection.create(
    doc,
    anchor <= head ? from : to,
    anchor <= head ? to : from,
  );
}
export function characterPositions(doc: PMNode, columnIndex: number): number[] {
  let offset = 0;
  for (let i = 0; i < columnIndex; i++) offset += doc.child(i).nodeSize;
  const positions: number[] = [];
  const graphemes = new Intl.Segmenter("ja", { granularity: "grapheme" });
  const visit = (node: PMNode, pos: number) => {
    if (node.isText) {
      for (const part of graphemes.segment(node.text!))
        positions.push(pos + part.index);
      positions.push(pos + node.nodeSize);
    } else {
      if (isContext(node)) {
        positions.push(pos, pos + node.nodeSize);
        if (node.isAtom) return;
      }
      if (
        node.type.name === "segment" ||
        node.type.name === "column" ||
        node.type.name === "raw"
      ) {
        positions.push(pos + 1);
        if (node.attrs.placeholder && node.textContent === "\u200b") return;
        positions.push(pos + node.nodeSize - 1);
      }
      node.forEach((child, at) => visit(child, pos + 1 + at));
    }
  };
  visit(doc.child(columnIndex), offset);
  return [...new Set(positions)].sort((a, b) => a - b);
}
export const moveCharacter =
  (forward: boolean, extend = false, word = false): Command =>
  (state, dispatch) => {
    const selection = state.selection;
    const move = (pos: number) => {
      dispatch?.(
        state.tr
          .setSelection(
            extend
              ? contextSelection(state.doc, selection.anchor, pos)
              : TextSelection.create(state.doc, pos),
          )
          .scrollIntoView(),
      );
      return true;
    };
    if (selection instanceof NodeSelection && !extend) {
      if (word) return move(forward ? selection.to : selection.from);
      return enterContext(forward)(state, dispatch);
    }
    if (!selection.empty && !extend)
      return move(forward ? selection.to : selection.from);
    const { $head, head } = selection;
    const adjacent = forward ? $head.nodeAfter : $head.nodeBefore;
    if (adjacent && isContext(adjacent)) {
      const start = forward ? head : head - adjacent.nodeSize;
      if (extend || word)
        return move(forward ? start + adjacent.nodeSize : start);
      if (adjacent.isAtom) return selectContext(start)(state, dispatch);
      const fields = contextFields(adjacent, start);
      return move(forward ? fields[0].from : fields.at(-1)!.to);
    }
    if (word) {
      const active = caretContext(state).path.at(-1);
      if (active) {
        const node = state.doc.nodeAt(active.pos)!;
        return move(forward ? active.pos + node.nodeSize : active.pos);
      }
      if (adjacent?.isText) {
        const words = [
          ...new Intl.Segmenter("ja", { granularity: "word" }).segment(
            adjacent.text!,
          ),
        ];
        const size = forward
          ? words[0].segment.length
          : words.at(-1)!.segment.length;
        return move(head + (forward ? size : -size));
      }
    }
    const positions = characterPositions(state.doc, $head.index(0));
    let target = forward
      ? positions.find((pos) => pos > head)
      : positions.filter((pos) => pos < head).at(-1);
    if (target === undefined) {
      const next = $head.index(0) + (forward ? 1 : -1);
      if (next >= 0 && next < state.doc.childCount) {
        const other = characterPositions(state.doc, next);
        target = forward ? other[0] : other.at(-1);
      }
    }
    return move(target ?? head);
  };
export const deleteContext =
  (backward: boolean): Command =>
  (state, dispatch) => {
    const selection = state.selection;
    if (selection instanceof NodeSelection && isContext(selection.node)) {
      dispatch?.(state.tr.deleteSelection().scrollIntoView());
      return true;
    }
    if (!selection.empty) return false;
    const { $head, head } = selection;
    const active = caretContext(state).path.at(-1);
    if (active) {
      const node = state.doc.nodeAt(active.pos)!;
      const field = contextFields(node, active.pos).find(
        (f) => head >= f.from && head <= f.to,
      );
      if (field && head === (backward ? field.from : field.to)) {
        const empty = Array.from({ length: node.childCount }, (_, i) =>
          node.child(i),
        ).every(
          (f) =>
            !f.content.size ||
            (f.attrs.placeholder && f.textContent === "\u200b"),
        );
        if (empty && (backward || node.childCount === 1)) {
          const tr = state.tr.delete(active.pos, active.pos + node.nodeSize);
          dispatch?.(
            tr
              .setSelection(TextSelection.create(tr.doc, active.pos))
              .scrollIntoView(),
          );
        }
        return true;
      }
    }
    const adjacent = backward ? $head.nodeBefore : $head.nodeAfter;
    if (adjacent && isContext(adjacent))
      return selectContext(backward ? head - adjacent.nodeSize : head)(
        state,
        dispatch,
      );
    // Own grapheme deletion inside fields so native deletion cannot merge field wrappers.
    if ($head.parent.type.name === "segment") {
      const positions = characterPositions(state.doc, $head.index(0));
      const target = backward
        ? positions.filter((p) => p < head).at(-1)
        : positions.find((p) => p > head);
      if (target !== undefined)
        dispatch?.(
          state.tr
            .delete(Math.min(head, target), Math.max(head, target))
            .scrollIntoView(),
        );
      return true;
    }
    return false;
  };
