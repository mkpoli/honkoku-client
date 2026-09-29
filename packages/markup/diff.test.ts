import { expect, test } from "bun:test";
import { diffSource, diffTokens } from "./diff";
test("character diff shows replacement, additions, deletion and empty saves", () => {
  expect(diffSource("山に花あり", "山に雪あり")).toEqual([
    { kind: "equal", text: "山に" },
    { kind: "delete", text: "花" },
    { kind: "insert", text: "雪" },
    { kind: "equal", text: "あり" },
  ]);
  expect(diffSource("", "字")).toEqual([{ kind: "insert", text: "字" }]);
  expect(diffSource("字", "")).toEqual([{ kind: "delete", text: "字" }]);
  expect(diffSource("", "")).toEqual([]);
});
test("supplementary kana and markup reconstruct both original sources", () => {
  for (const a of ["𛀂あ《振り仮名：峰｜みね》\n□", "aaaa", "abac", ""])
    for (const b of ["𛀃あ《振り仮名：峰｜ミネ》\n■", "baba", "a", ""]) {
      const changes = diffSource(a, b);
      expect(
        changes
          .filter((c) => c.kind !== "insert")
          .map((c) => c.text)
          .join(""),
      ).toBe(a);
      expect(
        changes
          .filter((c) => c.kind !== "delete")
          .map((c) => c.text)
          .join(""),
      ).toBe(b);
      expect(changes.every((c) => !/[\uD800-\uDFFF]/u.test(c.text))).toBe(true);
    }
});
test("long identical pages and short edit boundaries are inexpensive", () => {
  const source = "古文書".repeat(10000);
  expect(diffSource(source + "春", source + "秋")).toHaveLength(3);
});

test("width-only replacements are marked in both directions", () => {
  for (const [half, full] of [
    ["-", "－"],
    ["A", "Ａ"],
    [" ", "　"],
    ["A- ", "Ａ－　"],
    ["ｶﾞ", "ガ"],
    ["｡ｶﾀｶﾅ", "。カタカナ"],
    ["￦", "₩"],
    ["AＢ", "ＡB"],
  ]) {
    for (const [before, after] of [
      [half, full],
      [full, half],
    ]) {
      const changes = diffSource(`前${before}後`, `前${after}後`);
      expect(changes.filter((c) => c.kind !== "equal")).toEqual([
        { kind: "delete", text: before, widthOnly: true },
        { kind: "insert", text: after, widthOnly: true },
      ]);
      expect(changes.filter((c) => c.kind === "equal")).toEqual([
        { kind: "equal", text: "前" },
        { kind: "equal", text: "後" },
      ]);
    }
  }
});

test("ordinary replacements and non-width compatibility forms are not marked", () => {
  for (const [before, after] of [
    ["十", "一"],
    ["A", "Ｂ"],
    ["①", "1"],
    ["ﬀ", "ff"],
    ["\u00a0", " "],
    ["", "Ａ"],
    ["Ａ", ""],
    ["Ａ", "Ａ"],
    ["A①", "Ａ1"],
  ])
    expect(diffSource(before, after).some((c) => c.widthOnly)).toBe(false);
});

test("changed whitespace has distinct visible marks and accessible labels", () => {
  for (const kind of ["insert", "delete"] as const) {
    const text = "十 　\t\r\n\n\r一\u2028\u2029\u00a0\u200b";
    const tokens = diffTokens({ kind, text });
    expect(tokens.map((t) => t.text).join("")).toBe(text);
    expect(
      tokens.filter((t) => t.mark).map((t) => [t.mark, t.label, t.lineBreak]),
    ).toEqual([
      ["·", "半角空白", false],
      ["⬚", "全角空白", false],
      ["⇥", "タブ", false],
      ["↵", "改行", true],
      ["↵", "改行", true],
      ["↵", "改行", true],
      ["↵", "改行", true],
      ["↵", "改行", true],
      ["·", "空白（U+00A0）", false],
      ["·", "空白（U+200B）", false],
    ]);
    expect(tokens.filter((t) => !t.mark)).toEqual([
      { text: "十" },
      { text: "一" },
    ]);
  }
});

test("unchanged whitespace and literal mark characters stay untouched", () => {
  const text = "𛀂 ␣□↵　\t\n";
  expect(diffTokens({ kind: "equal", text })).toEqual([{ text }]);
  expect(diffTokens({ kind: "delete", text: "𛀂␣□↵" })).toEqual([
    { text: "𛀂␣□↵" },
  ]);
  expect(diffTokens({ kind: "insert", text: "" })).toEqual([]);
});

test("repeated whitespace keeps one mark per space and per line ending", () => {
  expect(
    diffTokens({ kind: "delete", text: " 　\n\n" }).map((t) => t.mark),
  ).toEqual(["·", "⬚", "↵", "↵"]);
});
