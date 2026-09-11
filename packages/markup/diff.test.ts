import { expect, test } from "bun:test";
import { diffSource } from "./diff";
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
