import { expect, test } from "bun:test";
import {
  EditorState,
  NodeSelection,
  TextSelection,
  type Command,
} from "prosemirror-state";
import {
  fromMarkup,
  toMarkup,
  moveCaret,
  insertText,
  insertSource,
  wrapSelection,
  insertOkurigana,
  annotation,
  schema,
  selectedMarkup,
  clipboardMarkup,
  applyPatches,
  sourcePatches,
} from "./index";
import {
  caretContext,
  caretPlugin,
  caretPluginKey,
  contextFields,
  contextSelection,
  deleteContext,
  enterContext,
  escapeContext,
  isContext,
  selectContext,
} from "./caret";

function editor(source: string) {
  let state = EditorState.create({
    doc: fromMarkup(source),
    plugins: [caretPlugin()],
  });
  const run = (command: Command) =>
    command(state, (tr) => {
      expect(applyPatches(toMarkup(state.doc), sourcePatches(tr))).toBe(
        toMarkup(tr.doc),
      );
      tr.doc.check();
      state = state.apply(tr);
    });
  return {
    get state() {
      return state;
    },
    get source() {
      return toMarkup(state.doc);
    },
    run,
    at(from: number, to = from) {
      run((s, d) => {
        d!(s.tr.setSelection(TextSelection.create(s.doc, from, to)));
        return true;
      });
    },
  };
}
const examples = [
  "讀＿レ￣ム",
  "未（いまだ｜ズ）",
  "《割書：一行｜二行》",
  "《割書：《振り仮名：峰｜みね》｜二行》",
];
const sequences = [
  [1, 2, "big:2", 7, 9, 10, 12],
  [1, 3, 4, 6, 7, 8, 9, 11, 12, 14],
  [1, 3, 4, 5, 7, 8, 9, 11],
  [1, 3, 5, 6, 8, 9, 10, 12, 14, 15, 16, 18],
];
for (const [i, source] of examples.entries()) {
  for (const horizontal of [false, true])
    test(`traversal positions and contexts ${source} ${horizontal ? "horizontal" : "vertical"}`, () => {
      const e = editor(source);
      const sequence = sequences[i];
      const snapshot = () =>
        e.state.selection instanceof NodeSelection
          ? `big:${e.state.selection.from}`
          : e.state.selection.head;
      expect(snapshot()).toBe(sequence[0]);
      for (const pos of sequence.slice(1)) {
        e.run(
          moveCaret(horizontal ? "ArrowRight" : "ArrowDown", false, horizontal),
        );
        expect(snapshot()).toBe(pos);
        expect(caretPluginKey.getState(e.state)).toEqual(caretContext(e.state));
      }
      // Atoms have one selected stop in each direction, with a caret on either side.
      const reverse =
        i === 0 ? [10, 9, 7, "big:2", 2, 1] : sequence.slice(0, -1).reverse();
      for (const pos of reverse) {
        e.run(
          moveCaret(horizontal ? "ArrowLeft" : "ArrowUp", false, horizontal),
        );
        expect(snapshot()).toBe(pos);
      }
      expect(e.source).toBe(source);
    });
  test(`all construct and field deletion boundaries ${source}`, () => {
    const contexts: { pos: number; size: number }[] = [];
    fromMarkup(source).descendants((node, pos) => {
      if (isContext(node)) contexts.push({ pos, size: node.nodeSize });
    });
    for (const { pos, size } of contexts) {
      for (const backwards of [true, false]) {
        const e = editor(source);
        e.at(backwards ? pos + size : pos);
        e.run(deleteContext(backwards));
        expect(e.state.selection).toBeInstanceOf(NodeSelection);
        expect(e.state.selection.from).toBe(pos);
        expect(e.source).toBe(source);
        e.run(deleteContext(!backwards));
        expect(
          e.state.doc.nodeAt(pos)?.eq(fromMarkup(source).nodeAt(pos)!),
        ).not.toBe(true);
      }
      const node = fromMarkup(source).nodeAt(pos)!;
      for (const field of contextFields(node, pos)) {
        for (const backwards of [true, false]) {
          const e = editor(source);
          e.at(backwards ? field.from : field.to);
          expect(e.run(deleteContext(backwards))).toBe(true);
          expect(e.source).toBe(source);
        }
      }
    }
  });
  test(`whole source copy, cut, replacement, entry and word movement ${source}`, () => {
    const doc = fromMarkup(source);
    doc.descendants((node, pos) => {
      if (!isContext(node)) return;
      const e = editor(source);
      e.run(selectContext(pos));
      const text = selectedMarkup(
        e.state.doc,
        e.state.selection.from,
        e.state.selection.to,
      );
      expect(toMarkup(fromMarkup(text))).toBe(text);
      const tr = e.state.tr
        .deleteSelection()
        .replaceSelection(clipboardMarkup(text));
      expect(toMarkup(tr.doc)).toBe(source);
      e.run(insertText("字"));
      expect(e.source).toBe(
        toMarkup(doc.replace(pos, pos + node.nodeSize, clipboardMarkup("字"))),
      );
      for (const forward of [true, false]) {
        const e = editor(source);
        e.run(selectContext(pos));
        e.run(enterContext(forward));
        const fields = contextFields(node, pos);
        expect(e.state.selection.head).toBe(
          fields.length
            ? forward
              ? fields[0].from
              : fields.at(-1)!.to
            : forward
              ? pos + node.nodeSize
              : pos,
        );
        e.at(forward ? pos : pos + node.nodeSize);
        e.run(moveCaret(forward ? "ArrowDown" : "ArrowUp", false, false, true));
        expect(e.state.selection.head).toBe(
          forward ? pos + node.nodeSize : pos,
        );
      }
    });
  });
}
test("Esc climbs nested contexts and context labels follow each field", () => {
  const e = editor(examples[3]);
  e.at(8);
  expect(caretContext(e.state)).toEqual({
    mode: "small",
    path: [
      { pos: 1, label: "割書 1行目", field: 0 },
      { pos: 3, label: "振り仮名 読み", field: 1 },
    ],
  });
  e.run(escapeContext);
  expect(e.state.selection.from).toBe(3);
  expect(caretContext(e.state).mode).toBe("big");
  e.run(escapeContext);
  expect(e.state.selection.from).toBe(1);
  expect(caretContext(e.state).path).toHaveLength(1);
  e.run(escapeContext);
  expect(e.state.selection.head).toBe(18);
  expect(caretContext(e.state)).toEqual({ mode: "text", path: [] });
});
test("Shift selects whole constructs from outside and preserves selections inside one field", () => {
  for (const source of examples) {
    const e = editor(source);
    e.state.doc.descendants((node, pos) => {
      if (!isContext(node)) return;
      e.at(pos);
      e.run(moveCaret("ArrowDown", true));
      expect([e.state.selection.from, e.state.selection.to]).toEqual([
        pos,
        pos + node.nodeSize,
      ]);
      e.at(pos + node.nodeSize);
      e.run(moveCaret("ArrowUp", true));
      expect([e.state.selection.from, e.state.selection.to]).toEqual([
        pos,
        pos + node.nodeSize,
      ]);
      for (const field of contextFields(node, pos)) {
        e.at(field.from);
        e.run(moveCaret("ArrowDown", true));
        expect(e.state.selection.from).toBe(field.from);
        expect(e.state.selection.to).toBeLessThanOrEqual(field.to);
        const selection = contextSelection(
          e.state.doc,
          field.from,
          pos + node.nodeSize,
        );
        expect([selection.from, selection.to]).toEqual([
          pos,
          pos + node.nodeSize,
        ]);
      }
    });
  }
});
test("Shift across a field boundary promotes the whole enclosing context", () => {
  const e = editor(examples[3]);
  e.at(9, 10);
  e.run(moveCaret("ArrowDown", true));
  expect([e.state.selection.from, e.state.selection.to]).toEqual([3, 12]);
  e.run(moveCaret("ArrowDown", true));
  expect([e.state.selection.from, e.state.selection.to]).toEqual([1, 18]);
});
test("empty constructs remove only with the specified deletion keys", () => {
  for (const kind of [
    "ruby",
    "warigaki",
    "misekechi",
    "kenten",
    "okurigana",
    "note",
    "place",
    "person",
    "date",
    "title",
  ]) {
    for (const backwards of [true, false]) {
      const values = ["ruby", "warigaki", "misekechi", "kenten"].includes(kind)
        ? ["", ""]
        : [""];
      const node = annotation(kind, values);
      let state = EditorState.create({
        doc: schema.nodes.doc.create(
          null,
          schema.nodes.column.create(null, node),
        ),
      });
      for (const field of contextFields(node, 1)) {
        const initial = state;
        state = state.apply(
          state.tr.setSelection(TextSelection.create(state.doc, field.from)),
        );
        deleteContext(backwards)(state, (tr) => {
          state = state.apply(tr);
        });
        expect(state.doc.firstChild!.childCount).toBe(
          backwards || values.length === 1 ? 0 : 1,
        );
        state = initial;
      }
    }
  }
});
test("Backspace from inside okurigana removes only a kana, then the empty shell", () => {
  const e = editor("讀＿レ￣ム");
  e.at(12);
  e.run(moveCaret("ArrowUp"));
  e.run(deleteContext(true));
  expect(e.source).toBe("讀＿レ￣");
  expect(caretContext(e.state).mode).toBe("small");
  e.run(deleteContext(true));
  expect(e.source).toBe("讀＿レ");
});
test("all named fields and arbitrary nesting are contexts; return and glyphs are atoms", () => {
  for (const source of [
    "《注記：欄外》",
    "《場所：京都》",
    "《人物：太郎》",
    "《日時：昨日》",
    "《題：日記》",
    "《圏点：字｜﹅》",
    "《見せ消ち：旧｜新》",
  ]) {
    const e = editor(source);
    e.run(moveCaret("ArrowDown"));
    expect(caretContext(e.state).mode).toBe("small");
    expect(e.state.selection.head).toBe(3);
  }
  for (const atom of ["＿レ", "□", "■", "＃１２"]) {
    const e = editor(atom);
    e.run(moveCaret("ArrowDown"));
    expect(caretContext(e.state).mode).toBe("big");
    e.run(enterContext());
    expect(e.state.selection.head).toBe(e.state.doc.content.size - 1);
  }
  let nested = "字";
  for (let i = 0; i < 16; i++)
    nested = `《割書：《振り仮名：${nested}｜よみ》｜二》`;
  const e = editor(nested);
  for (let i = 0; i < 32; i++) e.run(moveCaret("ArrowDown"));
  expect(caretContext(e.state).path).toHaveLength(32);
  expect(e.source).toBe(nested);
});
test("palette commands enter the first field and atoms leave a caret after them", () => {
  for (const command of [
    wrapSelection("ruby"),
    wrapSelection("warigaki"),
    wrapSelection("misekechi"),
    insertSource("《場所：京都》"),
  ]) {
    const e = editor("前後");
    e.at(2);
    e.run(command);
    expect(e.state.selection.head).toBe(4);
    expect(caretContext(e.state).mode).toBe("small");
  }
  const e = editor("前後");
  e.at(2);
  e.run(insertOkurigana("ム"));
  expect(e.state.selection.head).toBe(4);
  for (const atom of ["＿レ", "□", "■", "＃1"]) {
    const e = editor("前後");
    e.at(2);
    e.run(insertSource(atom));
    expect(caretContext(e.state).mode).toBe("text");
    expect(e.state.selection.$head.nodeBefore!.isAtom).toBe(true);
  }
});
