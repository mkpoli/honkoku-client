import { expect, test } from "bun:test";
import { fitTextScale, wrappingLines, type LineFit } from "./fit-text";

// Columns sized so exactly 600px remain once the rounding slack is kept free.
const room = (600 + 2) / 0.995;
const page = (...lengths: number[]): LineFit[] =>
  lengths.map((length) => ({ length, room }));

test("the longest line decides the scale when no line stands out", () => {
  expect(fitTextScale(page(500, 600, 750))).toBe(0.8);
  expect(fitTextScale(page(300, 400))).toBe(1.25);
});

test("while the text stays comfortable, even a far longer line is fitted whole", () => {
  const lines = page(700, 400, 400);
  expect(fitTextScale(lines)).toBe(0.85);
  expect(wrappingLines(lines, 0.85)).toEqual([]);
  // 4.7 times the heading, yet 85% reads well, so nothing wraps.
  expect(fitTextScale(page(150, 700))).toBe(0.85);
});

test("a line far longer than the rest wraps and the page keeps its size", () => {
  const lines = page(600, 600, 580, 1100);
  expect(fitTextScale(lines)).toBe(1);
  expect(wrappingLines(lines, 1)).toEqual([3]);
});

test("equally long outliers wrap together", () => {
  expect(fitTextScale(page(1100, 1100, 600, 600, 600, 600, 600, 600))).toBe(1);
});

test("lines that cannot fit even at the lower bound always wrap", () => {
  expect(fitTextScale(page(200, 5000))).toBe(1.25);
  const lines = page(1500, 600, 600);
  expect(fitTextScale(lines)).toBe(1);
  expect(wrappingLines(lines, 1)).toEqual([0]);
  // With every line hopeless, the text stays at a comfortable size.
  expect(fitTextScale(page(1500, 1500, 1500))).toBe(0.75);
});

test("headings and short entries do not turn body lines into outliers", () => {
  expect(fitTextScale(page(150, 150, 150, 150, 900, 920, 950))).toBe(0.63);
});

test("at most a quarter of the page wraps as outliers", () => {
  // Two long lines stand above the body. Of six lines only one may wrap, so
  // both are fitted; of eight, two may, and the body keeps its full size.
  expect(fitTextScale(page(1150, 1100, 600, 600, 600, 600))).toBe(0.52);
  expect(
    fitTextScale(page(1150, 1100, 600, 600, 600, 600, 600, 600)),
  ).toBe(1);
});

test("empty pages and blank lines leave the scale at its maximum", () => {
  expect(fitTextScale([])).toBe(1.25);
  expect(fitTextScale(page(0, 0))).toBe(1.25);
});

test("wrapping at a manual scale", () => {
  expect(wrappingLines(page(600), 1)).toEqual([]);
  expect(wrappingLines(page(600), 1.1)).toEqual([0]);
});
