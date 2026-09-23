import { describe, expect, test } from "bun:test";
import {
  choPairTemplate,
  suggestPageTemplate,
  usesChoPair,
} from "./scaffold";

const spread = (body: string) => `【右丁】\n${body}\n【左丁】\n${body}`;
const unmarked = (body: string) => body;

describe("page template from sibling pages", () => {
  test("見開きの本は右丁・左丁を最初から入れる", () => {
    expect(
      suggestPageTemplate([spread("一"), spread("二"), spread("三")]),
    ).toBe(choPairTemplate);
    expect(choPairTemplate).toBe("【右丁】\n\n【左丁】\n\n");
  });

  test("空ページと印だけの下書きは多数決に入れない", () => {
    expect(
      suggestPageTemplate([
        "",
        "   ",
        choPairTemplate,
        spread("本文がここに入る"),
        spread("続きの本文"),
      ]),
    ).toBe(choPairTemplate);
  });

  test("印だけのページが本文ページに勝たない", () => {
    expect(
      suggestPageTemplate([
        choPairTemplate,
        choPairTemplate,
        unmarked("本文だけのページ"),
        unmarked("別の本文だけのページ"),
      ]),
    ).toBeNull();
    expect(
      suggestPageTemplate([
        choPairTemplate,
        spread("見開きの本文"),
        spread("続きの見開き"),
      ]),
    ).toBe(choPairTemplate);
  });

  test("短い本文ページも多数決に入れる", () => {
    const short = ["一", "二", "三", "四", "五", "六", "七", "八", "九", "十"];
    expect(suggestPageTemplate(short)).toBeNull();
    expect(suggestPageTemplate([...short, spread("一")])).toBeNull();
    expect(
      suggestPageTemplate([
        unmarked("一"),
        unmarked("二"),
        spread("三"),
        spread("四"),
        spread("五"),
      ]),
    ).toBe(choPairTemplate);
  });

  test("本文中の言及は見開きに数えない", () => {
    expect(usesChoPair("凡例：見開きには【右丁】と【左丁】を付ける。")).toBe(
      false,
    );
    expect(usesChoPair(spread("一"))).toBe(true);
    expect(
      suggestPageTemplate([
        unmarked("凡例：見開きには【右丁】と【左丁】を付ける。"),
        unmarked("凡例二：同じく【右丁】と【左丁】の説明を書く。"),
      ]),
    ).toBeNull();
  });

  test("本紙がほぼ印のない書物では足さない", () => {
    expect(
      suggestPageTemplate([
        unmarked("藻汐草序\n上令下達乃治國之首\n也辨物通情方執政之\n要也今閱"),
        unmarked("蝦夷方言藻汐草乾之巻一\n語彙本文がここに続く\nさらに続き"),
        unmarked("語彙本文がここに続く\nさらに続き\nまた続く"),
      ]),
    ).toBeNull();
  });

  test("証拠が揃うまで足さない", () => {
    expect(suggestPageTemplate([])).toBeNull();
    expect(suggestPageTemplate([""])).toBeNull();
    expect(suggestPageTemplate([unmarked("短い表紙")])).toBeNull();
  });

  test("左右が揃っていないページは雛形にしない", () => {
    expect(usesChoPair("【右丁】\n一")).toBe(false);
    expect(usesChoPair("【左丁】\n一")).toBe(false);
    expect(
      suggestPageTemplate(["【右丁】\n一\n二\n三", "【左丁】\n一\n二\n三"]),
    ).toBeNull();
  });

  test("見開きが過半数なら雛形にする", () => {
    expect(
      suggestPageTemplate([
        spread("一"),
        spread("二"),
        unmarked("序文は本紙と同じくらいの長さの本文をここに置く"),
      ]),
    ).toBe(choPairTemplate);
    expect(
      suggestPageTemplate([
        spread("一"),
        unmarked("序文は本紙と同じくらいの長さの本文をここに置く"),
        unmarked("別の序文も本紙と同じくらいの長さにしておく"),
        unmarked("奥付も本紙相当の長さにしている場合を想定する"),
      ]),
    ).toBeNull();
  });
});
