import { expect, test } from "bun:test";
import { EditorState, TextSelection } from "prosemirror-state";
import { history, undo, redo } from "prosemirror-history";
import {
  columnSource,
  fromMarkup,
  toMarkup,
  sourcePatches,
  applyPatches,
  wrapSelection,
  okuriganaFromSelection,
  replaceSource,
  newColumn,
  moveCaret,
  schema,
  textareaSource,
} from "./index";
import { pageTexts } from "../markup/test-fixtures";
const root = new URL("../../", import.meta.url).pathname;
for (const pattern of [
  "fixtures/api/entry-*.json",
  "fixtures/api/firestore-pages-*.json",
  "fixtures/home/*.json",
]) {
  for (const file of new Bun.Glob(pattern).scanSync({ cwd: root }))
    test(`editor fixture ${file}`, async () => {
      for (const source of pageTexts(await Bun.file(root + file).json())) {
        const doc = fromMarkup(source);
        doc.check();
        expect(toMarkup(doc)).toBe(source);
      }
    });
}
function editor(source: string) {
  let state = EditorState.create({
    doc: fromMarkup(source),
    plugins: [history()],
  });
  let current = source;
  const dispatch = (tr: typeof state.tr) => {
    current = applyPatches(current, sourcePatches(tr));
    expect(current).toBe(toMarkup(tr.doc));
    tr.doc.check();
    state = state.apply(tr);
  };
  return {
    get state() {
      return state;
    },
    get source() {
      return current;
    },
    dispatch,
  };
}
test("local edits retain untouched annotation spelling and all other columns", () => {
  const e = editor(
    "一\r\n未（いまだ｜ズ）＃００１\r《割書：a｜b｜c｜d》\n《未知：□》",
  );
  const original = e.source;
  const second = e.state.doc.child(0).nodeSize;
  const tr = e.state.tr.insertText("X", second + 3, second + 4);
  const patch = sourcePatches(tr);
  expect(patch).toEqual([{ from: 3, to: 4, text: "X" }]);
  e.dispatch(tr);
  expect(e.source).toBe(original.replace("未", "X"));
  undo(e.state, e.dispatch);
  expect(e.source).toBe(original);
  redo(e.state, e.dispatch);
  expect(e.source).toBe(original.replace("未", "X"));
});
test("every annotation constituent is editable", () => {
  for (const source of [
    "《振り仮名：未｜いまだ｜ズ》",
    "《割書：a｜b｜c｜d》",
    "《見せ消ち：旧｜新》",
    "《圏点：語｜﹅》",
    "《題：題》",
    "《箱：字》",
    "《場所：町》",
    "《右線：行》",
    "＿レ",
    "￣ム",
    "《未知：a》",
  ]) {
    const e = editor(source);
    const positions: number[] = [];
    e.state.doc.descendants((n, pos) => {
      if (n.isText) positions.push(pos);
    });
    for (const pos of [...positions].reverse())
      e.dispatch(e.state.tr.insertText("追", pos));
    expect(e.source.match(/追/g)?.length).toBe(positions.length);
  }
});
test("ruby command, source edits and undo share one history", () => {
  const e = editor("峰\r\n原文");
  e.dispatch(e.state.tr.setSelection(TextSelection.create(e.state.doc, 1, 2)));
  expect(wrapSelection("ruby", "みね")(e.state, e.dispatch)).toBe(true);
  expect(e.source).toBe("《振り仮名：峰｜みね》\r\n原文");
  replaceSource("別\r次")(e.state, e.dispatch);
  expect(e.source).toBe("別\r次");
  undo(e.state, e.dispatch);
  expect(e.source).toBe("《振り仮名：峰｜みね》\r\n原文");
  undo(e.state, e.dispatch);
  expect(e.source).toBe("峰\r\n原文");
});
test("column insertion, deletion and nonadjacent patches", () => {
  const e = editor("一二\r\n三四\r五六\n七八");
  e.dispatch(e.state.tr.setSelection(TextSelection.create(e.state.doc, 2)));
  newColumn(e.state, e.dispatch);
  expect(e.source).toBe("一\r\n二\r\n三四\r五六\n七八");
  const tr = e.state.tr
    .insertText("A", 1)
    .insertText("Z", e.state.doc.content.size);
  expect(sourcePatches(tr)).toHaveLength(2);
  e.dispatch(tr);
  e.dispatch(e.state.tr.delete(0, e.state.doc.child(0).nodeSize));
});
test("grapheme navigation, column directions, Home and End", () => {
  const e = editor("葛\u{E0100}𬼂\nあいう");
  moveCaret("ArrowDown")(e.state, e.dispatch);
  expect(e.state.selection.from).toBe(4);
  moveCaret("ArrowLeft")(e.state, e.dispatch);
  expect(e.state.selection.$head.index(0)).toBe(1);
  moveCaret("ArrowRight")(e.state, e.dispatch);
  expect(e.state.selection.$head.index(0)).toBe(0);
  moveCaret("End")(e.state, e.dispatch);
  expect(e.state.selection.from).toBe(6);
  moveCaret("Home")(e.state, e.dispatch);
  expect(e.state.selection.from).toBe(1);
});
test("split next to a nested annotation produces valid columns", () => {
  const e = editor("《振り仮名：峰｜みね》続き");
  e.dispatch(e.state.tr.setSelection(TextSelection.create(e.state.doc, 3)));
  newColumn(e.state, e.dispatch);
  expect(e.source).toBe("《振り仮名：峰｜みね》\n続き");
});
test("malformed and empty documents remain schema-valid", () => {
  for (const text of [
    "",
    "\n",
    "《",
    "【",
    "《割書：｜｜｜》",
    "《振り仮名：｜｜》",
    "未（｜）",
    "𬼂\uD800",
  ]) {
    const doc = fromMarkup(text);
    doc.check();
    expect(toMarkup(doc)).toBe(text);
  }
  expect(
    schema.nodes.ruby.validContent(schema.nodes.column.create().content),
  ).toBe(false);
});

