import { describe, expect, test } from "bun:test";
import { parseGroups as parse, parseInline, renderInline } from "./index";
describe("transcription projection", () => {
  test("page halves and line breaks retain reading order", () => {
    const groups = parse("【右丁】\r\n一\r\n二\r\n【左丁】\n三");
    expect(groups.map((g) => [g.label, g.columns.length])).toEqual([
      ["右丁", 2],
      ["左丁", 1],
    ]);
  });
  test("known annotations project independently", () => {
    expect(
      parseInline(
        "《振り仮名：峰｜みね》《割書：a｜b》《見せ消ち：旧｜新》《圏点：語｜・》",
      ).map((n) => n.kind),
    ).toEqual(["ruby", "warichu", "correction", "emphasis"]);
    expect(renderInline(parseInline("《振り仮名：峰｜みね》"))).toContain(
      "<rt>みね</rt>",
    );
  });
  test("unfamiliar and malformed syntax survives as text", () => {
    const source = "《未知：あ｜い》【未閉じ《割書：あ》";
    expect(renderInline(parseInline(source))).toBe(source);
  });
  test("notes, comments and boxed glyphs", () => {
    expect(parseInline("【異本】■□〓＃１２※欄外").map((n) => n.kind)).toEqual([
      "editorial",
      "glyph",
      "glyph",
      "glyph",
      "reference",
      "comment",
    ]);
    expect(renderInline(parseInline("＃１２"))).toContain('data-note="12"');
  });
  test("return marks and okurigana attach to preceding grapheme", () => {
    expect(parseInline("讀＿レ￣ム")).toContainEqual({
      kind: "reading",
      base: "讀",
      returnMark: "レ",
      okurigana: "ム",
    });
    expect(parseInline("𬼂＿一").at(-1)).toMatchObject({
      base: "𬼂",
      returnMark: "一",
    });
    expect(parseInline("葛\u{E0100}＿レ").at(-1)).toMatchObject({
      base: "葛\u{E0100}",
    });
  });
  test("all data is escaped before HTML rendering", () => {
    const html = renderInline(
      parseInline(
        '<script>alert(1)</script>《振り仮名：<img>｜"onload"》【<svg>】',
      ),
    );
    expect(html).not.toContain("<script>");
    expect(html).not.toContain("<img>");
    expect(html).not.toContain("<svg>");
    expect(html).toContain("&lt;script&gt;");
  });
  test("rare kana and variation selectors remain intact", () => {
    expect(renderInline(parseInline("ゟヿ𬼂葛\u{E0100}"))).toBe(
      "ゟヿ𬼂葛\u{E0100}",
    );
  });
});

test("unlabelled leading and trailing breaks remain empty columns", () => {
  expect(parse("\n一\n")[0].columns).toEqual([
    [],
    [{ kind: "text", text: "一" }],
    [],
  ]);
});
