import { describe, expect, test } from "bun:test";
import { parseGroups } from "./index";
import {
  layoutTemplates,
  sectionLabel,
  sectionLabelsIn,
  sectionSlot,
  suggestPageTemplate,
} from "./layout";

const spread = (body: string) => `【右丁】\n${body}\n【左丁】\n${body}`;
const unmarked = (body: string) => body;
const byLabels = (...labels: string[]) =>
  layoutTemplates.find((t) => t.labels.join("\n") === labels.join("\n"))!
    .text;

describe("section markers", () => {
  test("版式の節見出しだけを節とみなす", () => {
    expect(sectionLabel("【右丁】")).toBe("右丁");
    expect(sectionLabel(" 【左頁】 ")).toBe("左頁");
    expect(sectionLabel("【右丁上段】")).toBe("右丁上段");
    expect(sectionLabel("【右丁・白紙】")).toBe("右丁・白紙");
    expect(sectionLabel("【右丁白紙】")).toBe("右丁白紙");
    expect(sectionLabel("【右側上段】")).toBe("右側上段");
    expect(sectionLabel("【上段】")).toBe("上段");
    expect(sectionLabel("【注】")).toBeNull();
    expect(sectionLabel("【表紙】")).toBeNull();
    expect(sectionLabel("【本文】")).toBeNull();
    expect(sectionLabel("凡例：【右丁】と【左丁】")).toBeNull();
  });

  test("行中の節見出しは本文のまま", () => {
    expect(parseGroups("凡例に【上段】と書く").map((g) => g.label)).toEqual([
      "",
    ]);
    expect(parseGroups("凡例に【上段】と書く")[0]!.columns).toHaveLength(1);
  });

  test("白紙半丁は右丁・左丁と同じ枠", () => {
    expect(sectionSlot("右丁・白紙")).toBe("右丁");
    expect(sectionSlot("右丁白紙")).toBe("右丁");
    expect(sectionSlot("左丁　文字無し")).toBe("左丁");
  });
});

describe("page template from sibling pages", () => {
  test("見開きの本は右丁・左丁を最初から入れる", () => {
    expect(
      suggestPageTemplate([spread("一"), spread("二"), spread("三")]),
    ).toBe(byLabels("右丁", "左丁"));
    expect(byLabels("右丁", "左丁")).toBe("【右丁】\n\n【左丁】\n\n");
  });

  test("右頁・左頁、上段・下段、右側・左側も拾う", () => {
    const cases: [string[], string][] = [
      [["【右頁】\n一\n【左頁】\n二", "【右頁】\n三\n【左頁】\n四"], "右頁|左頁"],
      [["【上段】\n一\n【下段】\n二", "【上段】\n三\n【下段】\n四"], "上段|下段"],
      [["【右側】\n一\n【左側】\n二", "【右側】\n三\n【左側】\n四"], "右側|左側"],
      [["【右帖】\n一\n【左帖】\n二", "【右帖】\n三\n【左帖】\n四"], "右帖|左帖"],
      [["【右】\n一\n【左】\n二", "【右】\n三\n【左】\n四"], "右|左"],
    ];
    for (const [siblings, key] of cases)
      expect(suggestPageTemplate(siblings)).toBe(
        byLabels(...key.split("|")),
      );
  });

  test("四分割と三段と丁内二段は、より大きな版式に決める", () => {
    const fourPage =
      "【右頁上段】\n一\n【右頁下段】\n二\n【左頁上段】\n三\n【左頁下段】\n四";
    expect(suggestPageTemplate([fourPage, fourPage])).toBe(
      byLabels("右頁上段", "右頁下段", "左頁上段", "左頁下段"),
    );
    const fourShort =
      "【右上段】\n一\n【右下段】\n二\n【左上段】\n三\n【左下段】\n四";
    expect(suggestPageTemplate([fourShort, fourShort])).toBe(
      byLabels("右上段", "右下段", "左上段", "左下段"),
    );
    const fourCho =
      "【右丁上段】\n一\n【右丁下段】\n二\n【左丁上段】\n三\n【左丁下段】\n四";
    expect(suggestPageTemplate([fourCho, fourCho])).toBe(
      byLabels("右丁上段", "右丁下段", "左丁上段", "左丁下段"),
    );
    const three = "【上段】\n一\n【中段】\n二\n【下段】\n三";
    expect(suggestPageTemplate([three, three])).toBe(
      byLabels("上段", "中段", "下段"),
    );
    const choTiers =
      "【右丁】\n一\n【上段】\n二\n【下段】\n三\n【左丁】\n四";
    expect(suggestPageTemplate([choTiers, choTiers])).toBe(
      byLabels("右丁", "上段", "下段", "左丁"),
    );
  });

  test("白紙半丁のページも右丁・左丁の本と数える", () => {
    expect(
      suggestPageTemplate([
        "【右丁・白紙】\n\n【左丁】\n本文",
        spread("続き"),
      ]),
    ).toBe(byLabels("右丁", "左丁"));
    expect(
      suggestPageTemplate([
        "【右丁白紙】\n\n【左丁】\n本文",
        spread("続き"),
      ]),
    ).toBe(byLabels("右丁", "左丁"));
  });

  test("空ページと印だけの下書きは多数決に入れない", () => {
    expect(
      suggestPageTemplate([
        "",
        "   ",
        byLabels("右丁", "左丁"),
        spread("本文がここに入る"),
        spread("続きの本文"),
      ]),
    ).toBe(byLabels("右丁", "左丁"));
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
    ).toBe(byLabels("右丁", "左丁"));
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
