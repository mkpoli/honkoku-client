import { describe, expect, test } from "bun:test";
import {
  layoutTemplates,
  sectionLabel,
  sectionLabelsIn,
  suggestPageTemplate,
} from "./layout";

const spread = (body: string) => `【右丁】\n${body}\n【左丁】\n${body}`;
const unmarked = (body: string) => body;
const template = (index: number) => layoutTemplates[index]!.text;

describe("section markers", () => {
  test("版式の節見出しだけを節とみなす", () => {
    expect(sectionLabel("【右丁】")).toBe("右丁");
    expect(sectionLabel(" 【左頁】 ")).toBe("左頁");
    expect(sectionLabel("【右丁上段】")).toBe("右丁上段");
    expect(sectionLabel("【右丁・白紙】")).toBe("右丁・白紙");
    expect(sectionLabel("【上段】")).toBe("上段");
    expect(sectionLabel("【注】")).toBeNull();
    expect(sectionLabel("【表紙】")).toBeNull();
    expect(sectionLabel("【本文】")).toBeNull();
    expect(sectionLabel("凡例：【右丁】と【左丁】")).toBeNull();
  });

  test("parse groups split on every section family", () => {
    expect(sectionLabelsIn("【右頁】\n一\n【左頁】\n二")).toEqual([
      "右頁",
      "左頁",
    ]);
    expect(
      sectionLabelsIn("【右頁上段】\n一\n【右頁下段】\n二\n【左頁上段】\n三"),
    ).toEqual(["右頁上段", "右頁下段", "左頁上段"]);
  });
});

describe("page template from sibling pages", () => {
  test("見開きの本は右丁・左丁を最初から入れる", () => {
    expect(
      suggestPageTemplate([spread("一"), spread("二"), spread("三")]),
    ).toBe(template(0));
    expect(template(0)).toBe("【右丁】\n\n【左丁】\n\n");
  });

  test("右頁・左頁、上段・下段、右側・左側も拾う", () => {
    const cases: [string[], number][] = [
      [["【右頁】\n一\n【左頁】\n二", "【右頁】\n三\n【左頁】\n四"], 1],
      [["【上段】\n一\n【下段】\n二", "【上段】\n三\n【下段】\n四"], 4],
      [["【右側】\n一\n【左側】\n二", "【右側】\n三\n【左側】\n四"], 5],
      [["【右帖】\n一\n【左帖】\n二", "【右帖】\n三\n【左帖】\n四"], 6],
      [["【右】\n一\n【左】\n二", "【右】\n三\n【左】\n四"], 7],
    ];
    for (const [siblings, index] of cases)
      expect(suggestPageTemplate(siblings)).toBe(template(index));
  });

  test("四分割の見開きは四つの節からなる雛形にする", () => {
    const page = "【右頁上段】\n一\n【右頁下段】\n二\n【左頁上段】\n三\n【左頁下段】\n四";
    expect(suggestPageTemplate([page, page])).toBe(template(2));
    const short = "【右上段】\n一\n【右下段】\n二\n【左上段】\n三\n【左下段】\n四";
    expect(suggestPageTemplate([short, short])).toBe(template(3));
    const tiered = "【右丁上段】\n一\n【左丁上段】\n二";
    expect(suggestPageTemplate([tiered, tiered])).toBe(template(8));
  });

  test("白紙半丁のページも右丁・左丁の本と数える", () => {
    expect(
      suggestPageTemplate([
        "【右丁・白紙】\n\n【左丁】\n本文",
        spread("続き"),
      ]),
    ).toBe(template(0));
  });

  test("空ページと印だけの下書きは多数決に入れない", () => {
    expect(
      suggestPageTemplate([
        "",
        "   ",
        template(0),
        spread("本文がここに入る"),
        spread("続きの本文"),
      ]),
    ).toBe(template(0));
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
    ).toBe(template(0));
  });

  test("本文中の言及は見開きに数えない", () => {
    expect(
      suggestPageTemplate([
        unmarked("凡例：見開きには【右丁】と【左丁】を付ける。"),
        unmarked("凡例二：同じく【右丁】と【左丁】の説明を書く。"),
      ]),
    ).toBeNull();
  });

  test("左右が揃っていないページは雛形にしない", () => {
    expect(
      suggestPageTemplate(["【右丁】\n一\n二\n三", "【左丁】\n一\n二\n三"]),
    ).toBeNull();
  });

  test("証拠が揃うまで足さない", () => {
    expect(suggestPageTemplate([])).toBeNull();
    expect(suggestPageTemplate([""])).toBeNull();
    expect(suggestPageTemplate([unmarked("短い表紙")])).toBeNull();
  });
});
