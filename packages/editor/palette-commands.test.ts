import { expect, test } from "bun:test";
import { EditorState, TextSelection, type Command } from "prosemirror-state";
import { fromMarkup, toMarkup } from "./index";
import { insertEditorial, insertCombiningMark } from "./palette-commands";
import { normalizePreset, presets, loadPresets, pageNotes } from "./presets";
function run(text: string, command: Command, from: number, to = from) {
  let state = EditorState.create({ doc: fromMarkup(text) });
  state = state.apply(
    state.tr.setSelection(TextSelection.create(state.doc, from, to)),
  );
  expect(
    command(state, (tr) => {
      state = state.apply(tr);
    }),
  ).toBe(true);
  return state;
}
test("note preset and selected text insert inline shells", () => {
  expect(toMarkup(run("春", insertEditorial("朱書"), 2).doc)).toBe(
    "春【朱書】",
  );
  expect(toMarkup(run("春夏秋", insertEditorial(), 2, 3).doc)).toBe(
    "春【夏】秋",
  );
});
test("empty note shell keeps the caret inside and accepts text", () => {
  let state = run("春", insertEditorial(), 2);
  expect(toMarkup(state.doc)).toBe("春【】");
  state = state.apply(state.tr.insertText("朱書"));
  expect(toMarkup(state.doc)).toBe("春【朱書】");
});
test("combining marks append to the preceding character without replacing selection", () => {
  expect(toMarkup(run("は", insertCombiningMark("\u3099"), 2).doc)).toBe(
    "は\u3099",
  );
  expect(toMarkup(run("は", insertCombiningMark("\u309a"), 1, 2).doc)).toBe(
    "は\u309a",
  );
});
test("notes accept short free text, reject nested shells, and filter stored built-ins", () => {
  expect(normalizePreset("注記", "文字なし")).toBe("文字なし");
  expect(normalizePreset("注記", "【朱書】")).toBeNull();
  expect(normalizePreset("注記", "一\n二")).toBeNull();
  expect(presets.注記.slice(0, 7).map((p) => p.text)).toEqual(pageNotes);
  expect(
    loadPresets({ getItem: () => '["私注","朱書","私注"]' }, "test", "注記"),
  ).toEqual(["私注"]);
});
