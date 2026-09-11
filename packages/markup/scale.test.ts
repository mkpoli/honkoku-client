import { expect, test } from "bun:test";
import {
  automaticTextScale,
  longestColumnLength,
  visualColumnLength,
  textScalePadding,
} from "./scale";

test("visual lengths count bases, half-size split lines, and note badges", () => {
  expect(visualColumnLength("《振り仮名：春｜はる》山")).toBe(2);
  expect(visualColumnLength("讀＿レ￣ム書")).toBe(2);
  expect(visualColumnLength("《割書：一二｜三四五六》七")).toBe(3);
  expect(visualColumnLength("《割書：《振り仮名：山｜やま》｜川川川川》")).toBe(
    2,
  );
  expect(visualColumnLength("《注記：破損》＃１２")).toBeCloseTo(2 + 18 / 17);
  expect(visualColumnLength("𛀂か\u3099")).toBe(2);
  expect(longestColumnLength("一\r\n\r【右丁】\n二三")).toBe(4);
});
test("automatic scale fits, caps enlargement, and exposes the wrapping floor", () => {
  expect(automaticTextScale(374, 20)).toEqual({ scale: 1, wraps: false });
  expect(automaticTextScale(1000, 20)).toEqual({ scale: 1.25, wraps: false });
  expect(automaticTextScale(187, 20)).toEqual({ scale: 0.5, wraps: false });
  expect(automaticTextScale(186, 20)).toEqual({ scale: 0.5, wraps: true });
  expect(automaticTextScale(0, 0)).toEqual({ scale: 0.5, wraps: true });
  expect(automaticTextScale(600, 0)).toEqual({ scale: 1.25, wraps: false });
});

test("kunten side advances are reserved without counting them as base characters", () => {
  expect(textScalePadding("讀＿レ￣ム書")).toBe(34 + 8.5);
  expect(textScalePadding("讀＿レ￣シテ書＿二￣ス")).toBe(34 + 25.5);
  expect(textScalePadding("《割書：讀＿レ￣ム｜書》")).toBe(34 + 4.25);
  expect(textScalePadding("《箱：字》")).toBe(40);
  expect(textScalePadding("■□〓")).toBe(40);
});
