import { test, expect } from "bun:test";
import {
  estimateRegion,
  glyphCards,
  regionUrl,
  singleGlyph,
} from "./glyph-regions";
import fixture from "../../../../fixtures/glyphs/attestations-候.json";
import type {
  GlyphAttestation,
  Canvas,
  GlyphOccurrence,
} from "@honkoku/client-api/types";
import type { CanvasLine } from "../../../../packages/client-api/ocr";
const canvas: Canvas = {
  id: "canvas",
  width: 1000,
  height: 1000,
  infoJsonUrl: "https://example.org/?IIIF=/a%2Fb/info.json",
};
const line: CanvasLine = {
  index: 0,
  x: 100,
  y: 100,
  width: 50,
  height: 500,
  text: "𛀁候文字列",
  confidence: 1,
  half: null,
};
const occurrence: GlyphOccurrence = {
  column: 0,
  plain: line.text,
  offset: 1,
  before: "𛀁",
  matched: "候",
  after: "文字列",
};
test("portrait and landscape boxes use scalar offset, grapheme advance and 30% padding", () => {
  expect(estimateRegion(line, occurrence, canvas)).toEqual([85, 170, 80, 160]);
  expect(
    estimateRegion({ ...line, width: 500, height: 50 }, occurrence, canvas),
  ).toEqual([170, 85, 160, 80]);
  expect(
    estimateRegion(
      { ...line, x: 0, y: 0 },
      { ...occurrence, offset: 0 },
      canvas,
    ),
  ).toEqual([0, 0, 65, 130]);
  expect(
    estimateRegion(
      line,
      { ...occurrence, plain: "𛀁\u{E0100}候文字列", offset: 2 },
      canvas,
    ),
  ).toEqual([85, 170, 80, 160]);
});
test("crop URL keeps query service and requests 192px on long side", () => {
  expect(regionUrl(canvas, [10, 20, 40, 60])).toBe(
    "https://example.org/?IIIF=/a%2Fb/10,20,40,60/,192/0/default.jpg",
  );
});
test("three OCR pages have crops, fourth stays text-only", () => {
  const pages = fixture.pages as unknown as GlyphAttestation[];
  expect(
    pages.slice(0, 3).every((p) => glyphCards(p).some((c) => c.crop)),
  ).toBe(true);
  expect(glyphCards(pages[3]).every((c) => !c.crop)).toBe(true);
});
test("accepts one grapheme including astral kana and variation sequences", () => {
  for (const c of ["候", "𛀁", "葛\u{E0100}", "か\u3099"])
    expect(singleGlyph(c)).toBe(true);
  for (const c of ["", "蝦夷", " "]) expect(singleGlyph(c)).toBe(false);
});
