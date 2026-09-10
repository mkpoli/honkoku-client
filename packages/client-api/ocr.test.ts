import { expect, test } from "bun:test";
import fixture from "../../fixtures/api/page-minna-ocr.json";
import { pageLines } from "./ocr";
import type { Page } from "./types";

test("minna fixture scales from a 3500px image into the canvas", () => {
  const result = pageLines(fixture.page as unknown as Page, fixture.canvas);
  expect(result.engine).toBe("minna");
  expect(result.estimated).toBe(false);
  expect(result.lines).toHaveLength(10);
  expect(result.lines[0].text).toBe("雪の花　　青");
  expect(result.lines[0].x).toBeCloseTo(2253 * 1.848);
  for (const line of result.lines) {
    expect(line.x + line.width).toBeLessThanOrEqual(fixture.canvas.width);
    expect(line.y + line.height).toBeLessThanOrEqual(fixture.canvas.height);
  }
});

test("NDL numeric strings stay in canvas coordinates and sort by centre then y", () => {
  const box = (x: string, y: number) => ({
    x,
    y,
    width: "10",
    height: "100",
    text: { text: "本文" },
  });
  const result = pageLines(
    {
      ocr: {
        ndl: {
          text: "",
          textBlocks: [box("10", 20), box("80", 30), box("80", 10)],
        },
      },
    },
    { width: 200, height: 200 },
  );
  expect(result.lines.map((l) => [l.x, l.y, l.half])).toEqual([
    [80, 10, "right"],
    [80, 30, "right"],
    [10, 20, "left"],
  ]);
  expect(result.estimated).toBe(false);
});

test("minna takes precedence, does not enlarge small images, and estimates overflowing boxes", () => {
  const page = {
    ocr: {
      minna: {
        text: "",
        lines: [{ x: 10, y: 20, width: 50, height: 100, raw: "本文" }],
      },
      ndl: "別本文",
    },
  };
  expect(pageLines(page, { width: 200, height: 200 }).lines[0].x).toBe(10);
  const result = pageLines(page, { width: 50, height: 60 });
  expect(result.estimated).toBe(true);
  expect(result.lines[0].y + result.lines[0].height).toBe(60);
  expect(
    pageLines({ ocr: { minna: "本文" } }, { width: 100, height: 100 }).lines,
  ).toEqual([]);
});

import { pageLinesWithLocal } from "./ocr";

test("local OCR takes precedence in canvas coordinates and preserves reading order", () => {
  const result = pageLinesWithLocal(
    {
      ocr: {
        minna: { lines: [{ x: 1, y: 1, width: 2, height: 2, raw: "site" }] },
        local: {
          width: 100,
          height: 200,
          lines: [
            {
              reading_order: 2,
              x: 70,
              y: 20,
              width: 10,
              height: 90,
              confidence: 0.8,
              plain: "second",
            },
            {
              reading_order: 1,
              x: 10,
              y: 20,
              width: 10,
              height: 90,
              confidence: 0.95,
              plain: "first",
            },
          ],
        },
      },
    } as unknown as Page,
    { width: 200, height: 400 },
  );
  expect(result.engine).toBe("local");
  expect(result.lines.map((l) => l.text)).toEqual(["first", "second"]);
  expect(result.lines[0].x).toBe(20);
  expect(result.lines[0].height).toBe(180);
  expect(result.estimated).toBe(false);
});