test("textarea edits preserve untouched CRLF and CR lines", () => {
  expect(textareaSource("一\r\n二\r三\n", "一\n追二\n三\n")).toBe(
    "一\r\n追二\r三\n",
  );
  expect(textareaSource("一\r\n二", "一\n二")).toBe("一\r\n二");
  expect(textareaSource("一\r\n二", "一\n新\n二")).toBe("一\r\n新\r\n二");
});

test("dialog annotations preserve four editable fields and surrounding source", async () => {
  const { insertAnnotation } = await import("./index");
  const e = editor("前後\r\n残す＃００１");
  e.dispatch(e.state.tr.setSelection(TextSelection.create(e.state.doc, 2)));
  expect(
    insertAnnotation("warigaki", ["一", "二", "三", "四"])(e.state, e.dispatch),
  ).toBe(true);
  expect(toMarkup(e.state.doc)).toBe(
    "前《割書：一｜二｜三｜四》後\r\n残す＃００１",
  );
  let second = -1;
  e.state.doc.descendants((node, pos) => {
    if (node.type.name === "segment" && node.attrs.role === "line-1")
      second = pos + 1;
  });
  e.dispatch(e.state.tr.insertText("追記", second + 1));
  expect(toMarkup(e.state.doc)).toBe(
    "前《割書：一｜二追記｜三｜四》後\r\n残す＃００１",
  );
});

test("vertical navigation enters an empty annotation field", () => {
  const e = editor("《割書：一｜》");
  let empty = -1;
  e.state.doc.descendants((node, pos) => {
    if (node.type.name === "segment" && !columnSource(node)) empty = pos + 1;
  });
  e.dispatch(
    e.state.tr.setSelection(TextSelection.create(e.state.doc, empty - 2)),
  );
  moveCaret("ArrowDown")(e.state, e.dispatch);
  expect(e.state.selection.head).toBe(empty);
  e.dispatch(e.state.tr.insertText("二"));
  expect(e.source).toBe("《割書：一｜二》");
});

test("nested shells navigate, serialize and enforce grammar", async () => {
  const { shellKey, selectColumn } = await import("./index");
  const e = editor("前後\n別列");
  e.dispatch(e.state.tr.setSelection(TextSelection.create(e.state.doc, 2)));
  expect(wrapSelection("warigaki")(e.state, e.dispatch)).toBe(true);
  expect(e.source).toBe("前《割書：｜》後\n別列");
  expect(wrapSelection("ruby")(e.state, e.dispatch)).toBe(true);
  e.dispatch(e.state.tr.insertText("峰"));
  shellKey("Tab")(e.state, e.dispatch);
  e.dispatch(e.state.tr.insertText("みね"));
  expect(wrapSelection("warigaki")(e.state, e.dispatch)).toBe(false);
  shellKey("ArrowRight")(e.state, e.dispatch);
  shellKey("Enter")(e.state, e.dispatch);
  e.dispatch(e.state.tr.insertText("二"));
  expect(e.source).toBe("前《割書：《振り仮名：峰｜みね》｜二》後\n別列");
  expect(toMarkup(fromMarkup(e.source))).toBe(e.source);
  shellKey("Enter")(e.state, e.dispatch);
  shellKey("Enter")(e.state, e.dispatch);
  shellKey("Enter")(e.state, e.dispatch);
  expect(e.source).toBe("前《割書：《振り仮名：峰｜みね》｜二｜｜》後\n別列");
  shellKey("Backspace")(e.state, e.dispatch);
  expect(e.source).toBe("前《割書：《振り仮名：峰｜みね》｜二｜》後\n別列");
  selectColumn(e.state, e.dispatch);
  expect(e.state.selection.from).toBe(1);
  selectColumn(e.state, e.dispatch);
  expect(e.state.selection.to).toBe(e.state.doc.content.size - 1);
});

test("removing the last empty warigaki line preserves its remaining text", async () => {
  const { shellKey } = await import("./index");
  const e = editor("前後");
  e.dispatch(e.state.tr.setSelection(TextSelection.create(e.state.doc, 2)));
  wrapSelection("warigaki")(e.state, e.dispatch);
  e.dispatch(e.state.tr.insertText("一"));
  shellKey("Enter")(e.state, e.dispatch);
  shellKey("Backspace")(e.state, e.dispatch);
  expect(e.source).toBe("前一後");
});

test("a selected kana run becomes katakana okurigana after the preceding character", () => {
  const cases: [string, number, number, string | false][] = [
    ["讀む", 2, 3, "讀￣ム"],
    ["讀むな", 2, 4, "讀￣ムナ"],
    ["告げテ", 2, 4, "告￣ゲテ"],
    ["讀む", 1, 3, false],
    ["むな", 1, 3, false],
  ];
  for (const [source, from, to, expected] of cases) {
    const e = editor(source);
    const tr = e.state.tr.setSelection(TextSelection.create(e.state.doc, from, to));
    e.dispatch(tr);
    const done = okuriganaFromSelection(e.state, e.dispatch);
    expect(done).toBe(expected !== false);
    if (expected !== false) expect(e.source).toBe(expected);
  }
});
