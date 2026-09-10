import { expect, test } from "bun:test";
import fixture from "../../fixtures/api/page-minna-ocr.json";
import { pageLines, type CanvasLine } from "../client-api/ocr";
import type { Page } from "../client-api/types";
import { alignColumns, dice, transcriptionColumns } from "./align";
const lines = (...texts: string[]): CanvasLine[] =>
  texts.map((text, index) => ({
    index: index + 10,
    text,
    x: 100 - index * 10,
    y: 0,
    width: 5,
    height: 100,
    confidence: 1,
    half: null,
  }));

test("fixture matches every source column in reading order", () => {
  const columns = transcriptionColumns(fixture.page.text);
  expect(columns).toHaveLength(8);
  expect(columns[0].sourceIndex).toBe(1);
  expect(
    alignColumns(
      columns.map((c) => c.text),
      pageLines(fixture.page as unknown as Page, fixture.canvas).lines,
    ),
  ).toEqual([1, 2, 3, 4, 6, 7, 8, 9]);
});
test("one column spans adjacent OCR lines", () => {
  expect(
    alignColumns(
      ["春はあけぼの夏は夜", "秋は夕暮れ"],
      lines("春はあけぼの", "夏は夜", "秋は夕暮れ"),
    ),
  ).toEqual([10, 12]);
});
test("two columns share a merged OCR line", () => {
  expect(
    alignColumns(
      ["春はあけぼの", "夏は夜", "秋は夕暮れ"],
      lines("春はあけぼの夏は夜", "秋は夕暮れ"),
    ),
  ).toEqual([10, 10, 11]);
});
test("missing and extra OCR lines do not shift matches", () => {
  expect(
    alignColumns(
      ["春はあけぼの", "海辺の松林", "秋は夕暮れ"],
      lines("春はあけぼの", "秋は夕暮れ"),
    ),
  ).toEqual([10, null, 11]);
  expect(
    alignColumns(
      ["春はあけぼの", "秋は夕暮れ"],
      lines("欄外の書入れ", "春はあけぼの", "別の注記", "秋は夕暮れ"),
    ),
  ).toEqual([11, 13]);
});
test("unsupported and empty columns remain unmatched", () => {
  expect(alignColumns(["山川", "", "あ"], lines("海辺", "い"))).toEqual([
    null,
    null,
    null,
  ]);
  expect(alignColumns([], [])).toEqual([]);
  expect(alignColumns(["本文"], [])).toEqual([null]);
});
test("folding, multiset bigrams and plain source columns", () => {
  expect(dice("カタカナ、　文字", "かたかな文字")).toBe(1);
  expect(dice("ああああ", "ああ")).toBe(0.5);
  expect(
    transcriptionColumns(
      "【右丁】\r\n\r\n《振り仮名：峰｜みね》＃1\n【左丁】\n《割書：一行｜二行》讀＿レ￣ム",
    ),
  ).toEqual([
    { sourceIndex: 2, text: "峰" },
    { sourceIndex: 4, text: "一行二行讀ム" },
  ]);
});
