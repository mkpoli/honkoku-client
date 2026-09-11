import { TextSelection, type Command } from "prosemirror-state";
import { closeHistory } from "prosemirror-history";
import { schema, insertSource } from "./index";

/** Insert an inline editorial note, leaving an empty shell editable. */
export const insertEditorial =
  (text?: string): Command =>
  (state, dispatch) => {
    const { from, to, $from, $to } = state.selection;
    if (!$from.sameParent($to) || $from.parent.type.name === "raw")
      return false;
    const content = text ?? state.doc.textBetween(from, to, "", "");
    if (/[【】\r\n]/u.test(content)) return false;
    if (content) return insertSource(`【${content}】`)(state, dispatch);
    const node = schema.nodes.raw.create(null, schema.text("【】"));
    const tr = closeHistory(state.tr).replaceSelectionWith(node, false);
    dispatch?.(
      tr.setSelection(TextSelection.create(tr.doc, from + 2)).scrollIntoView(),
    );
    return true;
  };

export const insertCombiningMark =
  (mark: string): Command =>
  (state, dispatch) => {
    const { $to, to } = state.selection;
    if (!/^[\u3099\u309a]$/u.test(mark) || !$to.nodeBefore?.isText)
      return false;
    dispatch?.(
      closeHistory(state.tr).insertText(mark, to, to).scrollIntoView(),
    );
    return true;
  };
