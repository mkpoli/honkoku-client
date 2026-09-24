import { test, expect } from "bun:test";
import { lineFrameInsets, lineFramePads, lineFrameRect } from "./line-frame";

test("vertical lines grow mostly in height so the whole column sits inside", () => {
  const frame = lineFrameRect({ x: 100, y: 50, width: 80, height: 1200 });
  expect(frame.y).toBeLessThan(50);
  expect(frame.y + frame.height).toBeGreaterThan(50 + 1200);
  expect(frame.height - 1200).toBeGreaterThan(frame.width - 80);
  expect(frame.x).toBeLessThan(100);
  expect(frame.x + frame.width).toBeGreaterThan(100 + 80);
});

test("horizontal lines grow mostly in width", () => {
  const frame = lineFrameRect({ x: 10, y: 20, width: 400, height: 40 });
  expect(frame.width - 400).toBeGreaterThan(frame.height - 40);
  expect(frame.x + frame.width).toBeGreaterThan(10 + 400);
});

test("tiny boxes still get the minimum margin on every side", () => {
  const frame = lineFrameRect({ x: 0, y: 0, width: 4, height: 20 });
  expect(frame.height).toBeGreaterThanOrEqual(20 + 2 * 28);
  expect(frame.width).toBeGreaterThanOrEqual(4 + 2 * 10);
});

test("the frame is exactly the line inset by the shared pads", () => {
  const line = { x: 10, y: 20, width: 200, height: 30 };
  const frame = lineFrameRect(line);
  const { padX, padY } = lineFramePads(line);
  expect(frame.x).toBe(line.x - padX);
  expect(frame.y).toBe(line.y - padY);
  expect(frame.width).toBe(line.width + 2 * padX);
  expect(frame.height).toBe(line.height + 2 * padY);
});

test("insets express the pads relative to the raw box", () => {
  const line = { x: 0, y: 0, width: 80, height: 1200 };
  const { padX, padY } = lineFramePads(line);
  const insets = lineFrameInsets(line);
  expect(insets.x).toBeCloseTo((padX / 80) * 100);
  expect(insets.y).toBeCloseTo((padY / 1200) * 100);
  expect(lineFrameInsets({ x: 0, y: 0, width: 0, height: 0 })).toEqual({
    x: 0,
    y: 0,
  });
});
