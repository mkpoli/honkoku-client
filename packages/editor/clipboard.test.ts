import { expect, test } from "bun:test";
import { EditorState, TextSelection } from "prosemirror-state";
import {
  clipboardMarkup,
  selectedMarkup,
  fromMarkup,
  toMarkup,
  insertOkurigana,
  extendOkurigana,
  deleteContext,
  NodeSelection,
  moveCaret,
} from "./index";
import { loadPresets, normalizePreset, presetKey } from "./presets";

for (const source of [
  "￣ニ",
  "＿レ",
  "《振り仮名：峰｜みね》",
  "《割書：a｜b》",
  "《割書：《振り仮名：峰｜みね》｜b》",
  "＃００１",
  "【注釈】",
  "【この文字は原本の汚れにより判読できない】",
  "《注記：この文字は原本の汚れにより判読できない》",
  "〔日本橋〕",
  "｛内蔵助｝",
  "＜安政二年＞",
  "｛＿レ｝",
  "未（いまだ｜ズ）",
]) {
  test(`whole node clipboard round trip: ${source}`, () => {
    const doc = fromMarkup(source);
    const text = selectedMarkup(doc, 1, doc.content.size - 1);
    expect(text).toBe(source);
    const state = EditorState.create({ doc: fromMarkup("甲乙") });
    const tr = state.tr
      .setSelection(TextSelection.create(state.doc, 2))
      .replaceSelection(clipboardMarkup(text));
    expect(toMarkup(tr.doc)).toBe(`甲${source}乙`);
  });
}
test("partial kunten copy keeps the prefix, including a visible single kana", () => {
  expect(selectedMarkup(fromMarkup("讀￣ニシテ"), 4, 5)).toBe("￣ニ");
  expect(selectedMarkup(fromMarkup("讀￣ニシテ"), 5, 7)).toBe("￣シテ");
  expect(selectedMarkup(fromMarkup("￣ニ"), 3, 4)).toBe("￣ニ");
  expect(selectedMarkup(fromMarkup("＿レ"), 3, 4)).toBe("＿レ");
});
test("partial shells unwrap, nested whole shells retain their spelling", () => {
  const doc = fromMarkup("《割書：《振り仮名：峰｜みね》｜後半》");
  const inner = doc.firstChild!.firstChild!.firstChild!.firstChild!;
  expect(selectedMarkup(doc, 3, 3 + inner.nodeSize)).toBe(
    "《振り仮名：峰｜みね》",
  );
  expect(selectedMarkup(doc, 5, 6)).toBe("峰");
  expect(selectedMarkup(fromMarkup("《振り仮名：山川｜やまかわ》"), 3, 4)).toBe(
    "山",
  );
  expect(selectedMarkup(fromMarkup("一二\r\n三四"), 2, 6)).toBe("二\r\n三");
});
test("okurigana insertion enters its field and deletion selects the whole context first", () => {
  let state = EditorState.create({ doc: fromMarkup("故山川") });
  const dispatch = (tr: typeof state.tr) => {
    state = state.apply(tr);
  };
  dispatch(state.tr.setSelection(TextSelection.create(state.doc, 1, 3)));
  expect(insertOkurigana("ニ")(state, dispatch)).toBe(true);
  expect(toMarkup(state.doc)).toBe("故山￣ニ川");
  expect(state.selection.from).toBe(5);
  moveCaret("ArrowDown")(state, dispatch);
  moveCaret("ArrowDown")(state, dispatch);
  expect(extendOkurigana("シテ")(state, dispatch)).toBe(true);
  expect(toMarkup(state.doc)).toBe("故山￣ニシテ川");
  const end = state.selection.from;
  moveCaret("ArrowUp")(state, dispatch);
  expect(state.selection.from).toBe(end - 2);
  moveCaret("ArrowDown")(state, dispatch);
  expect(state.selection.from).toBe(end);
  expect(deleteContext(true)(state, dispatch)).toBe(true);
  expect(state.selection).toBeInstanceOf(NodeSelection);
  expect(toMarkup(state.doc)).toBe("故山￣ニシテ川");
  deleteContext(true)(state, dispatch);
  expect(toMarkup(state.doc)).toBe("故山川");
  dispatch(state.tr.setSelection(TextSelection.create(state.doc, 1)));
  expect(insertOkurigana("ニ")(state, dispatch)).toBe(false);
});
test("typing after a raw okurigana prefix forms a node", () => {
  let state = EditorState.create({ doc: fromMarkup("故￣") });
  state = state.apply(
    state.tr.setSelection(
      TextSelection.create(state.doc, state.doc.content.size - 1),
    ),
  );
  extendOkurigana("ニ")(state, (tr) => {
    state = state.apply(tr);
  });
  expect(state.doc.firstChild!.lastChild!.type.name).toBe("okurigana");
  expect(toMarkup(state.doc)).toBe("故￣ニ");
});
test("presets normalize kana and isolate accounts and groups", () => {
  expect(normalizePreset("送り仮名", "にして")).toBe("ニシテ");
  expect(normalizePreset("送り仮名", "ニ".repeat(9))).toBeNull();
  expect(normalizePreset("常用字", "𠮷")).toBe("𠮷");
  const storage = {
    getItem: (key: string) =>
      key === presetKey("a", "送り仮名")
        ? '["ニナリテ", "ニナリテ", 12, "invalid"]'
        : null,
  };
  expect(loadPresets(storage, "a", "送り仮名")).toEqual(["ニナリテ"]);
  expect(loadPresets(storage, "b", "送り仮名")).toEqual([]);
  expect(loadPresets(storage, "a", "常用句")).toEqual([]);
});

test("IME commits normalize a typed prefix and extend a preceding kunten", async () => {
  const { normalizeTypedOkurigana } = await import("./index");
  for (const initial of ["故", "故￣ニ"]) {
    let state = EditorState.create({ doc: fromMarkup(initial) });
    const text = initial === "故" ? "￣ニシテ" : "シテ";
    state = state.apply(
      state.tr
        .setSelection(
          TextSelection.create(state.doc, state.doc.content.size - 1),
        )
        .insertText(text),
    );
    expect(
      normalizeTypedOkurigana(state, (tr) => {
        state = state.apply(tr);
      }),
    ).toBe(true);
    expect(toMarkup(state.doc)).toBe("故￣ニシテ");
    expect(state.doc.firstChild!.childCount).toBe(2);
  }
});
